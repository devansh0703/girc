"""Compare joint mode prediction against per-axis composition.

The transform mode is a pair of per-axis transforms. A joint model has
to learn the correlation between the two axes in one shot, which the
measurement shows is hard. Predicting each axis separately and
composing the results may be easier, because the geometry of a
neighbourhood says more about the horizontal transform than about the
joint mode.

This script measures both, and also measures the rate-distortion cost
of the mismatch, which is what actually decides whether the mechanism
pays.
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

# The valid modes, indexed by the per-axis classes.
MODE_OF = {
    (0, 0): 0,
    (2, 2): 1,
    (1, 2): 2,
    (2, 1): 3,
    (1, 1): 4,
}


def compose(horizontal: int, vertical: int) -> int:
    """The mode the two per-axis predictions imply.

    A mode is either the plain DCT-II on both axes, or a multiple
    transform selection mode, in which neither axis is the DCT-II. So a
    predicted DCT-II on either axis forces the plain DCT-II, and
    otherwise the two predictions combine directly.
    """
    if horizontal == 0 or vertical == 0:
        return 0
    return MODE_OF[(horizontal, vertical)]


def main() -> int:
    source = sys.argv[1] if len(sys.argv) > 1 else "bench-out/training.jsonl"
    destination = (
        sys.argv[2] if len(sys.argv) > 2 else "bench-out/composition.json"
    )

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

    def fit(labels):
        return GradientBoostingClassifier(
            random_state=0, n_estimators=80, max_depth=4
        ).fit(features[train], labels)

    joint = fit(modes)
    horizontal_model = fit(horizontal)
    vertical_model = fit(vertical)

    joint_prediction = joint.predict(features[test])
    horizontal_prediction = horizontal_model.predict(features[test])
    vertical_prediction = vertical_model.predict(features[test])

    composed = np.array(
        [
            compose(int(h), int(v))
            for h, v in zip(horizontal_prediction, vertical_prediction)
        ]
    )

    distribution = collections.Counter(modes.tolist())
    results = {
        "majority": distribution.most_common(1)[0][1] / len(modes),
        "joint_model": accuracy_score(modes[test], joint_prediction),
        "composed_per_axis": accuracy_score(modes[test], composed),
        "horizontal_model": accuracy_score(horizontal[test], horizontal_prediction),
        "vertical_model": accuracy_score(vertical[test], vertical_prediction),
    }
    for name, value in results.items():
        print(f"{name:22s} {value:.4f}", flush=True)

    with open(destination, "w") as handle:
        json.dump(results, handle, indent=2)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
