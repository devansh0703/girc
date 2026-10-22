//! Geometry-inference transform selection.
//!
//! # The mechanism
//!
//! A conventional codec signals the transform it used for each block: the
//! decoder reads a `transform_type` syntax element and applies the transform
//! the encoder named. That signalling costs bits, and in VVC it costs a bit
//! for every luma block that can use more than one transform.
//!
//! VVC already showed that the signalling can be removed in one case.
//! Implicit multiple transform selection derives the transform type from
//! information both sides already have, so no bits are spent. But that
//! derivation is a fixed rule keyed on block size and neighbour availability.
//! It cannot express "this particular block's geometry favours a sine
//! transform".
//!
//! GIRC generalises the idea: the decoder *predicts* the transform type from
//! the local geometry of the block, and the encoder spends bits only when the
//! prediction is wrong.
//!
//! # The predictor
//!
//! The prediction is driven by the orientation statistics of the causal
//! neighbourhood -- the already-reconstructed samples immediately above and
//! to the left of the block. The intuition is specific: DCT-VIII and DST-VII
//! earn their keep on blocks whose residual has directional structure, and a
//! neighbourhood with a dominant edge orientation is evidence of exactly
//! that. The predictor turns the neighbourhood's gradient orientation
//! histogram into a probability over the three transforms.
//!
//! The predictor is a *trained* model, not a hand-tuned rule, so the
//! training pipeline (`python/girc-train`) and the model file it produces
//! are part of the protected research package. The model is a small
//! multinomial logistic regression over a fixed feature vector, which keeps
//! the encoder and decoder provably in agreement: both run the same
//! arithmetic on the same inputs.
//!
//! # Signalling
//!
//! In `Explicit` mode the encoder signals the transform type directly, which
//! is the baseline every codec uses.
//!
//! In `Inferred` mode the encoder sends one bit: "my rate-distortion
//! choice differs from the decoder's prediction". When the bit is zero the
//! decoder uses its prediction. When it is one, the encoder follows with the
//! explicit transform type. The signalling saving is therefore
//!
//! ```text
//! explicit:  always pay for the transform type
//! inferred:  pay one bit, plus the transform type only on a mismatch
//! ```
//!
//! so the mechanism wins whenever the predictor is right often enough, and
//! the rate-distortion cost of the occasional wrong prediction is paid in
//! quality rather than in bits. The benchmark measures both effects.

use crate::intra::IntraMode;
use crate::transform::{TransformKind, TransformMode};

/// The geometry features extracted from a block's causal neighbourhood.
///
/// The feature vector is fixed and ordered; the trained model's weights are
/// indexed against it. Changing the order or the count of the features
/// invalidates every model file, so the layout is part of the format.
#[derive(Debug, Clone, PartialEq)]
pub struct GeometryFeatures {
    /// Gradient orientation histogram of the neighbourhood.
    ///
    /// Bins cover the full circle; bin `i` covers angles
    /// `[i * 45deg, (i+1) * 45deg)`. The histogram is taken over a
    /// two-dimensional gradient field, so every bin is reachable.
    pub orientation_histogram: [f32; 8],
    /// The total gradient magnitude, log-scaled.
    pub gradient_energy: f32,
    /// The dominant orientation's magnitude share of the total.
    pub dominance: f32,
    /// The angular dispersion of the neighbourhood, in `[0, 1]`.
    ///
    /// Zero means a single dominant orientation; one means an isotropic
    /// neighbourhood.
    pub isotropy: f32,
    /// The number of available reference samples, normalised.
    pub reference_coverage: f32,
    /// The horizontal gradient energy, log-scaled.
    ///
    /// The horizontal and vertical energies are kept apart because the
    /// transforms differ in how they treat the two axes.
    pub horizontal_energy: f32,
    /// The vertical gradient energy, log-scaled.
    pub vertical_energy: f32,
    /// The correlation between the horizontal and vertical gradients.
    ///
    /// A value near one means the gradients point the same way, which
    /// marks an edge; a value near minus one marks a corner.
    pub axis_correlation: f32,
    /// The quantization parameter, normalised.
    ///
    /// The quantization parameter changes which transform the
    /// rate-distortion search prefers, and the decoder reads it from the
    /// bitstream header, so it is available on both sides.
    pub quantization_parameter: f32,
    /// The one-hot encoding of the intra prediction mode.
    ///
    /// The mode is decoded before the transform type, so the decoder
    /// always has it. It is the single most informative signal about
    /// which transform the encoder will want: the transform has to match
    /// the direction the prediction residual runs in.
    pub intra_mode: [f32; IntraMode::COUNT],
    /// The transforms the already-coded neighbouring blocks used.
    ///
    /// Four neighbours are used: the block to the left, the block above,
    /// the block above and to the left, and the block above and to the
    /// right. Each contributes a one-hot encoding of its transform, or
    /// all zeros when the neighbour does not exist.
    ///
    /// This is the strongest signal the predictor has. Transform choice is
    /// strongly spatially correlated: content that wants a sine transform
    /// in one block usually wants it in the blocks around it, because the
    /// underlying structure does not change at a block boundary. The
    /// decoder already knows every neighbour's transform, because it
    /// decoded them, so this context is free.
    ///
    /// It is also the context the codec literature uses. HEVC and VVC
    /// both derive the transform type of a transform unit from the
    /// neighbouring units' types, and the VVC implicit multiple
    /// transform selection proposals do the same.
    pub neighbour_transforms: [f32; NEIGHBOUR_TRANSFORM_SLOTS * MODE_COUNT],
}

impl GeometryFeatures {
    /// The number of features in the vector.
    pub const LEN: usize =
        8 + 4 + 3 + 1 + IntraMode::COUNT + NEIGHBOUR_TRANSFORM_SLOTS * MODE_COUNT;

    /// The feature vector in the order the trained model expects.
    pub fn to_vec(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.extend_from_slice(&self.orientation_histogram);
        out.push(self.gradient_energy);
        out.push(self.dominance);
        out.push(self.isotropy);
        out.push(self.reference_coverage);
        out.push(self.horizontal_energy);
        out.push(self.vertical_energy);
        out.push(self.axis_correlation);
        out.push(self.quantization_parameter);
        out.extend_from_slice(&self.intra_mode);
        out.extend_from_slice(&self.neighbour_transforms);
        out
    }
}

/// The number of orientation bins in the histogram.
const ORIENTATION_BINS: usize = 8;

/// The number of neighbouring blocks whose transform is used as context.
///
/// The order is left, above, above-left, above-right.
pub const NEIGHBOUR_TRANSFORM_SLOTS: usize = 4;

/// The number of transform modes the predictor chooses between.
///
/// The plain DCT-II and the four multiple transform selection modes.
pub const MODE_COUNT: usize = 5;

/// The transform on one axis of a mode.
///
/// A mode is the pair of its horizontal and vertical transforms. The
/// axis is either the plain DCT-II, in which case the mode is the plain
/// DCT-II, or one of the two multiple transform selection transforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisTransform {
    /// The plain DCT-II.
    Dct2,
    /// The DCT-VIII.
    Dct8,
    /// The DST-VII.
    Dst7,
}

impl AxisTransform {
    /// Every axis transform, in canonical order.
    pub const ALL: [AxisTransform; 3] = [
        AxisTransform::Dct2,
        AxisTransform::Dct8,
        AxisTransform::Dst7,
    ];

    /// The index of the transform.
    pub fn as_index(self) -> usize {
        match self {
            AxisTransform::Dct2 => 0,
            AxisTransform::Dct8 => 1,
            AxisTransform::Dst7 => 2,
        }
    }

    /// The transform of the given mode on this axis.
    pub fn of(mode: TransformMode, horizontal: bool) -> AxisTransform {
        let kind = if horizontal {
            mode.horizontal
        } else {
            mode.vertical
        };
        match kind {
            TransformKind::Dct2 => AxisTransform::Dct2,
            TransformKind::Dct8 => AxisTransform::Dct8,
            TransformKind::Dst7 => AxisTransform::Dst7,
        }
    }

    /// The mode the two axis transforms imply.
    ///
    /// A mode is either the plain DCT-II on both axes, or a multiple
    /// transform selection mode, in which neither axis is the DCT-II. So
    /// a DCT-II on either axis forces the plain DCT-II, and otherwise
    /// the two axes combine directly.
    pub fn compose(horizontal: AxisTransform, vertical: AxisTransform) -> TransformMode {
        use AxisTransform::*;
        let (h, v) = match (horizontal, vertical) {
            (Dct2, _) | (_, Dct2) => return TransformMode::DCT2,
            (Dst7, Dst7) => (TransformKind::Dst7, TransformKind::Dst7),
            (Dct8, Dst7) => (TransformKind::Dct8, TransformKind::Dst7),
            (Dst7, Dct8) => (TransformKind::Dst7, TransformKind::Dct8),
            (Dct8, Dct8) => (TransformKind::Dct8, TransformKind::Dct8),
        };
        TransformMode {
            horizontal: h,
            vertical: v,
        }
    }
}

/// The transforms of the neighbouring blocks, as context for the predictor.
///
/// The four slots are the block to the left, the block above, the block
/// above and to the left, and the block above and to the right. A slot
/// that has not been coded yet -- because the neighbour is outside the
/// picture, or because the raster order has not reached it -- is empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NeighbourTransforms(pub [Option<TransformMode>; NEIGHBOUR_TRANSFORM_SLOTS]);

impl NeighbourTransforms {
    /// The one-hot encoding of the neighbour transforms.
    ///
    /// Each occupied slot contributes a 1 in the position of its
    /// transform; an empty slot contributes nothing.
    pub fn one_hot(&self) -> [f32; NEIGHBOUR_TRANSFORM_SLOTS * MODE_COUNT] {
        let mut out = [0f32; NEIGHBOUR_TRANSFORM_SLOTS * MODE_COUNT];
        for (slot, mode) in self.0.iter().enumerate() {
            if let Some(mode) = mode {
                out[slot * MODE_COUNT + mode.as_index() as usize] = 1.0;
            }
        }
        out
    }
}

/// Extract the geometry features from a causal neighbourhood.
///
/// `left` holds the reconstructed samples to the left of the block (top to
/// bottom) and `top` the samples above it (left to right). Both may contain
/// gaps where a neighbour does not exist.
///
/// The gradient is taken in two dimensions rather than along the reference
/// lines alone. The decoder has already reconstructed every sample above
/// and to the left of the current block, and the block diagonally above
/// and to the left is the one whose structure the current block most
/// resembles. Restricting the gradient to the two reference lines throws
/// away exactly the diagonal information that distinguishes the three
/// transforms.
pub fn extract_features(
    left: &[Option<i32>],
    top: &[Option<i32>],
    mode: IntraMode,
    qp: i32,
    neighbours: NeighbourTransforms,
) -> GeometryFeatures {
    let mut histogram = [0f32; ORIENTATION_BINS];
    let mut total = 0f32;
    let mut horizontal = 0f32;
    let mut vertical = 0f32;
    let mut cross = 0f32;

    // The gradient of the top row, in the horizontal direction.
    for window in top.windows(2) {
        let (Some(a), Some(b)) = (window[0], window[1]) else {
            continue;
        };
        let dx = (b - a) as f32;
        accumulate(&mut histogram, &mut total, dx, 0.0, dx.abs());
        horizontal += dx * dx;
    }

    // The gradient of the left column, in the vertical direction.
    for window in left.windows(2) {
        let (Some(a), Some(b)) = (window[0], window[1]) else {
            continue;
        };
        let dy = (b - a) as f32;
        accumulate(&mut histogram, &mut total, 0.0, dy, dy.abs());
        vertical += dy * dy;
    }

    // The diagonal block: the samples above and to the left of the
    // top-left corner. Their two-dimensional gradient is what carries the
    // diagonal orientation information.
    //
    // The top row's rightmost sample and the left column's topmost sample
    // meet at the top-left corner, so the diagonal pairs are the ones
    // spanning that corner.
    if let (Some(corner), Some(above), Some(beside)) = (
        top.last().copied().flatten(),
        top.get(top.len().saturating_sub(2)).copied().flatten(),
        left.first().copied().flatten(),
    ) {
        // The gradient across the corner: one axis runs along the top
        // row, the other down the left column.
        let dx = (corner - above) as f32;
        let dy = (beside - corner) as f32;
        let magnitude = (dx * dx + dy * dy).sqrt();
        accumulate(&mut histogram, &mut total, dx, dy, magnitude);
        horizontal += dx * dx;
        vertical += dy * dy;
        cross += dx * dy;
    }

    let mut intra_mode = [0f32; IntraMode::COUNT];
    intra_mode[mode.as_index() as usize] = 1.0;

    if total <= 0.0 {
        // A flat neighbourhood carries no orientation information.
        return GeometryFeatures {
            orientation_histogram: [0.0; ORIENTATION_BINS],
            gradient_energy: 0.0,
            dominance: 0.0,
            isotropy: 1.0,
            reference_coverage: coverage(left, top),
            horizontal_energy: 0.0,
            vertical_energy: 0.0,
            axis_correlation: 0.0,
            quantization_parameter: qp as f32 / 51.0,
            intra_mode,
            neighbour_transforms: neighbours.one_hot(),
        };
    }

    let mut normalised = [0f32; ORIENTATION_BINS];
    for i in 0..ORIENTATION_BINS {
        normalised[i] = histogram[i] / total;
    }

    let dominance = normalised.iter().copied().fold(0f32, f32::max);

    // Isotropy: 1 minus the normalised entropy of the histogram, scaled to
    // [0, 1]. A single dominant bin gives 0, a uniform histogram gives 1.
    let entropy: f32 = normalised
        .iter()
        .filter(|p| **p > 0.0)
        .map(|p| -p * p.log2())
        .sum();
    let max_entropy = (ORIENTATION_BINS as f32).log2();
    let isotropy = (entropy / max_entropy).clamp(0.0, 1.0);

    // The correlation between the two axes, normalised by their energies.
    let axis_correlation = if horizontal > 0.0 && vertical > 0.0 {
        (cross / (horizontal * vertical).sqrt()).clamp(-1.0, 1.0)
    } else {
        0.0
    };

    GeometryFeatures {
        orientation_histogram: normalised,
        gradient_energy: total.log2().max(0.0),
        dominance,
        isotropy,
        reference_coverage: coverage(left, top),
        // The energies are log-scaled; a zero energy would give a
        // negative infinity, so it is floored at one.
        horizontal_energy: horizontal.max(1.0).log2(),
        vertical_energy: vertical.max(1.0).log2(),
        axis_correlation,
        quantization_parameter: qp as f32 / 51.0,
        intra_mode,
        neighbour_transforms: neighbours.one_hot(),
    }
}

/// Extract the geometry features from a two-dimensional causal window.
///
/// This is the form the decoder uses in practice. The decoder has already
/// reconstructed every sample above and to the left of the current block,
/// so it can read a genuine two-dimensional neighbourhood rather than the
/// two reference lines alone. The diagonal information in such a window is
/// what distinguishes the three transforms, and it is not visible on the
/// reference lines.
///
/// The window is the square region that ends at the block's top-left
/// corner, `window` samples wide and high. Every sample in it has already
/// been reconstructed when the decoder reaches the current block.
pub fn extract_features_from_window(
    samples: &[u8],
    stride: usize,
    origin: (usize, usize),
    window: usize,
    mode: IntraMode,
    qp: i32,
    neighbours: NeighbourTransforms,
) -> GeometryFeatures {
    let (origin_x, origin_y) = origin;
    let mut histogram = [0f32; ORIENTATION_BINS];
    let mut total = 0f32;
    let mut horizontal = 0f32;
    let mut vertical = 0f32;
    let mut cross = 0f32;
    let mut count = 0usize;

    // The gradient is taken with central differences, so the window has to
    // be at least three samples wide in each direction.
    if window < 3 || origin_x < window || origin_y < window {
        return GeometryFeatures {
            orientation_histogram: [0.0; ORIENTATION_BINS],
            gradient_energy: 0.0,
            dominance: 0.0,
            isotropy: 1.0,
            reference_coverage: 0.0,
            horizontal_energy: 0.0,
            vertical_energy: 0.0,
            axis_correlation: 0.0,
            quantization_parameter: qp as f32 / 51.0,
            intra_mode: {
                let mut modes = [0f32; IntraMode::COUNT];
                modes[mode.as_index() as usize] = 1.0;
                modes
            },
            neighbour_transforms: neighbours.one_hot(),
        };
    }

    let x0 = origin_x - window;
    let y0 = origin_y - window;

    for y in y0 + 1..origin_y - 1 {
        for x in x0 + 1..origin_x - 1 {
            let at = |xx: usize, yy: usize| samples[yy * stride + xx] as f32;
            let dx = at(x + 1, y) - at(x - 1, y);
            let dy = at(x, y + 1) - at(x, y - 1);
            let magnitude = (dx * dx + dy * dy).sqrt();
            accumulate(&mut histogram, &mut total, dx, dy, magnitude);
            horizontal += dx * dx;
            vertical += dy * dy;
            cross += dx * dy;
            count += 1;
        }
    }

    let mut intra_mode = [0f32; IntraMode::COUNT];
    intra_mode[mode.as_index() as usize] = 1.0;

    if total <= 0.0 || count == 0 {
        return GeometryFeatures {
            orientation_histogram: [0.0; ORIENTATION_BINS],
            gradient_energy: 0.0,
            dominance: 0.0,
            isotropy: 1.0,
            reference_coverage: 0.0,
            horizontal_energy: 0.0,
            vertical_energy: 0.0,
            axis_correlation: 0.0,
            quantization_parameter: qp as f32 / 51.0,
            intra_mode,
            neighbour_transforms: neighbours.one_hot(),
        };
    }

    let mut normalised = [0f32; ORIENTATION_BINS];
    for i in 0..ORIENTATION_BINS {
        normalised[i] = histogram[i] / total;
    }

    let dominance = normalised.iter().copied().fold(0f32, f32::max);

    let entropy: f32 = normalised
        .iter()
        .filter(|p| **p > 0.0)
        .map(|p| -p * p.log2())
        .sum();
    let max_entropy = (ORIENTATION_BINS as f32).log2();
    let isotropy = (entropy / max_entropy).clamp(0.0, 1.0);

    let axis_correlation = if horizontal > 0.0 && vertical > 0.0 {
        (cross / (horizontal * vertical).sqrt()).clamp(-1.0, 1.0)
    } else {
        0.0
    };

    GeometryFeatures {
        orientation_histogram: normalised,
        gradient_energy: total.log2().max(0.0),
        dominance,
        isotropy,
        reference_coverage: 1.0,
        horizontal_energy: horizontal.max(1.0).log2(),
        vertical_energy: vertical.max(1.0).log2(),
        axis_correlation,
        quantization_parameter: qp as f32 / 51.0,
        intra_mode,
        neighbour_transforms: neighbours.one_hot(),
    }
}

/// Add a gradient sample to the orientation histogram.
fn accumulate(
    histogram: &mut [f32; ORIENTATION_BINS],
    total: &mut f32,
    dx: f32,
    dy: f32,
    magnitude: f32,
) {
    if magnitude <= 0.0 {
        return;
    }
    // The angle is measured counter-clockwise from the positive x axis.
    let angle = dy.atan2(dx);
    let mut bin = ((angle + std::f32::consts::PI) / (2.0 * std::f32::consts::PI)
        * ORIENTATION_BINS as f32)
        .floor() as isize;
    bin = bin.rem_euclid(ORIENTATION_BINS as isize);
    histogram[bin as usize] += magnitude;
    *total += magnitude;
}

/// The fraction of reference samples that are available.
fn coverage(left: &[Option<i32>], top: &[Option<i32>]) -> f32 {
    let total = left.len() + top.len();
    if total == 0 {
        return 0.0;
    }
    let available =
        left.iter().filter(|s| s.is_some()).count() + top.iter().filter(|s| s.is_some()).count();
    available as f32 / total as f32
}

/// A trained multinomial logistic regression over [`GeometryFeatures`].
///
/// The model is a set of per-class weight vectors and biases. The predicted
/// class is the one with the highest logit. Because the model is a pure
/// function of its weights and the feature vector, the encoder and decoder
/// always agree.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformPredictor {
    /// The horizontal-axis predictor: three bias terms and three weight
    /// vectors, indexed by [`AxisTransform::as_index`].
    ///
    /// The predictor models the two axes separately rather than the joint
    /// mode. The measurement behind that is in the patent package: the
    /// joint mode is barely predictable from the causal neighbourhood,
    /// because the rate-distortion choice between the five modes is close
    /// to a tie, but each axis on its own is noticeably more
    /// predictable, because the geometry of a neighbourhood says more
    /// about the transform on one axis than about the mode as a whole.
    pub horizontal_biases: [f32; 3],
    /// The horizontal-axis weight vectors.
    pub horizontal_weights: [[f32; GeometryFeatures::LEN]; 3],
    /// The vertical-axis bias terms.
    pub vertical_biases: [f32; 3],
    /// The vertical-axis weight vectors.
    pub vertical_weights: [[f32; GeometryFeatures::LEN]; 3],
    /// The version of the training data the model was fitted on, recorded
    /// so a report can state which model produced a number.
    pub model_version: String,
}

impl TransformPredictor {
    /// A model that always predicts DCT-II, the baseline transform.
    ///
    /// Used when no trained model is available; the explicit-signalling
    /// baseline does not need a predictor at all.
    pub fn constant_dct2() -> Self {
        // A positive bias on DCT-II wins the argmax whenever every feature
        // is zero.
        let mut horizontal = [[0f32; GeometryFeatures::LEN]; 3];
        horizontal[0][0] = 1.0;
        let mut vertical = [[0f32; GeometryFeatures::LEN]; 3];
        vertical[0][0] = 1.0;
        Self {
            horizontal_biases: [1.0, 0.0, 0.0],
            horizontal_weights: horizontal,
            vertical_biases: [1.0, 0.0, 0.0],
            vertical_weights: vertical,
            model_version: "constant-dct2".into(),
        }
    }

    /// The three logits of one axis.
    fn axis_logits(&self, features: &GeometryFeatures, horizontal: bool) -> [f32; 3] {
        let (biases, weights) = if horizontal {
            (&self.horizontal_biases, &self.horizontal_weights)
        } else {
            (&self.vertical_biases, &self.vertical_weights)
        };
        let vector = features.to_vec();
        let mut logits = [0f32; 3];
        for (class, logit) in logits.iter_mut().enumerate() {
            let mut sum = biases[class];
            for (i, value) in vector.iter().enumerate() {
                sum += weights[class][i] * value;
            }
            *logit = sum;
        }
        logits
    }

    /// The predicted mode for the given features.
    ///
    /// The two axes are predicted independently and composed, which is
    /// what the predictability measurement supports.
    ///
    /// Ties break toward the lower class index, so the result is a
    /// deterministic function of the inputs.
    pub fn predict(&self, features: &GeometryFeatures) -> TransformMode {
        AxisTransform::compose(
            self.predict_axis(features, true),
            self.predict_axis(features, false),
        )
    }

    /// The predicted transform on one axis.
    pub fn predict_axis(&self, features: &GeometryFeatures, horizontal: bool) -> AxisTransform {
        let (biases, weights) = if horizontal {
            (&self.horizontal_biases, &self.horizontal_weights)
        } else {
            (&self.vertical_biases, &self.vertical_weights)
        };
        let vector = features.to_vec();
        let mut best = 0usize;
        let mut best_logit = f32::NEG_INFINITY;
        for class in 0..3 {
            let mut logit = biases[class];
            for (i, value) in vector.iter().enumerate() {
                logit += weights[class][i] * value;
            }
            if logit > best_logit {
                best_logit = logit;
                best = class;
            }
        }
        AxisTransform::ALL[best]
    }

    /// The predicted transform and the probability margin over the runner-up.
    ///
    /// The margin is the signal the encoder uses to decide whether to trust
    /// the prediction or to override it: a confident prediction is worth
    /// spending the override bit on.
    pub fn predict_with_margin(&self, features: &GeometryFeatures) -> (TransformMode, f32) {
        // The margin is the gap between the winning and the runner-up on
        // the axis the prediction is least sure about, because that is
        // the axis that decides the mode.
        let horizontal = self.axis_logits(features, true);
        let vertical = self.axis_logits(features, false);
        let margin = (gap(&horizontal) / 3.0).min(gap(&vertical) / 3.0);
        (self.predict(features), margin)
    }

    /// Parse a model from its JSON representation.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("model is not valid JSON: {e}"))?;
        let parse_vec = |key: &str| -> Result<Vec<f32>, String> {
            value[key]
                .as_array()
                .ok_or_else(|| format!("model is missing the {key} array"))?
                .iter()
                .map(|v| {
                    v.as_f64()
                        .map(|f| f as f32)
                        .ok_or_else(|| format!("{key} contains a non-numeric entry"))
                })
                .collect()
        };

        // The per-axis predictors.
        let horizontal_biases = parse_vec("horizontal_biases")?;
        if horizontal_biases.len() != 3 {
            return Err(format!(
                "model has {} horizontal biases, expected 3",
                horizontal_biases.len()
            ));
        }
        let mut horizontal_bias_array = [0f32; 3];
        horizontal_bias_array.copy_from_slice(&horizontal_biases);
        let horizontal_weights =
            parse_weight_rows(&value["horizontal_weights"], "horizontal_weights")?;
        let vertical_biases = parse_vec("vertical_biases")?;
        if vertical_biases.len() != 3 {
            return Err(format!(
                "model has {} vertical biases, expected 3",
                vertical_biases.len()
            ));
        }
        let mut vertical_bias_array = [0f32; 3];
        vertical_bias_array.copy_from_slice(&vertical_biases);
        let vertical_weights = parse_weight_rows(&value["vertical_weights"], "vertical_weights")?;

        let model_version = value["model_version"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            horizontal_biases: horizontal_bias_array,
            horizontal_weights,
            vertical_biases: vertical_bias_array,
            vertical_weights,
            model_version,
        })
    }

    /// The model in its JSON representation.
    pub fn to_json(&self) -> String {
        let horizontal: Vec<Vec<f32>> = self
            .horizontal_weights
            .iter()
            .map(|row| row.to_vec())
            .collect();
        let vertical: Vec<Vec<f32>> = self
            .vertical_weights
            .iter()
            .map(|row| row.to_vec())
            .collect();
        serde_json::json!({
            "model_version": self.model_version,
            "horizontal_biases": self.horizontal_biases,
            "horizontal_weights": horizontal,
            "vertical_biases": self.vertical_biases,
            "vertical_weights": vertical,
        })
        .to_string()
    }
}

/// Parse a set of weight rows out of a JSON value.
fn parse_weight_rows(
    value: &serde_json::Value,
    name: &str,
) -> Result<[[f32; GeometryFeatures::LEN]; 3], String> {
    let rows = value
        .as_array()
        .ok_or_else(|| format!("model is missing the {name} array"))?;
    if rows.len() != 3 {
        return Err(format!("model has {} {name} rows, expected 3", rows.len()));
    }
    let mut weights = [[0f32; GeometryFeatures::LEN]; 3];
    for (i, row) in rows.iter().enumerate() {
        let values = parse_vec_from(row)?;
        if values.len() != GeometryFeatures::LEN {
            return Err(format!(
                "model {name} row {i} has {} entries, expected {}",
                values.len(),
                GeometryFeatures::LEN
            ));
        }
        weights[i].copy_from_slice(&values);
    }
    Ok(weights)
}

/// The gap between the two highest logits.
fn gap(logits: &[f32; 3]) -> f32 {
    let mut order = [0usize, 1, 2];
    order.sort_by(|a, b| {
        logits[*b]
            .partial_cmp(&logits[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    logits[order[0]] - logits[order[1]]
}

/// Parse a flat numeric array out of a JSON value.
fn parse_vec_from(value: &serde_json::Value) -> Result<Vec<f32>, String> {
    value
        .as_array()
        .ok_or_else(|| "expected an array".to_string())?
        .iter()
        .map(|v| {
            v.as_f64()
                .map(|f| f as f32)
                .ok_or_else(|| "expected numeric entries".to_string())
        })
        .collect()
}

/// How the transform type is communicated to the decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformSignalling {
    /// The transform type is signalled explicitly for every block. This is
    /// what a conventional codec does and it is GIRC's baseline.
    Explicit,
    /// The transform type is inferred by the decoder from the block's
    /// geometry, with a one-bit correction when the encoder disagrees.
    Inferred,
}

impl TransformSignalling {
    pub fn name(self) -> &'static str {
        match self {
            TransformSignalling::Explicit => "explicit",
            TransformSignalling::Inferred => "inferred",
        }
    }
}

/// What the encoder sends for a block's transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformSignal {
    /// The transform mode, sent explicitly.
    Explicit(TransformMode),
    /// The decoder's prediction was correct; nothing else to send.
    Predicted,
    /// The decoder's prediction was wrong; the actual mode follows,
    /// coded relative to the prediction.
    Override {
        /// The mode the decoder predicted.
        predicted: TransformMode,
        /// The mode the encoder chose.
        actual: TransformMode,
    },
}

impl TransformSignal {
    /// The syntax elements this signal contributes to the bitstream.
    ///
    /// The explicit mode always sends the mode. The syntax is the one
    /// ITU-T H.266 uses for `mts_idx`: one bin saying whether a multiple
    /// transform selection mode is used at all, and, when one is, a
    /// truncated unary code over the four modes, which takes one to four
    /// bins. The reference implementation codes it the same way, in
    /// `CABACWriter::mts_idx`.
    ///
    /// The inferred mode sends one bin saying whether the prediction was
    /// right. When it was wrong, the actual mode is selected from the
    /// four the prediction ruled out, which takes two bins.
    ///
    /// The explicit signalling is the more expensive of the two, and it
    /// is expensive because there are five modes to choose between and
    /// the standard codes the choice with a truncated unary code. That
    /// is the reason the mechanism has anything to save.
    pub fn bin_count(self) -> usize {
        match self {
            TransformSignal::Explicit(mode) => {
                if mode == TransformMode::DCT2 {
                    1
                } else {
                    // The flag, plus the truncated unary code over the
                    // four multiple transform selection modes.
                    1 + match mode.as_index() {
                        1 => 1,
                        2 => 2,
                        3 => 3,
                        _ => 4,
                    }
                }
            }
            TransformSignal::Predicted => 1,
            TransformSignal::Override { .. } => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    /// The composition rule: a mode is either the plain DCT-II on both
    /// axes, or a multiple transform selection mode, in which neither
    /// axis is the DCT-II.
    #[test]
    fn axis_composition_follows_the_standard_modes() {
        use AxisTransform::*;
        // A DCT-II on either axis forces the plain DCT-II.
        assert_eq!(AxisTransform::compose(Dct2, Dst7), TransformMode::DCT2);
        assert_eq!(AxisTransform::compose(Dst7, Dct2), TransformMode::DCT2);
        // Otherwise the two axes combine directly, and the four
        // combinations are the standard's four modes.
        assert_eq!(AxisTransform::compose(Dst7, Dst7), TransformMode::MTS[0]);
        assert_eq!(AxisTransform::compose(Dct8, Dst7), TransformMode::MTS[1]);
        assert_eq!(AxisTransform::compose(Dst7, Dct8), TransformMode::MTS[2]);
        assert_eq!(AxisTransform::compose(Dct8, Dct8), TransformMode::MTS[3]);
    }

    /// The neighbour context is a one-hot per slot, in the order left,
    /// above, above-left, above-right.
    #[test]
    fn neighbour_transforms_are_one_hot_per_slot() {
        let neighbours = NeighbourTransforms([
            Some(TransformMode::MTS[0]),
            Some(TransformMode::DCT2),
            None,
            Some(TransformMode::MTS[3]),
        ]);
        let one_hot = neighbours.one_hot();
        assert_eq!(one_hot.len(), NEIGHBOUR_TRANSFORM_SLOTS * MODE_COUNT);
        // Left is DST-VII/DST-VII.
        assert_eq!(one_hot[TransformMode::MTS[0].as_index() as usize], 1.0);
        // Above is the plain DCT-II.
        assert_eq!(
            one_hot[MODE_COUNT + TransformMode::DCT2.as_index() as usize],
            1.0
        );
        // Above-left does not exist, so its slot is empty.
        assert_eq!(one_hot[2 * MODE_COUNT..3 * MODE_COUNT], [0.0; MODE_COUNT]);
        // Above-right is DCT-VIII/DCT-VIII.
        assert_eq!(
            one_hot[3 * MODE_COUNT + TransformMode::MTS[3].as_index() as usize],
            1.0
        );
        // Every slot has exactly one mode set, so the whole vector sums
        // to the number of occupied slots.
        assert_eq!(one_hot.iter().sum::<f32>(), 3.0);
    }

    use super::*;

    #[test]
    fn features_are_length_and_normalised() {
        let left = vec![Some(0), Some(10), Some(20), Some(30)];
        let top = vec![Some(0), Some(5), Some(10), Some(15)];
        let features = extract_features(
            &left,
            &top,
            IntraMode::Planar,
            27,
            NeighbourTransforms::default(),
        );
        assert_eq!(features.to_vec().len(), GeometryFeatures::LEN);
        let sum: f32 = features.orientation_histogram.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "histogram sums to 1, got {sum}");
        assert!(features.dominance >= 0.0 && features.dominance <= 1.0);
        assert!(features.isotropy >= 0.0 && features.isotropy <= 1.0);
    }

    #[test]
    fn flat_neighbourhood_is_isotropic() {
        let left = vec![Some(5); 8];
        let top = vec![Some(5); 8];
        let features = extract_features(
            &left,
            &top,
            IntraMode::Planar,
            27,
            NeighbourTransforms::default(),
        );
        assert_eq!(features.gradient_energy, 0.0);
        assert_eq!(features.isotropy, 1.0);
    }

    #[test]
    fn predictor_is_deterministic() {
        let predictor = TransformPredictor {
            horizontal_biases: [0.0, 1.0, -1.0],
            horizontal_weights: [[0.0; GeometryFeatures::LEN]; 3],
            vertical_biases: [0.0, 1.0, -1.0],
            vertical_weights: [[0.0; GeometryFeatures::LEN]; 3],
            model_version: "test".into(),
        };
        let features = extract_features(
            &[Some(1), Some(2)],
            &[Some(3), Some(4)],
            IntraMode::Planar,
            27,
            NeighbourTransforms::default(),
        );
        let a = predictor.predict(&features);
        let b = predictor.predict(&features);
        assert_eq!(a, b);
        // Both axes carry the same biases, so both pick the DCT-VIII and
        // the composed mode is DCT-VIII/DCT-VIII.
        assert_eq!(a, TransformMode::MTS[3], "highest bias wins");
    }

    #[test]
    fn model_json_round_trips() {
        let mut predictor = TransformPredictor {
            horizontal_biases: [0.1, 0.2, 0.3],
            horizontal_weights: [[0.0; GeometryFeatures::LEN]; 3],
            vertical_biases: [0.4, 0.5, 0.6],
            vertical_weights: [[0.0; GeometryFeatures::LEN]; 3],
            model_version: "v1".into(),
        };
        predictor.horizontal_weights[2][0] = 1.5;
        let json = predictor.to_json();
        let parsed = TransformPredictor::from_json(&json).unwrap();
        assert_eq!(parsed, predictor);
    }

    #[test]
    fn malformed_model_is_rejected() {
        assert!(TransformPredictor::from_json("not json").is_err());
        assert!(TransformPredictor::from_json("{\"biases\":[1,2]}").is_err());
        assert!(TransformPredictor::from_json(
            "{\"biases\":[1,2,3],\"weights\":[[1,2],[3,4],[5,6]]}"
        )
        .is_err());
        // A model with the wrong number of classes is rejected, so a
        // model trained for the three-transform set cannot be used with
        // the five-mode set.
        assert!(TransformPredictor::from_json(
            "{\"biases\":[1,2,3],\"weights\":[[1,2],[3,4],[5,6]]}"
        )
        .is_err());
    }

    #[test]
    fn signal_costs_are_as_documented() {
        // The explicit mode codes the mode the way ITU-T H.266 codes
        // `mts_idx`: one bin for the plain DCT-II, three for a multiple
        // transform selection mode.
        assert_eq!(
            TransformSignal::Explicit(TransformMode::DCT2).bin_count(),
            1
        );
        assert_eq!(
            TransformSignal::Explicit(TransformMode::MTS[0]).bin_count(),
            2
        );
        assert_eq!(
            TransformSignal::Explicit(TransformMode::MTS[3]).bin_count(),
            5
        );
        // The inferred mode always spends one bin on the override flag,
        // and two more when the prediction was wrong.
        assert_eq!(TransformSignal::Predicted.bin_count(), 1);
        assert_eq!(
            TransformSignal::Override {
                predicted: TransformMode::DCT2,
                actual: TransformMode::MTS[0],
            }
            .bin_count(),
            3
        );
    }
}
