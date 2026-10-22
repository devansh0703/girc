"""Measure how predictable each transform axis is, separately.

The transform mode is a pair of per-axis transforms, so the joint
choice can be split into a horizontal choice and a vertical choice. VVC
treats the two axes separately in its own transform-selection code, so
this is the natural decomposition to measure.

The script writes its result to the path given as the first argument.
"""

import collections
import json
import sys

import numpy as np
from sklearn.ensemble import GradientBoostingClassifier
from sklearn.metrics import accuracy_score
from sklearn.model_selection import train_test_split

# The mode is a pair of per-axis transforms:
# 0 = (DCT-II, DCT-II), 1 = (DST-VII, DST-VII),
# 2 = (DCT-VIII, DST-VII), 3 = (DST-VII, DCT-VIII),
# 4 = (DCT-VIII, DCT-VIII).
# The per-axis classes are 0 = DCT-II, 1 = DCT-VIII, 2 = DST-VII.
HORIZONTAL = {0: 0, 1: 2, 2: 1, 3: 2, 4: 1}
VERTICAL = {0: 0, 1: 2, 2: 2, 3: 1, 4: 1}


def main() -> int:
    source = sys.argv[1] if len(sys.argv) > 1 else "bench-out/training.jsonl"
    destination = sys.argv[2] if len(sys.argv) > 2 else "bench-out/axis_predictability.json"

    features = []
    modes = []
    with open(source) as handle:
        for line in handle:
            record = json.loads(line)
            features.append(record["features"])
            modes.append(record["transform"])
    features = np.asarray(features)
    modes = np.asarray(modes)

    horizontal = np.array([HORIZONTAL[m] for m in modes])
    vertical = np.array([VERTICAL[m] for m in modes])

    train, test = train_test_split(
        np.arange(len(modes)), test_size=0.2, random_state=20261022, stratify=modes
    )

    results = {}
    for name, labels in (
        ("joint", modes),
        ("horizontal", horizontal),
        ("vertical", vertical),
    ):
        distribution = collections.Counter(labels.tolist())
        model = GradientBoostingClassifier(
            random_state=0, n_estimators=80, max_depth=4
        ).fit(features[train], labels[train])
        results[name] = {
            "classes": len(distribution),
            "majority": distribution.most_common(1)[0][1] / len(labels),
            "ceiling": accuracy_score(labels[test], model.predict(features[test])),
        }
        print(
            f"{name:11s} {results[name]['classes']} classes  "
            f"majority {results[name]['majority']:.4f}  "
            f"ceiling {results[name]['ceiling']:.4f}",
            flush=True,
        )

    with open(destination, "w") as handle:
        json.dump(results, handle, indent=2)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
