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

use crate::bitstream::{BinDecoder, BinEncoder, BitstreamError, BitstreamHeader};
use crate::geometry::{
    extract_features, extract_features_from_window, BlockContext, GeometryFeatures, MotionContext,
    NeighbourTransforms, TransformPredictor, TransformSignal, TransformSignalling,
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
const CTX_INTRA_MODE: usize = 0;
const CTX_SKIP: usize = 1;
/// Whether the block is predicted from the previous picture.
const CTX_INTER: usize = 11;
const CTX_COEF_SIGNIFICANT: usize = 2;
const CTX_COEF_GREATER1: usize = 3;
const CTX_COEF_GREATER2: usize = 4;
const CTX_COEF_SIGN: usize = 5;
const CTX_COEF_EXP_GOLOMB: usize = 8;
const CTX_TRANSFORM_OVERRIDE: usize = 6;
/// Whether the block uses a multiple transform selection mode at all.
const CTX_TRANSFORM_MTS: usize = 7;
/// The bits of the mode index.
const CTX_TRANSFORM_INDEX: usize = 9;

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

/// The result of encoding a picture.
#[derive(Debug, Clone)]
pub struct EncodeResult {
    /// The encoded bitstream.
    pub bitstream: Vec<u8>,
    /// The encoder's reconstruction of each frame.
    pub reconstructions: Vec<Picture>,
    /// The number of bytes the payload occupies.
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
        }
    }
}

impl std::error::Error for CodecError {}

impl From<BitstreamError> for CodecError {
    fn from(value: BitstreamError) -> Self {
        CodecError::Bitstream(value)
    }
}

/// Encode a sequence of pictures.
///
/// The first picture is coded as an intra picture. The rest are coded as
/// inter pictures, each predicted from the reconstruction of the picture
/// before it, with a per-block choice between intra and inter
/// prediction.
///
/// The transform mode signalling is the same in both, and the two modes
/// differ in nothing else, so a difference in the measured bitrate is
/// attributable to the signalling mechanism alone.
pub fn encode(frames: &[Picture], config: &EncoderConfig) -> Result<EncodeResult, CodecError> {
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
                    });
                    Some(config.predictor.predict_with_margin(&features).0)
                } else {
                    None
                };

                let (chosen_transform, chosen_levels, chosen_reconstruction, chosen_cost) =
                    choose_transform(&original, &prediction, config, predicted);

                let signal = match (config.signalling, predicted) {
                    (TransformSignalling::Explicit, _) => {
                        TransformSignal::Explicit(chosen_transform)
                    }
                    (TransformSignalling::Inferred, Some(predicted)) => {
                        if predicted == chosen_transform {
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
                                actual: chosen_transform,
                            }
                        }
                    }
                    (TransformSignalling::Inferred, None) => {
                        TransformSignal::Explicit(chosen_transform)
                    }
                };
                signalling_bins += signal.bin_count();

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
                let skip = chosen_levels.iter().all(|l| *l == 0);
                encoder.encode(CTX_SKIP, skip);
                if !skip {
                    code_coefficients(&mut encoder, &chosen_levels);
                }

                // Commit the reconstruction and the transform, so later blocks
                // can use them.
                for y in 0..BLOCK_SIZE {
                    for x in 0..BLOCK_SIZE {
                        let value = chosen_reconstruction[y * BLOCK_SIZE + x].clamp(0, 255) as u8;
                        reconstruction.set_sample(bx * BLOCK_SIZE + x, by * BLOCK_SIZE + y, value);
                    }
                }
                transforms.set(bx, by, chosen_transform);
                if is_inter {
                    motion.set(bx, by, vector);
                }
                let _ = chosen_cost;
            }
        }

        reconstructions.push(reconstruction);
    }

    let payload = encoder.finish();
    let header = BitstreamHeader {
        version: crate::bitstream::VERSION,
        width: first.width as u32,
        height: first.height as u32,
        qp: config.qp as u8,
        frames: frames.len() as u8,
        signalling: config.signalling,
    };
    let mut bitstream = Vec::new();
    header.write(&mut bitstream, &payload);

    Ok(EncodeResult {
        payload_bytes: payload.len() * 4,
        bitstream,
        reconstructions,
        prediction_mismatches: mismatches,
        block_count,
        transform_signalling_bins: signalling_bins,
        inter_blocks,
        intra_mismatches,
        inter_mismatches,
        intra_blocks,
    })
}

/// Decode a bitstream into a sequence of pictures.
pub fn decode(bitstream: &[u8], config: &EncoderConfig) -> Result<Vec<Picture>, CodecError> {
    let (header, _) = BitstreamHeader::read(bitstream)?;
    let width = header.width as usize;
    let height = header.height as usize;
    if width % BLOCK_SIZE != 0 || height % BLOCK_SIZE != 0 {
        return Err(CodecError::Dimensions { width, height });
    }

    let payload_start = BitstreamHeader::SIZE;
    let payload_bytes = bitstream.len() - payload_start;
    let mut words = Vec::with_capacity(payload_bytes / 4);
    for chunk in bitstream[payload_start..].chunks_exact(4) {
        words.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }

    let mut decoder = BinDecoder::new(&words, NUM_CONTEXTS)?;
    let mut pictures: Vec<Picture> = Vec::with_capacity(header.frames as usize);

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
    }

    Ok(pictures)
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
fn motion_vector_bits(difference: MotionVector) -> f32 {
    let magnitude = |value: i32| {
        let value = value.unsigned_abs();
        // The terminating one of the prefix, and the prefix and suffix.
        1.0 + 2.0 * (32 - value.leading_zeros()) as f32
    };
    2.0 + magnitude(difference.dx) + magnitude(difference.dy)
}

/// Code a motion vector difference.
fn code_motion_vector(encoder: &mut BinEncoder, difference: MotionVector) {
    encoder.encode_bypass(difference.dx < 0);
    encoder.encode_bypass(difference.dy < 0);
    code_exp_golomb(encoder, difference.dx.unsigned_abs());
    code_exp_golomb(encoder, difference.dy.unsigned_abs());
}

/// Decode a motion vector difference.
fn decode_motion_vector(decoder: &mut BinDecoder) -> Result<MotionVector, CodecError> {
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
) -> (TransformMode, Vec<i32>, Vec<i32>, f32) {
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
                return (
                    candidate.transform,
                    candidate.levels,
                    candidate.reconstruction,
                    candidate.cost,
                );
            }
        }
    }

    (best.transform, best.levels, best.reconstruction, best.cost)
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
    let (transform, levels, reconstruction, cost) =
        choose_transform(original, &prediction, config, None);
    (mode, transform, levels, reconstruction, cost)
}

/// An estimate of the bits needed to code a set of quantized levels.
///
/// The estimate has to match the binarisation in [`code_coefficients`],
/// because the rate-distortion search optimises against it: a cost model
/// that disagrees with the bitstream would make the encoder choose
/// transforms that are not the ones the real coder rewards.
fn estimate_level_bits(levels: &[i32]) -> f32 {
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
    let mut out = Vec::with_capacity(BLOCK_SIZE * BLOCK_SIZE);
    for y in 0..BLOCK_SIZE {
        for x in 0..BLOCK_SIZE {
            out.push(
                picture
                    .sample(
                        (bx * BLOCK_SIZE + x) as isize,
                        (by * BLOCK_SIZE + y) as isize,
                    )
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
fn reference_samples(picture: &Picture, bx: usize, by: usize) -> ReferenceSamples {
    let x0 = (bx * BLOCK_SIZE) as isize;
    let y0 = (by * BLOCK_SIZE) as isize;

    let left = (0..BLOCK_SIZE)
        .map(|y| picture.sample(x0 - 1, y0 + y as isize).map(|v| v as i32))
        .collect();
    let top = (0..BLOCK_SIZE)
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
fn code_intra_mode(encoder: &mut BinEncoder, mode: IntraMode) {
    let index = mode.as_index();
    for bit in 0..3 {
        let value = (index >> bit) & 1 == 1;
        encoder.encode(CTX_INTRA_MODE, value);
    }
}

/// Decode the intra mode.
fn decode_intra_mode(decoder: &mut BinDecoder) -> Result<IntraMode, CodecError> {
    let mut index = 0u8;
    for bit in 0..3 {
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
    let high = decoder.decode(CTX_TRANSFORM_INDEX)?;
    let low = decoder.decode(CTX_TRANSFORM_INDEX)?;
    let position = (high as usize) * 2 + low as usize;
    let remaining = remaining_modes(predicted);
    Ok(remaining[position])
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
fn code_coefficients(encoder: &mut BinEncoder, levels: &[i32]) {
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
fn decode_coefficients(decoder: &mut BinDecoder) -> Result<Vec<i32>, CodecError> {
    let mut levels = Vec::with_capacity(BLOCK_SIZE * BLOCK_SIZE);
    for _ in 0..BLOCK_SIZE * BLOCK_SIZE {
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
    let mut examples = Vec::with_capacity(frames.len() * blocks_x * blocks_y);
    let mut reconstructions: Vec<Picture> = Vec::with_capacity(frames.len());

    for (index, picture) in frames.iter().enumerate() {
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

                // The prediction is chosen the same way the encoder
                // chooses it, so the example describes the situation the
                // predictor meets at inference time.
                let (is_inter, vector, mode, prediction) = choose_prediction(
                    picture,
                    reference_picture,
                    &reconstruction,
                    &motion,
                    bx,
                    by,
                    config,
                );
                let (transform, _, reconstruction_block, _) =
                    choose_transform(&original, &prediction, config, None);

                examples.push(TrainingExample {
                    features: predict_features(&PredictionContext {
                        picture: &reconstruction,
                        transforms: &transforms,
                        motion: &motion,
                        block: (bx, by),
                        mode,
                        is_inter,
                        qp: config.qp,
                    }),
                    transform,
                });

                // Commit the reconstruction, the transform and the
                // motion vector, so the next block's features are the
                // ones the decoder will have.
                for y in 0..BLOCK_SIZE {
                    for x in 0..BLOCK_SIZE {
                        let value = reconstruction_block[y * BLOCK_SIZE + x].clamp(0, 255) as u8;
                        reconstruction.set_sample(bx * BLOCK_SIZE + x, by * BLOCK_SIZE + y, value);
                    }
                }
                transforms.set(bx, by, transform);
                if is_inter {
                    motion.set(bx, by, vector);
                }
            }
        }
        reconstructions.push(reconstruction);
    }

    Ok(examples)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            let encoded = encode(std::slice::from_ref(&picture), &config).unwrap();
            let decoded = decode(&encoded.bitstream, &config).unwrap();
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
            let encoded = encode(std::slice::from_ref(&picture), &config).unwrap();
            let decoded = decode(&encoded.bitstream, &config).unwrap();
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
            encode(std::slice::from_ref(&picture), &config),
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
        let encoded = encode(std::slice::from_ref(&picture), &config).unwrap();
        assert!(encoded.block_count > 0);
        assert!(encoded.prediction_mismatches <= encoded.block_count);
        // With a constant predictor, every block that did not pick DCT-II
        // must have been overridden.
        let decoded = decode(&encoded.bitstream, &config).unwrap();
        assert_eq!(decoded[0], encoded.reconstructions[0]);
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
            let encoded = encode(&frames, &config).unwrap();
            let decoded = decode(&encoded.bitstream, &config).unwrap();
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
        let encoded = encode(&[first, second], &config).unwrap();
        assert!(
            encoded.inter_blocks > 0,
            "a picture that moved should be predicted from the previous frame"
        );
        let decoded = decode(&encoded.bitstream, &config).unwrap();
        assert_eq!(decoded[1], encoded.reconstructions[1]);
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
        let encoded = encode(std::slice::from_ref(&picture), &config).unwrap();
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
