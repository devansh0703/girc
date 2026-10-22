//! The shipped predictor model has to load.
//!
//! The model is an artifact that the training pipeline writes and the
//! codec reads, and the two are in separate languages, so nothing in
//! either language checks that they agree. A model whose recorded
//! feature count did not match the features the codec produces was
//! shipped once, and read with every weight at the wrong position.
//! These tests are the check that was missing.

use std::path::PathBuf;

use girc_core::geometry::{GeometryFeatures, TransformPredictor};
use girc_core::transform::TransformMode;

/// The path to the shipped model, relative to the crate.
fn model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models/girc-predictor.json")
}

/// The shipped model, if it is there.
fn shipped() -> Option<TransformPredictor> {
    let path = model_path();
    let text = std::fs::read_to_string(path).ok()?;
    Some(TransformPredictor::from_json(&text).expect("the shipped model must load"))
}

#[test]
fn the_shipped_model_loads() {
    let Some(predictor) = shipped() else {
        panic!("the shipped model is missing at {}", model_path().display());
    };
    // The model records the feature count it was trained on, and the
    // loader checks it against the features the codec produces, so a
    // model trained on an older layout is refused here rather than
    // read with every weight at the wrong position.
    assert_eq!(
        predictor.weights[0].len(),
        GeometryFeatures::LEN,
        "the shipped model's weights do not match the codec's features"
    );
}

#[test]
fn the_shipped_model_predicts_a_real_mode() {
    let Some(predictor) = shipped() else {
        panic!("the shipped model is missing at {}", model_path().display());
    };
    // A flat neighbourhood, which is the simplest thing a predictor
    // can be asked about. The prediction has to be one of the five
    // modes and it has to be the same every time.
    let features = GeometryFeatures {
        orientation_histogram: [0.0; 8],
        gradient_energy: 0.0,
        dominance: 0.0,
        isotropy: 1.0,
        reference_coverage: 1.0,
        horizontal_energy: 0.0,
        vertical_energy: 0.0,
        axis_correlation: 0.0,
        quantization_parameter: 27.0 / 51.0,
        intra_mode: [1.0; 10],
        neighbour_transforms: [0.0; 20],
        is_inter: 0.0,
        motion: [0.0; 6],
        prediction: [0.0; 11],
    };
    let mode = predictor.predict(&features);
    assert!(
        TransformMode::ALL.contains(&mode),
        "the predictor produced a mode that is not in the set"
    );
    assert_eq!(
        mode,
        predictor.predict(&features),
        "the prediction is not stable"
    );
}

#[test]
fn the_shipped_model_is_not_the_constant() {
    let Some(predictor) = shipped() else {
        panic!("the shipped model is missing at {}", model_path().display());
    };
    // The constant predictor always answers the plain DCT-II, and
    // weighs every feature at zero. The shipped model has to be a
    // learned one, or the whole training pipeline is decoration.
    let constant = TransformPredictor::constant_dct2();
    assert_ne!(
        predictor.weights, constant.weights,
        "the shipped model is the constant predictor"
    );
    assert_ne!(
        predictor.model_version, constant.model_version,
        "the shipped model is the constant predictor"
    );
}
