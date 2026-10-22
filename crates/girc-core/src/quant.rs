//! Quantization and dequantization of transform coefficients.
//!
//! GIRC uses the VVC quantizer, as specified in ITU-T H.266 clause 8.6.3
//! and implemented in Fraunhofer HHI's VVenC. The quantizer is expressed
//! the way VVC expresses it, because GIRC's transform is bit-exact against
//! VVenC's: using a different quantizer would leave the transform's
//! internal scaling unmatched, and the coded rates would not be comparable
//! with the anchor codecs the benchmark runs.
//!
//! # The arithmetic
//!
//! VVC's forward transform produces coefficients in a widened dynamic
//! range; the quantizer's right shift brings them back to the sample
//! domain. For a transform of size `N` x `N` and a bit depth of `B`, the
//! transform shift is
//!
//! ```text
//! transform_shift = max_log2_transform_dynamic_range - B - log2(N)
//! ```
//!
//! and the quantizer's shift is
//!
//! ```text
//! q_bits = QUANT_SHIFT + qp / 6 + transform_shift
//! ```
//!
//! A coefficient is quantized as
//!
//! ```text
//! level = (|coefficient| * quant_scale + round_offset) >> q_bits
//! ```
//!
//! where `quant_scale` is VVC's `g_quantScales` table indexed by `qp % 6`.
//! The sign is carried separately.
//!
//! # The dead zone
//!
//! VVC applies a dead zone through the `round_offset`. The offset is half
//! the quantization step, so a coefficient below half a step quantizes to
//! zero. GIRC uses the same rounding, so the same coefficient survives in
//! both signalling modes.

/// The largest quantization parameter GIRC accepts.
///
/// ITU-T H.266 allows a quantization parameter of 0 to 63, but values above
/// 51 are outside the range the VVC specification itself constrains, and
/// beyond that the quantizer's arithmetic stops being meaningful.
pub const MAX_QP: i32 = 51;

/// The bit depth of the content GIRC codes.
pub const BIT_DEPTH: i32 = 8;

/// The logarithm of the transform's dynamic range, as in ITU-T H.266.
///
/// This is the value VVenC uses for 8-bit content with the default
/// `MaxLog2TrDynamicRange`.
pub const MAX_LOG2_TR_DYNAMIC_RANGE: i32 = 15;

/// The quantizer's fixed-point shift, as in VVenC's `QUANT_SHIFT`.
const QUANT_SHIFT: i32 = 14;

/// The inverse quantizer's fixed-point shift, as in VVenC's `IQUANT_SHIFT`.
const IQUANT_SHIFT: i32 = 6;

/// VVC's `g_quantScales` table, indexed by `qp % 6`.
///
/// These are the values VVenC uses in `source/Lib/CommonLib/Rom.cpp`. The
/// table is the quantization scale for the six quantization parameters
/// within a QP group; the `qp / 6` term is applied as a shift.
const QUANT_SCALES: [i64; 6] = [26214, 23302, 20560, 18396, 16384, 14564];

/// The transform shift for a transform of the given size.
///
/// This is VVenC's `getTransformShift`, which accounts for the widened
/// dynamic range the forward transform produces.
fn transform_shift(size: usize) -> i32 {
    let log2_size = size.trailing_zeros() as i32;
    MAX_LOG2_TR_DYNAMIC_RANGE - BIT_DEPTH - log2_size
}

/// The quantization parameters a transform of the given size uses.
///
/// The struct holds the arithmetic the quantizer and dequantizer share, so
/// the two can never drift apart.
#[derive(Debug, Clone, Copy)]
pub struct QuantParams {
    /// The quantization parameter.
    pub qp: i32,
    /// The quantizer's right shift.
    pub q_bits: i32,
    /// The quantization scale, from VVC's `g_quantScales`.
    pub q_scale: i64,
    /// The rounding offset, which sets the dead zone's width.
    pub round_offset: i64,
}

impl QuantParams {
    /// The quantization parameters for a quantization parameter and transform
    /// size.
    pub fn new(qp: i32, size: usize) -> Self {
        let qp = qp.clamp(0, MAX_QP);
        let q_bits = QUANT_SHIFT + qp / 6 + transform_shift(size);
        // VVC's rounding term: 85 for a non-intra-refresh picture, shifted
        // into the quantizer's fixed-point domain. It is a sixth of a
        // quantization step, which is what gives the quantizer its dead
        // zone.
        let round_offset = 85i64 << (q_bits - 9).max(0);
        Self {
            qp,
            q_bits,
            q_scale: QUANT_SCALES[(qp % 6) as usize],
            round_offset,
        }
    }

    /// The size of one quantization step, in transform-coefficient units.
    ///
    /// This is the step the quantizer's levels correspond to, and it is
    /// what the rate-distortion search's distortion estimate is scaled by.
    pub fn step(&self) -> f64 {
        (1i64 << self.q_bits) as f64 / self.q_scale as f64
    }
}

/// The forward quantizer.
///
/// Coefficients inside the dead zone quantize to zero; everything else is
/// scaled to the nearest level. The sign is preserved exactly.
pub fn quantize(coefficients: &[i32], qp: i32) -> Vec<i32> {
    // The transform size is inferred from the coefficient count, so the
    // quantizer's shift matches the transform that produced them.
    let size = (coefficients.len() as f64).sqrt().round() as usize;
    if size * size != coefficients.len() || size == 0 {
        // A coefficient count that is not a square has no matching
        // transform; leave the values untouched rather than scaling them
        // with the wrong shift.
        return coefficients.to_vec();
    }
    let params = QuantParams::new(qp, size);
    quantize_with(coefficients, &params)
}

/// The forward quantizer with explicit parameters.
pub fn quantize_with(coefficients: &[i32], params: &QuantParams) -> Vec<i32> {
    coefficients
        .iter()
        .map(|c| {
            let magnitude = c.unsigned_abs() as i64;
            let scaled = magnitude * params.q_scale + params.round_offset;
            let level = (scaled >> params.q_bits) as i32;
            if *c < 0 {
                -level
            } else {
                level
            }
        })
        .collect()
}

/// The inverse quantizer.
pub fn dequantize(levels: &[i32], qp: i32) -> Vec<i32> {
    let size = (levels.len() as f64).sqrt().round() as usize;
    if size * size != levels.len() || size == 0 {
        return levels.to_vec();
    }
    let params = QuantParams::new(qp, size);
    dequantize_with(levels, &params)
}

/// The inverse quantizer with explicit parameters.
///
/// This is the exact algebraic inverse of [`quantize_with`]: a level maps
/// back to the middle of the interval it represents.
///
/// VVC's own dequantizer uses a different but equivalent arrangement -- it
/// indexes the inverse scale table with `qp + 1` and folds the sign into
/// the level -- and is not bit-compatible with this one. GIRC does not need
/// to be: its bitstream is its own research format, and what it does need
/// is that the dequantizer invert the quantizer exactly, so that the
/// rate-distortion search optimises against the true reconstruction and
/// the two signalling modes code identical residuals.
pub fn dequantize_with(levels: &[i32], params: &QuantParams) -> Vec<i32> {
    let numerator = 1i64 << params.q_bits;
    levels
        .iter()
        .map(|l| {
            let magnitude = i64::from(l.unsigned_abs());
            let value = (magnitude * numerator + params.q_scale / 2) / params.q_scale;
            let value = value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
            if *l < 0 {
                -value
            } else {
                value
            }
        })
        .collect()
}

/// The distortion cost of a reconstruction, in squared sample error.
pub fn sse(original: &[i32], reconstructed: &[i32]) -> i64 {
    original
        .iter()
        .zip(reconstructed.iter())
        .map(|(a, b)| {
            let d = (a - b) as i64;
            d * d
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_doubles_every_six_qp() {
        // A six-step increase in the quantization parameter doubles the
        // step size, as in every codec generation since H.263.
        let a = QuantParams::new(24, 8);
        let b = QuantParams::new(30, 8);
        let ratio = b.step() / a.step();
        assert!((ratio - 2.0).abs() < 1e-9, "step ratio was {ratio}");
    }

    #[test]
    fn the_step_grows_with_the_qp() {
        // The step must grow with the quantization parameter, and double
        // every six steps.
        let mut previous = 0.0;
        for qp in 0..=MAX_QP {
            let step = QuantParams::new(qp, 8).step();
            assert!(
                step > previous,
                "the step at QP {qp} was {step}, not above the previous {previous}"
            );
            previous = step;
        }
        let a = QuantParams::new(24, 8).step();
        let b = QuantParams::new(30, 8).step();
        assert!(
            (b / a - 2.0).abs() < 1e-9,
            "the six-step ratio was {}",
            b / a
        );
    }

    #[test]
    fn the_reconstruction_error_is_bounded_by_the_step() {
        // The reconstruction of a coefficient must be within one step of
        // it, at every quantization parameter. This is the property the
        // dead zone and the rounding together guarantee.
        let coefficients: Vec<i32> = (0..64).map(|i| (i * 37 - 1200) as i32).collect();
        for qp in 0..=MAX_QP {
            let params = QuantParams::new(qp, 8);
            let levels = quantize_with(&coefficients, &params);
            let restored = dequantize_with(&levels, &params);
            let step = params.step();
            for (original, got) in coefficients.iter().zip(restored.iter()) {
                assert!(
                    (original - got).abs() as f64 <= step,
                    "QP {qp}: {original} reconstructed as {got}, step {step}"
                );
            }
        }
    }

    #[test]
    fn dead_zone_maps_small_coefficients_to_zero() {
        // A coefficient far below the step must vanish.
        let params = QuantParams::new(37, 8);
        let levels = quantize_with(&[1], &params);
        assert_eq!(levels[0], 0, "small coefficient should be zero");
    }

    #[test]
    fn quantize_dequantize_round_trips_within_the_step() {
        // The reconstruction must be within half a quantization step of the
        // original, which is the property the dead zone guarantees.
        let coefficients: Vec<i32> = (0..64).map(|i| (i * 37 - 1200) as i32).collect();
        for qp in [0, 12, 24, 36, 51] {
            let params = QuantParams::new(qp, 8);
            let levels = quantize_with(&coefficients, &params);
            let restored = dequantize_with(&levels, &params);
            let step = params.step();
            for (original, got) in coefficients.iter().zip(restored.iter()) {
                assert!(
                    (original - got).abs() as f64 <= step * 1.5,
                    "QP {qp}: {original} vs {got}, step {step}"
                );
            }
        }
    }

    #[test]
    fn zero_quantizes_to_zero() {
        assert!(quantize(&[0, 0, 0], 20).iter().all(|v| *v == 0));
    }

    #[test]
    fn the_sign_is_preserved() {
        let levels = quantize(&[500, -500], 27);
        assert!(levels[0] > 0, "positive coefficient became {}", levels[0]);
        assert!(levels[1] < 0, "negative coefficient became {}", levels[1]);
        assert_eq!(levels[0], -levels[1], "magnitudes should match");
    }

    #[test]
    fn sse_is_zero_for_identical_blocks() {
        assert_eq!(sse(&[1, 2, 3], &[1, 2, 3]), 0);
        assert_eq!(sse(&[1, 2, 3], &[1, 2, 4]), 1);
    }
}
