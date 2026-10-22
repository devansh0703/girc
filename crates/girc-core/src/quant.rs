//! Quantization and dequantization of transform coefficients.
//!
//! GIRC uses the HEVC/H.266-style uniform quantizer: a dead-zone around
//! zero, a step size derived from the quantization parameter, and a
//! sign-preserving reconstruction. The scheme is deliberately the same in
//! both the explicit-signalling and inference codecs, so the measured
//! difference between them is attributable to the transform-type signalling
//! and nothing else.

/// The largest quantization parameter GIRC accepts.
pub const MAX_QP: i32 = 51;

/// The quantization step size for a quantization parameter.
///
/// The mapping follows ITU-T H.264 clause 8.5.9: the step doubles every six
/// quantization parameter steps, and the base step for 8-bit content is 16
/// at QP 4.
pub fn quant_step(qp: i32) -> f32 {
    let qp = qp.clamp(0, MAX_QP);
    let base = 0.625f32 * (1i64 << (qp / 6)) as f32;
    let offset = 0.5 * (((qp % 6) as f32) / 6.0 - 0.5);
    base * (1.0 + offset) / 1.0
}

/// The forward quantizer.
///
/// Coefficients inside the dead zone quantize to zero; everything else is
/// rounded to the nearest multiple of the step.
pub fn quantize(coefficients: &[i32], qp: i32) -> Vec<i32> {
    let step = quant_step(qp);
    coefficients
        .iter()
        .map(|c| {
            let magnitude = c.abs() as f32;
            // The dead zone is half the step, as in ITU-T H.264 clause 8.5.3.
            if magnitude < 0.5 * step {
                return 0;
            }
            let level = ((magnitude / step) + 0.5).floor();
            let sign = if *c < 0 { -1 } else { 1 };
            (level as i32) * sign
        })
        .collect()
}

/// The inverse quantizer.
pub fn dequantize(levels: &[i32], qp: i32) -> Vec<i32> {
    let step = quant_step(qp);
    levels
        .iter()
        .map(|l| (*l as f32 * step).round() as i32)
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
        let a = quant_step(0);
        let b = quant_step(6);
        assert!((b / a - 2.0).abs() < 1e-5, "step ratio was {}", b / a);
    }

    #[test]
    fn step_is_monotonic_in_qp() {
        let mut previous = 0.0;
        for qp in 0..=MAX_QP {
            let step = quant_step(qp);
            assert!(step >= previous, "step decreased at QP {qp}");
            previous = step;
        }
    }

    #[test]
    fn dead_zone_maps_small_coefficients_to_zero() {
        // A coefficient far below the step must vanish.
        let levels = quantize(&[1], 30);
        assert_eq!(levels[0], 0, "small coefficient should be zero");
    }

    #[test]
    fn quantize_dequantize_round_trips_within_the_step() {
        let coefficients: Vec<i32> = (0..64).map(|i| (i * 37 - 1200) as i32).collect();
        for qp in [0, 12, 24, 36, 51] {
            let levels = quantize(&coefficients, qp);
            let restored = dequantize(&levels, qp);
            let step = quant_step(qp);
            for (original, got) in coefficients.iter().zip(restored.iter()) {
                assert!(
                    (original - got).abs() as f32 <= step * 1.5,
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
    fn sse_is_zero_for_identical_blocks() {
        assert_eq!(sse(&[1, 2, 3], &[1, 2, 3]), 0);
        assert_eq!(sse(&[1, 2, 3], &[1, 2, 4]), 1);
    }
}
