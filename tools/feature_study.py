"""Measure the joint model on the records, with and without the prediction features.

The predictor has to predict one of five transform modes from the causal
neighbourhood, and the rate at which it is right is what decides how
often the encoder has to spend the correction bit. The measurement
that matters is therefore the joint accuracy against the majority
class, and the ceiling over the feature set.

The features are compared in three sets:

  all         every feature, including the prediction's own orientation.
  no-pred     the features without the prediction's orientation, which
              is what the model had before.
  pred-only   only the prediction's own orientation, to see how much
              of the signal is in it.

The prediction's orientation is the block's own prediction plane, which
both the encoder and the decoder have before the transform type is
needed. The encoder chooses the transform that best compacts the
residual, and the residual inherits the prediction's directional
structure, so the prediction's orientation is the most direct evidence
of the transform the encoder will want. For an inter block the
prediction comes from the previous frame at a motion-compensated
position, so it carries information the causal window of the current
frame does not have at all.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np
from sklearn.ensemble import HistGradientBoostingClassifier
from sklearn.linear_model import LogisticRegression
from sklearn.model_selection import train_test_split

MODES = [
    "DCT-II",
    "DST-VII/DST-VII",
    "DCT-VIII/DST-VII",
    "DST-VII/DCT-VIII",
    "DCT-VIII/DCT-VIII",
]

# The prediction's features are the last eleven of the vector: the
# eight orientation bins, the two axis energies and the correlation.
PREDICTION_FEATURE_COUNT = 11


def load(path: Path, limit: int | None = None):
    features: list[list[float]] = []
    labels: list[int] = []
    with path.open() as handle:
        for line in handle:
            record = json.loads(line)
            features.append(record["features"])
            labels.append(int(record["transform"]))
            if limit and len(labels) >= limit:
                break
    return np.asarray(features, dtype=np.float32), np.asarray(labels, dtype=np.int64)


def main() -> int:
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "bench-out/train-pred/mixed.jsonl")
    limit = int(sys.argv[2]) if len(sys.argv) > 2 else 300_000

    print(f"reading {path}")
    features, labels = load(path, limit or None)
    total = features.shape[1]
    print(f"  {labels.shape[0]} records, {total} features")

    prediction_start = total - PREDICTION_FEATURE_COUNT
    without_prediction = features[:, :prediction_start]
    only_prediction = features[:, prediction_start:]

    indices = np.arange(labels.shape[0])
    train_index, test_index = train_test_split(
        indices, test_size=0.2, random_state=20261022, stratify=labels
    )
    majority = float(
        np.bincount(labels[test_index], minlength=len(MODES)).max()
    ) / len(test_index)
    print(f"\nmajority on the test set: {majority * 100:.2f}%")

    print("\nclass distribution:")
    for index, name in enumerate(MODES):
        share = float((labels == index).mean())
        print(f"  {name:20s} {share * 100:5.2f}%")

    for name, matrix in (
        ("all features", features),
        ("without prediction", without_prediction),
        ("prediction only", only_prediction),
    ):
        train_x, test_x = matrix[train_index], matrix[test_index]
        train_y, test_y = labels[train_index], labels[test_index]

        model = LogisticRegression(max_iter=1000, random_state=20261022).fit(
            train_x, train_y
        )
        linear = float((model.predict(test_x) == test_y).mean())

        ceiling_model = HistGradientBoostingClassifier(
            max_iter=300, learning_rate=0.1, random_state=20261022
        )
        ceiling_model.fit(train_x, train_y)
        ceiling = float((ceiling_model.predict(test_x) == test_y).mean())

        print(f"\n{name} ({matrix.shape[1]} features)")
        print(f"  logistic   {linear * 100:6.2f}%   lift {(linear - majority) * 100:+.2f}")
        print(f"  ceiling    {ceiling * 100:6.2f}%   lift {(ceiling - majority) * 100:+.2f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
