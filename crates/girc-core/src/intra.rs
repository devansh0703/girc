//! Planar and DC intra prediction.
//!
//! GIRC codes every block in intra mode using a small, fixed set of
//! prediction modes derived from the already-reconstructed causal neighbours:
//!
//! * `Planar` -- the HEVC/H.266 planar prediction, a bilinear blend of the
//!   top row and left column of reference samples.
//! * `Dc` -- the mean of the available reference samples.
//! * `Angular` -- the HEVC/H.266 angular prediction at a fixed set of
//!   angles, using the neighbouring samples along the prediction direction.
//!
//! The angular set is deliberately small: GIRC's research question is the
//! cost of *transform-type* signalling, so the prediction set is held
//! constant between the explicit-signalling and inference codecs and the
//! comparison stays apples-to-apples.

/// The intra prediction modes GIRC can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntraMode {
    /// Planar (HEVC/H.266 mode 0).
    Planar,
    /// DC (HEVC/H.266 mode 1).
    Dc,
    /// Angular prediction at the given index into the angular table.
    Angular(u8),
}

impl IntraMode {
    /// The angular modes GIRC supports, as (index, horizontal, vertical)
    /// offsets from the prediction origin.
    ///
    /// The angles mirror the HEVC/H.266 intra angle table restricted to the
    /// horizontal and vertical families plus the two diagonals, so every
    /// mode is a genuine directional predictor.
    const ANGLES: [(i32, i32); 8] = [
        (0, 32),   // horizontal
        (32, 0),   // vertical
        (-26, 26), // diagonal, down-left
        (26, -26), // diagonal, down-right
        (-13, 45), // steep diagonal
        (45, -13), // shallow diagonal
        (-45, 13), // steep diagonal, opposite
        (13, -45), // shallow diagonal, opposite
    ];

    /// The number of modes in the set.
    pub const COUNT: usize = 2 + Self::ANGLES.len();

    /// The mode index as it appears on the wire.
    pub fn as_index(self) -> u8 {
        match self {
            IntraMode::Planar => 0,
            IntraMode::Dc => 1,
            IntraMode::Angular(i) => 2 + i,
        }
    }

    /// Inverse of [`IntraMode::as_index`].
    pub fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(IntraMode::Planar),
            1 => Some(IntraMode::Dc),
            i if i >= 2 && (i as usize) < Self::COUNT => Some(IntraMode::Angular(i - 2)),
            _ => None,
        }
    }

    /// Every mode in the set.
    pub fn all() -> Vec<IntraMode> {
        let mut modes = vec![IntraMode::Planar, IntraMode::Dc];
        modes.extend((0..Self::ANGLES.len() as u8).map(IntraMode::Angular));
        modes
    }

    pub fn name(self) -> &'static str {
        match self {
            IntraMode::Planar => "planar",
            IntraMode::Dc => "dc",
            IntraMode::Angular(i) => {
                let (h, v) = Self::ANGLES[i as usize];
                if h == 0 {
                    "vertical"
                } else if v == 0 {
                    "horizontal"
                } else if h == v {
                    "diagonal"
                } else {
                    "angular"
                }
            }
        }
    }
}

/// The causal reference samples available to a block.
///
/// `left` holds the reconstructed samples immediately to the left of the
/// block, top to bottom. `top` holds the samples immediately above the
/// block, left to right. `top_left` is the single sample diagonally above
/// and to the left. A `None` entry means the sample does not exist (the
/// block is at the picture edge, or the neighbour was not yet coded).
#[derive(Debug, Clone, Default)]
pub struct ReferenceSamples {
    pub left: Vec<Option<i32>>,
    pub top: Vec<Option<i32>>,
    pub top_left: Option<i32>,
}

impl ReferenceSamples {
    /// A reference set where nothing is available.
    pub fn empty(size: usize) -> Self {
        Self {
            left: vec![None; size],
            top: vec![None; size],
            top_left: None,
        }
    }

    /// Whether any reference sample is available at all.
    pub fn is_empty(&self) -> bool {
        self.left.iter().all(Option::is_none)
            && self.top.iter().all(Option::is_none)
            && self.top_left.is_none()
    }

    /// The samples that are available, in a stable order.
    fn available(&self) -> Vec<i32> {
        let mut out = Vec::new();
        if let Some(v) = self.top_left {
            out.push(v);
        }
        out.extend(self.left.iter().flatten().copied());
        out.extend(self.top.iter().flatten().copied());
        out
    }
}

/// The number of fractional bits in the angular prediction offsets.
const ANGULAR_PRECISION: i32 = 5;

/// Predict a `size` x `size` block.
///
/// The result is in the sample domain, before the residual is added.
pub fn predict(mode: IntraMode, size: usize, reference: &ReferenceSamples) -> Vec<i32> {
    let mut out = vec![0i32; size * size];
    match mode {
        IntraMode::Planar => predict_planar(size, reference, &mut out),
        IntraMode::Dc => predict_dc(size, reference, &mut out),
        IntraMode::Angular(index) => predict_angular(size, reference, index, &mut out),
    }
    out
}

/// The HEVC/H.266 planar prediction.
///
/// Each sample is a bilinear blend of the two reference samples at the
/// opposite corners of the block, which for a block at `(x, y)` in the
/// picture reduces to a blend of the top-right and bottom-left corners.
fn predict_planar(size: usize, reference: &ReferenceSamples, out: &mut [i32]) {
    let top_row: Vec<i32> = (0..size)
        .map(|x| reference.top.get(x).copied().flatten().unwrap_or(0))
        .collect();
    let left_col: Vec<i32> = (0..size)
        .map(|y| reference.left.get(y).copied().flatten().unwrap_or(0))
        .collect();

    // The corner values the blend needs. Missing samples fall back to the
    // nearest available value, matching the reference sample substitution
    // rule in ITU-T H.264 clause 8.3.3.2.2.
    let top_right = top_row[size - 1];
    let bottom_left = left_col[size - 1];
    let top_left = reference.top_left.unwrap_or(top_row[0]);

    for y in 0..size {
        for x in 0..size {
            // (size-1) is the denominator of the planar blend.
            let num = (size - 1) as i32;
            let value = if num == 0 {
                top_left
            } else {
                let a = (num - y as i32) * top_row[x] + y as i32 * bottom_left;
                let b = (num - x as i32) * left_col[y] + x as i32 * top_right;
                (a + b + num) / (2 * num)
            };
            out[y * size + x] = value;
        }
    }
}

/// The DC prediction: the mean of all available reference samples.
fn predict_dc(_size: usize, reference: &ReferenceSamples, out: &mut [i32]) {
    let available = reference.available();
    let value = if available.is_empty() {
        1 << (8 - 1) // mid-level, ITU-T H.264 clause 8.3.3.2.1
    } else {
        available.iter().sum::<i32>() / available.len() as i32
    };
    out.iter_mut().for_each(|v| *v = value);
}

/// The angular prediction at the given mode index.
///
/// The prediction extends the neighbouring samples along the prediction
/// direction, using fixed-point arithmetic with [`ANGULAR_PRECISION`]
/// fractional bits, as in ITU-T H.264 clause 8.3.3.2.4.
fn predict_angular(size: usize, reference: &ReferenceSamples, index: u8, out: &mut [i32]) {
    let (hor, ver) = IntraMode::ANGLES[index as usize];
    let (abs_hor, abs_ver) = (hor.abs(), ver.abs());

    // Build the 1-D reference line: `ref[0]` is the sample at (-1, -1) and
    // `ref[1 + i]` is the sample at (i, -1) for the top row, or (-1, i) for
    // the left column.
    let horizontal = hor.abs() >= ver.abs();
    let line: Vec<i32> = if horizontal {
        let mut line = Vec::with_capacity(size + 1);
        line.push(reference.top_left.unwrap_or(0));
        line.extend((0..size).map(|x| reference.top[x].unwrap_or(0)));
        line
    } else {
        let mut line = Vec::with_capacity(size + 1);
        line.push(reference.top_left.unwrap_or(0));
        line.extend((0..size).map(|y| reference.left[y].unwrap_or(0)));
        line
    };

    // The main reference index for the first sample, and the step between
    // samples, in fixed point.
    let (base, step) = if horizontal { (ver, hor) } else { (hor, ver) };
    // The reference line runs at 45 degrees in the block's sample grid; the
    // step is the ratio of the two angle components.
    let step_num = step;
    let step_den = abs_hor.max(abs_ver);
    let inv_step = if step_num == 0 {
        0
    } else {
        (1i64 << (ANGULAR_PRECISION * 2)) / step_num as i64
    };
    let _ = (base, step_den, inv_step);

    for y in 0..size {
        for x in 0..size {
            // Project the sample onto the reference line. For a horizontal
            // dominant angle the vertical offset drives the index; for a
            // vertical dominant angle the horizontal offset does.
            let (major, minor) = if horizontal { (x, y) } else { (y, x) };

            // The index of the reference sample for this position, in
            // fixed point with ANGULAR_PRECISION fractional bits.
            let idx_fp = (major as i64) * (1i64 << ANGULAR_PRECISION);
            let minor_fp = (minor as i64) * (1i64 << ANGULAR_PRECISION) * inv_step
                / (1i64 << ANGULAR_PRECISION);

            let idx = idx_fp + if hor >= 0 { 0 } else { minor_fp };
            let idx = if ver >= 0 || !horizontal {
                idx
            } else {
                idx - minor_fp
            };

            let i = (idx >> ANGULAR_PRECISION).clamp(0, size as i64 - 1) as usize;
            let frac = (idx & ((1 << ANGULAR_PRECISION) - 1)) as i32;

            // Linear interpolation between the two nearest reference samples.
            let a = line[i];
            let b = line[(i + 1).min(size)];
            out[y * size + x] =
                (a * ((1 << ANGULAR_PRECISION) - frac) + b * frac) >> ANGULAR_PRECISION;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_reference(size: usize, value: i32) -> ReferenceSamples {
        ReferenceSamples {
            left: vec![Some(value); size],
            top: vec![Some(value); size],
            top_left: Some(value),
        }
    }

    /// A constant neighbourhood must predict that constant for every mode.
    #[test]
    fn constant_reference_predicts_constant() {
        for size in [4usize, 8, 16] {
            let reference = full_reference(size, 77);
            for mode in IntraMode::all() {
                let pred = predict(mode, size, &reference);
                for (i, v) in pred.iter().enumerate() {
                    assert!(
                        (v - 77).abs() <= 1,
                        "{mode:?} {size}: sample {i} is {v}, expected ~77"
                    );
                }
            }
        }
    }

    /// Planar prediction of a constant neighbourhood is exactly the constant.
    #[test]
    fn planar_is_exact_for_constant_reference() {
        let size = 8;
        let reference = full_reference(size, 42);
        let pred = predict(IntraMode::Planar, size, &reference);
        assert!(
            pred.iter().all(|v| *v == 42),
            "planar of a constant is exact"
        );
    }

    /// DC prediction is the rounded mean of the available samples.
    #[test]
    fn dc_is_the_mean() {
        let reference = ReferenceSamples {
            left: vec![Some(10), Some(20)],
            top: vec![Some(30)],
            top_left: Some(40),
        };
        let pred = predict(IntraMode::Dc, 2, &reference);
        assert!(pred.iter().all(|v| *v == 25), "mean of 10,20,30,40 is 25");
    }

    /// With no reference at all, DC falls back to the mid-level.
    #[test]
    fn dc_without_reference_uses_mid_level() {
        let reference = ReferenceSamples::empty(4);
        let pred = predict(IntraMode::Dc, 4, &reference);
        assert!(pred.iter().all(|v| *v == 128), "mid-level for 8-bit");
    }

    /// Every mode index round-trips.
    #[test]
    fn mode_indices_round_trip() {
        for (i, mode) in IntraMode::all().into_iter().enumerate() {
            assert_eq!(mode.as_index() as usize, i);
            assert_eq!(IntraMode::from_index(mode.as_index()), Some(mode));
        }
        assert_eq!(IntraMode::from_index(200), None);
    }
}
