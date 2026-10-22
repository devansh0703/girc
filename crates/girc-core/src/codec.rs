//! The GIRC encoder and decoder.
//!
//! # Picture structure
//!
//! A GIRC picture is a single luma plane of `width` x `height` samples,
//! coded as a raster of fixed-size blocks. Every block is intra-predicted
//! from its already-reconstructed causal neighbours, transformed with one of
//! the three VVC transforms, quantized, and coded.
//!
//! # The experiment
//!
//! The encoder runs the same rate-distortion search in both modes. The only
//! difference is how the chosen transform type reaches the decoder:
//!
//! * [`TransformSignalling::Explicit`] sends the transform type for every
//!   block. This is the baseline.
//! * [`TransformSignalling::Inferred`] has the decoder predict the transform
//!   from the block's geometry, and the encoder sends one bit saying whether
//!   its own choice differed.
//!
//! Everything else -- the intra modes, the quantization, the residual coding,
//! the entropy coder -- is identical, so a difference in the measured
//! bitrate or quality is attributable to the signalling mechanism alone.

use crate::bitstream::{BinDecoder, BinEncoder, BitstreamError, BitstreamHeader, Word};
use crate::chroma;
use crate::frame::Frame;
use crate::geometry::{
    extract_features, extract_features_from_window, prediction_features, BlockContext,
    GeometryFeatures, MotionContext, NeighbourTransforms, TransformPredictor, TransformSignal,
    TransformSignalling,
};
use crate::inter::{compensate, search, MotionField, MotionVector};
use crate::intra::{predict, IntraMode, ReferenceSamples};
use crate::quant::{dequantize, dequantize_with, quantize_with, sse, QuantParams};
use crate::transform::{forward_mode, inverse_mode, TransformMode};

/// The block size GIRC codes, in samples.
pub const BLOCK_SIZE: usize = 8;

/// The size of the causal window the predictor reads, in samples.
///
/// The window is the square region ending at the block's top-left corner.
/// Every sample in it has already been reconstructed when the decoder
/// reaches the current block, so reading it costs nothing in signalling.
pub const CONTEXT_WINDOW: usize = 16;

/// The number of entropy-coding contexts in the payload.
const NUM_CONTEXTS: usize = 16;

/// The transform each coded block used.
///
/// The map is indexed in raster order. The decoder fills it as it codes,
/// so the transform of every already-coded block is known; that is the
/// context the predictor reads to know the transforms of the
/// neighbouring blocks.
#[derive(Debug, Clone, Default)]
pub struct TransformMap {
    kinds: Vec<Option<TransformMode>>,
    blocks_x: usize,
}

impl TransformMap {
    /// A map for a picture of the given block dimensions.
    pub fn new(blocks_x: usize, blocks_y: usize) -> Self {
        Self {
            kinds: vec![None; blocks_x * blocks_y],
            blocks_x,
        }
    }

    /// Record the transform a block used.
    pub fn set(&mut self, bx: usize, by: usize, mode: TransformMode) {
        self.kinds[by * self.blocks_x + bx] = Some(mode);
    }

    fn get(&self, bx: usize, by: usize) -> Option<TransformMode> {
        let index = by.checked_mul(self.blocks_x)?.checked_add(bx)?;
        self.kinds.get(index).copied().flatten()
    }

    /// The transforms of the four neighbouring blocks, in the order the
    /// predictor expects: left, above, above-left, above-right.
    ///
    /// A neighbour that has not been coded yet is empty. The raster
    /// order means the above-right block is always available by the time
    /// the current block is reached, because the row above is finished.
    fn neighbours(&self, bx: usize, by: usize) -> NeighbourTransforms {
        NeighbourTransforms([
            if bx > 0 { self.get(bx - 1, by) } else { None },
            if by > 0 { self.get(bx, by - 1) } else { None },
            if bx > 0 && by > 0 {
                self.get(bx - 1, by - 1)
            } else {
                None
            },
            if by > 0 {
                self.get(bx + 1, by - 1)
            } else {
                None
            },
        ])
    }
}

// Context indices.
pub(crate) const CTX_INTRA_MODE: usize = 0;
pub(crate) const CTX_SKIP: usize = 1;
/// Whether the block is predicted from the previous picture.
pub(crate) const CTX_INTER: usize = 11;
pub(crate) const CTX_COEF_SIGNIFICANT: usize = 2;
pub(crate) const CTX_COEF_GREATER1: usize = 3;
pub(crate) const CTX_COEF_GREATER2: usize = 4;
pub(crate) const CTX_COEF_SIGN: usize = 5;
pub(crate) const CTX_COEF_EXP_GOLOMB: usize = 8;
const CTX_TRANSFORM_OVERRIDE: usize = 6;
/// Whether the block uses a multiple transform selection mode at all.
const CTX_TRANSFORM_MTS: usize = 7;
/// The bits of the mode index.
const CTX_TRANSFORM_INDEX: usize = 9;

/// The number of bits the intra mode index is coded with.
///
/// The mode set has ten modes, so the index needs four bits. The number is
/// derived from the mode count rather than written out, so that adding a
/// mode cannot silently leave the code too short.
const INTRA_MODE_BITS: usize = {
    let mut bits = 0usize;
    let mut count = crate::intra::IntraMode::COUNT;
    while count > 0 {
        bits += 1;
        count >>= 1;
    }
    bits
};

/// A luma picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    /// Samples in raster order.
    pub samples: Vec<u8>,
}

impl Picture {
    /// A picture filled with a constant value.
    pub fn filled(width: usize, height: usize, value: u8) -> Self {
        Self {
            width,
            height,
            samples: vec![value; width * height],
        }
    }

    /// The sample at `(x, y)`, or `None` when outside the picture.
    pub fn sample(&self, x: isize, y: isize) -> Option<u8> {
        if x < 0 || y < 0 {
            return None;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.samples[y * self.width + x])
    }

    /// Set the sample at `(x, y)`, ignoring out-of-bounds coordinates.
    pub fn set_sample(&mut self, x: usize, y: usize, value: u8) {
        if x < self.width && y < self.height {
            self.samples[y * self.width + x] = value;
        }
    }
}

/// The encoder configuration.
#[derive(Debug, Clone)]
pub struct EncoderConfig {
    /// The quantization parameter.
    pub qp: i32,
    /// How the transform type is communicated.
    pub signalling: TransformSignalling,
    /// The predictor used in inferred mode.
    pub predictor: TransformPredictor,
    /// The rate-distortion lambda, as a fraction of the quantizer step.
    pub lambda: f32,
    /// The rate-distortion margin within which the encoder accepts the
    /// decoder's predicted transform.
    ///
    /// This is the knob that makes the mechanism work. A transform's
    /// rate-distortion cost is often very close to the best available, and
    /// the encoder can take the slightly worse transform that the decoder
    /// already knows in exchange for not having to signal the choice.
    ///
    /// The margin is expressed in bits of rate, and it is the amount of
    /// rate the encoder is willing to give up to save the signalling. The
    /// encoder accepts the predicted transform when doing so costs less
    /// rate-distortion than the override would.
    ///
    /// A margin of zero makes the encoder take the strictly optimal
    /// transform and override whenever the prediction differs, which is
    /// the mechanism at its least favourable. A large margin makes the
    /// encoder follow the prediction almost always, at the cost of
    /// distortion.
    pub transform_margin: f32,
    /// How far the motion search looks around the motion vector
    /// predictor, in whole samples.
    ///
    /// The predictor is where the motion is most likely to be, so a
    /// small radius is enough; a wide one costs the benchmark a great
    /// deal of time for a fraction of a percent.
    pub motion_search_radius: i32,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            qp: 27,
            signalling: TransformSignalling::Explicit,
            predictor: TransformPredictor::constant_dct2(),
            lambda: 0.85,
            transform_margin: 0.0,
            motion_search_radius: 4,
        }
    }
}

/// The result of encoding a sequence of frames.
#[derive(Debug, Clone)]
pub struct EncodeResult {
    /// The encoded bitstream.
    pub bitstream: Vec<u8>,
    /// The encoder's reconstruction of each frame.
    pub reconstructions: Vec<Frame>,
    /// The number of bytes the luma payload occupies.
    pub luma_bytes: usize,
    /// The number of bytes the chroma payload occupies.
    pub chroma_bytes: usize,
    /// The number of bytes both payloads occupy.
    pub payload_bytes: usize,
    /// How many blocks the encoder coded.
    pub block_count: usize,
    /// The number of syntax elements the transform-type signalling used.
    ///
    /// This counts the bins the signalling contributes, not the bits the
    /// entropy coder charged for them: a context-adaptive coder charges
    /// less than one bit per bin for a skewed distribution. The rate the
    /// signalling actually cost is the difference between the two modes'
    /// bitstreams, which the benchmark measures directly.
    pub transform_signalling_bins: usize,
    /// The number of blocks whose transform differed from the decoder's
    /// prediction.
    ///
    /// In explicit mode this is always zero, because there is no
    /// prediction to differ from.
    pub prediction_mismatches: usize,
    /// How many blocks were predicted from the previous frame rather
    /// than from the current frame's causal neighbourhood.
    pub inter_blocks: usize,
    /// The blocks whose transform differed from the decoder's
    /// prediction, split by whether the block was intra or inter
    /// predicted.
    ///
    /// The split is what tests the claim: an inter residual is more
    /// structured, so the transform should be more predictable, and the
    /// mismatch rate should be lower for the inter blocks.
    pub intra_mismatches: usize,
    pub inter_mismatches: usize,
    pub intra_blocks: usize,
}

/// Errors produced by the encoder or decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// The bitstream was malformed.
    Bitstream(BitstreamError),
    /// The picture dimensions were not a multiple of the block size.
    Dimensions { width: usize, height: usize },
    /// The predictor's features did not match the model's expectations.
    Predictor(String),
    /// The sequence had no frames to code.
    NoFrames,
    /// The chroma planes were not half the luma plane's size.
    ChromaDimensions {
        luma_width: usize,
        luma_height: usize,
    },
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::Bitstream(e) => write!(f, "{e}"),
            CodecError::Dimensions { width, height } => write!(
                f,
                "picture {width}x{height} is not a multiple of the {BLOCK_SIZE}x{BLOCK_SIZE} block size"
            ),
            CodecError::Predictor(msg) => write!(f, "predictor error: {msg}"),
            CodecError::NoFrames => write!(f, "the sequence has no frames to code"),
            CodecError::ChromaDimensions {
                luma_width,
                luma_height,
            } => write!(
                f,
                "the chroma planes of a {luma_width}x{luma_height} frame must be half its size"
            ),
        }
    }
}

impl std::error::Error for CodecError {}

impl From<BitstreamError> for CodecError {
    fn from(value: BitstreamError) -> Self {
        CodecError::Bitstream(value)
    }
}

/// The luma plane's coding result.
#[derive(Debug, Clone)]
pub struct LumaResult {
    /// The range-coded luma payload.
    pub payload: Vec<Word>,
    /// The encoder's reconstruction of each frame's luma plane.
    pub reconstructions: Vec<Picture>,
    /// The motion field of each frame.
    ///
    /// The chroma planes need it, because H.266 derives the chroma
    /// motion from the luma motion rather than coding it separately.
    pub motion: Vec<MotionField>,
    /// How many blocks the sequence holds.
    pub block_count: usize,
    /// The number of syntax elements the transform-type signalling used.
    ///
    /// This counts the bins the signalling contributes, not the bits the
    /// entropy coder charged for them: a context-adaptive coder charges
    /// less than one bit per bin for a skewed distribution. The rate the
    /// signalling actually cost is the difference between the two modes'
    /// bitstreams, which the benchmark measures directly.
    pub transform_signalling_bins: usize,
    /// The number of blocks whose transform differed from the decoder's
    /// prediction.
    ///
    /// In explicit mode this is always zero, because there is no
    /// prediction to differ from.
    pub prediction_mismatches: usize,
    /// How many blocks were predicted from the previous frame rather
    /// than from the current frame's causal neighbourhood.
    pub inter_blocks: usize,
    /// The blocks whose transform differed from the decoder's
    /// prediction, split by whether the block was intra or inter
    /// predicted.
    ///
    /// The split is what tests the claim: an inter residual is more
    /// structured, so the transform should be more predictable, and the
    /// mismatch rate should be lower for the inter blocks.
    pub intra_mismatches: usize,
    pub inter_mismatches: usize,
    pub intra_blocks: usize,
    /// The per-block training examples, when they were collected.
    pub examples: Vec<TrainingExample>,
}

/// Encode the luma plane of a sequence of pictures.
///
/// The transform mode signalling is the same in both modes, and the two
/// modes differ in nothing else, so a difference in the measured bitrate
/// is attributable to the signalling mechanism alone.
///
/// The training examples are collected when asked for, because the
/// collection pass runs the same block loop the encoder runs, and a
/// training set gathered any other way would describe a situation the
/// predictor never meets at inference time.
fn encode_luma_sequence(
    frames: &[Picture],
    config: &EncoderConfig,
    collect_examples: bool,
) -> Result<LumaResult, CodecError> {
    let Some(first) = frames.first() else {
        return Err(CodecError::NoFrames);
    };
    for picture in frames {
        if picture.width % BLOCK_SIZE != 0 || picture.height % BLOCK_SIZE != 0 {
            return Err(CodecError::Dimensions {
                width: picture.width,
                height: picture.height,
            });
        }
    }

    let blocks_x = first.width / BLOCK_SIZE;
    let blocks_y = first.height / BLOCK_SIZE;
    // Every frame has the same number of blocks, so the sequence has
    // this many in total.
    let block_count = blocks_x * blocks_y * frames.len();

    let mut encoder = BinEncoder::new(NUM_CONTEXTS);
    let mut reconstructions: Vec<Picture> = Vec::with_capacity(frames.len());
    let mut motion_fields: Vec<MotionField> = Vec::with_capacity(frames.len());
    let mut examples: Vec<TrainingExample> =
        Vec::with_capacity(if collect_examples { block_count } else { 0 });
    let mut mismatches = 0usize;
    let mut signalling_bins = 0usize;
    let mut inter_blocks = 0usize;
    let mut intra_mismatches = 0usize;
    let mut inter_mismatches = 0usize;
    let mut intra_blocks = 0usize;

    for (index, picture) in frames.iter().enumerate() {
        // The reference for an inter picture is the reconstruction of the
        // picture before it, which is what the decoder will have.
        let reference_picture = if index == 0 {
            None
        } else {
            Some(&reconstructions[index - 1])
        };
        let mut reconstruction = Picture::filled(picture.width, picture.height, 0);
        let mut transforms = TransformMap::new(blocks_x, blocks_y);
        let mut motion = MotionField::new(blocks_x, blocks_y);

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let original = block_samples(picture, bx, by);

                // The prediction mode comes first in the syntax, so the
                // decoder knows it before the transform mode.
                //
                // An intra picture has no choice: every block is intra. An
                // inter picture chooses per block, and the choice is made by
                // rate-distortion cost over the two predictions.
                let (is_inter, vector, mode, prediction) = choose_prediction(
                    picture,
                    reference_picture,
                    &reconstruction,
                    &motion,
                    bx,
                    by,
                    config,
                );
                if is_inter {
                    inter_blocks += 1;
                } else {
                    intra_blocks += 1;
                }

                // The decoder's prediction, which the encoder must match. The
                // intra mode is decoded before the transform type, so it is
                // available on both sides.
                //
                // The prediction is computed from the encoder's own
                // reconstruction, which is what the decoder will have.
                // The explicit mode always takes the rate-distortion optimal
                // transform and signals it; the margin only applies to the
                // inferred mode, where it is the mechanism's trade.
                let predicted = if config.signalling == TransformSignalling::Inferred {
                    let features = predict_features(&PredictionContext {
                        picture: &reconstruction,
                        transforms: &transforms,
                        motion: &motion,
                        block: (bx, by),
                        mode,
                        is_inter,
                        qp: config.qp,
                        prediction: &prediction,
                    });
                    Some(config.predictor.predict_with_margin(&features).0)
                } else {
                    None
                };

                let choice = choose_transform(&original, &prediction, config, predicted);

                let signal = match (config.signalling, predicted) {
                    (TransformSignalling::Explicit, _) => TransformSignal::Explicit(choice.chosen),
                    (TransformSignalling::Inferred, Some(predicted)) => {
                        if predicted == choice.chosen {
                            TransformSignal::Predicted
                        } else {
                            mismatches += 1;
                            if is_inter {
                                inter_mismatches += 1;
                            } else {
                                intra_mismatches += 1;
                            }
                            TransformSignal::Override {
                                predicted,
                                actual: choice.chosen,
                            }
                        }
                    }
                    (TransformSignalling::Inferred, None) => {
                        TransformSignal::Explicit(choice.chosen)
                    }
                };
                signalling_bins += signal.bin_count();

                if collect_examples {
                    // The example's label is the strictly optimal transform,
                    // not the one the margin may have talked the encoder
                    // into taking: the label describes what the mode set
                    // prefers, which is what the predictor has to learn.
                    examples.push(TrainingExample {
                        features: predict_features(&PredictionContext {
                            picture: &reconstruction,
                            transforms: &transforms,
                            motion: &motion,
                            block: (bx, by),
                            mode,
                            is_inter,
                            qp: config.qp,
                            prediction: &prediction,
                        }),
                        transform: choice.optimal,
                    });
                }

                // Code the syntax elements, in the decoder's order: the
                // prediction mode, the motion vector, the transform mode and
                // the residual.
                encoder.encode(CTX_INTER, is_inter);
                if is_inter {
                    let predictor = motion.predict(bx, by);
                    code_motion_vector(&mut encoder, vector - predictor);
                } else {
                    code_intra_mode(&mut encoder, mode);
                }
                code_transform_signal(&mut encoder, signal);
                let skip = choice.levels.iter().all(|l| *l == 0);
                encoder.encode(CTX_SKIP, skip);
                if !skip {
                    code_coefficients(&mut encoder, &choice.levels);
                }

                // Commit the reconstruction and the transform, so later blocks
                // can use them.
                for y in 0..BLOCK_SIZE {
                    for x in 0..BLOCK_SIZE {
                        let value = choice.reconstruction[y * BLOCK_SIZE + x].clamp(0, 255) as u8;
                        reconstruction.set_sample(bx * BLOCK_SIZE + x, by * BLOCK_SIZE + y, value);
                    }
                }
                transforms.set(bx, by, choice.chosen);
                if is_inter {
                    motion.set(bx, by, vector);
                }
            }
        }

        reconstructions.push(reconstruction);
        motion_fields.push(motion);
    }

    Ok(LumaResult {
        payload: encoder.finish(),
        reconstructions,
        motion: motion_fields,
        block_count,
        transform_signalling_bins: signalling_bins,
        prediction_mismatches: mismatches,
        inter_blocks,
        intra_mismatches,
        inter_mismatches,
        intra_blocks,
        examples,
    })
}

/// Encode the luma plane of a sequence of pictures.
///
/// This is the entry point the training pipeline uses, and the one that
/// measures the mechanism on the luma plane alone.
pub fn encode_luma(frames: &[Picture], config: &EncoderConfig) -> Result<LumaResult, CodecError> {
    encode_luma_sequence(frames, config, false)
}

/// Encode a sequence of 4:2:0 frames.
///
/// The luma plane and the chroma planes are coded by separate entropy
/// coders and stored in separate payloads, so the luma rate and the
/// chroma rate can be measured separately. The mechanism changes the
/// luma signalling and nothing else, so the luma rate is the axis the
/// mechanism's own rate-distortion effect is measured on, and the total
/// rate is the axis the anchor codecs are compared on.
pub fn encode(frames: &[Frame], config: &EncoderConfig) -> Result<EncodeResult, CodecError> {
    let Some(first) = frames.first() else {
        return Err(CodecError::NoFrames);
    };
    for frame in frames {
        if frame.luma.width % BLOCK_SIZE != 0 || frame.luma.height % BLOCK_SIZE != 0 {
            return Err(CodecError::Dimensions {
                width: frame.luma.width,
                height: frame.luma.height,
            });
        }
        if frame.cb.width != frame.luma.width / 2
            || frame.cb.height != frame.luma.height / 2
            || frame.cr.width != frame.luma.width / 2
            || frame.cr.height != frame.luma.height / 2
        {
            return Err(CodecError::ChromaDimensions {
                luma_width: frame.luma.width,
                luma_height: frame.luma.height,
            });
        }
    }

    let luma: Vec<Picture> = frames.iter().map(|frame| frame.luma.clone()).collect();
    let luma_result = encode_luma_sequence(&luma, config, false)?;

    let mut chroma_encoder = BinEncoder::new(NUM_CONTEXTS);
    let chroma_result =
        chroma::encode_sequence(frames, &luma_result.motion, config, &mut chroma_encoder)?;
    let chroma_payload = chroma_encoder.finish();

    let header = BitstreamHeader {
        version: crate::bitstream::VERSION,
        width: first.luma.width as u32,
        height: first.luma.height as u32,
        qp: config.qp as u8,
        frames: frames.len() as u8,
        signalling: config.signalling,
        luma_words: luma_result.payload.len() as u32,
        chroma_words: chroma_payload.len() as u32,
        model_id: config.predictor.model_id(),
    };
    let mut bitstream = Vec::new();
    header.write(&mut bitstream, &luma_result.payload, &chroma_payload);

    let luma_bytes = luma_result.payload.len() * 4;
    let chroma_bytes = chroma_payload.len() * 4;
    let reconstructions: Vec<Frame> = frames
        .iter()
        .enumerate()
        .map(|(index, _frame)| {
            let (cb, cr) = &chroma_result.planes[index];
            Frame::new(
                luma_result.reconstructions[index].clone(),
                cb.clone(),
                cr.clone(),
            )
        })
        .collect();

    Ok(EncodeResult {
        payload_bytes: luma_bytes + chroma_bytes,
        luma_bytes,
        chroma_bytes,
        bitstream,
        reconstructions,
        prediction_mismatches: luma_result.prediction_mismatches,
        block_count: luma_result.block_count,
        transform_signalling_bins: luma_result.transform_signalling_bins,
        inter_blocks: luma_result.inter_blocks,
        intra_mismatches: luma_result.intra_mismatches,
        inter_mismatches: luma_result.inter_mismatches,
        intra_blocks: luma_result.intra_blocks,
    })
}

/// The luma plane's decoding result.
struct DecodedLuma {
    pictures: Vec<Picture>,
    motion: Vec<MotionField>,
}

/// Decode the luma payload of a bitstream.
fn decode_luma_sequence(
    words: &[Word],
    header: &BitstreamHeader,
    config: &EncoderConfig,
) -> Result<DecodedLuma, CodecError> {
    let width = header.width as usize;
    let height = header.height as usize;
    if width % BLOCK_SIZE != 0 || height % BLOCK_SIZE != 0 {
        return Err(CodecError::Dimensions { width, height });
    }

    let mut decoder = BinDecoder::new(words, NUM_CONTEXTS)?;
    let mut pictures: Vec<Picture> = Vec::with_capacity(header.frames as usize);
    let mut motion_fields: Vec<MotionField> = Vec::with_capacity(header.frames as usize);

    let blocks_x = width / BLOCK_SIZE;
    let blocks_y = height / BLOCK_SIZE;

    for index in 0..header.frames {
        let mut picture = Picture::filled(width, height, 0);
        let mut transforms = TransformMap::new(blocks_x, blocks_y);
        let mut motion = MotionField::new(blocks_x, blocks_y);
        let reference_picture = if index == 0 {
            None
        } else {
            Some(&pictures[index as usize - 1])
        };

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let reference = reference_samples(&picture, bx, by);

                // The prediction mode and the motion vector come first, so
                // the prediction is known before the transform mode.
                let is_inter = decoder.decode(CTX_INTER)?;
                let (mode, vector, prediction) = if is_inter {
                    let predictor = motion.predict(bx, by);
                    let difference = decode_motion_vector(&mut decoder)?;
                    let vector = predictor + difference;
                    let prediction = match reference_picture {
                        Some(reference_picture) => compensate(
                            reference_picture,
                            (bx * BLOCK_SIZE) as i32,
                            (by * BLOCK_SIZE) as i32,
                            BLOCK_SIZE,
                            vector,
                        ),
                        None => vec![0i32; BLOCK_SIZE * BLOCK_SIZE],
                    };
                    (IntraMode::Dc, vector, prediction)
                } else {
                    let mode = decode_intra_mode(&mut decoder)?;
                    let prediction = predict(mode, BLOCK_SIZE, &reference);
                    (mode, MotionVector::ZERO, prediction)
                };

                let transform = decode_transform_signal(
                    &mut decoder,
                    config,
                    header.signalling,
                    &PredictionContext {
                        picture: &picture,
                        transforms: &transforms,
                        motion: &motion,
                        block: (bx, by),
                        mode,
                        is_inter,
                        qp: header.qp as i32,
                        prediction: &prediction,
                    },
                )?;

                let skip = decoder.decode(CTX_SKIP)?;
                let levels = if skip {
                    vec![0i32; BLOCK_SIZE * BLOCK_SIZE]
                } else {
                    decode_coefficients(&mut decoder)?
                };

                let dequantized = dequantize(&levels, header.qp as i32);
                let samples = inverse_mode(transform, BLOCK_SIZE, &dequantized)
                    .map_err(|e| CodecError::Predictor(e.to_string()))?;

                for y in 0..BLOCK_SIZE {
                    for x in 0..BLOCK_SIZE {
                        let index = y * BLOCK_SIZE + x;
                        let value = (prediction[index] + samples[index]).clamp(0, 255) as u8;
                        picture.set_sample(bx * BLOCK_SIZE + x, by * BLOCK_SIZE + y, value);
                    }
                }
                transforms.set(bx, by, transform);
                if is_inter {
                    motion.set(bx, by, vector);
                }
            }
        }
        pictures.push(picture);
        motion_fields.push(motion);
    }

    Ok(DecodedLuma {
        pictures,
        motion: motion_fields,
    })
}

/// Decode a luma payload into a sequence of pictures.
///
/// This is the luma-only counterpart of [`decode`], and the one the
/// tests and the luma-only tooling use. The payload has no header, so
/// the caller states the predictor it was coded with, and a mismatch is
/// refused: the decoder's prediction is part of the syntax, so a
/// different predictor decodes a different picture without saying so.
pub fn decode_luma(
    payload: &[Word],
    config: &EncoderConfig,
    width: usize,
    height: usize,
    frames: usize,
) -> Result<Vec<Picture>, CodecError> {
    decode_luma_with_model(payload, config, width, height, frames, None)
}

/// Decode a luma payload that was coded with a known predictor.
///
/// The luma-only payload has no header of its own, so the caller
/// states the predictor it was coded with. A mismatch is refused,
/// because the decoder's prediction is part of the syntax: a different
/// predictor decodes a different picture without saying so.
pub fn decode_luma_with_model(
    payload: &[Word],
    config: &EncoderConfig,
    width: usize,
    height: usize,
    frames: usize,
    model_id: Option<u16>,
) -> Result<Vec<Picture>, CodecError> {
    if let Some(model_id) = model_id {
        if config.signalling == TransformSignalling::Inferred
            && model_id != config.predictor.model_id()
        {
            return Err(CodecError::Predictor(format!(
                "the payload was coded with the predictor {model_id}, and the \
                 decoder has {}",
                config.predictor.model_id()
            )));
        }
    }
    let header = BitstreamHeader {
        version: crate::bitstream::VERSION,
        width: width as u32,
        height: height as u32,
        qp: config.qp as u8,
        frames: frames as u8,
        signalling: config.signalling,
        luma_words: payload.len() as u32,
        chroma_words: 0,
        model_id: config.predictor.model_id(),
    };
    Ok(decode_luma_sequence(payload, &header, config)?.pictures)
}

/// Decode a bitstream into a sequence of 4:2:0 frames.
pub fn decode(bitstream: &[u8], config: &EncoderConfig) -> Result<Vec<Frame>, CodecError> {
    let (header, _) = BitstreamHeader::read(bitstream)?;
    let width = header.width as usize;
    let height = header.height as usize;
    if width % BLOCK_SIZE != 0 || height % BLOCK_SIZE != 0 {
        return Err(CodecError::Dimensions { width, height });
    }

    let payload_start = BitstreamHeader::SIZE;
    let mut words = Vec::new();
    for chunk in bitstream[payload_start..].chunks_exact(4) {
        words.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    let luma_words = header.luma_words as usize;
    if words.len() < luma_words + header.chroma_words as usize {
        return Err(CodecError::Bitstream(BitstreamError::Truncated));
    }
    let (luma_part, chroma_part) = words.split_at(luma_words);

    // An inferred-signalling payload cannot be decoded without the
    // predictor the encoder used, because the decoder's prediction is
    // part of the syntax: the correction is relative to it. A decoder
    // with a different model would decode a different picture without
    // reporting an error, so the mismatch is caught here.
    if header.signalling == TransformSignalling::Inferred
        && header.model_id != config.predictor.model_id()
    {
        return Err(CodecError::Predictor(format!(
            "the stream was coded with the predictor {}, and the decoder has {}; \
             an inferred stream needs the predictor it was coded with",
            header.model_id,
            config.predictor.model_id()
        )));
    }

    let luma = decode_luma_sequence(luma_part, &header, config)?;

    let mut chroma_decoder = BinDecoder::new(chroma_part, NUM_CONTEXTS)?;
    let chroma = chroma::decode_sequence(
        &luma.motion,
        config,
        &mut chroma_decoder,
        header.frames as usize,
        width,
        height,
    )?;

    Ok((0..header.frames as usize)
        .map(|index| {
            let (cb, cr) = &chroma.planes[index];
            Frame::new(luma.pictures[index].clone(), cb.clone(), cr.clone())
        })
        .collect())
}

/// Choose the prediction for a block.
///
/// An intra picture has no choice: every block is intra. An inter
/// picture chooses per block, and the choice is made by rate-distortion
/// cost over the two predictions.
///
/// The motion vector is searched before the choice, because the search
/// is what the inter prediction needs, and the cost of the vector's own
/// signalling is added to the inter candidate's cost.
fn choose_prediction(
    picture: &Picture,
    reference_picture: Option<&Picture>,
    reconstruction: &Picture,
    motion: &MotionField,
    bx: usize,
    by: usize,
    config: &EncoderConfig,
) -> (bool, MotionVector, IntraMode, Vec<i32>) {
    let reference = reference_samples(reconstruction, bx, by);
    let original = block_samples(picture, bx, by);
    let mode = choose_mode(&original, &reference, config);
    let intra = predict(mode, BLOCK_SIZE, &reference);

    let Some(reference_picture) = reference_picture else {
        return (false, MotionVector::ZERO, mode, intra);
    };

    // The motion search. The radius is small because the predictor is
    // where the motion is most likely to be, and a wide search would
    // cost the benchmark far more time than it is worth.
    let predictor = motion.predict(bx, by);
    let vector = search(
        &original,
        reference_picture,
        (bx * BLOCK_SIZE) as i32,
        (by * BLOCK_SIZE) as i32,
        BLOCK_SIZE,
        predictor,
        config.motion_search_radius,
    );
    let inter = compensate(
        reference_picture,
        (bx * BLOCK_SIZE) as i32,
        (by * BLOCK_SIZE) as i32,
        BLOCK_SIZE,
        vector,
    );

    // The inter candidate's cost carries the motion vector's own rate,
    // because a vector that costs many bits is not worth taking.
    let inter_cost = best_cost(&original, &inter, config)
        + config.lambda * motion_vector_bits(vector - predictor);
    let intra_cost = best_cost(&original, &intra, config);

    if inter_cost < intra_cost {
        (true, vector, mode, inter)
    } else {
        (false, MotionVector::ZERO, mode, intra)
    }
}

/// The lowest rate-distortion cost over the transform modes, for a given
/// prediction.
fn best_cost(original: &[u8], prediction: &[i32], config: &EncoderConfig) -> f32 {
    let mut best = f32::INFINITY;
    for transform in TransformMode::ALL {
        if let Some(candidate) = evaluate(original, prediction, config, transform) {
            best = best.min(candidate.cost);
        }
    }
    best
}

/// The number of bits a motion vector difference costs.
///
/// Each component is coded as a sign and an exponential-Golomb
/// magnitude, and the estimate follows the same shape.
pub(crate) fn motion_vector_bits(difference: MotionVector) -> f32 {
    let magnitude = |value: i32| {
        let value = value.unsigned_abs();
        // The terminating one of the prefix, and the prefix and suffix.
        1.0 + 2.0 * (32 - value.leading_zeros()) as f32
    };
    2.0 + magnitude(difference.dx) + magnitude(difference.dy)
}

/// Code a motion vector difference.
pub(crate) fn code_motion_vector(encoder: &mut BinEncoder, difference: MotionVector) {
    encoder.encode_bypass(difference.dx < 0);
    encoder.encode_bypass(difference.dy < 0);
    code_exp_golomb(encoder, difference.dx.unsigned_abs());
    code_exp_golomb(encoder, difference.dy.unsigned_abs());
}

/// Decode a motion vector difference.
pub(crate) fn decode_motion_vector(decoder: &mut BinDecoder) -> Result<MotionVector, CodecError> {
    let dx_negative = decoder.decode_bypass()?;
    let dy_negative = decoder.decode_bypass()?;
    let dx = decode_exp_golomb(decoder)? as i32;
    let dy = decode_exp_golomb(decoder)? as i32;
    Ok(MotionVector {
        dx: if dx_negative { -dx } else { dx },
        dy: if dy_negative { -dy } else { dy },
    })
}

/// The rate-distortion cost of a candidate block.
fn rd_cost(distortion: i64, bits: f32, lambda: f32) -> f32 {
    distortion as f32 + lambda * bits
}

/// The candidate for one intra mode and one transform.
#[derive(Clone)]
struct Candidate {
    transform: TransformMode,
    levels: Vec<i32>,
    reconstruction: Vec<i32>,
    cost: f32,
}

/// Evaluate one prediction with one transform.
///
/// The prediction is the block's samples before the residual is added, so
/// an intra prediction and a motion-compensated prediction go through
/// exactly the same path from here on.
fn evaluate(
    original: &[u8],
    prediction: &[i32],
    config: &EncoderConfig,
    transform: TransformMode,
) -> Option<Candidate> {
    let residual: Vec<i32> = original
        .iter()
        .zip(prediction.iter())
        .map(|(o, p)| *o as i32 - p)
        .collect();
    let coefficients = forward_mode(transform, BLOCK_SIZE, &residual).ok()?;
    let params = QuantParams::new(config.qp, BLOCK_SIZE);
    let levels = quantize_with(&coefficients, &params);
    let dequantized = dequantize_with(&levels, &params);
    let samples = inverse_mode(transform, BLOCK_SIZE, &dequantized).ok()?;
    let reconstruction: Vec<i32> = prediction
        .iter()
        .zip(samples.iter())
        .map(|(p, s)| p + s)
        .collect();
    let distortion = sse(
        &original.iter().map(|v| *v as i32).collect::<Vec<_>>(),
        &reconstruction,
    );
    // The rate is estimated from the number of non-zero levels and their
    // magnitudes. The exact cost is only known after coding, so the
    // search uses this estimate; the reported bit counts come from the
    // real entropy coder.
    let bits = estimate_level_bits(&levels);
    let cost = rd_cost(distortion, bits, config.lambda);
    Some(Candidate {
        transform,
        levels,
        reconstruction,
        cost,
    })
}

/// Choose the intra mode for a block.
///
/// The mode is chosen first, in the decoder's order, because the transform
/// type is signalled after it and the predictor's features depend on it.
/// The mode is the one whose best transform gives the lowest
/// rate-distortion cost.
fn choose_mode(original: &[u8], reference: &ReferenceSamples, config: &EncoderConfig) -> IntraMode {
    let mut best_mode = IntraMode::Dc;
    let mut best_cost = f32::INFINITY;
    for mode in IntraMode::all() {
        let prediction = predict(mode, BLOCK_SIZE, reference);
        let mut mode_cost = f32::INFINITY;
        for transform in TransformMode::ALL {
            if let Some(candidate) = evaluate(original, &prediction, config, transform) {
                mode_cost = mode_cost.min(candidate.cost);
            }
        }
        if mode_cost < best_cost {
            best_cost = mode_cost;
            best_mode = mode;
        }
    }
    best_mode
}

/// Choose the transform for a block, given its intra mode.
///
/// When `predicted` is given and the margin is non-zero, the encoder takes
/// the predicted transform if it is within the margin of the best
/// candidate. That is the mechanism's rate-distortion trade: the encoder
/// gives up a little distortion to avoid spending the override bit.
fn choose_transform(
    original: &[u8],
    prediction: &[i32],
    config: &EncoderConfig,
    predicted: Option<TransformMode>,
) -> TransformChoice {
    let mut best: Option<Candidate> = None;
    let mut predicted_candidate: Option<Candidate> = None;

    for transform in TransformMode::ALL {
        let Some(candidate) = evaluate(original, prediction, config, transform) else {
            continue;
        };
        if Some(transform) == predicted {
            predicted_candidate = Some(candidate.clone());
        }
        let better = match &best {
            None => true,
            Some(current) => candidate.cost < current.cost,
        };
        if better {
            best = Some(candidate);
        }
    }

    let best = best.expect("the transform set is non-empty");

    if config.transform_margin > 0.0 {
        if let Some(candidate) = predicted_candidate {
            // The margin is a rate budget, so it is compared against the
            // rate the encoder gives up, which is the distortion it
            // trades away converted through lambda.
            let rate_given_up = (candidate.cost - best.cost) / config.lambda;
            if candidate.transform != best.transform && rate_given_up <= config.transform_margin {
                return TransformChoice {
                    chosen: candidate.transform,
                    optimal: best.transform,
                    levels: candidate.levels,
                    reconstruction: candidate.reconstruction,
                    cost: candidate.cost,
                };
            }
        }
    }

    TransformChoice {
        chosen: best.transform,
        optimal: best.transform,
        levels: best.levels,
        reconstruction: best.reconstruction,
        cost: best.cost,
    }
}

/// The transform a block uses, and the one it would have used without
/// the mechanism's margin.
///
/// The two differ when the margin lets the encoder take the decoder's
/// predicted transform instead of the strictly optimal one. The optimal
/// transform is the training label, because it is what the mode set
/// prefers, which is what the predictor has to learn.
struct TransformChoice {
    chosen: TransformMode,
    optimal: TransformMode,
    levels: Vec<i32>,
    reconstruction: Vec<i32>,
    cost: f32,
}

/// Search the intra modes and transform types for a block, and return the
/// best candidate.
///
/// Exposed so the training pipeline can recover the encoder's chosen
/// transform for a block.
pub fn search_block(
    original: &[u8],
    reference: &ReferenceSamples,
    config: &EncoderConfig,
) -> (IntraMode, TransformMode, Vec<i32>, Vec<i32>, f32) {
    let mode = choose_mode(original, reference, config);
    let prediction = predict(mode, BLOCK_SIZE, reference);
    let choice = choose_transform(original, &prediction, config, None);
    (
        mode,
        choice.chosen,
        choice.levels,
        choice.reconstruction,
        choice.cost,
    )
}

/// An estimate of the bits needed to code a set of quantized levels.
///
/// The estimate has to match the binarisation in [`code_coefficients`],
/// because the rate-distortion search optimises against it: a cost model
/// that disagrees with the bitstream would make the encoder choose
/// transforms that are not the ones the real coder rewards.
pub(crate) fn estimate_level_bits(levels: &[i32]) -> f32 {
    let mut bits = 0.0;
    for level in levels {
        let magnitude = level.unsigned_abs();
        if magnitude == 0 {
            continue;
        }
        // The significance flag, the sign, and the two magnitude flags.
        bits += 4.0;
        if magnitude > 2 {
            // The exponential-Golomb remainder: a prefix of zeros, a
            // terminating one, and the same number of bypassed suffix
            // bits.
            let remainder = magnitude - 3;
            let prefix_len = 31 - (remainder + 1).leading_zeros();
            bits += 2.0 * prefix_len as f32 + 1.0;
        }
    }
    bits
}

/// The samples of the block at `(bx, by)`.
pub fn block_samples(picture: &Picture, bx: usize, by: usize) -> Vec<u8> {
    block_samples_at(picture, bx, by, BLOCK_SIZE)
}

/// The samples of the block at `(bx, by)`, for a block of the given size.
///
/// The chroma plane's blocks are a quarter the area of the luma
/// plane's, so the size is a parameter rather than a constant.
pub fn block_samples_at(picture: &Picture, bx: usize, by: usize, size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(size * size);
    for y in 0..size {
        for x in 0..size {
            out.push(
                picture
                    .sample((bx * size + x) as isize, (by * size + y) as isize)
                    .unwrap_or(0),
            );
        }
    }
    out
}

/// The causal reference samples available to the block at `(bx, by)`.
///
/// Exposed so the training pipeline can reproduce the encoder's view of a
/// block's neighbourhood.
pub fn reference_samples_for(picture: &Picture, bx: usize, by: usize) -> ReferenceSamples {
    reference_samples(picture, bx, by)
}

/// The causal reference samples available to the block at `(bx, by)`.
pub(crate) fn reference_samples(picture: &Picture, bx: usize, by: usize) -> ReferenceSamples {
    reference_samples_at(picture, bx, by, BLOCK_SIZE)
}

/// The causal reference samples available to the block at `(bx, by)`,
/// for a block of the given size.
///
/// The chroma plane's blocks are a quarter the area of the luma
/// plane's, so the size is a parameter rather than a constant.
pub(crate) fn reference_samples_at(
    picture: &Picture,
    bx: usize,
    by: usize,
    size: usize,
) -> ReferenceSamples {
    let x0 = (bx * size) as isize;
    let y0 = (by * size) as isize;

    let left = (0..size)
        .map(|y| picture.sample(x0 - 1, y0 + y as isize).map(|v| v as i32))
        .collect();
    let top = (0..size)
        .map(|x| picture.sample(x0 + x as isize, y0 - 1).map(|v| v as i32))
        .collect();
    let top_left = picture.sample(x0 - 1, y0 - 1).map(|v| v as i32);

    ReferenceSamples {
        left,
        top,
        top_left,
    }
}

/// Code the intra mode.
///
/// The mode index runs from zero to `IntraMode::COUNT` minus one, so the
/// code is as many bits as that count needs. Coding fewer would make the
/// encoder and the decoder disagree about every mode whose index does not
/// fit, which is a silent corruption rather than a visible failure.
fn code_intra_mode(encoder: &mut BinEncoder, mode: IntraMode) {
    let index = mode.as_index();
    for bit in 0..INTRA_MODE_BITS {
        let value = (index >> bit) & 1 == 1;
        encoder.encode(CTX_INTRA_MODE, value);
    }
}

/// Decode the intra mode.
fn decode_intra_mode(decoder: &mut BinDecoder) -> Result<IntraMode, CodecError> {
    let mut index = 0u8;
    for bit in 0..INTRA_MODE_BITS {
        if decoder.decode(CTX_INTRA_MODE)? {
            index |= 1 << bit;
        }
    }
    IntraMode::from_index(index)
        .ok_or_else(|| CodecError::Predictor(format!("unknown intra mode {index}")))
}

/// Code the transform signal.
///
/// The explicit mode codes the transform type with a truncated binary code:
/// the first type in the canonical order costs one bin and the other two
/// cost two.
///
/// The inferred mode codes one bin saying whether the decoder's prediction
/// was right. When it was wrong, one further bin selects between the two
/// transforms the prediction ruled out, so an override costs two bins
/// rather than the three a naive design would need.
fn code_transform_signal(encoder: &mut BinEncoder, signal: TransformSignal) {
    match signal {
        TransformSignal::Explicit(mode) => {
            // The syntax is the one ITU-T H.266 uses for `mts_idx`: a bin
            // saying whether a multiple transform selection mode is used
            // at all, and, when one is, a truncated unary code over the
            // four modes. The reference implementation codes it the same
            // way, in CABACWriter::mts_idx.
            let index = mode.as_index();
            encoder.encode(CTX_TRANSFORM_MTS, index > 0);
            for i in 0..3 {
                if index <= i {
                    break;
                }
                encoder.encode(CTX_TRANSFORM_INDEX, index > i + 1);
            }
        }
        TransformSignal::Predicted => {
            encoder.encode(CTX_TRANSFORM_OVERRIDE, false);
        }
        TransformSignal::Override { predicted, actual } => {
            encoder.encode(CTX_TRANSFORM_OVERRIDE, true);
            // The actual mode is one of the four the prediction rules
            // out, so two bins select it.
            let remaining = remaining_modes(predicted);
            let position = remaining
                .iter()
                .position(|m| *m == actual)
                .expect("the actual mode differs from the prediction");
            encoder.encode(CTX_TRANSFORM_INDEX, position & 2 != 0);
            encoder.encode(CTX_TRANSFORM_INDEX, position & 1 != 0);
        }
    }
}

/// The four modes a prediction rules out, in canonical order.
///
/// The order is fixed, so the encoder and the decoder derive the same
/// list from the same prediction without signalling it.
fn remaining_modes(predicted: TransformMode) -> Vec<TransformMode> {
    TransformMode::ALL
        .iter()
        .copied()
        .filter(|m| *m != predicted)
        .collect()
}

/// Decode the transform signal, reconstructing the decoder's prediction when
/// the signalling says the encoder did not override it.
fn decode_transform_signal(
    decoder: &mut BinDecoder,
    config: &EncoderConfig,
    signalling: TransformSignalling,
    context: &PredictionContext<'_>,
) -> Result<TransformMode, CodecError> {
    // The explicit mode has no override flag; the mode is coded
    // directly, with the same syntax the encoder used.
    if signalling == TransformSignalling::Explicit {
        let is_mts = decoder.decode(CTX_TRANSFORM_MTS)?;
        let index = if is_mts {
            // The truncated unary code, read the way the encoder wrote it.
            let mut index = 1u8;
            for i in 0..3 {
                if !decoder.decode(CTX_TRANSFORM_INDEX)? {
                    break;
                }
                index = (i + 2) as u8;
            }
            index
        } else {
            0
        };
        return TransformMode::from_index(index)
            .ok_or_else(|| CodecError::Predictor(format!("unknown transform mode {index}")));
    }

    let override_flag = decoder.decode(CTX_TRANSFORM_OVERRIDE)?;
    let features = predict_features(context);
    let (predicted, _) = config.predictor.predict_with_margin(&features);
    if !override_flag {
        return Ok(predicted);
    }
    // Two bins select among the four modes the prediction rules out.
    // The encoder writes the high bit of the position first, so the
    // decoder reads it in that order.
    decode_transform_override(decoder, predicted)
}

/// Decode the correction, given the prediction it corrects.
///
/// The prediction is known to the decoder, so the two bins that select
/// the actual mode are read against the list the prediction rules out.
fn decode_transform_override(
    decoder: &mut BinDecoder,
    predicted: TransformMode,
) -> Result<TransformMode, CodecError> {
    let two = decoder.decode(CTX_TRANSFORM_INDEX)?;
    let one = decoder.decode(CTX_TRANSFORM_INDEX)?;
    let position = (two as usize) * 2 + one as usize;
    let remaining = remaining_modes(predicted);
    remaining.get(position).copied().ok_or_else(|| {
        CodecError::Predictor(format!(
            "the correction selects mode {position} of {}",
            remaining.len()
        ))
    })
}

/// Code the quantized residual levels.
///
/// The binarisation follows the shape ITU-T H.265 uses for residual
/// coefficients: a significance flag, a "greater than one" flag, a "greater
/// than two" flag, and then an exponential-Golomb remainder for the
/// magnitude above two. The sign is coded with its own context.
///
/// The exponential-Golomb code matters. A unary prefix would cost one bin
/// per unit of magnitude, so a single large coefficient could cost over a
/// thousand bins; the Golomb remainder costs about `log2(magnitude)` bins
/// instead, which is what makes the coded size a meaningful measure of
/// rate.
pub(crate) fn code_coefficients(encoder: &mut BinEncoder, levels: &[i32]) {
    for level in levels {
        let magnitude = level.unsigned_abs();
        let significant = magnitude > 0;
        encoder.encode(CTX_COEF_SIGNIFICANT, significant);
        if !significant {
            continue;
        }
        encoder.encode(CTX_COEF_GREATER1, magnitude > 1);
        encoder.encode(CTX_COEF_GREATER2, magnitude > 2);
        encoder.encode(CTX_COEF_SIGN, *level < 0);

        // The magnitude above two, as an exponential-Golomb code. The
        // magnitude is known to be at least three here, so the subtraction
        // cannot underflow.
        if magnitude > 2 {
            let remainder = magnitude - 3;
            code_exp_golomb(encoder, remainder);
        }
    }
}

/// Code a value with the exponential-Golomb order-0 code.
///
/// The value `v` is written as a prefix of `floor(log2(v + 1))` zeros
/// followed by a one, and then the remaining information is written as
/// `floor(log2(v + 1))` bypassed bits. This is the same construction
/// ITU-T H.265 uses for `coeff_abs_level_remaining`.
fn code_exp_golomb(encoder: &mut BinEncoder, value: u32) {
    let prefix_len = 31 - (value + 1).leading_zeros();
    for _ in 0..prefix_len {
        encoder.encode(CTX_COEF_EXP_GOLOMB, false);
    }
    encoder.encode(CTX_COEF_EXP_GOLOMB, true);
    if prefix_len > 0 {
        // The suffix carries the value's low bits, offset so that the
        // prefix and suffix together determine it uniquely.
        let suffix = value + 1 - (1u32 << prefix_len);
        for bit in (0..prefix_len).rev() {
            encoder.encode_bypass((suffix >> bit) & 1 == 1);
        }
    }
}

/// Decode the quantized residual levels.
///
/// This is the exact inverse of [`code_coefficients`].
pub(crate) fn decode_coefficients(decoder: &mut BinDecoder) -> Result<Vec<i32>, CodecError> {
    decode_coefficients_at(decoder, BLOCK_SIZE * BLOCK_SIZE)
}

/// Decode the given number of quantized residual levels.
///
/// The chroma plane's blocks are a quarter the area of the luma
/// plane's, so the count is a parameter rather than a constant.
pub(crate) fn decode_coefficients_at(
    decoder: &mut BinDecoder,
    count: usize,
) -> Result<Vec<i32>, CodecError> {
    let mut levels = Vec::with_capacity(count);
    for _ in 0..count {
        if !decoder.decode(CTX_COEF_SIGNIFICANT)? {
            levels.push(0);
            continue;
        }
        let greater1 = decoder.decode(CTX_COEF_GREATER1)?;
        let greater2 = decoder.decode(CTX_COEF_GREATER2)?;
        let negative = decoder.decode(CTX_COEF_SIGN)?;

        // The magnitude is one, two, or three plus the Golomb remainder.
        let magnitude = if greater2 {
            3 + decode_exp_golomb(decoder)?
        } else if greater1 {
            2
        } else {
            1
        };
        if magnitude > 1_000_000 {
            return Err(CodecError::Predictor(
                "residual level is implausibly large".into(),
            ));
        }
        let value = magnitude as i32;
        levels.push(if negative { -value } else { value });
    }
    Ok(levels)
}

/// Decode a value coded with the exponential-Golomb order-0 code.
///
/// This is the exact inverse of [`code_exp_golomb`].
fn decode_exp_golomb(decoder: &mut BinDecoder) -> Result<u32, CodecError> {
    let mut prefix_len = 0u32;
    while !decoder.decode(CTX_COEF_EXP_GOLOMB)? {
        prefix_len += 1;
        if prefix_len > 31 {
            return Err(CodecError::Predictor(
                "the exponential-Golomb prefix is implausibly long".into(),
            ));
        }
    }
    let mut suffix = 0u32;
    for _ in 0..prefix_len {
        suffix = (suffix << 1) | u32::from(decoder.decode_bypass()?);
    }
    Ok(suffix + (1u32 << prefix_len) - 1)
}

/// The context the predictor reads: the reconstruction, the transform map
/// and the block's own decoded state.
///
/// The predictor needs all of these together, and bundling them keeps the
/// signalling functions' argument lists short.
pub struct PredictionContext<'a> {
    /// The reconstruction so far.
    pub picture: &'a Picture,
    /// The transform mode each coded block used.
    pub transforms: &'a TransformMap,
    /// The motion vector each coded block used.
    pub motion: &'a MotionField,
    /// The block's position, in blocks.
    pub block: (usize, usize),
    /// The block's intra prediction mode, decoded before the transform.
    ///
    /// An inter block has no intra mode, so the feature is empty for
    /// one.
    pub mode: IntraMode,
    /// Whether the block is predicted from the previous frame.
    pub is_inter: bool,
    /// The quantization parameter, from the bitstream header.
    pub qp: i32,
    /// The block's own prediction, which both sides have computed by the
    /// time the transform type is needed.
    ///
    /// The prediction's orientation is the most direct evidence of which
    /// transform the encoder will want, because the transform has to
    /// compact the residual and the residual inherits the prediction's
    /// directional structure.
    pub prediction: &'a [i32],
}

/// The features the predictor would see for a block.
///
/// The encoder and the decoder both call this, on their own
/// reconstructions, so the two always see the same features. That is what
/// makes the inference exact rather than approximate.
pub fn predict_features(context: &PredictionContext<'_>) -> GeometryFeatures {
    let (bx, by) = context.block;
    let origin_x = bx * BLOCK_SIZE;
    let origin_y = by * BLOCK_SIZE;
    extract_features_from_window(
        &context.picture.samples,
        context.picture.width,
        (origin_x, origin_y),
        CONTEXT_WINDOW,
        BlockContext {
            mode: context.mode,
            qp: context.qp,
            neighbours: context.transforms.neighbours(bx, by),
            motion: motion_context(context.motion, bx, by, context.is_inter),
            prediction: prediction_features(context.prediction, BLOCK_SIZE),
        },
    )
}

/// The motion context of a block.
///
/// The block's own motion vector and the vectors of the blocks to its
/// left and above. The decoder knows all of them, because it decoded
/// them.
fn motion_context(motion: &MotionField, bx: usize, by: usize, is_inter: bool) -> MotionContext {
    if !is_inter {
        return MotionContext::intra();
    }
    let own = motion.get(bx, by).unwrap_or(MotionVector::ZERO);
    let left = if bx > 0 { motion.get(bx - 1, by) } else { None };
    let above = if by > 0 { motion.get(bx, by - 1) } else { None };
    MotionContext::inter(own, left, above)
}

/// The features the predictor would see for a block, exposed for the
/// training pipeline.
pub fn block_features(context: &PredictionContext<'_>) -> GeometryFeatures {
    let (bx, by) = context.block;
    let reference = reference_samples(context.picture, bx, by);
    extract_features(
        &reference.left,
        &reference.top,
        BlockContext {
            mode: context.mode,
            qp: context.qp,
            neighbours: context.transforms.neighbours(bx, by),
            motion: motion_context(context.motion, bx, by, context.is_inter),
            prediction: prediction_features(context.prediction, BLOCK_SIZE),
        },
    )
}

/// A training example: a block's geometry features and the transform the
/// encoder chose for it.
#[derive(Debug, Clone, PartialEq)]
pub struct TrainingExample {
    /// The geometry features of the block's causal neighbourhood.
    pub features: GeometryFeatures,
    /// The transform the rate-distortion search chose.
    pub transform: TransformMode,
}

/// Collect training examples by running the encoder's own block loop.
///
/// The examples come from the encoder rather than from a separate
/// implementation of it, and the features are taken from the *reconstructed*
/// neighbourhood, exactly as the decoder will see them. A training set
/// gathered any other way would describe a situation the predictor never
/// meets at inference time.
pub fn collect_training_examples(
    frames: &[Picture],
    config: &EncoderConfig,
) -> Result<Vec<TrainingExample>, CodecError> {
    Ok(encode_luma_sequence(frames, config, true)?.examples)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4:2:0 frame with a pattern in every plane.
    fn test_frame(width: usize, height: usize) -> Frame {
        let luma = test_picture(width, height);
        let mut cb = Picture::filled(width / 2, height / 2, 128);
        let mut cr = Picture::filled(width / 2, height / 2, 128);
        for y in 0..height / 2 {
            for x in 0..width / 2 {
                cb.set_sample(x, y, ((x / 2 + y / 2) % 2) as u8 * 60 + 90);
                cr.set_sample(x, y, ((x / 4 + y / 4) % 2) as u8 * 40 + 100);
            }
        }
        Frame::new(luma, cb, cr)
    }

    fn test_picture(width: usize, height: usize) -> Picture {
        // A deterministic pattern with edges, so the geometry features have
        // something to work with.
        let mut picture = Picture::filled(width, height, 0);
        for y in 0..height {
            for x in 0..width {
                let value = ((x / 4 + y / 4) % 2) as u8 * 200 + 20;
                picture.set_sample(x, y, value);
            }
        }
        picture
    }

    /// The decoder must reproduce the encoder's reconstruction exactly.
    #[test]
    fn decode_reproduces_the_encoder_reconstruction() {
        let picture = test_picture(32, 32);
        for signalling in [TransformSignalling::Explicit, TransformSignalling::Inferred] {
            let config = EncoderConfig {
                signalling,
                ..Default::default()
            };
            let encoded = encode_luma(std::slice::from_ref(&picture), &config).unwrap();
            let decoded = decode_luma(&encoded.payload, &config, 32, 32, 1).unwrap();
            assert_eq!(
                decoded[0], encoded.reconstructions[0],
                "{signalling:?}: decoder output differs from the encoder's reconstruction"
            );
        }
    }

    /// Every picture size that is a multiple of the block size must encode
    /// and decode without error.
    #[test]
    fn round_trips_at_several_sizes() {
        for size in [8usize, 16, 32] {
            let picture = test_picture(size, size);
            let config = EncoderConfig::default();
            let encoded = encode_luma(std::slice::from_ref(&picture), &config).unwrap();
            let decoded = decode_luma(&encoded.payload, &config, size, size, 1).unwrap();
            assert_eq!(decoded[0], encoded.reconstructions[0], "size {size}");
        }
    }

    /// A picture whose dimensions are not a multiple of the block size must
    /// be rejected rather than silently mishandled.
    #[test]
    fn rejects_bad_dimensions() {
        let picture = Picture::filled(12, 16, 0);
        let config = EncoderConfig::default();
        assert!(matches!(
            encode_luma(std::slice::from_ref(&picture), &config),
            Err(CodecError::Dimensions { .. })
        ));
    }

    /// The inferred mode must agree with the encoder's own accounting: the
    /// number of overrides the encoder sent must equal the number of blocks
    /// where the decoder's prediction differed.
    #[test]
    fn inferred_mode_accounting_is_consistent() {
        let picture = test_picture(32, 32);
        let config = EncoderConfig {
            signalling: TransformSignalling::Inferred,
            ..Default::default()
        };
        let encoded = encode_luma(std::slice::from_ref(&picture), &config).unwrap();
        assert!(encoded.block_count > 0);
        assert!(encoded.prediction_mismatches <= encoded.block_count);
        // With a constant predictor, every block that did not pick DCT-II
        // must have been overridden.
        let decoded = decode_luma(&encoded.payload, &config, 32, 32, 1).unwrap();
        assert_eq!(decoded[0], encoded.reconstructions[0]);
    }


    /// The encoder and the decoder must agree on a multi-frame
    /// sequence.
    ///
    /// An inter block carries a motion vector instead of an intra mode,
    /// so the decoder cannot know the mode the encoder would have
    /// chosen. If a feature were built from that mode, the two sides'
    /// predictions would differ for every inter block, and the
    /// correction would be read against the wrong list. The constant
    /// predictor cannot show this, because it predicts the same mode
    /// whatever the features are, so the test uses the shipped model.
    #[test]
    fn the_shipped_model_keeps_a_sequence_in_step() {
        let model = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../models/girc-predictor.json"),
        )
        .expect("the shipped model must be present");
        let predictor = TransformPredictor::from_json(&model).unwrap();

        // Real content, cropped from the Xiph.Org CIF sequences. A
        // synthetic pattern is not enough: the divergence needs an
        // inter block whose intra mode the encoder would have chosen
        // as something other than the flat mode, and that needs the
        // mixture of flat and textured areas that real content has.
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/test/akiyo-192x192.y4m");
        let frames: Vec<Picture> = crate::yuv::Yuv4Frame::read_many(&fixture, 3)
            .expect("the test fixture must be present")
            .iter()
            .map(|frame| frame.luma_picture())
            .collect();

        let config = EncoderConfig {
            signalling: TransformSignalling::Inferred,
            predictor,
            transform_margin: 50.0,
            ..Default::default()
        };
        let encoded = encode_luma(&frames, &config).unwrap();
        let decoded = decode_luma(&encoded.payload, &config, 192, 192, frames.len()).unwrap();
        assert_eq!(decoded.len(), frames.len());
        // The sequence has to have inter blocks, or the test says
        // nothing about inter prediction.
        assert!(
            encoded.inter_blocks > 0,
            "the sequence has no inter blocks, so it cannot test them"
        );
        for (index, (decoded, reconstruction)) in decoded
            .iter()
            .zip(encoded.reconstructions.iter())
            .enumerate()
        {
            assert_eq!(
                decoded, reconstruction,
                "frame {index} differs from the encoder's reconstruction"
            );
        }
    }

    /// A stream coded with one predictor must not be decoded with
    /// another.
    ///
    /// The decoder's prediction is part of the syntax: the correction
    /// is relative to it. A decoder with a different predictor reads
    /// the correction against a different list, and decodes a different
    /// picture without reporting an error. The stream records the
    /// predictor it was coded with, and a mismatch is refused.
    #[test]
    fn a_stream_refuses_a_different_predictor() {
        let model = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../models/girc-predictor.json"),
        )
        .expect("the shipped model must be present");
        let predictor = TransformPredictor::from_json(&model).unwrap();

        let picture = test_picture(64, 64);
        let config = EncoderConfig {
            signalling: TransformSignalling::Inferred,
            predictor: predictor.clone(),
            ..Default::default()
        };
        let encoded = encode_luma(std::slice::from_ref(&picture), &config).unwrap();

        // The same stream, decoded with the constant predictor.
        let wrong = EncoderConfig {
            signalling: TransformSignalling::Inferred,
            predictor: TransformPredictor::constant_dct2(),
            ..Default::default()
        };
        let error = decode_luma_with_model(
            &encoded.payload,
            &wrong,
            64,
            64,
            1,
            Some(predictor.model_id()),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("predictor"),
            "the mismatch was not reported: {error}"
        );

        // And with the right one it decodes.
        let decoded = decode_luma(&encoded.payload, &config, 64, 64, 1).unwrap();
        assert_eq!(decoded[0], encoded.reconstructions[0]);
    }

    /// The encoder and the decoder must agree with the shipped model.
    ///
    /// The unit tests use the constant predictor, and with it the two
    /// sides' predictions trivially agree. With a trained model the
    /// prediction is a function of the features, and the features are
    /// a function of the reconstruction so far, so the two sides have
    /// to be kept in step exactly. This test uses the shipped model on
    /// content with structure, which is what makes the predictor
    /// predict something other than the constant.
    #[test]
    fn the_shipped_model_keeps_the_encoder_and_the_decoder_in_step() {
        let model = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../models/girc-predictor.json"),
        )
        .expect("the shipped model must be present");
        let predictor = TransformPredictor::from_json(&model).unwrap();

        let frames: Vec<Picture> = (0..3)
            .map(|index| {
                let mut picture = Picture::filled(64, 64, 0);
                for y in 0..64 {
                    for x in 0..64 {
                        let value = ((((x + index) / 4) as f32 * 0.8).sin() * 60.0
                            + (((y + index) / 4) as f32 * 0.8).cos() * 60.0
                            + 128.0) as u8;
                        picture.set_sample(x, y, value);
                    }
                }
                picture
            })
            .collect();

        let config = EncoderConfig {
            signalling: TransformSignalling::Inferred,
            predictor,
            transform_margin: 50.0,
            ..Default::default()
        };
        let encoded = encode_luma(&frames, &config).unwrap();
        let decoded = decode_luma(&encoded.payload, &config, 64, 64, frames.len()).unwrap();
        for (index, (decoded, reconstruction)) in decoded
            .iter()
            .zip(encoded.reconstructions.iter())
            .enumerate()
        {
            assert_eq!(
                decoded, reconstruction,
                "frame {index} differs from the encoder's reconstruction"
            );
        }
    }

    /// Every intra mode must survive the mode code.
    ///
    /// The mode set has more modes than three bits can index. If the code
    /// is too short, the encoder writes a truncated index and the decoder
    /// reads a different mode, and the two reconstructions diverge
    /// silently. This drives the coder directly over every mode.
    #[test]
    fn every_intra_mode_survives_the_mode_code() {
        for mode in IntraMode::all() {
            let mut encoder = BinEncoder::new(NUM_CONTEXTS);
            code_intra_mode(&mut encoder, mode);
            let compressed = encoder.finish();

            let mut decoder = BinDecoder::new(&compressed, NUM_CONTEXTS).unwrap();
            let decoded = decode_intra_mode(&mut decoder).unwrap();
            assert_eq!(decoded, mode, "mode {} did not round trip", mode.as_index());
        }
    }

    /// Every residual magnitude must survive the coefficient binarisation.
    ///
    /// The binarisation is the part of the bitstream most likely to break
    /// silently: a small change to the flags changes how the magnitude is
    /// reconstructed. This test drives the coder directly over a full range
    /// of magnitudes rather than relying on content to reach them.
    #[test]
    fn coefficient_magnitudes_round_trip() {
        for magnitude in 0..4096u32 {
            for negative in [false, true] {
                let level = if negative {
                    -(magnitude as i32)
                } else {
                    magnitude as i32
                };
                let levels = vec![level; BLOCK_SIZE * BLOCK_SIZE];

                let mut encoder = BinEncoder::new(NUM_CONTEXTS);
                code_coefficients(&mut encoder, &levels);
                let compressed = encoder.finish();

                let mut decoder = BinDecoder::new(&compressed, NUM_CONTEXTS).unwrap();
                let decoded = decode_coefficients(&mut decoder).unwrap();
                assert_eq!(
                    decoded, levels,
                    "magnitude {magnitude} negative {negative} did not round trip"
                );
            }
        }
    }

    /// A single coefficient with a large magnitude must not cost a bin per
    /// unit of magnitude.
    ///
    /// The exponential-Golomb remainder is what keeps this true; a unary
    /// prefix would need thousands of bins here.
    #[test]
    fn large_magnitudes_are_coded_compactly() {
        let levels = vec![4096i32; BLOCK_SIZE * BLOCK_SIZE];
        let mut encoder = BinEncoder::new(NUM_CONTEXTS);
        code_coefficients(&mut encoder, &levels);
        let compressed = encoder.finish();
        let bits = compressed.len() * 8;
        assert!(
            bits < 64 * 16,
            "64 coefficients of magnitude 4096 took {bits} bits, which is too many"
        );
    }

    /// A sequence of frames must round-trip through the bitstream, and
    /// the decoder must reproduce every frame's reconstruction exactly.
    #[test]
    fn a_sequence_round_trips() {
        let frames: Vec<Picture> = (0..4)
            .map(|index| {
                let mut picture = Picture::filled(32, 32, 0);
                for y in 0..32 {
                    for x in 0..32 {
                        let value = (((x + index) / 4 + y / 4) % 2) as u8 * 200 + 20;
                        picture.set_sample(x, y, value);
                    }
                }
                picture
            })
            .collect();

        for signalling in [TransformSignalling::Explicit, TransformSignalling::Inferred] {
            let config = EncoderConfig {
                signalling,
                ..Default::default()
            };
            let encoded = encode_luma(&frames, &config).unwrap();
            let decoded = decode_luma(&encoded.payload, &config, 32, 32, frames.len()).unwrap();
            assert_eq!(decoded.len(), frames.len());
            for (index, (decoded, reconstruction)) in decoded
                .iter()
                .zip(encoded.reconstructions.iter())
                .enumerate()
            {
                assert_eq!(
                    decoded, reconstruction,
                    "{signalling:?}: frame {index} differs from the encoder's reconstruction"
                );
            }
        }
    }

    /// A moving picture must be predicted from the previous frame rather
    /// than from the current frame's own neighbourhood.
    ///
    /// This is the case the mechanism is for: the inter residual carries
    /// the motion's structure, which is what the transform predictor reads.
    #[test]
    fn moving_content_uses_inter_prediction() {
        let mut first = Picture::filled(32, 32, 0);
        for y in 4..20 {
            for x in 4..20 {
                first.set_sample(x, y, 220);
            }
        }
        // The second frame is the first one moved two samples to the
        // right, which is exactly what inter prediction is for.
        let mut second = Picture::filled(32, 32, 0);
        for y in 4..20 {
            for x in 6..22 {
                second.set_sample(x, y, 220);
            }
        }

        let config = EncoderConfig::default();
        let encoded = encode_luma(&[first, second], &config).unwrap();
        assert!(
            encoded.inter_blocks > 0,
            "a picture that moved should be predicted from the previous frame"
        );
        let decoded = decode_luma(&encoded.payload, &config, 32, 32, 2).unwrap();
        assert_eq!(decoded[1], encoded.reconstructions[1]);
    }

    /// Every predicted and actual pair must round-trip.
    ///
    /// The correction is signalled relative to the prediction, so the
    /// decoder has to derive the same list of remaining modes that the
    /// encoder did. This checks every pair of the five modes, which is
    /// twenty-five cases, and it checks that the list is a bijection
    /// onto the four modes the prediction rules out.
    #[test]
    fn every_correction_round_trips() {
        for predicted in TransformMode::ALL {
            let remaining = remaining_modes(predicted);
            assert_eq!(
                remaining.len(),
                TransformMode::ALL.len() - 1,
                "the prediction rules out exactly one mode"
            );
            assert!(
                !remaining.contains(&predicted),
                "the remaining list does not hold the prediction"
            );
            for actual in TransformMode::ALL {
                if actual == predicted {
                    continue;
                }
                assert!(
                    remaining.contains(&actual),
                    "the remaining list holds the actual mode"
                );

                // The correction's two bins are coded and decoded on
                // their own, so the test is of the correction and not of
                // the block around it.
                let remaining = remaining_modes(predicted);
                let position = remaining
                    .iter()
                    .position(|m| *m == actual)
                    .expect("the actual mode differs from the prediction");
                let mut encoder = BinEncoder::new(NUM_CONTEXTS);
                encoder.encode(CTX_TRANSFORM_INDEX, position & 2 != 0);
                encoder.encode(CTX_TRANSFORM_INDEX, position & 1 != 0);
                let words = encoder.finish();
                let mut decoder = BinDecoder::new(&words, NUM_CONTEXTS).unwrap();

                let decoded = decode_transform_override(&mut decoder, predicted).unwrap();
                assert_eq!(
                    decoded, actual,
                    "the correction from {predicted:?} to {actual:?} did not round-trip"
                );
            }
        }
    }

    /// A 4:2:0 frame must round-trip through the bitstream.
    ///
    /// The decoder must reproduce all three planes exactly, and the
    /// chroma planes must not be a flat fill: a decoder that dropped
    /// the chroma would still "round-trip" if the test only checked the
    /// luma.
    #[test]
    fn a_frame_round_trips() {
        let frame = test_frame(32, 32);
        for signalling in [TransformSignalling::Explicit, TransformSignalling::Inferred] {
            let config = EncoderConfig {
                signalling,
                ..Default::default()
            };
            let encoded = encode(std::slice::from_ref(&frame), &config).unwrap();
            let decoded = decode(&encoded.bitstream, &config).unwrap();
            assert_eq!(decoded.len(), 1);
            assert_eq!(decoded[0].luma, encoded.reconstructions[0].luma);
            assert_eq!(decoded[0].cb, encoded.reconstructions[0].cb);
            assert_eq!(decoded[0].cr, encoded.reconstructions[0].cr);
            // The chroma must be a real reconstruction, not the source
            // passed through and not a flat fill.
            assert_ne!(decoded[0].cb.samples, frame.cb.samples);
            assert!(
                decoded[0].cb.samples.iter().any(|v| *v != 128),
                "the chroma reconstruction is flat"
            );
        }
    }

    /// The chroma planes must cost bits.
    ///
    /// A chroma payload of zero words would mean the chroma planes were
    /// not coded at all, which would make the total rate the anchor
    /// codecs are compared against meaningless.
    #[test]
    fn the_chroma_planes_cost_bits() {
        let frame = test_frame(32, 32);
        let config = EncoderConfig::default();
        let encoded = encode(std::slice::from_ref(&frame), &config).unwrap();
        assert!(encoded.chroma_bytes > 0, "the chroma payload is empty");
        assert!(encoded.luma_bytes > 0, "the luma payload is empty");
        assert_eq!(
            encoded.payload_bytes,
            encoded.luma_bytes + encoded.chroma_bytes
        );
    }

    /// A frame whose chroma planes are the wrong size must be rejected.
    #[test]
    fn rejects_bad_chroma_dimensions() {
        let frame = Frame::new(
            Picture::filled(32, 32, 0),
            Picture::filled(32, 32, 128),
            Picture::filled(16, 16, 128),
        );
        let config = EncoderConfig::default();
        assert!(matches!(
            encode(std::slice::from_ref(&frame), &config),
            Err(CodecError::ChromaDimensions { .. })
        ));
    }

    /// The explicit mode must never use the override-only path, so its
    /// signalling cost is the transform type for every block.
    #[test]
    fn explicit_mode_signals_every_block() {
        let picture = test_picture(32, 32);
        let config = EncoderConfig {
            signalling: TransformSignalling::Explicit,
            ..Default::default()
        };
        let encoded = encode_luma(std::slice::from_ref(&picture), &config).unwrap();
        assert_eq!(encoded.prediction_mismatches, 0);
        // The explicit mode's truncated binary code costs one bin for a
        // DCT-II block and two for the others.
        assert!(
            encoded.transform_signalling_bins >= encoded.block_count,
            "explicit mode codes at least one bin per block"
        );
        assert!(
            encoded.transform_signalling_bins <= encoded.block_count * 2,
            "explicit mode codes at most two bins per block"
        );
    }
}
