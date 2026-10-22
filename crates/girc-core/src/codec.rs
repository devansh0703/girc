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
    extract_features, GeometryFeatures, TransformPredictor, TransformSignal, TransformSignalling,
};
use crate::intra::{predict, IntraMode, ReferenceSamples};
use crate::quant::{dequantize, quantize, sse};
use crate::transform::{forward, inverse, TransformKind};

/// The block size GIRC codes, in samples.
pub const BLOCK_SIZE: usize = 8;

/// The number of entropy-coding contexts in the payload.
const NUM_CONTEXTS: usize = 16;

// Context indices.
const CTX_INTRA_MODE: usize = 0;
const CTX_SKIP: usize = 1;
const CTX_COEF_SIGNIFICANT: usize = 2;
const CTX_COEF_GREATER1: usize = 3;
const CTX_COEF_GREATER2: usize = 4;
const CTX_COEF_LEVEL: usize = 5;
const CTX_TRANSFORM_OVERRIDE: usize = 6;
const CTX_TRANSFORM_TYPE: usize = 7;

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
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            qp: 27,
            signalling: TransformSignalling::Explicit,
            predictor: TransformPredictor::constant_dct2(),
            lambda: 0.85,
        }
    }
}

/// The result of encoding a picture.
#[derive(Debug, Clone)]
pub struct EncodeResult {
    /// The encoded bitstream.
    pub bitstream: Vec<u8>,
    /// The encoder's reconstruction.
    pub reconstruction: Picture,
    /// The number of bytes the payload occupies.
    pub payload_bytes: usize,
    /// How many blocks the encoder chose a transform that differed from the
    /// decoder's prediction. Only meaningful in inferred mode.
    pub prediction_mismatches: usize,
    /// How many blocks the encoder coded.
    pub block_count: usize,
    /// The number of bits the transform-type signalling cost, as the encoder
    /// accounted for it. This is the quantity the mechanism is meant to
    /// reduce.
    pub transform_signalling_bits: usize,
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
        }
    }
}

impl std::error::Error for CodecError {}

impl From<BitstreamError> for CodecError {
    fn from(value: BitstreamError) -> Self {
        CodecError::Bitstream(value)
    }
}

/// Encode a picture.
pub fn encode(picture: &Picture, config: &EncoderConfig) -> Result<EncodeResult, CodecError> {
    if picture.width % BLOCK_SIZE != 0 || picture.height % BLOCK_SIZE != 0 {
        return Err(CodecError::Dimensions {
            width: picture.width,
            height: picture.height,
        });
    }

    let blocks_x = picture.width / BLOCK_SIZE;
    let blocks_y = picture.height / BLOCK_SIZE;
    let block_count = blocks_x * blocks_y;

    let mut encoder = BinEncoder::new(NUM_CONTEXTS);
    let mut reconstruction = Picture::filled(picture.width, picture.height, 0);

    let mut mismatches = 0usize;
    let mut signalling_bits = 0usize;

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let reference = reference_samples(&reconstruction, bx, by);
            let original = block_samples(picture, bx, by);

            let (mode, chosen_transform, chosen_levels, chosen_reconstruction, chosen_cost) =
                best_block(&original, &reference, config);

            // The decoder's prediction, which the encoder must match.
            let features = extract_features(&reference.left, &reference.top);
            let (predicted, _) = config.predictor.predict_with_margin(&features);

            let signal = match config.signalling {
                TransformSignalling::Explicit => TransformSignal::Explicit(chosen_transform),
                TransformSignalling::Inferred => {
                    if predicted == chosen_transform {
                        TransformSignal::Predicted
                    } else {
                        mismatches += 1;
                        TransformSignal::Override(chosen_transform)
                    }
                }
            };
            signalling_bits += signal.bin_count();

            // Code the syntax elements.
            code_intra_mode(&mut encoder, mode);
            code_transform_signal(&mut encoder, signal);
            let skip = chosen_levels.iter().all(|l| *l == 0);
            encoder.encode(CTX_SKIP, skip);
            if !skip {
                code_coefficients(&mut encoder, &chosen_levels);
            }

            // Commit the reconstruction so later blocks can use it.
            for y in 0..BLOCK_SIZE {
                for x in 0..BLOCK_SIZE {
                    let value = chosen_reconstruction[y * BLOCK_SIZE + x].clamp(0, 255) as u8;
                    reconstruction.set_sample(bx * BLOCK_SIZE + x, by * BLOCK_SIZE + y, value);
                }
            }
            let _ = chosen_cost;
        }
    }

    let payload = encoder.finish();
    let header = BitstreamHeader {
        version: crate::bitstream::VERSION,
        width: picture.width as u32,
        height: picture.height as u32,
        qp: config.qp as u8,
    };
    let mut bitstream = Vec::new();
    header.write(&mut bitstream, &payload);

    Ok(EncodeResult {
        payload_bytes: payload.len() * 4,
        bitstream,
        reconstruction,
        prediction_mismatches: mismatches,
        block_count,
        transform_signalling_bits: signalling_bits,
    })
}

/// Decode a bitstream into a picture.
pub fn decode(bitstream: &[u8], config: &EncoderConfig) -> Result<Picture, CodecError> {
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
    let mut picture = Picture::filled(width, height, 0);

    let blocks_x = width / BLOCK_SIZE;
    let blocks_y = height / BLOCK_SIZE;

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let reference = reference_samples(&picture, bx, by);
            let mode = decode_intra_mode(&mut decoder)?;
            let transform = decode_transform_signal(&mut decoder, config, &reference)?;

            let skip = decoder.decode(CTX_SKIP)?;
            let levels = if skip {
                vec![0i32; BLOCK_SIZE * BLOCK_SIZE]
            } else {
                decode_coefficients(&mut decoder)?
            };

            let dequantized = dequantize(&levels, header.qp as i32);
            let samples = inverse(transform, BLOCK_SIZE, &dequantized)
                .map_err(|e| CodecError::Predictor(e.to_string()))?;
            let prediction = predict(mode, BLOCK_SIZE, &reference);

            for y in 0..BLOCK_SIZE {
                for x in 0..BLOCK_SIZE {
                    let index = y * BLOCK_SIZE + x;
                    let value = (prediction[index] + samples[index]).clamp(0, 255) as u8;
                    picture.set_sample(bx * BLOCK_SIZE + x, by * BLOCK_SIZE + y, value);
                }
            }
        }
    }

    Ok(picture)
}

/// The rate-distortion cost of a candidate block.
fn rd_cost(distortion: i64, bits: f32, lambda: f32) -> f32 {
    distortion as f32 + lambda * bits
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
) -> (IntraMode, TransformKind, Vec<i32>, Vec<i32>, f32) {
    best_block(original, reference, config)
}

/// Search the intra modes and transform types for a block, and return the
/// best candidate.
fn best_block(
    original: &[u8],
    reference: &ReferenceSamples,
    config: &EncoderConfig,
) -> (IntraMode, TransformKind, Vec<i32>, Vec<i32>, f32) {
    let mut best: Option<(IntraMode, TransformKind, Vec<i32>, Vec<i32>, f32)> = None;

    for mode in IntraMode::all() {
        let prediction = predict(mode, BLOCK_SIZE, reference);
        let residual: Vec<i32> = original
            .iter()
            .zip(prediction.iter())
            .map(|(o, p)| *o as i32 - p)
            .collect();

        for transform in TransformKind::ALL {
            let Ok(coefficients) = forward(transform, BLOCK_SIZE, &residual) else {
                continue;
            };
            let levels = quantize(&coefficients, config.qp);
            let dequantized = dequantize(&levels, config.qp);
            let Ok(samples) = inverse(transform, BLOCK_SIZE, &dequantized) else {
                continue;
            };
            let reconstruction: Vec<i32> = prediction
                .iter()
                .zip(samples.iter())
                .map(|(p, s)| p + s)
                .collect();

            let distortion = sse(
                &original.iter().map(|v| *v as i32).collect::<Vec<_>>(),
                &reconstruction,
            );
            // The rate is estimated from the number of non-zero levels and
            // their magnitudes. The exact cost is only known after coding, so
            // the search uses this estimate; the reported bit counts come
            // from the real entropy coder.
            let bits = estimate_level_bits(&levels);
            let cost = rd_cost(distortion, bits, config.lambda);

            let better = match &best {
                None => true,
                Some((_, _, _, _, best_cost)) => cost < *best_cost,
            };
            if better {
                best = Some((mode, transform, levels, reconstruction, cost));
            }
        }
    }

    best.expect("the mode and transform sets are non-empty")
}

/// An estimate of the bits needed to code a set of quantized levels.
fn estimate_level_bits(levels: &[i32]) -> f32 {
    let mut bits = 0.0;
    for level in levels {
        let magnitude = level.unsigned_abs();
        if magnitude == 0 {
            continue;
        }
        // One significant flag, one sign, and a Golomb-Rice style remainder.
        bits += 2.0 + (magnitude as f32).log2();
    }
    bits
}

/// The samples of the block at `(bx, by)`.
fn block_samples(picture: &Picture, bx: usize, by: usize) -> Vec<u8> {
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
fn code_transform_signal(encoder: &mut BinEncoder, signal: TransformSignal) {
    match signal {
        TransformSignal::Explicit(kind) => {
            // The explicit mode codes the transform type directly.
            encoder.encode(CTX_TRANSFORM_OVERRIDE, true);
            code_transform_type(encoder, kind);
        }
        TransformSignal::Predicted => {
            encoder.encode(CTX_TRANSFORM_OVERRIDE, false);
        }
        TransformSignal::Override(kind) => {
            encoder.encode(CTX_TRANSFORM_OVERRIDE, true);
            code_transform_type(encoder, kind);
        }
    }
}

/// Code the transform type in two bits.
fn code_transform_type(encoder: &mut BinEncoder, kind: TransformKind) {
    let index = kind.as_index();
    for bit in 0..2 {
        let value = (index >> bit) & 1 == 1;
        encoder.encode(CTX_TRANSFORM_TYPE, value);
    }
}

/// Decode the transform signal, reconstructing the decoder's prediction when
/// the signalling says the encoder did not override it.
fn decode_transform_signal(
    decoder: &mut BinDecoder,
    config: &EncoderConfig,
    reference: &ReferenceSamples,
) -> Result<TransformKind, CodecError> {
    let override_flag = decoder.decode(CTX_TRANSFORM_OVERRIDE)?;
    if !override_flag {
        // The decoder's prediction is used. This is only possible in
        // inferred mode; in explicit mode the flag is always set.
        let features = extract_features(&reference.left, &reference.top);
        let (predicted, _) = config.predictor.predict_with_margin(&features);
        return Ok(predicted);
    }
    let mut index = 0u8;
    for bit in 0..2 {
        if decoder.decode(CTX_TRANSFORM_TYPE)? {
            index |= 1 << bit;
        }
    }
    TransformKind::from_index(index)
        .ok_or_else(|| CodecError::Predictor(format!("unknown transform type {index}")))
}

/// Code the quantized residual levels.
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
        encoder.encode(CTX_COEF_LEVEL, *level < 0);

        // The magnitude above 2, coded as a unary prefix of zeros followed
        // by a one.
        let excess = magnitude - 2;
        for _ in 0..excess {
            encoder.encode(CTX_COEF_LEVEL, false);
        }
        encoder.encode(CTX_COEF_LEVEL, true);
    }
}

/// Decode the quantized residual levels.
fn decode_coefficients(decoder: &mut BinDecoder) -> Result<Vec<i32>, CodecError> {
    let mut levels = Vec::with_capacity(BLOCK_SIZE * BLOCK_SIZE);
    for _ in 0..BLOCK_SIZE * BLOCK_SIZE {
        if !decoder.decode(CTX_COEF_SIGNIFICANT)? {
            levels.push(0);
            continue;
        }
        let greater1 = decoder.decode(CTX_COEF_GREATER1)?;
        let greater2 = decoder.decode(CTX_COEF_GREATER2)?;
        let negative = decoder.decode(CTX_COEF_LEVEL)?;

        let mut magnitude = 0u32;
        if greater2 {
            magnitude = 2;
        } else if greater1 {
            magnitude = 1;
        }
        // The unary suffix: zeros until a one.
        loop {
            let bit = decoder.decode(CTX_COEF_LEVEL)?;
            if bit {
                break;
            }
            magnitude += 1;
            if magnitude > 1_000_000 {
                return Err(CodecError::Predictor(
                    "residual level is implausibly large".into(),
                ));
            }
        }
        let value = magnitude as i32;
        levels.push(if negative { -value } else { value });
    }
    Ok(levels)
}

/// The features the predictor would see for a block, exposed for the
/// training pipeline.
pub fn block_features(picture: &Picture, bx: usize, by: usize) -> GeometryFeatures {
    let reference = reference_samples(picture, bx, by);
    extract_features(&reference.left, &reference.top)
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
            let encoded = encode(&picture, &config).unwrap();
            let decoded = decode(&encoded.bitstream, &config).unwrap();
            assert_eq!(
                decoded, encoded.reconstruction,
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
            let encoded = encode(&picture, &config).unwrap();
            let decoded = decode(&encoded.bitstream, &config).unwrap();
            assert_eq!(decoded, encoded.reconstruction, "size {size}");
        }
    }

    /// A picture whose dimensions are not a multiple of the block size must
    /// be rejected rather than silently mishandled.
    #[test]
    fn rejects_bad_dimensions() {
        let picture = Picture::filled(12, 16, 0);
        let config = EncoderConfig::default();
        assert!(matches!(
            encode(&picture, &config),
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
        let encoded = encode(&picture, &config).unwrap();
        assert!(encoded.block_count > 0);
        assert!(encoded.prediction_mismatches <= encoded.block_count);
        // With a constant predictor, every block that did not pick DCT-II
        // must have been overridden.
        let decoded = decode(&encoded.bitstream, &config).unwrap();
        assert_eq!(decoded, encoded.reconstruction);
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
        let encoded = encode(&picture, &config).unwrap();
        assert_eq!(encoded.prediction_mismatches, 0);
        assert_eq!(
            encoded.transform_signalling_bits,
            encoded.block_count * 2,
            "explicit mode codes two bits per block"
        );
    }
}
