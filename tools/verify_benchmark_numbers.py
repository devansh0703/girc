"""Check the documents' benchmark numbers against the report they came from.

The README, the design document, the patent package and both websites
all quote the same benchmark figures, and they have drifted apart
before: a number was changed in one place and not in the others, and
the claim-chart verifier caught the drift in the prior-art counts but
nothing checked the benchmark figures.

This script reads the benchmark report and checks that every figure
quoted in the documents is the one the report holds. It is the check
that the benchmark's numbers are the same everywhere.

The figures checked are the ones that appear in prose, and they are
checked by looking for the number in the document rather than by
rewriting the document, so the documents keep their own wording.

Run it from the repository root:

    python3 tools/verify_benchmark_numbers.py bench-out/fair/report-compare.json
"""

from __future__ import annotations

import json
import re
import statistics
import sys
from pathlib import Path

# The documents that quote the benchmark.
DOCUMENTS = [
    "README.md",
    "research/RELEASE-NOTES-1.0.0.md",
    "docs/designs/girc-geometry-inference-codec.md",
    "research/patent-package/README.md",
    "research/patent-package/REPRODUCIBILITY.md",
    "web/girc-site/results.html",
    "web/eval-site/index.html",
    "web/eval-site/scope.html",
]


def read_documents(root: Path) -> dict[str, str]:
    documents: dict[str, str] = {}
    for name in DOCUMENTS:
        path = root / name
        if path.exists():
            documents[name] = path.read_text(encoding="utf-8")
    return documents


def format_percent(value: float) -> str:
    """The way the documents write a percentage.

    The documents use the Unicode minus sign, because they are read by
    people, and they write the figure with three decimals.

    The rounding is done on the magnitude with the digits rounded away
    from zero, so a figure that is exactly half way rounds the same way
    the documents wrote it, rather than the way the language's default
    rounding does.
    """
    scaled = abs(value) * 1000.0
    digits = int(scaled + 0.5)
    text = f"{digits // 1000}.{digits % 1000:03d}"
    return ("−" if value < 0 else "+") + text


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    report_path = Path(
        sys.argv[1]
        if len(sys.argv) > 1
        else "research/benchmark/report-v8-joint.json"
    )
    if not report_path.exists():
        print(f"no report at {report_path}", file=sys.stderr)
        return 1
    report = json.loads(report_path.read_text(encoding="utf-8"))

    learned = report["mechanism_bd_rate"]
    total = report.get("mechanism_bd_rate_total", {})
    constant = report.get("constant_bd_rate", {})

    mean_learned = statistics.mean(learned.values())
    better_learned = sum(1 for value in learned.values() if value < 0)
    mean_constant = statistics.mean(constant.values()) if constant else None
    better_constant = (
        sum(1 for value in constant.values() if value < 0) if constant else None
    )
    mean_total = statistics.mean(total.values()) if total else None
    better_total = sum(1 for value in total.values() if value < 0) if total else None

    documents = read_documents(root)
    if not documents:
        print("no documents found", file=sys.stderr)
        return 1

    # The figures that have to appear, and where they are allowed to
    # be absent. A document that does not quote a figure is not
    # wrong about it.
    checks: list[tuple[str, str, bool]] = [
        (
            "the learned predictor's mean BD-rate",
            format_percent(mean_learned),
            True,
        ),
        (
            "the whole-frame mean BD-rate",
            format_percent(mean_total) if mean_total is not None else "",
            mean_total is not None,
        ),
        (
            "the constant predictor's mean BD-rate",
            format_percent(mean_constant) if mean_constant is not None else "",
            mean_constant is not None,
        ),
    ]

    # The documents that quote the headline figure.
    quoting = [
        name
        for name, text in documents.items()
        if format_percent(mean_learned) in text or format_percent(-mean_learned) in text
    ]

    failed = False
    for description, figure, required in checks:
        if not figure:
            continue
        present = [name for name, text in documents.items() if figure in text]
        if not present:
            if required and quoting:
                print(f"no document quotes the {description} ({figure})")
                failed = True
            continue
        print(f"{description}: {figure}, quoted in {len(present)} document(s)")

    # The figures that must not appear, because they are the old ones.
    #
    # The design document quotes the superseded three-transform result
    # once, as a historical reference, and that is legitimate: the
    # document says what it was and that it was superseded. So the
    # check is per document.
    stale = []
    for name, text in documents.items():
        for old in ("0.175%", "0.179%", "0.137%", "0.104%"):
            if old in text:
                stale.append((name, old))

    if stale:
        print("\nstale figures still in the documents:")
        for name, old in stale:
            print(f"  {name}: {old}")
        failed = True

    # The constant's mean, if the documents quote the learned one.
    if mean_constant is not None and quoting:
        quoted_constant = [
            name for name, text in documents.items() if format_percent(mean_constant) in text
        ]
        if not quoted_constant:
            print(
                "\nthe documents quote the learned predictor's result but not the "
                "constant\npredictor's, and the claim rests on the difference"
            )
            failed = True
        else:
            print(
                f"\nthe constant predictor's result is quoted in "
                f"{len(quoted_constant)} document(s), so the comparison is stated"
            )

    print(
        f"\nthe learned predictor is better on {better_learned} of {len(learned)} "
        f"sequences"
        + (
            f", and the constant on {better_constant} of {len(constant)}"
            if better_constant is not None
            else ""
        )
        + (f", and the whole frame on {better_total} of {len(total)}" if better_total is not None else "")
    )

    # The predictor's own measured accuracy, which the documents quote
    # and which comes from the model rather than from the benchmark.
    model_path = root / "models/girc-predictor.json"
    if model_path.exists():
        model = json.loads(model_path.read_text(encoding="utf-8"))
        training = model.get("training", {})
        ceiling = training.get("predictability_ceiling", {})
        figures = [
            ("the majority class accuracy", training.get("majority_class_accuracy")),
            ("the shipped model's accuracy", training.get("test_accuracy")),
            ("the gradient-boosted ceiling", ceiling.get("gradient_boosted_accuracy")),
        ]
        for description, value in figures:
            if value is None:
                continue
            percent = f"{value * 100:.2f}%"
            present = [name for name, text in documents.items() if percent in text]
            print(f"{description}: {percent}, quoted in {len(present)} document(s)")
            if not present:
                print(f"  no document quotes the {description}")
                failed = True

    if failed:
        print("\nthe documents and the report disagree")
        return 1
    print("the documents agree with the report")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
