"""GIRC predictor training pipeline.

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

from __future__ import annotations

import argparse
import hashlib
import json
import math
import platform
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Iterator, Sequence

import numpy as np

# The transform classes, in the order `girc-core` indexes them.
TRANSFORM_NAMES = ("DCT-II", "DCT-VIII", "DST-VII")
NUM_CLASSES = len(TRANSFORM_NAMES)

# The feature layout, matching `girc_core::geometry::GeometryFeatures`:
# eight orientation-histogram bins, then gradient energy, dominance,
# isotropy and reference coverage.
NUM_ORIENTATION_BINS = 8
FEATURE_NAMES = (
    *[f"orientation_bin_{i}" for i in range(NUM_ORIENTATION_BINS)],
    "gradient_energy",
    "dominance",
    "isotropy",
    "reference_coverage",
)
NUM_FEATURES = len(FEATURE_NAMES)

# The default QP the collection pass uses. The predictor must work across
# the QP range the codec operates in, so the collection pass sweeps it.
COLLECTION_QPS = (22, 27, 32, 37)


@dataclass(frozen=True)
class Sample:
    """One training example."""

    features: np.ndarray
    transform: int

    def to_json(self) -> dict:
        return {
            "features": [float(v) for v in self.features],
            "transform": int(self.transform),
        }

    @staticmethod
    def from_json(payload: dict) -> "Sample":
        return Sample(
            features=np.asarray(payload["features"], dtype=np.float64),
            transform=int(payload["transform"]),
        )


def collect_samples(
    sequences: Sequence[Path],
    output: Path,
    qps: Iterable[int] = COLLECTION_QPS,
) -> int:
    """Collect training samples by running the GIRC encoder.

    The encoder is driven through its command-line interface so the
    collected labels are exactly the transform the rate-distortion search
    chose, not a re-implementation of it.
    """
    binary = _find_binary("girc")
    output.parent.mkdir(parents=True, exist_ok=True)
    total = 0
    with output.open("w", encoding="utf-8") as handle:
        for sequence in sequences:
            for qp in qps:
                payload = _run_collector(binary, sequence, qp)
                for record in payload:
                    handle.write(json.dumps(record) + "\n")
                    total += 1
            print(f"collected from {sequence.name}", file=sys.stderr)
    return total


def _run_collector(binary: Path, sequence: Path, qp: int) -> list[dict]:
    """Run the encoder in collection mode and return the raw records."""
    result = subprocess.run(
        [
            str(binary),
            "collect",
            "--input",
            str(sequence),
            "--qp",
            str(qp),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    return [json.loads(line) for line in result.stdout.splitlines() if line.strip()]


def fit_model(
    input_path: Path,
    output_path: Path,
    model_version: str,
    seed: int = 20261022,
) -> dict:
    """Fit the multinomial logistic regression and export it.

    The fit uses scikit-learn's `LogisticRegression` with a fixed random
    state, so the exported weights are reproducible from the recorded
    training data.
    """
    from sklearn.linear_model import LogisticRegression
    from sklearn.model_selection import train_test_split

    samples = list(read_samples(input_path))
    if len(samples) < 100:
        raise SystemExit(
            f"only {len(samples)} samples were collected; at least 100 are needed"
        )

    features = np.stack([s.features for s in samples])
    labels = np.asarray([s.transform for s in samples], dtype=np.int64)

    # Hold out a fifth of the data so the reported accuracy is on unseen
    # blocks rather than on the training set.
    stratify = labels if len(np.unique(labels)) > 1 else None
    x_train, x_test, y_train, y_test = train_test_split(
        features, labels, test_size=0.2, random_state=seed, stratify=stratify
    )

    model = LogisticRegression(max_iter=2000, random_state=seed, C=1.0)
    model.fit(x_train, y_train)

    train_accuracy = float((model.predict(x_train) == y_train).mean())
    test_accuracy = float((model.predict(x_test) == y_test).mean())

    # Per-class accuracy, which shows whether the model has learned the
    # directional transforms or only the dominant one.
    per_class = {}
    for index, name in enumerate(TRANSFORM_NAMES):
        mask = y_test == index
        if mask.sum() == 0:
            per_class[name] = None
        else:
            per_class[name] = float((model.predict(x_test[mask]) == index).mean())

    payload = {
        "model_version": model_version,
        "feature_count": NUM_FEATURES,
        "feature_names": list(FEATURE_NAMES),
        "class_names": list(TRANSFORM_NAMES),
        "biases": [float(b) for b in model.intercept_],
        "weights": [[float(w) for w in row] for row in model.coef_],
        "training": {
            "samples": len(samples),
            "train_samples": int(len(y_train)),
            "test_samples": int(len(y_test)),
            "train_accuracy": train_accuracy,
            "test_accuracy": test_accuracy,
            "per_class_test_accuracy": per_class,
            "random_state": seed,
            "class_distribution": {
                name: int((labels == index).sum()) for index, name in enumerate(TRANSFORM_NAMES)
            },
        },
        "data": {
            "path": str(input_path),
            "sha256": sha256_of(input_path),
        },
        "environment": environment_info(),
    }

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    return payload


def evaluate_model(
    model_path: Path,
    sequences: Sequence[Path],
    qps: Iterable[int] = COLLECTION_QPS,
) -> dict:
    """Evaluate the exported model's prediction accuracy on held-out content.

    The evaluation runs the real encoder in inferred mode and compares the
    number of blocks where the decoder's prediction differed from the
    encoder's choice. That mismatch rate is the quantity the mechanism's
    cost depends on, so it is the number worth reporting.
    """
    payload = json.loads(model_path.read_text(encoding="utf-8"))
    binary = _find_binary("girc")
    total_blocks = 0
    total_mismatches = 0
    per_sequence = {}

    for sequence in sequences:
        blocks = 0
        mismatches = 0
        for qp in qps:
            result = subprocess.run(
                [
                    str(binary),
                    "evaluate",
                    "--input",
                    str(sequence),
                    "--qp",
                    str(qp),
                    "--model",
                    str(model_path),
                ],
                capture_output=True,
                text=True,
                check=True,
            )
            stats = json.loads(result.stdout)
            blocks += stats["block_count"]
            mismatches += stats["prediction_mismatches"]
        per_sequence[sequence.stem] = {
            "block_count": blocks,
            "prediction_mismatches": mismatches,
            "mismatch_rate": mismatches / blocks if blocks else 0.0,
        }
        total_blocks += blocks
        total_mismatches += mismatches

    return {
        "model_version": payload.get("model_version"),
        "per_sequence": per_sequence,
        "block_count": total_blocks,
        "prediction_mismatches": total_mismatches,
        "mismatch_rate": total_mismatches / total_blocks if total_blocks else 0.0,
    }


def read_samples(path: Path) -> Iterator[Sample]:
    """Read the training samples from a JSONL file."""
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if line:
                yield Sample.from_json(json.loads(line))


def sha256_of(path: Path) -> str:
    """The SHA-256 checksum of a file."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def environment_info() -> dict:
    """The environment the model was trained in, for reproducibility."""
    return {
        "python": platform.python_version(),
        "platform": platform.platform(),
        "numpy": np.__version__,
    }


def _find_binary(name: str) -> Path:
    """Find a GIRC binary, preferring the workspace build."""
    local = Path("target/release") / name
    if local.exists():
        return local
    local_debug = Path("target/debug") / name
    if local_debug.exists():
        return local_debug
    found = shutil_which(name)
    if found:
        return Path(found)
    raise SystemExit(
        f"could not find the {name} binary; run `cargo build --release` first"
    )


def shutil_which(name: str) -> str | None:
    """A minimal `which` implementation, to avoid a dependency."""
    import os
    import shutil

    return shutil.which(name)


def main(argv: Sequence[str] | None = None) -> int:
    """The command-line entry point."""
    parser = argparse.ArgumentParser(prog="girc-train", description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    collect = sub.add_parser("collect", help="collect training samples")
    collect.add_argument("--sequences", nargs="+", type=Path, required=True)
    collect.add_argument("--output", type=Path, required=True)

    fit = sub.add_parser("fit", help="fit and export the model")
    fit.add_argument("--input", type=Path, required=True)
    fit.add_argument("--output", type=Path, required=True)
    fit.add_argument("--model-version", default="v1")

    evaluate = sub.add_parser("evaluate", help="evaluate an exported model")
    evaluate.add_argument("--model", type=Path, required=True)
    evaluate.add_argument("--sequences", nargs="+", type=Path, required=True)
    evaluate.add_argument("--output", type=Path)

    args = parser.parse_args(argv)

    if args.command == "collect":
        count = collect_samples(args.sequences, args.output)
        print(f"collected {count} samples into {args.output}", file=sys.stderr)
        return 0

    if args.command == "fit":
        payload = fit_model(args.input, args.output, args.model_version)
        training = payload["training"]
        print(
            f"model {payload['model_version']}: "
            f"train accuracy {training['train_accuracy']:.4f}, "
            f"test accuracy {training['test_accuracy']:.4f} "
            f"on {training['test_samples']} held-out blocks",
            file=sys.stderr,
        )
        return 0

    if args.command == "evaluate":
        report = evaluate_model(args.model, args.sequences)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(report, indent=2))
        return 0

    parser.error(f"unknown command {args.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
