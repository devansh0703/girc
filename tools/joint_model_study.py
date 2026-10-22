"""Is a joint model over the five modes better than composing two axes?

The shipped predictor models the two axes separately and composes them
with a hard rule, and the composition loses: the joint accuracy comes
out below the constant it is measured against, even though each axis on
its own is well above it.

The reason is structural. The five modes are not a product space. The
plain DCT-II appears on an axis only in the mode where both axes are
the plain DCT-II, and the four multiple transform selection modes are
correlated, so the product of the two axis marginals is not the joint
distribution. Composing two independent axis models cannot recover it.

This script measures the alternatives on the real records:

  constant    always the plain DCT-II, the majority class.
  hard        the shipped rule, from the two per-axis argmaxes.
  product     the mode with the highest product of axis probabilities.
  joint       one model over the five modes, fitted directly.
  joint+axis  one model over the five modes, given the per-axis
              probabilities as extra features, so it can use the axes
              where they help and ignore them where they do not.

The last one is the interesting one, because it keeps the per-axis
models, which are the more predictable of the two, and lets a joint
model decide how much to trust them.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

MODES = [
    "DCT-II",
    "DST-VII/DST-VII",
    "DCT-VIII/DST-VII",
    "DST-VII/DCT-VIII",
    "DCT-VIII/DCT-VIII",
]

AXIS_OF_MODE = {
    0: (0, 0),
    1: (2, 2),
    2: (1, 2),
    3: (2, 1),
    4: (1, 1),
}
AXIS_OF_MODE_INVERSE = {pair: mode for mode, pair in AXIS_OF_MODE.items()}


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


def axis_targets(labels: np.ndarray):
    horizontal = np.empty_like(labels)
    vertical = np.empty_like(labels)
    for mode, (h, v) in AXIS_OF_MODE.items():
        mask = labels == mode
        horizontal[mask] = h
        vertical[mask] = v
    return horizontal, vertical


def compose_hard(horizontal: np.ndarray, vertical: np.ndarray) -> np.ndarray:
    out = np.zeros(horizontal.shape[0], dtype=np.int64)
    for i, (h, v) in enumerate(zip(horizontal, vertical)):
        if h != 0 and v != 0:
            out[i] = AXIS_OF_MODE_INVERSE[(int(h), int(v))]
    return out


def compose_product(horizontal_proba: np.ndarray, vertical_proba: np.ndarray):
    out = np.zeros(horizontal_proba.shape[0], dtype=np.int64)
    best = np.full(horizontal_proba.shape[0], -1.0)
    for mode, (h, v) in AXIS_OF_MODE.items():
        score = horizontal_proba[:, h] * vertical_proba[:, v]
        better = score > best
        best = np.where(better, score, best)
        out = np.where(better, mode, out)
    return out


def softmax(logits: np.ndarray) -> np.ndarray:
    shifted = logits - logits.max(axis=1, keepdims=True)
    exponentiated = np.exp(shifted)
    return exponentiated / exponentiated.sum(axis=1, keepdims=True)


def main() -> int:
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "bench-out/training-mixed.jsonl")
    model_path = Path(
        sys.argv[2] if len(sys.argv) > 2 else "models/girc-predictor.json"
    )

    from sklearn.linear_model import LogisticRegression
    from sklearn.model_selection import train_test_split

    print(f"reading {path}")
    features, labels = load(path)
    print(f"  {labels.shape[0]} records, {features.shape[1]} features")

    model = json.loads(model_path.read_text())
    horizontal_weights = np.asarray(model["horizontal_weights"], dtype=np.float32)
    vertical_weights = np.asarray(model["vertical_weights"], dtype=np.float32)
    horizontal_biases = np.asarray(model["horizontal_biases"], dtype=np.float32)
    vertical_biases = np.asarray(model["vertical_biases"], dtype=np.float32)

    horizontal, vertical = axis_targets(labels)
    horizontal_proba = softmax(features @ horizontal_weights.T + horizontal_biases)
    vertical_proba = softmax(features @ vertical_weights.T + vertical_biases)

    indices = np.arange(labels.shape[0])
    train_index, test_index = train_test_split(
        indices, test_size=0.2, random_state=20261022, stratify=labels
    )
    train_x, test_x = features[train_index], features[test_index]
    train_y, test_y = labels[train_index], labels[test_index]

    majority = float(
        np.bincount(test_y, minlength=len(MODES)).max()
    ) / test_y.shape[0]
    print(f"\nmajority on the test set: {majority * 100:.2f}%")

    constant = np.zeros(test_y.shape[0], dtype=np.int64)
    hard = compose_hard(
        horizontal_proba.argmax(axis=1)[test_index],
        vertical_proba.argmax(axis=1)[test_index],
    )
    product = compose_product(horizontal_proba[test_index], vertical_proba[test_index])

    results = [("constant (majority)", constant), ("hard (shipped)", hard), ("product", product)]

    # A joint model over the five modes.
    joint = LogisticRegression(max_iter=1000, random_state=20261022).fit(train_x, train_y)
    results.append(("joint logistic", joint.predict(test_x)))

    # A joint model given the per-axis probabilities as well.
    axis_features = np.concatenate(
        [horizontal_proba, vertical_proba], axis=1
    )
    joint_axis = LogisticRegression(max_iter=1000, random_state=20261022).fit(
        np.concatenate([train_x, axis_features[train_index]], axis=1), train_y
    )
    results.append(
        (
            "joint + axis",
            joint_axis.predict(
                np.concatenate([test_x, axis_features[test_index]], axis=1)
            ),
        )
    )

    print("\njoint mode accuracy:")
    for name, composed in results:
        accuracy = float((composed == test_y).mean())
        print(
            f"  {name:20s} {accuracy * 100:6.2f}%   "
            f"lift over majority {(accuracy - majority) * 100:+.2f} points"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
