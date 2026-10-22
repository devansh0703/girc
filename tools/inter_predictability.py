"""Measure whether an inter residual is more predictable than an intra one.

The claim the patent package makes about inter prediction is that a
motion-compensated residual carries more structure than an intra
residual, so the transform mode should be more predictable on the
inter blocks.

This measures that directly: it fits one model per axis on the intra
blocks and one on the inter blocks, and reports each one's accuracy
against the majority baseline for its own class distribution. The
comparison is only meaningful if the two are measured the same way, so
the blocks are split by the `is_inter` feature.

    python3 tools/inter_predictability.py bench-out/training-mixed.jsonl
"""

from __future__ import annotations

import collections
import json
import sys

import numpy as np
from sklearn.linear_model import LogisticRegression
from sklearn.metrics import accuracy_score
from sklearn.model_selection import train_test_split

# The mode is a pair of per-axis transforms:
# 0 = (DCT-II, DCT-II), 1 = (DST-VII, DST-VII),
# 2 = (DCT-VIII, DST-VII), 3 = (DST-VII, DCT-VIII),
# 4 = (DCT-VIII, DCT-VIII).
# The per-axis classes are 0 = DCT-II, 1 = DCT-VIII, 2 = DST-VII.
HORIZONTAL = {0: 0, 1: 2, 2: 1, 3: 2, 4: 1}
VERTICAL = {0: 0, 1: 2, 2: 2, 3: 1, 4: 1}

#: The index of the `is_inter` feature in the vector.
#:
#: The vector is the orientation histogram, four scalars, three axis
#: energies, the quantization parameter, the intra mode one-hot, the
#: neighbour transform one-hot, the inter flag, and the motion block.
IS_INTER = 46


def load(path: str) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """The features, the modes, and the inter flag."""
    features = []
    modes = []
    with open(path) as handle:
        for line in handle:
            record = json.loads(line)
            features.append(record["features"])
            modes.append(record["transform"])
    matrix = np.asarray(features, dtype=np.float32)
    return matrix, np.asarray(modes), matrix[:, IS_INTER] > 0.5


def fit_axis(features: np.ndarray, labels: np.ndarray, axis: dict) -> tuple[float, float]:
    """The accuracy of a per-axis model, and the majority baseline.

    The model is a logistic regression, the same kind the exported
    model uses, so the number is comparable with the model's own.
    """
    targets = np.asarray([axis[int(mode)] for mode in labels])
    if len(set(targets.tolist())) < 2:
        return 0.0, 0.0

    counts = collections.Counter(targets.tolist())
    baseline = max(counts.values()) / len(targets)

    x_train, x_test, y_train, y_test = train_test_split(
        features, targets, test_size=0.2, random_state=0, stratify=targets
    )
    model = LogisticRegression(max_iter=400, n_jobs=-1)
    model.fit(x_train, y_train)
    accuracy = accuracy_score(y_test, model.predict(x_test))
    return accuracy, baseline


def main() -> int:
    source = sys.argv[1] if len(sys.argv) > 1 else "bench-out/training-mixed.jsonl"
    destination = (
        sys.argv[2] if len(sys.argv) > 2 else "bench-out/inter_predictability.json"
    )

    features, modes, is_inter = load(source)
    print(f"{len(modes)} records from {source}")

    results: dict = {}
    for name, mask in [("intra", ~is_inter), ("inter", is_inter)]:
        if mask.sum() == 0:
            continue
        subset = features[mask]
        labels = modes[mask]
        entry = {"records": int(mask.sum())}
        for axis_name, axis in [("horizontal", HORIZONTAL), ("vertical", VERTICAL)]:
            accuracy, baseline = fit_axis(subset, labels, axis)
            entry[axis_name] = {
                "accuracy": accuracy,
                "majority_baseline": baseline,
                "gain_over_baseline": accuracy - baseline,
            }
            print(
                f"  {name:<6} {axis_name:<11} accuracy {accuracy:.4f} "
                f"baseline {baseline:.4f} gain {accuracy - baseline:+.4f}"
            )
        results[name] = entry

    if "intra" in results and "inter" in results:
        gains = {
            axis: results["inter"][axis]["gain_over_baseline"]
            - results["intra"][axis]["gain_over_baseline"]
            for axis in ("horizontal", "vertical")
        }
        results["inter_minus_intra_gain"] = gains
        print(f"  inter minus intra, gain over baseline: {gains}")

    with open(destination, "w", encoding="utf-8") as handle:
        json.dump(results, handle, indent=2)
    print(f"wrote {destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
