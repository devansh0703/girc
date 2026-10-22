"""Measure the mechanism's cost directly, rather than the prediction's accuracy.

The mechanism does not trade in accuracy. It trades the signalling it
saves against the distortion it pays when the prediction is wrong and
the correction is sent. A predictor that is right 44% of the time but
whose mistakes are cheap to correct can beat one that is right 50% of
the time.

So the quantity that matters is the net, and it can be measured
directly from the records, without running the whole codec:

  for each block, the encoder has a cost for the mode it chose and a
  cost for the mode the predictor would have chosen. The difference,
  divided by lambda, is the rate the encoder gives up by taking the
  prediction. The mechanism takes the prediction when that difference
  is within the margin.

This script computes the net rate saving of a predictor over the
explicit baseline, per margin, and compares predictors:

  constant    always the plain DCT-II, which is the majority class.
  joint       the shipped model.
  oracle      the best possible predictor, which always predicts the
              mode the encoder chose. This is the upper bound the
              mechanism could reach, and it is what says whether the
              features or the margin is the binding constraint.

The oracle needs the encoder's cost for every mode, which the codec
does not record. So this script measures the *signalling* side and the
*mismatch* side, and the codec's own benchmark measures the net. The
point here is to compare predictors on the same footing, using the
mismatch rate and the correction cost, so the comparison is
apples-to-apples.
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

# The bins the explicit signalling costs, in the syntax the codec uses:
# one bin for the plain DCT-II, and a truncated unary code over the
# four multiple transform selection modes.
EXPLICIT_BINS = [1, 2, 3, 4, 4]

# The bins the inferred signalling costs: one when the prediction is
# right, and three when it is wrong, because the correction selects
# among the four modes the prediction rules out.
INFERRED_BINS_RIGHT = 1
INFERRED_BINS_WRONG = 3


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


def softmax(logits: np.ndarray) -> np.ndarray:
    shifted = logits - logits.max(axis=1, keepdims=True)
    exponentiated = np.exp(shifted)
    return exponentiated / exponentiated.sum(axis=1, keepdims=True)


def main() -> int:
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "bench-out/train-pred/mixed.jsonl")
    model_path = Path(
        sys.argv[2] if len(sys.argv) > 2 else "models/girc-predictor.json"
    )
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else 0

    print(f"reading {path}")
    features, labels = load(path, limit or None)
    model = json.loads(model_path.read_text())
    weights = np.asarray(model["weights"], dtype=np.float32)
    biases = np.asarray(model["biases"], dtype=np.float32)
    print(f"  {labels.shape[0]} records, model {model['model_version']}")

    predicted = (features @ weights.T + biases).argmax(axis=1)

    explicit = np.asarray([EXPLICIT_BINS[m] for m in labels], dtype=np.float64)
    constant = np.zeros(labels.shape[0], dtype=np.int64)
    constant_bins = np.full(labels.shape[0], INFERRED_BINS_WRONG, dtype=np.float64)
    constant_bins[labels == 0] = INFERRED_BINS_RIGHT

    learned_bins = np.where(
        predicted == labels, INFERRED_BINS_RIGHT, INFERRED_BINS_WRONG
    ).astype(np.float64)
    oracle_bins = np.full(labels.shape[0], INFERRED_BINS_RIGHT, dtype=np.float64)

    print("\nsignalling bins per block, and the mismatch rate:")
    for name, inferred, prediction in (
        ("explicit (baseline)", None, None),
        ("constant DCT-II", constant_bins, constant),
        ("learned model", learned_bins, predicted),
        ("oracle", oracle_bins, labels),
    ):
        if inferred is None:
            print(f"  {name:22s} {explicit.mean():6.3f} bins")
            continue
        mismatch = float((prediction != labels).mean())
        saving = explicit.mean() - inferred.mean()
        print(
            f"  {name:22s} {inferred.mean():6.3f} bins   "
            f"mismatch {mismatch * 100:5.2f}%   "
            f"saving {saving:+.3f} bins/block"
        )

    print(
        "\nThe oracle's saving is the most the mechanism can save, and the "
        "constant's\nsaving is what a predictor that learns nothing achieves. "
        "The gap between them\nis what the predictor has to earn, and the "
        "distortion it pays for the\nwrong answers is not in these numbers; "
        "it is in the codec's own benchmark."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
