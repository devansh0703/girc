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

use crate::inter::MotionVector;
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
    /// Whether the block is predicted from the previous frame.
    ///
    /// An inter block has no intra prediction mode, so the mode feature
    /// is empty for one, and the predictor has to know that.
    pub is_inter: f32,
    /// The motion context: the block's own motion vector and the motion
    /// vectors of the blocks to its left and above.
    ///
    /// Each vector contributes its two components, scaled so a large
    /// vector does not dominate the feature.
    ///
    /// A motion-compensated residual carries the motion vector's
    /// structure, and the decoder knows the vector because it decoded
    /// it. This is the context that makes the prediction of an inter
    /// block's transform mode better than the prediction of an intra
    /// block's.
    pub motion: [f32; MOTION_FEATURE_COUNT],
    /// The orientation structure of the block's own prediction.
    ///
    /// The prediction is the block's content minus its residual, and
    /// the encoder chooses the transform that best compacts the
    /// residual. A residual inherits the directional structure of the
    /// prediction it is subtracted from, so the prediction's own
    /// gradient orientation is the most direct evidence of which
    /// transform the encoder will want.
    ///
    /// The decoder has the prediction, because the prediction mode and
    /// the motion vector are both decoded before the transform type.
    /// For an inter block the prediction comes from the previous frame
    /// at a motion-compensated position, so this carries information
    /// the causal window of the current frame does not have at all.
    ///
    /// The features are the same orientation histogram the causal
    /// window produces, over the prediction block: the eight bins, the
    /// two axis energies and their correlation.
    pub prediction: [f32; PREDICTION_FEATURE_COUNT],
}

impl GeometryFeatures {
    /// The number of features in the vector.
    pub const LEN: usize = 8
        + 4
        + 3
        + 1
        + IntraMode::COUNT
        + NEIGHBOUR_TRANSFORM_SLOTS * MODE_COUNT
        + 1
        + MOTION_FEATURE_COUNT
        + PREDICTION_FEATURE_COUNT;

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
        out.push(self.is_inter);
        out.extend_from_slice(&self.motion);
        out.extend_from_slice(&self.prediction);
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

/// The per-block context the features are extracted in.
///
/// The features of a block depend on what the decoder already knows
/// about it: the block's intra mode, the quantization parameter, the
/// transform modes of its neighbours and its motion context. They are
/// bundled so the extraction functions take one context rather than
/// four separate arguments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockContext {
    /// The block's intra prediction mode.
    pub mode: IntraMode,
    /// The quantization parameter.
    pub qp: i32,
    /// The transform modes of the blocks to the left, above, above-left
    /// and above-right.
    pub neighbours: NeighbourTransforms,
    /// The block's motion context.
    pub motion: MotionContext,
    /// The orientation structure of the block's own prediction.
    pub prediction: [f32; PREDICTION_FEATURE_COUNT],
}

impl BlockContext {
    /// The context of an intra block.
    pub fn intra(mode: IntraMode, qp: i32) -> Self {
        Self {
            mode,
            qp,
            neighbours: NeighbourTransforms::default(),
            motion: MotionContext::intra(),
            prediction: [0.0; PREDICTION_FEATURE_COUNT],
        }
    }
}

/// The number of motion features.
///
/// The block's own motion vector and the motion vectors of the blocks
/// to its left and above, two components each.
pub const MOTION_FEATURE_COUNT: usize = 6;

/// The number of features the prediction's own orientation contributes.
///
/// The same shape as the causal window's: the eight orientation bins,
/// the horizontal and vertical energies, and the correlation between
/// them.
pub const PREDICTION_FEATURE_COUNT: usize = 8 + 2 + 1;

/// The motion context of a block.
///
/// The block's own motion vector and the motion vectors of the blocks to
/// its left and above, plus the flag that says whether the block is
/// predicted from the previous frame at all.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionContext {
    /// Whether the block is predicted from the previous frame.
    pub is_inter: f32,
    /// The features: the block's vector, then the left and above ones.
    pub features: [f32; MOTION_FEATURE_COUNT],
}

impl Default for MotionContext {
    fn default() -> Self {
        Self {
            is_inter: 0.0,
            features: [0.0; MOTION_FEATURE_COUNT],
        }
    }
}

impl MotionContext {
    /// The context of a block that is not predicted from the previous
    /// frame.
    pub fn intra() -> Self {
        Self::default()
    }

    /// The context of a block that is, with its own motion vector and
    /// the motion vectors of the blocks to its left and above.
    ///
    /// The components are scaled by a sixteenth and clipped, so a large
    /// vector is a large number rather than an unbounded one.
    pub fn inter(
        own: MotionVector,
        left: Option<MotionVector>,
        above: Option<MotionVector>,
    ) -> Self {
        let scale = |value: i32| (value as f32 / 16.0).clamp(-4.0, 4.0);
        let mut features = [0.0f32; MOTION_FEATURE_COUNT];
        features[0] = scale(own.dx);
        features[1] = scale(own.dy);
        if let Some(left) = left {
            features[2] = scale(left.dx);
            features[3] = scale(left.dy);
        }
        if let Some(above) = above {
            features[4] = scale(above.dx);
            features[5] = scale(above.dy);
        }
        Self {
            is_inter: 1.0,
            features,
        }
    }
}

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
    context: BlockContext,
) -> GeometryFeatures {
    let BlockContext {
        mode,
        qp,
        neighbours,
        motion,
        prediction,
    } = context;
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
            is_inter: motion.is_inter,
            motion: motion.features,
            prediction,
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
        is_inter: motion.is_inter,
        motion: motion.features,
        prediction,
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
    context: BlockContext,
) -> GeometryFeatures {
    let BlockContext {
        mode,
        qp,
        neighbours,
        motion,
        prediction,
    } = context;
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
            is_inter: motion.is_inter,
            motion: motion.features,
            prediction,
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
            is_inter: motion.is_inter,
            motion: motion.features,
            prediction,
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
        is_inter: motion.is_inter,
        motion: motion.features,
        prediction,
    }
}

/// The orientation structure of a prediction block.
///
/// The block is a square of samples, and the features are the same
/// orientation histogram the causal window produces: the eight bins of
/// the gradient's direction, the energy along each axis, and the
/// correlation between them.
///
/// The gradient is taken with central differences, the same as the
/// window's, so the two sets of features are comparable.
pub fn prediction_features(samples: &[i32], size: usize) -> [f32; PREDICTION_FEATURE_COUNT] {
    let mut out = [0f32; PREDICTION_FEATURE_COUNT];
    if size < 3 {
        return out;
    }
    let mut histogram = [0f32; ORIENTATION_BINS];
    let mut total = 0f32;
    let mut horizontal = 0f32;
    let mut vertical = 0f32;
    let mut cross = 0f32;

    for y in 1..size - 1 {
        for x in 1..size - 1 {
            let at = |xx: usize, yy: usize| samples[yy * size + xx] as f32;
            let dx = at(x + 1, y) - at(x - 1, y);
            let dy = at(x, y + 1) - at(x, y - 1);
            let magnitude = (dx * dx + dy * dy).sqrt();
            accumulate(&mut histogram, &mut total, dx, dy, magnitude);
            horizontal += dx * dx;
            vertical += dy * dy;
            cross += dx * dy;
        }
    }

    if total <= 0.0 {
        // A flat prediction carries no orientation information.
        return out;
    }

    for i in 0..ORIENTATION_BINS {
        out[i] = histogram[i] / total;
    }
    out[8] = horizontal.max(1.0).log2();
    out[9] = vertical.max(1.0).log2();
    out[10] = if horizontal > 0.0 && vertical > 0.0 {
        (cross / (horizontal * vertical).sqrt()).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    out
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
    /// The bias term of each of the five modes.
    ///
    /// The model is over the joint mode, not over the two axes
    /// separately, and that is a measured choice. The five modes are
    /// not a product space: the plain DCT-II appears on an axis only in
    /// the mode where both axes are the plain DCT-II, and the four
    /// multiple transform selection modes are correlated. Composing two
    /// independent per-axis models therefore cannot recover the joint
    /// distribution, and the measurement confirmed it: each axis on its
    /// own was well above the majority class, and the composed mode was
    /// below it. One model over the five modes is both simpler and
    /// better.
    pub biases: [f32; MODE_COUNT],
    /// The weight vector of each of the five modes.
    pub weights: [[f32; GeometryFeatures::LEN]; MODE_COUNT],
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
        // A positive bias on the plain DCT-II wins the argmax whenever
        // every feature is zero.
        let mut weights = [[0f32; GeometryFeatures::LEN]; MODE_COUNT];
        weights[0][0] = 1.0;
        Self {
            biases: [1.0, 0.0, 0.0, 0.0, 0.0],
            weights,
            model_version: "constant-dct2".into(),
        }
    }

    /// The three logits of one axis.
    /// The logit of each of the five modes.
    fn mode_logits(&self, features: &GeometryFeatures) -> [f32; MODE_COUNT] {
        let vector = features.to_vec();
        let mut logits = [0f32; MODE_COUNT];
        for (class, logit) in logits.iter_mut().enumerate() {
            let mut sum = self.biases[class];
            for (i, value) in vector.iter().enumerate() {
                sum += self.weights[class][i] * value;
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
        let logits = self.mode_logits(features);
        let mut best = 0usize;
        let mut best_logit = f32::NEG_INFINITY;
        for (class, logit) in logits.iter().enumerate() {
            if *logit > best_logit {
                best_logit = *logit;
                best = class;
            }
        }
        TransformMode::ALL[best]
    }

    /// The predicted transform on one axis.
    /// The predicted transform and the probability margin over the runner-up.
    ///
    /// The margin is the signal the encoder uses to decide whether to trust
    /// the prediction or to override it: a confident prediction is worth
    /// spending the override bit on.
    pub fn predict_with_margin(&self, features: &GeometryFeatures) -> (TransformMode, f32) {
        // The margin is the gap between the winning mode and the
        // runner-up, which is how sure the prediction is.
        let logits = self.mode_logits(features);
        (self.predict(features), gap(&logits))
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

        // The joint predictor over the five modes.
        let biases = parse_vec("biases")?;
        if biases.len() != MODE_COUNT {
            return Err(format!(
                "model has {} biases, expected {MODE_COUNT}",
                biases.len()
            ));
        }
        let mut bias_array = [0f32; MODE_COUNT];
        bias_array.copy_from_slice(&biases);
        let weights = parse_weight_rows(&value["weights"], "weights")?;

        // The model records the feature count it was trained on, and
        // it has to match the features this code produces. A model
        // trained on an older feature layout would otherwise be read
        // with every weight at the wrong position, and the predictor
        // would quietly predict nonsense.
        if let Some(count) = value["feature_count"].as_u64() {
            if count as usize != GeometryFeatures::LEN {
                return Err(format!(
                    "model was trained on {count} features, this codec produces {}",
                    GeometryFeatures::LEN
                ));
            }
        }

        let model_version = value["model_version"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            biases: bias_array,
            weights,
            model_version,
        })
    }

    /// The model in its JSON representation.
    pub fn to_json(&self) -> String {
        let weights: Vec<Vec<f32>> = self.weights.iter().map(|row| row.to_vec()).collect();
        serde_json::json!({
            "model_version": self.model_version,
            "feature_count": GeometryFeatures::LEN,
            "biases": self.biases,
            "weights": weights,
        })
        .to_string()
    }
}

/// Parse a set of weight rows out of a JSON value.
fn parse_weight_rows(
    value: &serde_json::Value,
    name: &str,
) -> Result<[[f32; GeometryFeatures::LEN]; MODE_COUNT], String> {
    let rows = value
        .as_array()
        .ok_or_else(|| format!("model is missing the {name} array"))?;
    if rows.len() != MODE_COUNT {
        return Err(format!(
            "model has {} {name} rows, expected {MODE_COUNT}",
            rows.len()
        ));
    }
    let mut weights = [[0f32; GeometryFeatures::LEN]; MODE_COUNT];
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
fn gap(logits: &[f32; MODE_COUNT]) -> f32 {
    let mut order = [0usize, 1, 2, 3, 4];
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
        let features = extract_features(&left, &top, BlockContext::intra(IntraMode::Planar, 27));
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
        let features = extract_features(&left, &top, BlockContext::intra(IntraMode::Planar, 27));
        assert_eq!(features.gradient_energy, 0.0);
        assert_eq!(features.isotropy, 1.0);
    }

    #[test]
    fn predictor_is_deterministic() {
        let predictor = TransformPredictor {
            biases: [0.0, 0.0, 1.0, -1.0, -2.0],
            weights: [[0.0; GeometryFeatures::LEN]; MODE_COUNT],
            model_version: "test".into(),
        };
        let features = extract_features(
            &[Some(1), Some(2)],
            &[Some(3), Some(4)],
            BlockContext::intra(IntraMode::Planar, 27),
        );
        let a = predictor.predict(&features);
        let b = predictor.predict(&features);
        assert_eq!(a, b);
        // The highest bias wins, so the DCT-VIII/DST-VII mode is chosen.
        assert_eq!(a, TransformMode::MTS[1], "highest bias wins");
    }

    #[test]
    fn model_json_round_trips() {
        let mut predictor = TransformPredictor {
            biases: [0.1, 0.2, 0.3, 0.4, 0.5],
            weights: [[0.0; GeometryFeatures::LEN]; MODE_COUNT],
            model_version: "v1".into(),
        };
        predictor.weights[2][0] = 1.5;
        let json = predictor.to_json();
        let parsed = TransformPredictor::from_json(&json).unwrap();
        assert_eq!(parsed, predictor);
    }

    #[test]
    fn a_model_trained_on_another_feature_layout_is_rejected() {
        // A model that says it was trained on a different number of
        // features is rejected, rather than read with every weight at
        // the wrong position.
        let model = TransformPredictor::constant_dct2();
        let json = model.to_json();
        let wrong = json.replace(
            &format!("\"feature_count\":{}", GeometryFeatures::LEN),
            "\"feature_count\":46",
        );
        let error = TransformPredictor::from_json(&wrong).unwrap_err();
        assert!(error.contains("features"), "{error}");
    }

    #[test]
    fn malformed_model_is_rejected() {
        assert!(TransformPredictor::from_json("not json").is_err());
        assert!(TransformPredictor::from_json("{\"biases\":[1,2]}").is_err());
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
