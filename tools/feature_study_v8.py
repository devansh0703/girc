"""Measure what the decoder-computable features can actually predict.

The predictor has to predict one of five transform modes from the causal
neighbourhood, and the rate at which it is right decides how often the
encoder has to spend a correction bit. The measurement that matters is
the joint accuracy against the majority class.

The earlier measurement was taken with features that carried the intra
mode of an inter block, which the encoder knows and the decoder does
not. That inflated the accuracy and produced a model a decoder could
not use. This study is taken on the corrected features, and it adds the
question that the corrected numbers raise: is there more signal
available from the neighbourhood, or is the transform mode close to
unpredictable from what a decoder has?

Four feature sets are compared:

  all        every feature.
  no-pred    without the prediction's own orientation, which is what
             the model had before that feature was added.
  pred-only  only the prediction's own orientation.
  no-motion  without the motion context, to see how much of the signal
             is in the motion rather than in the texture.

The last one matters because the motion context is the one feature
group that only an inter block has, and the corrected model has to
work for both kinds of block.
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


def load(path: Path) -> tuple[np.ndarray, np.ndarray]:
    features: list[list[float]] = []
    labels: list[int] = []
    with path.open() as handle:
        for line in handle:
            record = json.loads(line)
            features.append(record["features"])
            labels.append(record["transform"])
    return np.asarray(features, dtype=np.float32), np.asarray(labels)


# The feature names come from the exported model, so the study always
# indexes the same layout the codec produces.
MODEL_NAMES: list[str] = []


def subsets(names: list[str]) -> dict[str, list[int]]:
    def pick(predicate) -> list[int]:
        return [i for i, name in enumerate(names) if predicate(name)]

    return {
        "all": list(range(len(names))),
        "no-pred": pick(lambda n: not n.startswith("prediction_")),
        "pred-only": pick(lambda n: n.startswith("prediction_")),
        "no-motion": pick(lambda n: not n.startswith("motion_")),
        "no-neighbours": pick(lambda n: not n.startswith("neighbour_")),
        "no-intra-mode": pick(lambda n: not n.startswith("intra_mode_")),
    }


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: feature_study_v8.py <records.jsonl> [model.json]", file=sys.stderr)
        return 2
    records = Path(argv[1])
    model_path = Path(argv[2]) if len(argv) > 2 else None
    if model_path is not None:
        global MODEL_NAMES
        MODEL_NAMES = json.loads(model_path.read_text())["feature_names"]

    features, labels = load(records)
    print(f"{len(labels)} records")

    train_x, test_x, train_y, test_y = train_test_split(
        features, labels, test_size=0.2, random_state=20261022, stratify=labels
    )
    counts = np.bincount(train_y, minlength=len(MODES))
    majority = float(counts.max()) / float(counts.sum())
    print(f"the majority class is {MODES[int(counts.argmax())]} at {majority:.4f}")

    for name, indices in subsets(MODEL_NAMES).items():
        if not indices:
            continue
        index = np.asarray(indices)
        model = LogisticRegression(max_iter=400, C=1.0, n_jobs=-1)
        model.fit(train_x[:, index], train_y)
        accuracy = model.score(test_x[:, index], test_y)
        print(
            f"{name:16s} {len(indices):3d} features  "
            f"logistic {accuracy:.4f}  lift {accuracy - majority:+.4f}"
        )

    # The ceiling over the full feature set, with a model that can use
    # non-linear structure.
    model = HistGradientBoostingClassifier(max_iter=200, random_state=20261022)
    model.fit(train_x, train_y)
    ceiling = model.score(test_x, test_y)
    print(f"{'ceiling':16s} {features.shape[1]:3d} features  boosted   {ceiling:.4f}  "
          f"lift {ceiling - majority:+.4f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
