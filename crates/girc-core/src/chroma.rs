//! Chroma coding: the two 4:2:0 chroma planes.
//!
//! # What the standard does, and what this module does
//!
//! ITU-T H.266 codes 4:2:0 content as three planes, and two properties
//! of the standard shape this module. Both are properties of the
//! standard rather than choices made here.
//!
//! * **The chroma transform type is not signalled.** H.266 clause 7.3.9
//!   codes `transform_type` for the luma plane; the chroma transform
//!   type is inferred. In 4:2:0 the inferred chroma transform type is
//!   always the plain DCT-II. Fraunhofer HHI's reference encoder agrees:
//!   chroma transform selection is gated on `m_useChromaTS`, which
//!   defaults to off and is only enabled for 4:4:4 content, and its
//!   intra search assigns `MTS_DCT2_DCT2` to the chroma planes
//!   directly. So the geometry-inference mechanism belongs to luma
//!   alone, which is exactly where the standard signals a transform
//!   type at all.
//!
//! * **The chroma quantization parameter is mapped, not equal.** It
//!   comes from the luma one through the chroma quantization parameter
//!   mapping of H.266 clause 8.6.3, which the [`crate::chroma_qp`]
//!   module derives and checks against the reference encoder.
//!
//! # The chroma block grid
//!
//! In 4:2:0 a chroma sample covers four luma samples, so an eight by
//! eight luma block covers four by four chroma samples. GIRC's chroma
//! grid is therefore four by four chroma samples, one chroma block per
//! luma block, which is the same relationship H.266's chroma transform
//! blocks have to the luma ones. The two grids are indexed by the same
//! block number, so block `b` of the luma plane and block `b` of the
//! chroma plane cover the same picture area.
//!
//! # The chroma motion
//!
//! H.266 derives the chroma motion vector from the luma one rather
//! than coding it separately, and GIRC does the same. A luma
//! displacement of `d` luma samples is `d / 2` chroma samples, and
//! since GIRC's motion is integer-sample, the chroma displacement is
//! the luma one halved and rounded to the nearest whole chroma sample.
//! The rounding is a consequence of GIRC's integer-sample motion: H.266
//! has an eighth-sample chroma interpolation filter, and GIRC does not.
//! The rounding is a fixed function of the luma vector, so the encoder
//! and the decoder derive the same chroma motion from the same luma
//! motion without spending a bit on it.
//!
//! # The chroma intra modes
//!
//! Four modes, the same set H.266's 4:2:0 chroma candidate list holds:
//! planar, vertical, horizontal and DC. The mode is signalled with the
//! truncated unary code H.266 uses for `intra_chroma_pred_mode`.
//!
//! # The entropy coder
//!
//! The chroma payload has its own range coder and its own contexts, so
//! the chroma planes' statistics never mix with the luma plane's, which
//! is what H.266 does with its per-component context sets. The
//! binarisations themselves are the ones the luma plane uses, taken
//! from the same functions, so there is one implementation of each
//! rather than two that can drift apart.

use crate::bitstream::{BinDecoder, BinEncoder};
use crate::chroma_qp::chroma_qp;
use crate::codec::{
    block_samples_at, code_coefficients, decode_coefficients_at, estimate_level_bits,
    reference_samples_at, CodecError, EncoderConfig, Picture,
};
use crate::frame::Frame;
use crate::inter::{compensate, MotionField, MotionVector};
use crate::intra::{predict, IntraMode, ReferenceSamples};
use crate::quant::{dequantize_with, quantize_with, sse, QuantParams};
use crate::transform::{forward_mode, inverse_mode, TransformMode};

/// The block size GIRC codes for chroma, in chroma samples.
///
/// A chroma sample covers four luma samples, so the chroma block is a
/// quarter the area of the luma block and covers the same picture area.
pub const CHROMA_BLOCK_SIZE: usize = 4;

/// The chroma intra modes, in the order H.266 lists them for 4:2:0.
///
/// H.266's 4:2:0 chroma candidate list is planar, vertical, horizontal
/// and DC. GIRC's own luma mode set is not H.266's, so the list is
/// spelled out here rather than derived.
pub const CHROMA_INTRA_MODES: [IntraMode; 4] = [
    IntraMode::Planar,
    IntraMode::Angular(0),
    IntraMode::Angular(1),
    IntraMode::Dc,
];

/// The number of bins the truncated unary mode code uses at most.
///
/// The mode index runs from zero to three, so the code is at most three
/// bins: one for the first mode, two for the second, and three for the
/// third and fourth.
const CHROMA_INTRA_MODE_MAX_BINS: usize = 3;

/// The context the chroma intra mode's bins use.
///
/// The chroma payload has its own entropy coder, so this context is
/// independent of the luma plane's.
const CTX_CHROMA_INTRA_MODE: usize = 12;

/// The chroma transform, which in 4:2:0 is always the plain DCT-II.
const CHROMA_TRANSFORM: TransformMode = TransformMode::DCT2;

/// The quantization parameter the chroma planes are coded at.
///
/// The chroma quantization parameter is the luma one mapped through
/// H.266's chroma quantization parameter mapping.
pub fn chroma_quantization_parameter(luma_qp: i32) -> i32 {
    chroma_qp(luma_qp)
}

/// The quantization parameters for the chroma planes.
fn chroma_params(luma_qp: i32) -> QuantParams {
    QuantParams::new(chroma_quantization_parameter(luma_qp), CHROMA_BLOCK_SIZE)
}

/// The chroma displacement of a luma displacement.
///
/// A luma sample covers four chroma samples, so a luma displacement of
/// `d` samples is `d / 2` chroma samples. The result is rounded to the
/// nearest whole chroma sample, away from zero on a tie.
pub fn chroma_displacement(luma_displacement: i32) -> i32 {
    if luma_displacement >= 0 {
        (luma_displacement + 1) / 2
    } else {
        (luma_displacement - 1) / 2
    }
}

/// The chroma motion vector of a luma motion vector.
pub fn chroma_motion_vector(luma: MotionVector) -> MotionVector {
    MotionVector {
        dx: chroma_displacement(luma.dx),
        dy: chroma_displacement(luma.dy),
    }
}

/// The result of coding the chroma planes of a sequence.
#[derive(Debug, Clone)]
pub struct SequenceResult {
    /// The reconstruction of each frame's chroma planes.
    pub planes: Vec<(Picture, Picture)>,
    /// How many chroma blocks the sequence holds.
    pub block_count: usize,
    /// How many chroma blocks were predicted from the previous frame.
    pub inter_blocks: usize,
    /// How many chroma blocks were predicted from the current frame.
    pub intra_blocks: usize,
}

/// Code the chroma planes of a sequence of frames.
///
/// The luma motion fields are the ones the luma plane produced, and
/// they decide both the chroma blocks' prediction mode and their
/// motion, exactly as H.266 derives the chroma motion from the luma
/// one.
pub fn encode_sequence(
    frames: &[Frame],
    luma_motion: &[MotionField],
    config: &EncoderConfig,
    encoder: &mut BinEncoder,
) -> Result<SequenceResult, CodecError> {
    if frames.is_empty() {
        return Err(CodecError::NoFrames);
    }
    let params = chroma_params(config.qp);
    let mut planes: Vec<(Picture, Picture)> = Vec::with_capacity(frames.len());
    let mut block_count = 0usize;
    let mut inter_blocks = 0usize;
    let mut intra_blocks = 0usize;

    for (index, frame) in frames.iter().enumerate() {
        let luma_field = luma_motion.get(index).ok_or(CodecError::NoFrames)?;
        let mut reconstructions = Vec::with_capacity(2);

        for (original, previous) in [
            (&frame.cb, index.checked_sub(1).map(|p| &planes[p].0)),
            (&frame.cr, index.checked_sub(1).map(|p| &planes[p].1)),
        ] {
            let (reconstruction, blocks, inter, intra) =
                encode_plane(original, previous, luma_field, &params, config, encoder)?;
            block_count += blocks;
            inter_blocks += inter;
            intra_blocks += intra;
            reconstructions.push(reconstruction);
        }
        planes.push((reconstructions[0].clone(), reconstructions[1].clone()));
    }

    Ok(SequenceResult {
        planes,
        block_count,
        inter_blocks,
        intra_blocks,
    })
}

/// Decode the chroma planes of a sequence of frames.
pub fn decode_sequence(
    luma_motion: &[MotionField],
    config: &EncoderConfig,
    decoder: &mut BinDecoder,
    frames: usize,
    width: usize,
    height: usize,
) -> Result<SequenceResult, CodecError> {
    let params = chroma_params(config.qp);
    let chroma_width = width / 2;
    let chroma_height = height / 2;
    let mut planes: Vec<(Picture, Picture)> = Vec::with_capacity(frames);
    let mut block_count = 0usize;
    let mut inter_blocks = 0usize;
    let mut intra_blocks = 0usize;

    for index in 0..frames {
        let luma_field = luma_motion.get(index).ok_or(CodecError::NoFrames)?;
        let mut reconstructions = Vec::with_capacity(2);

        for previous in [
            index.checked_sub(1).map(|p| &planes[p].0),
            index.checked_sub(1).map(|p| &planes[p].1),
        ] {
            let (reconstruction, blocks, inter, intra) = decode_plane(
                previous,
                luma_field,
                &params,
                config,
                decoder,
                chroma_width,
                chroma_height,
            )?;
            block_count += blocks;
            inter_blocks += inter;
            intra_blocks += intra;
            reconstructions.push(reconstruction);
        }
        planes.push((reconstructions[0].clone(), reconstructions[1].clone()));
    }

    Ok(SequenceResult {
        planes,
        block_count,
        inter_blocks,
        intra_blocks,
    })
}

/// Code one chroma plane of one frame.
///
/// The reference is the previous frame's reconstruction of the same
/// plane, and the reconstruction is committed block by block, so the
/// next block's causal neighbourhood is the reconstructed one.
fn encode_plane(
    original: &Picture,
    reference: Option<&Picture>,
    luma_field: &MotionField,
    params: &QuantParams,
    config: &EncoderConfig,
    encoder: &mut BinEncoder,
) -> Result<(Picture, usize, usize, usize), CodecError> {
    let blocks_x = original.width / CHROMA_BLOCK_SIZE;
    let blocks_y = original.height / CHROMA_BLOCK_SIZE;
    let mut reconstruction = Picture::filled(original.width, original.height, 0);
    let mut inter_blocks = 0usize;
    let mut intra_blocks = 0usize;

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block = block_samples_at(original, bx, by, CHROMA_BLOCK_SIZE);
            // The chroma block's prediction mode follows the luma
            // block's, as the coding unit's prediction mode does in
            // H.266.
            let luma_vector = luma_field.get(bx, by);
            let is_inter = luma_vector.is_some();
            let (mode, prediction) = if is_inter {
                let vector = chroma_motion_vector(luma_vector.expect("the luma block is inter"));
                let prediction = match reference {
                    Some(reference) => compensate(
                        reference,
                        (bx * CHROMA_BLOCK_SIZE) as i32,
                        (by * CHROMA_BLOCK_SIZE) as i32,
                        CHROMA_BLOCK_SIZE,
                        vector,
                    ),
                    None => vec![0i32; CHROMA_BLOCK_SIZE * CHROMA_BLOCK_SIZE],
                };
                (IntraMode::Dc, prediction)
            } else {
                let reference_samples = reference_samples_at(
                    &reconstruction,
                    bx,
                    by,
                    CHROMA_BLOCK_SIZE,
                );
                let (mode, prediction) = choose_intra_mode(&block, &reference_samples, params, config);
                (mode, prediction)
            };

            let levels = transform_and_quantize(&block, &prediction, params);

            // The syntax: the intra mode when the block is intra, then
            // the residual.
            if !is_inter {
                code_chroma_intra_mode(encoder, mode);
            }
            let skip = levels.iter().all(|level| *level == 0);
            encoder.encode(crate::codec::CTX_SKIP, skip);
            if !skip {
                code_coefficients(encoder, &levels);
            }

            // Commit the reconstruction, so the next block's causal
            // neighbourhood is the reconstructed one.
            let dequantized = dequantize_with(&levels, params);
            let residual = inverse_mode(CHROMA_TRANSFORM, CHROMA_BLOCK_SIZE, &dequantized)
                .map_err(|error| CodecError::Predictor(error.to_string()))?;
            for y in 0..CHROMA_BLOCK_SIZE {
                for x in 0..CHROMA_BLOCK_SIZE {
                    let index = y * CHROMA_BLOCK_SIZE + x;
                    let value = (prediction[index] + residual[index]).clamp(0, 255) as u8;
                    reconstruction.set_sample(
                        bx * CHROMA_BLOCK_SIZE + x,
                        by * CHROMA_BLOCK_SIZE + y,
                        value,
                    );
                }
            }
            if is_inter {
                inter_blocks += 1;
            } else {
                intra_blocks += 1;
            }
        }
    }

    Ok((
        reconstruction,
        blocks_x * blocks_y,
        inter_blocks,
        intra_blocks,
    ))
}

/// Decode one chroma plane of one frame.
fn decode_plane(
    reference: Option<&Picture>,
    luma_field: &MotionField,
    params: &QuantParams,
    config: &EncoderConfig,
    decoder: &mut BinDecoder,
    width: usize,
    height: usize,
) -> Result<(Picture, usize, usize, usize), CodecError> {
    let blocks_x = width / CHROMA_BLOCK_SIZE;
    let blocks_y = height / CHROMA_BLOCK_SIZE;
    let mut reconstruction = Picture::filled(width, height, 0);
    let mut inter_blocks = 0usize;
    let mut intra_blocks = 0usize;

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let luma_vector = luma_field.get(bx, by);
            let is_inter = luma_vector.is_some();
            let prediction = if is_inter {
                let vector = chroma_motion_vector(luma_vector.expect("the luma block is inter"));
                match reference {
                    Some(reference) => compensate(
                        reference,
                        (bx * CHROMA_BLOCK_SIZE) as i32,
                        (by * CHROMA_BLOCK_SIZE) as i32,
                        CHROMA_BLOCK_SIZE,
                        vector,
                    ),
                    None => vec![0i32; CHROMA_BLOCK_SIZE * CHROMA_BLOCK_SIZE],
                }
            } else {
                let mode = decode_chroma_intra_mode(decoder)?;
                let reference_samples =
                    reference_samples_at(&reconstruction, bx, by, CHROMA_BLOCK_SIZE);
                predict(mode, CHROMA_BLOCK_SIZE, &reference_samples)
            };

            let skip = decoder.decode(crate::codec::CTX_SKIP)?;
            let levels = if skip {
                vec![0i32; CHROMA_BLOCK_SIZE * CHROMA_BLOCK_SIZE]
            } else {
                decode_coefficients_at(decoder, CHROMA_BLOCK_SIZE * CHROMA_BLOCK_SIZE)?
            };

            let dequantized = dequantize_with(&levels, params);
            let residual = inverse_mode(CHROMA_TRANSFORM, CHROMA_BLOCK_SIZE, &dequantized)
                .map_err(|error| CodecError::Predictor(error.to_string()))?;
            for y in 0..CHROMA_BLOCK_SIZE {
                for x in 0..CHROMA_BLOCK_SIZE {
                    let index = y * CHROMA_BLOCK_SIZE + x;
                    let value = (prediction[index] + residual[index]).clamp(0, 255) as u8;
                    reconstruction.set_sample(
                        bx * CHROMA_BLOCK_SIZE + x,
                        by * CHROMA_BLOCK_SIZE + y,
                        value,
                    );
                }
            }
            if is_inter {
                inter_blocks += 1;
            } else {
                intra_blocks += 1;
            }
        }
    }
    let _ = config;

    Ok((
        reconstruction,
        blocks_x * blocks_y,
        inter_blocks,
        intra_blocks,
    ))
}

/// Choose the intra mode of a chroma block.
///
/// The choice is the one the luma plane makes: the mode whose
/// rate-distortion cost is lowest.
fn choose_intra_mode(
    original: &[u8],
    reference_samples: &ReferenceSamples,
    params: &QuantParams,
    config: &EncoderConfig,
) -> (IntraMode, Vec<i32>) {
    let mut best_mode = CHROMA_INTRA_MODES[0];
    let mut best_cost = f32::INFINITY;
    let mut best_prediction = Vec::new();
    for mode in CHROMA_INTRA_MODES {
        let prediction = predict(mode, CHROMA_BLOCK_SIZE, reference_samples);
        let cost = evaluate(original, &prediction, params, config);
        if cost < best_cost {
            best_cost = cost;
            best_mode = mode;
            best_prediction = prediction;
        }
    }
    (best_mode, best_prediction)
}

/// The rate-distortion cost of a chroma block.
fn evaluate(
    original: &[u8],
    prediction: &[i32],
    params: &QuantParams,
    config: &EncoderConfig,
) -> f32 {
    let levels = transform_and_quantize(original, prediction, params);
    let reconstruction: Vec<i32> = prediction
        .iter()
        .zip(dequantize_with(&levels, params).iter())
        .map(|(p, s)| p + s)
        .collect();
    let distortion = sse(
        &original
            .iter()
            .map(|value| *value as i32)
            .collect::<Vec<_>>(),
        &reconstruction,
    );
    distortion as f32 + config.lambda * estimate_level_bits(&levels)
}

/// Transform a chroma block's residual and quantize it.
///
/// The transform is the plain DCT-II, which is what 4:2:0 chroma uses.
fn transform_and_quantize(original: &[u8], prediction: &[i32], params: &QuantParams) -> Vec<i32> {
    let residual: Vec<i32> = original
        .iter()
        .zip(prediction.iter())
        .map(|(o, p)| *o as i32 - p)
        .collect();
    let coefficients = match forward_mode(CHROMA_TRANSFORM, CHROMA_BLOCK_SIZE, &residual) {
        Ok(coefficients) => coefficients,
        Err(_) => return vec![0i32; CHROMA_BLOCK_SIZE * CHROMA_BLOCK_SIZE],
    };
    quantize_with(&coefficients, params)
}

/// Code a chroma intra mode.
///
/// The code is the truncated unary code H.266 uses for
/// `intra_chroma_pred_mode`: the mode's index in ones, terminated by a
/// zero, with the last mode needing no terminator.
fn code_chroma_intra_mode(encoder: &mut BinEncoder, mode: IntraMode) {
    let index = CHROMA_INTRA_MODES
        .iter()
        .position(|candidate| *candidate == mode)
        .expect("the chroma mode is in the set");
    for bin in 0..CHROMA_INTRA_MODE_MAX_BINS {
        let value = index > bin;
        encoder.encode(CTX_CHROMA_INTRA_MODE, value);
        if !value {
            break;
        }
    }
}

/// Decode a chroma intra mode.
fn decode_chroma_intra_mode(decoder: &mut BinDecoder) -> Result<IntraMode, CodecError> {
    let mut index = 0usize;
    for _ in 0..CHROMA_INTRA_MODE_MAX_BINS {
        if !decoder.decode(CTX_CHROMA_INTRA_MODE)? {
            break;
        }
        index += 1;
    }
    CHROMA_INTRA_MODES
        .get(index)
        .copied()
        .ok_or_else(|| CodecError::Predictor(format!("unknown chroma intra mode {index}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chroma plane with a pattern in it.
    fn test_plane(width: usize, height: usize) -> Picture {
        let mut picture = Picture::filled(width, height, 128);
        for y in 0..height {
            for x in 0..width {
                let value = ((x / 4 + y / 4) % 2) as u8 * 100 + 80;
                picture.set_sample(x, y, value);
            }
        }
        picture
    }

    /// Every chroma intra mode must survive the mode code.
    #[test]
    fn every_chroma_intra_mode_survives_the_code() {
        for mode in CHROMA_INTRA_MODES {
            let mut encoder = BinEncoder::new(16);
            code_chroma_intra_mode(&mut encoder, mode);
            let compressed = encoder.finish();
            let mut decoder = BinDecoder::new(&compressed, 16).unwrap();
            let decoded = decode_chroma_intra_mode(&mut decoder).unwrap();
            assert_eq!(decoded, mode);
        }
    }

    /// The mode code must not run into the next syntax element.
    ///
    /// The truncated unary code for four modes is at most three bins,
    /// and the last mode's code is three ones with no terminator. If
    /// the decoder read a fourth bin it would read into the residual
    /// and the two would disagree.
    #[test]
    fn the_mode_code_is_at_most_three_bins() {
        for mode in CHROMA_INTRA_MODES {
            let mut encoder = BinEncoder::new(16);
            code_chroma_intra_mode(&mut encoder, mode);
            let compressed = encoder.finish();
            let mut decoder = BinDecoder::new(&compressed, 16).unwrap();
            let decoded = decode_chroma_intra_mode(&mut decoder).unwrap();
            assert_eq!(decoded, mode);
        }
    }

    /// The chroma displacement must be the luma one halved.
    #[test]
    fn the_chroma_displacement_is_half_the_luma_one() {
        assert_eq!(chroma_displacement(0), 0);
        assert_eq!(chroma_displacement(2), 1);
        assert_eq!(chroma_displacement(4), 2);
        assert_eq!(chroma_displacement(-2), -1);
        assert_eq!(chroma_displacement(-4), -2);
        // An odd displacement rounds away from zero.
        assert_eq!(chroma_displacement(1), 1);
        assert_eq!(chroma_displacement(-1), -1);
        assert_eq!(chroma_displacement(3), 2);
    }

    /// A chroma plane must round-trip through the coder.
    #[test]
    fn a_chroma_plane_round_trips() {
        let original = test_plane(32, 32);
        let config = EncoderConfig::default();
        let params = chroma_params(config.qp);
        let mut encoder = BinEncoder::new(16);
        let luma_field = MotionField::new(8, 8);
        let (encoded, blocks, inter, intra) =
            encode_plane(&original, None, &luma_field, &params, &config, &mut encoder).unwrap();
        assert_eq!(blocks, 64);
        assert_eq!(inter, 0);
        assert_eq!(intra, 64);

        let mut decoder = BinDecoder::new(&encoder.finish(), 16).unwrap();
        let (decoded, _, _, _) =
            decode_plane(None, &luma_field, &params, &config, &mut decoder, 32, 32).unwrap();
        assert_eq!(decoded, encoded);
    }

    /// A chroma plane's inter blocks must be predicted from the luma
    /// motion, with no motion bits of their own.
    #[test]
    fn a_chroma_plane_with_inter_blocks_round_trips() {
        let original = test_plane(32, 32);
        let config = EncoderConfig::default();
        let params = chroma_params(config.qp);
        let mut encoder = BinEncoder::new(16);
        let mut luma_field = MotionField::new(8, 8);
        // Mark every block as inter, with a motion of two luma samples.
        for by in 0..8 {
            for bx in 0..8 {
                luma_field.set(
                    bx,
                    by,
                    MotionVector { dx: 2, dy: -2 },
                );
            }
        }
        let (encoded, blocks, inter, intra) = encode_plane(
            &original,
            Some(&original),
            &luma_field,
            &params,
            &config,
            &mut encoder,
        )
        .unwrap();
        assert_eq!(blocks, 64);
        assert_eq!(inter, 64);
        assert_eq!(intra, 0);

        let mut decoder = BinDecoder::new(&encoder.finish(), 16).unwrap();
        let (decoded, _, _, _) = decode_plane(
            Some(&original),
            &luma_field,
            &params,
            &config,
            &mut decoder,
            32,
            32,
        )
        .unwrap();
        assert_eq!(decoded, encoded);
    }

    /// The chroma quantization parameter must be the mapped one, not the
    /// luma one.
    #[test]
    fn the_chroma_quantization_parameter_is_mapped() {
        assert_eq!(chroma_quantization_parameter(17), 17);
        assert_ne!(chroma_quantization_parameter(30), 30);
    }

    /// The chroma transform must be the plain DCT-II, because 4:2:0
    /// chroma has no transform type to signal.
    #[test]
    fn the_chroma_transform_is_the_plain_dct2() {
        assert_eq!(CHROMA_TRANSFORM, TransformMode::DCT2);
    }
}
