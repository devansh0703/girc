"""Compare the composition rules on the real records, with the shipped weights.

The shipped predictor models the two axes separately and composes them
with a hard rule: if either axis predicts the plain DCT-II, the mode is
the plain DCT-II. The question this script answers is whether that
rule is the best way to turn two axis predictions into one mode, and
whether the learned predictor beats the constant it is measured
against.

The rules compared:

  constant    always the plain DCT-II, which is the majority class.
              This is the baseline: a learned predictor that cannot beat
              it is decoration.
  hard        the shipped rule: a DCT-II on either axis forces the
              plain DCT-II, otherwise the two axes combine.
  product     the axes are scored independently and the mode with the
              highest product of axis probabilities is chosen. The
              joint distribution is the product of the two axis
              distributions, so this is the maximum-likelihood mode.
  joint       a single model over the five modes, which is the ceiling
              for the features.

The shipped model's own weights are used, so what is measured is what
ships.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

# The joint modes, in the canonical order the codec uses.
MODES = [
    "DCT-II",
    "DST-VII/DST-VII",
    "DCT-VIII/DST-VII",
    "DST-VII/DCT-VIII",
    "DCT-VIII/DCT-VIII",
]

# The axis transform each mode uses on each axis.
#   0 DCT-II, 1 DCT-VIII, 2 DST-VII
AXIS_OF_MODE = {
    0: (0, 0),
    1: (2, 2),
    2: (1, 2),
    3: (2, 1),
    4: (1, 1),
}

# The mode a pair of axis transforms implies.
AXIS_OF_MODE_INVERSE = {pair: mode for mode, pair in AXIS_OF_MODE.items()}


def load(path: Path, limit: int | None = None) -> tuple[np.ndarray, np.ndarray]:
    features: list[list[float]] = []
    labels: list[int] = []
    with path.open() as handle:
        for line in handle:
            record = json.loads(line)
            features.append(record["features"])
            # The collector writes the mode's index, not its name.
            labels.append(int(record["transform"]))
            if limit and len(labels) >= limit:
                break
    return np.asarray(features, dtype=np.float32), np.asarray(labels, dtype=np.int64)


def axis_targets(labels: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    horizontal = np.empty_like(labels)
    vertical = np.empty_like(labels)
    for mode, (h, v) in AXIS_OF_MODE.items():
        mask = labels == mode
        horizontal[mask] = h
        vertical[mask] = v
    return horizontal, vertical


def compose_hard(horizontal: np.ndarray, vertical: np.ndarray) -> np.ndarray:
    """The shipped rule: a DCT-II on either axis forces the plain DCT-II."""
    out = np.zeros(horizontal.shape[0], dtype=np.int64)
    for i, (h, v) in enumerate(zip(horizontal, vertical)):
        if h != 0 and v != 0:
            out[i] = AXIS_OF_MODE_INVERSE[(int(h), int(v))]
    return out


def compose_product(
    horizontal_proba: np.ndarray, vertical_proba: np.ndarray
) -> np.ndarray:
    """The mode with the highest product of the two axis probabilities."""
    out = np.zeros(horizontal_proba.shape[0], dtype=np.int64)
    best_score = np.full(horizontal_proba.shape[0], -1.0)
    for mode, (h, v) in AXIS_OF_MODE.items():
        score = horizontal_proba[:, h] * vertical_proba[:, v]
        better = score > best_score
        best_score = np.where(better, score, best_score)
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
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else 0

    print(f"reading {path}")
    features, labels = load(path, limit or None)
    print(f"  {labels.shape[0]} records, {features.shape[1]} features")

    model = json.loads(model_path.read_text())
    horizontal_weights = np.asarray(model["horizontal_weights"], dtype=np.float32)
    vertical_weights = np.asarray(model["vertical_weights"], dtype=np.float32)
    horizontal_biases = np.asarray(model["horizontal_biases"], dtype=np.float32)
    vertical_biases = np.asarray(model["vertical_biases"], dtype=np.float32)
    if horizontal_weights.shape[1] != features.shape[1]:
        print(
            f"  the model wants {horizontal_weights.shape[1]} features, "
            f"the records have {features.shape[1]}"
        )
        return 1

    horizontal, vertical = axis_targets(labels)

    horizontal_logits = features @ horizontal_weights.T + horizontal_biases
    vertical_logits = features @ vertical_weights.T + vertical_biases
    horizontal_proba = softmax(horizontal_logits)
    vertical_proba = softmax(vertical_logits)

    horizontal_prediction = horizontal_proba.argmax(axis=1)
    vertical_prediction = vertical_proba.argmax(axis=1)

    majority = float(np.bincount(labels, minlength=len(MODES)).max()) / labels.shape[0]
    horizontal_majority = (
        float(np.bincount(horizontal, minlength=3).max()) / horizontal.shape[0]
    )
    vertical_majority = (
        float(np.bincount(vertical, minlength=3).max()) / vertical.shape[0]
    )

    print("\nclass distribution:")
    for index, name in enumerate(MODES):
        share = float((labels == index).mean())
        print(f"  {name:20s} {share * 100:5.2f}%")

    print("\nper-axis accuracy of the shipped model:")
    print(
        f"  horizontal {float((horizontal_prediction == horizontal).mean()) * 100:.2f}%"
        f"   majority {horizontal_majority * 100:.2f}%"
    )
    print(
        f"  vertical   {float((vertical_prediction == vertical).mean()) * 100:.2f}%"
        f"   majority {vertical_majority * 100:.2f}%"
    )

    constant = np.zeros(labels.shape[0], dtype=np.int64)
    hard = compose_hard(horizontal_prediction, vertical_prediction)
    product = compose_product(horizontal_proba, vertical_proba)

    print("\njoint mode accuracy:")
    for name, composed in (
        ("constant (majority)", constant),
        ("hard (shipped)", hard),
        ("product", product),
    ):
        accuracy = float((composed == labels).mean())
        print(
            f"  {name:20s} {accuracy * 100:6.2f}%   "
            f"lift over majority {(accuracy - majority) * 100:+.2f} points"
        )

    # The ceiling: a single model over the five modes, on the same
    # features. It is the best any model over these features can do.
    print("\nfitting the joint ceiling on the same features")
    from sklearn.ensemble import HistGradientBoostingClassifier
    from sklearn.model_selection import train_test_split

    indices = np.arange(labels.shape[0])
    train_index, test_index = train_test_split(
        indices, test_size=0.2, random_state=20261022, stratify=labels
    )
    ceiling_model = HistGradientBoostingClassifier(
        max_iter=300, learning_rate=0.1, random_state=20261022
    )
    ceiling_model.fit(features[train_index], labels[train_index])
    ceiling = float(
        (ceiling_model.predict(features[test_index]) == labels[test_index]).mean()
    )
    test_majority = float(
        np.bincount(labels[test_index], minlength=len(MODES)).max()
    ) / len(test_index)
    print(f"  joint ceiling        {ceiling * 100:6.2f}%")
    print(f"  majority on the test {test_majority * 100:6.2f}%")
    print(f"  ceiling lift         {(ceiling - test_majority) * 100:+.2f} points")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
