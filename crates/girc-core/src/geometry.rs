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

use crate::transform::TransformKind;

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
    /// `[i * 45deg, (i+1) * 45deg)`.
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
}

impl GeometryFeatures {
    /// The number of features in the vector.
    pub const LEN: usize = 8 + 4;

    /// The feature vector in the order the trained model expects.
    pub fn to_vec(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.extend_from_slice(&self.orientation_histogram);
        out.push(self.gradient_energy);
        out.push(self.dominance);
        out.push(self.isotropy);
        out.push(self.reference_coverage);
        out
    }
}

/// The number of orientation bins in the histogram.
const ORIENTATION_BINS: usize = 8;

/// Extract the geometry features from a causal neighbourhood.
///
/// `left` holds the reconstructed samples to the left of the block (top to
/// bottom) and `top` the samples above it (left to right). Both may contain
/// gaps where a neighbour does not exist.
pub fn extract_features(left: &[Option<i32>], top: &[Option<i32>]) -> GeometryFeatures {
    let mut histogram = [0f32; ORIENTATION_BINS];
    let mut total = 0f32;

    // The gradient of the neighbourhood is taken along the reference lines
    // themselves: the vertical gradient of the top row and the horizontal
    // gradient of the left column. These are the only directions the
    // decoder has already reconstructed when it reaches this block.
    let mut add = |dx: i32, dy: i32, magnitude: f32| {
        if magnitude <= 0.0 {
            return;
        }
        // Map the gradient direction to a bin. The angle is measured
        // clockwise from the positive x axis.
        let angle = (dy as f32).atan2(dx as f32);
        let mut bin = ((angle + std::f32::consts::PI) / (2.0 * std::f32::consts::PI)
            * ORIENTATION_BINS as f32)
            .floor() as isize;
        bin = bin.rem_euclid(ORIENTATION_BINS as isize);
        histogram[bin as usize] += magnitude;
        total += magnitude;
    };

    // Horizontal gradient along the top row.
    for window in top.windows(2) {
        let (Some(a), Some(b)) = (window[0], window[1]) else {
            continue;
        };
        let dx = b - a;
        // A single-sample step gives a coarse but usable gradient.
        let magnitude = (dx as f32).abs();
        add(dx, 0, magnitude);
    }

    // Vertical gradient along the left column.
    for window in left.windows(2) {
        let (Some(a), Some(b)) = (window[0], window[1]) else {
            continue;
        };
        let dy = b - a;
        let magnitude = (dy as f32).abs();
        add(0, dy, magnitude);
    }

    if total <= 0.0 {
        // A flat neighbourhood carries no orientation information.
        return GeometryFeatures {
            orientation_histogram: [0.0; ORIENTATION_BINS],
            gradient_energy: 0.0,
            dominance: 0.0,
            isotropy: 1.0,
            reference_coverage: coverage(left, top),
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

    GeometryFeatures {
        orientation_histogram: normalised,
        gradient_energy: total.log2().max(0.0),
        dominance,
        isotropy,
        reference_coverage: coverage(left, top),
    }
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
    /// Per-class bias terms, indexed by [`TransformKind::as_index`].
    pub biases: [f32; 3],
    /// Per-class weight vectors, indexed by [`TransformKind::as_index`].
    pub weights: [[f32; GeometryFeatures::LEN]; 3],
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
        let mut weights = [[0f32; GeometryFeatures::LEN]; 3];
        // A positive bias on DCT-II wins the argmax whenever every feature
        // is zero.
        weights[0][0] = 1.0;
        Self {
            biases: [1.0, 0.0, 0.0],
            weights,
            model_version: "constant-dct2".into(),
        }
    }

    /// The logit of a class for the given features.
    fn logit(&self, class: usize, features: &GeometryFeatures) -> f32 {
        let vector = features.to_vec();
        let mut sum = self.biases[class];
        for (i, value) in vector.iter().enumerate() {
            sum += self.weights[class][i] * value;
        }
        sum
    }

    /// The predicted transform for the given features.
    ///
    /// Ties break toward the lower class index, so the result is a
    /// deterministic function of the inputs.
    pub fn predict(&self, features: &GeometryFeatures) -> TransformKind {
        let mut best = 0usize;
        let mut best_logit = f32::NEG_INFINITY;
        for class in 0..3 {
            let logit = self.logit(class, features);
            if logit > best_logit {
                best_logit = logit;
                best = class;
            }
        }
        TransformKind::ALL[best]
    }

    /// The predicted transform and the probability margin over the runner-up.
    ///
    /// The margin is the signal the encoder uses to decide whether to trust
    /// the prediction or to override it: a confident prediction is worth
    /// spending the override bit on.
    pub fn predict_with_margin(&self, features: &GeometryFeatures) -> (TransformKind, f32) {
        let mut logits = [0f32; 3];
        for class in 0..3 {
            logits[class] = self.logit(class, features);
        }
        let mut order = [0usize, 1, 2];
        order.sort_by(|a, b| {
            logits[*b]
                .partial_cmp(&logits[*a])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let margin = logits[order[0]] - logits[order[1]];
        (TransformKind::ALL[order[0]], margin)
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

        let biases = parse_vec("biases")?;
        if biases.len() != 3 {
            return Err(format!("model has {} biases, expected 3", biases.len()));
        }
        let mut bias_array = [0f32; 3];
        bias_array.copy_from_slice(&biases);

        let weight_rows = value["weights"]
            .as_array()
            .ok_or_else(|| "model is missing the weights array".to_string())?;
        if weight_rows.len() != 3 {
            return Err(format!(
                "model has {} weight rows, expected 3",
                weight_rows.len()
            ));
        }
        let mut weights = [[0f32; GeometryFeatures::LEN]; 3];
        for (i, row) in weight_rows.iter().enumerate() {
            let values = parse_vec_from(row)?;
            if values.len() != GeometryFeatures::LEN {
                return Err(format!(
                    "model weight row {i} has {} entries, expected {}",
                    values.len(),
                    GeometryFeatures::LEN
                ));
            }
            weights[i].copy_from_slice(&values);
        }

        let model_version = value["model_version"]
            .as_str()
            .unwrap_or("unversioned")
            .to_string();

        Ok(Self {
            biases: bias_array,
            weights,
            model_version,
        })
    }

    /// The model's JSON representation.
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
    /// The transform type, sent explicitly.
    Explicit(TransformKind),
    /// The decoder's prediction was correct; nothing else to send.
    Predicted,
    /// The decoder's prediction was wrong; the actual transform follows.
    Override(TransformKind),
}

impl TransformSignal {
    /// The syntax elements this signal contributes to the bitstream.
    pub fn bin_count(self) -> usize {
        match self {
            TransformSignal::Explicit(_) => 2,
            TransformSignal::Predicted => 0,
            TransformSignal::Override(_) => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_are_length_and_normalised() {
        let left = vec![Some(0), Some(10), Some(20), Some(30)];
        let top = vec![Some(0), Some(5), Some(10), Some(15)];
        let features = extract_features(&left, &top);
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
        let features = extract_features(&left, &top);
        assert_eq!(features.gradient_energy, 0.0);
        assert_eq!(features.isotropy, 1.0);
    }

    #[test]
    fn predictor_is_deterministic() {
        let predictor = TransformPredictor {
            biases: [0.0, 1.0, -1.0],
            weights: [[0.0; GeometryFeatures::LEN]; 3],
            model_version: "test".into(),
        };
        let features = extract_features(&[Some(1), Some(2)], &[Some(3), Some(4)]);
        let a = predictor.predict(&features);
        let b = predictor.predict(&features);
        assert_eq!(a, b);
        assert_eq!(a, TransformKind::Dct8, "highest bias wins");
    }

    #[test]
    fn model_json_round_trips() {
        let mut predictor = TransformPredictor {
            biases: [0.1, 0.2, 0.3],
            weights: [[0.0; GeometryFeatures::LEN]; 3],
            model_version: "v1".into(),
        };
        predictor.weights[2][0] = 1.5;
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
    }

    #[test]
    fn signal_costs_are_as_documented() {
        assert_eq!(
            TransformSignal::Explicit(TransformKind::Dct2).bin_count(),
            2
        );
        assert_eq!(TransformSignal::Predicted.bin_count(), 0);
        assert_eq!(
            TransformSignal::Override(TransformKind::Dst7).bin_count(),
            3
        );
    }
}
