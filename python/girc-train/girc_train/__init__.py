"""GIRC predictor training.

Trains the transform-type predictor that GIRC's geometry-inference mode
uses, and exports it in the format `girc-core` loads.

The pipeline is:

1. Run the GIRC encoder in *collection* mode over a set of training
   sequences. For every block, the encoder records the geometry features of
   the block's causal neighbourhood and the transform the rate-distortion
   search actually chose.
2. Fit a multinomial logistic regression over those features, so the model
   predicts the transform the encoder would choose.
3. Export the model as JSON, with the feature layout, the model version and
   a checksum of the training data recorded alongside it.

The exported model is the *protected artefact* in the patent package: the
training method, the training data and the resulting weights are what a
codec team would evaluate.

Usage::

    python3 -m girc_train collect --sequences data/seq/*.y4m \\
        --output bench-out/training.jsonl
    python3 -m girc_train fit --input bench-out/training.jsonl \\
        --output models/girc-predictor.json
    python3 -m girc_train evaluate --model models/girc-predictor.json \\
        --sequences data/seq/*.y4m
"""

from girc_train.pipeline import (
    COLLECTION_QPS,
    FEATURE_NAMES,
    NUM_CLASSES,
    NUM_FEATURES,
    NUM_ORIENTATION_BINS,
    TRANSFORM_NAMES,
    Sample,
    collect_samples,
    evaluate_model,
    fit_model,
    main,
    read_samples,
)

__all__ = [
    "COLLECTION_QPS",
    "FEATURE_NAMES",
    "NUM_CLASSES",
    "NUM_FEATURES",
    "NUM_ORIENTATION_BINS",
    "TRANSFORM_NAMES",
    "Sample",
    "collect_samples",
    "evaluate_model",
    "fit_model",
    "main",
    "read_samples",
]
