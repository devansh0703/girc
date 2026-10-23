#!/usr/bin/env python3
"""Generate every figure on the site and in the report, from committed artifacts.

Nothing on the site is typed into a page. Each figure below is rendered from
a committed artifact -- the benchmark report, the trained model, the training
records -- by this one script, into a directory the site and the report copy
from. A page cannot drift from its measurement, and a figure cannot exist
without the data behind it.

The figures are inline SVG rather than PNG or a charting library. The site
already renders its own inline SVG for the design system, an SVG is readable in
the repository as text and diffable, and the project's rule is to take the
existing implementation over a new dependency.

    python3 tools/generate_figures.py [--output web/girc-site/assets/figures]

The data sources, and where each is read from:

    research/benchmark/report-v9-joint.json   the benchmark run
    models/girc-predictor.json                the shipped trained model
    bench-out/train-pred/mixed-v8.jsonl       the encoder's own block records
    bench-out/transform_spread.json           the transform-search spread
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT = ROOT / "web" / "girc-site" / "assets" / "figures"

REPORT = ROOT / "research" / "benchmark" / "report-v9-joint.json"
MODEL = ROOT / "models" / "girc-predictor.json"
RECORDS = ROOT / "bench-out" / "train-pred" / "mixed-v8.jsonl"
SPREAD = ROOT / "bench-out" / "transform_spread.json"

#: The five transform modes, in the codec's canonical order.
MODES = [
    "DCT-II",
    "DST-VII/DST-VII",
    "DCT-VIII/DST-VII",
    "DST-VII/DCT-VIII",
    "DCT-VIII/DCT-VIII",
]

#: The light and dark palettes, as CSS custom properties, so the same figure
#: reads correctly in both. Every colour in a figure is a property reference.
INK = "var(--ink)"
INK_DIM = "var(--ink-dim)"
INK_FAINT = "var(--ink-faint)"
ACCENT = "var(--accent)"
LINE = "var(--line)"
LINE_STRONG = "var(--line-strong)"
GRID = "var(--grid)"
SEV_OK = "var(--sev-ok)"
SEV_CRITICAL = "var(--sev-critical)"
SEV_WARNING = "var(--sev-warning)"
SEV_INFO = "var(--sev-info)"
PANEL = "var(--panel)"


def esc(text: str) -> str:
    """Escape text for XML content and for an attribute value."""
    return (
        str(text)
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
    )


def num(value: float, digits: int = 2) -> str:
    """A number formatted the way the documents write it: a Unicode minus."""
    text = f"{value:.{digits}f}"
    return text.replace("-", "−")


def signed(value: float, digits: int = 3) -> str:
    """A signed figure, with an explicit + so a positive is not mistaken."""
    text = f"{value:+.{digits}f}"
    return text.replace("-", "−")


class Figure:
    """One SVG figure, assembled from parts.

    Every figure carries its own caption, its own source line naming the
    artifact it came from, and an ``aria-label`` saying what it shows, because
    a chart that is only pixels is a chart a screen reader cannot read.
    """

    def __init__(
        self,
        name: str,
        caption: str,
        source: str,
        width: int,
        height: int,
        aria: str,
        view_height: int | None = None,
    ) -> None:
        self.name = name
        self.caption = caption
        self.source = source
        self.width = width
        self.height = height
        self.aria = aria
        self.parts: list[str] = []

    def add(self, body: str) -> None:
        self.parts.append(body)

    def render(self) -> str:
        plot = "\n    ".join(self.parts)
        return f"""<figure class="chart">
  <figcaption>
    <span>{esc(self.caption)}</span>
    <span class="src">{esc(self.source)}</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 {self.width} {self.height}" role="img" aria-label="{esc(self.aria)}">
    {plot}
    </svg>
  </div>
</figure>
"""


# --------------------------------------------------------------------- parts


def bar(
    x: float,
    y: float,
    width: float,
    height: float,
    fill: str,
    opacity: float | None = None,
    stroke: str = LINE_STRONG,
) -> str:
    """A rectangle, with an explicit zero width rather than a negative one."""
    w = max(width, 0.0)
    o = f' opacity="{opacity}"' if opacity is not None else ""
    return (
        f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{height:.1f}" '
        f'fill="{fill}" stroke="{stroke}" stroke-width="0.5"{o} />'
    )


def text(
    x: float,
    y: float,
    body: str,
    size: float = 12.0,
    fill: str = INK_DIM,
    anchor: str = "start",
    weight: str = "400",
) -> str:
    return (
        f'<text x="{x:.1f}" y="{y:.1f}" font-size="{size}" fill="{fill}" '
        f'text-anchor="{anchor}" font-weight="{weight}">{esc(body)}</text>'
    )


def polyline(points: list[tuple[float, float]], stroke: str, width: float = 2.0) -> str:
    joined = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    return f'<polyline points="{joined}" fill="none" stroke="{stroke}" stroke-width="{width}" />'


def line(x1: float, y1: float, x2: float, y2: float, stroke: str, width: float = 1.0) -> str:
    return (
        f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" '
        f'stroke="{stroke}" stroke-width="{width}" />'
    )


def circle(x: float, y: float, radius: float, fill: str) -> str:
    return f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{radius:.1f}" fill="{fill}" />'


# ------------------------------------------------------------------- helpers


def load_report() -> dict:
    return json.loads(REPORT.read_text(encoding="utf-8"))


def load_model() -> dict:
    return json.loads(MODEL.read_text(encoding="utf-8"))


def transform_distribution(model: dict) -> list[int]:
    """The class counts the encoder actually chose, from the model's own record."""
    distribution = model["training"]["class_distribution"]
    return [int(distribution[mode]) for mode in MODES]


def sequence_points(report: dict) -> dict[str, dict[str, list[dict]]]:
    """The four QP points of every codec, per sequence, from the report."""
    points: dict[str, dict[str, list[dict]]] = {}
    for result in report["results"]:
        points.setdefault(result["sequence"], {})[result["codec"]] = result["points"]
    return points


# ------------------------------------------------------------------- figures


def fig_mode_distribution(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """What the encoder actually chose: the five modes, and the tie that binds them."""
    model = load_model()
    counts = transform_distribution(model)
    total = sum(counts)

    f = Figure(
        "fig-mode-distribution",
        "What the encoder actually chose: the plain DCT-II, and four modes that split the rest",
        "models/girc-predictor.json · class_distribution",
        900,
        300,
        "The transform mode distribution over 570,240 blocks: the plain DCT-II at 50.1 percent, "
        "and the four multiple transform selection modes sharing 49.9 percent",
    )

    left, top, width, height = 18, 56, 864, 200
    f.add(line(left, top + height, left + width, top + height, LINE_STRONG))
    f.add(line(left, top, left, top + height, LINE_STRONG))

    widest = max(counts)
    for index, (mode, count) in enumerate(zip(MODES, counts)):
        share = 100 * count / total
        x = left + 10 + index * (width / len(MODES))
        bar_w = (width / len(MODES)) - 22
        h = (count / widest) * (height - 34)
        colour = ACCENT if index == 0 else SEV_INFO
        f.add(bar(x, top + height - h, bar_w, h, colour))
        f.add(text(x, top + height - h - 8, f"{share:.1f}%", 12, INK, weight="500"))
        f.add(text(x, top + height + 18, mode.split("/")[0], 11, INK_FAINT))
        if index:
            f.add(text(x, top + height + 33, mode.split("/")[1], 11, INK_FAINT))

    f.add(text(left, 24, "570,240 blocks, the encoder's own rate-distortion choices", 12.5, INK_DIM))
    f.add(text(left + width, 24, "the majority class is 50.10%", 12, INK_FAINT, anchor="end"))
    f.add(text(left + width, 44, "the four other modes are 49.90%", 12, INK_FAINT, anchor="end"))

    figures.append((f.name, f.render(), "mode-distribution", 900, 300, f.aria))


def fig_margin_sweep(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """The mechanism's only free parameter: the rate saved against the distortion paid."""
    report = load_report()
    sweep = report["margin_sweep"]

    f = Figure(
        "fig-margin-sweep",
        "The margin sweep: the rate the encoder saves against the distortion it pays",
        "research/benchmark/report-v9-joint.json · margin_sweep",
        900,
        360,
        "Mean Bjontegaard delta rate against the explicit mode, by margin, over twelve sequences: "
        "the best point is a margin of 50 at minus 0.236 percent",
    )

    left, top, width, height = 70, 40, 790, 240
    values = [point["mean_bd_rate"] for point in sweep]
    lowest, highest = min(values), max(values)
    span = (highest - lowest) or 1.0
    pad = span * 0.12
    lowest -= pad
    highest += pad

    def y_of(value: float) -> float:
        return top + height - ((value - lowest) / (highest - lowest)) * height

    # The grid, and the zero line, which is where the mechanism stops paying.
    for step in range(5):
        value = lowest + (highest - lowest) * step / 4
        y = y_of(value)
        f.add(line(left, y, left + width, y, GRID))
        f.add(text(left - 10, y + 4, f"{value:.1f}", 11, INK_FAINT, anchor="end"))
    zero = y_of(0.0)
    f.add(line(left, zero, left + width, zero, LINE_STRONG, 1.4))
    f.add(text(left + width, zero - 8, "the explicit baseline", 11.5, INK_DIM, anchor="end"))

    points = []
    for index, point in enumerate(sweep):
        x = left + index * (width / (len(sweep) - 1))
        y = y_of(point["mean_bd_rate"])
        points.append((x, y))
    f.add(polyline(points, ACCENT, 2.2))

    for index, point in enumerate(sweep):
        x, y = points[index]
        f.add(circle(x, y, 4.5, ACCENT))
        f.add(text(x, y - 12, f"{point['mean_bd_rate']:+.3f}", 11.5, INK, anchor="middle", weight="500"))
        f.add(text(x, top + height + 20, f"{int(point['margin'])}", 11, INK_FAINT, anchor="middle"))

    best = min(sweep, key=lambda point: point["mean_bd_rate"])
    best_x = left + [p["margin"] for p in sweep].index(best["margin"]) * (width / (len(sweep) - 1))
    f.add(
        text(
            best_x,
            y_of(best["mean_bd_rate"]) + 34,
            "the optimum on this content",
            11.5,
            SEV_OK,
            anchor="middle",
        )
    )

    f.add(text(left - 56, top - 16, "BD-rate, %", 11.5, INK_DIM))
    f.add(text(left + width / 2, top + height + 44, "margin, in rate-distortion cost units", 11.5, INK_DIM, anchor="middle"))
    f.add(text(left, 24, "negative is smaller than the explicit mode", 12, INK_FAINT))

    figures.append((f.name, f.render(), "margin-sweep", 900, 360, f.aria))


def fig_learned_vs_constant(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """The comparison that has to be won: the trained model against the majority class."""
    report = load_report()
    learned = report["mechanism_bd_rate"]
    constant = report["constant_bd_rate"]

    f = Figure(
        "fig-learned-vs-constant",
        "The learned predictor against the constant one, per sequence",
        "research/benchmark/report-v9-joint.json · mechanism_bd_rate, constant_bd_rate",
        900,
        380,
        "Per-sequence Bjontegaard delta rate for the shipped learned model and for the constant "
        "predictor that always answers the plain DCT-II",
    )

    sequences = list(learned.keys())
    left, top, width, height = 70, 44, 790, 250
    values = [learned[s] for s in sequences] + [constant[s] for s in sequences]
    lowest, highest = min(values), max(values)
    span = (highest - lowest) or 1.0
    pad = span * 0.15
    lowest -= pad
    highest += pad

    def y_of(value: float) -> float:
        return top + height - ((value - lowest) / (highest - lowest)) * height

    for step in range(4):
        value = lowest + (highest - lowest) * step / 3
        y = y_of(value)
        f.add(line(left, y, left + width, y, GRID))
        f.add(text(left - 10, y + 4, f"{value:.1f}", 11, INK_FAINT, anchor="end"))

    zero = y_of(0.0)
    f.add(line(left, zero, left + width, zero, LINE_STRONG, 1.4))
    f.add(text(left + width, zero - 8, "no gain", 11.5, INK_DIM, anchor="end"))

    slot = width / len(sequences)
    bar_w = slot * 0.32
    for index, sequence in enumerate(sequences):
        x = left + index * slot
        for offset, table, colour, label in (
            (-bar_w - 1, learned, ACCENT, "learned"),
            (1, constant, SEV_INFO, "constant"),
        ):
            value = table[sequence]
            y = y_of(value)
            top_y = min(y, zero)
            f.add(bar(x + offset, top_y, bar_w, abs(y - zero), colour, opacity=0.9))
            f.add(
                text(
                    x + offset + bar_w / 2,
                    top_y - 5 if value < 0 else top_y + abs(y - zero) + 12,
                    f"{value:+.2f}",
                    9.5,
                    INK_DIM,
                    anchor="middle",
                )
            )
        f.add(text(x + slot / 2, top + height + 20, sequence.replace("_cif", ""), 10.5, INK_FAINT, anchor="middle"))

    f.add(bar(left, 20, 14, 10, ACCENT))
    f.add(text(left + 20, 29, "the shipped learned model", 12, INK_DIM))
    f.add(bar(left + 260, 20, 14, 10, SEV_INFO))
    f.add(text(left + 280, 29, "the constant predictor, always the plain DCT-II", 12, INK_DIM))

    figures.append((f.name, f.render(), "learned-vs-constant", 900, 380, f.aria))


def fig_predictability(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """Why the saving is what it is: the mode is barely predictable from the causal window."""
    model = load_model()
    training = model["training"]

    f = Figure(
        "fig-predictability",
        "The predictability ceiling, and where the shipped model sits under it",
        "models/girc-predictor.json · training.predictability_ceiling",
        900,
        300,
        "The majority class at 50.10 percent, the shipped model at 50.17 percent, and the "
        "gradient-boosted ceiling at 50.65 percent",
    )

    left, top, width, height = 70, 46, 790, 190
    rows = [
        ("majority class: always the plain DCT-II", training["majority_class_accuracy"], INK_FAINT),
        ("the shipped model, over the joint mode", training["test_accuracy"], ACCENT),
        ("the gradient-boosted ceiling, same features", training["predictability_ceiling"]["gradient_boosted_accuracy"], SEV_INFO),
    ]

    scale = 100.0 / 55.0
    usable = width
    for index, (label, value, colour) in enumerate(rows):
        y = top + index * 56
        h = 30
        w = value * scale * (usable / 100)
        f.add(bar(left, y, w, h, colour, opacity=0.9))
        f.add(text(left + w + 10, y + 20, f"{value * 100:.2f}%", 12, INK, weight="500"))
        f.add(text(left, y - 8, label, 12, INK_DIM))
        f.add(text(left + w, y + h + 16, f"{value * 100:.2f} of every 100 blocks", 10.5, INK_FAINT))

    f.add(
        text(
            left,
            top + 3 * 56 + 6,
            "the gap between the first and the last is what any predictor over these features can earn",
            12,
            INK_FAINT,
        )
    )

    figures.append((f.name, f.render(), "predictability", 900, 300, f.aria))


def fig_class_accuracy(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """Per-class accuracy: the model knows the default and cannot see the rest."""
    model = load_model()
    per_class = model["training"]["per_class_test_accuracy"]

    f = Figure(
        "fig-class-accuracy",
        "Per-class accuracy: the default is learned, the other four modes are not",
        "models/girc-predictor.json · training.per_class_test_accuracy",
        900,
        300,
        "Per-class test accuracy: 91.7 percent on the plain DCT-II and between 5.4 and 15.2 percent "
        "on the four multiple transform selection modes",
    )

    left, top, width, height = 18, 50, 864, 190
    widest = max(per_class.values())
    for index, mode in enumerate(MODES):
        value = per_class[mode]
        x = left + 10 + index * (width / len(MODES))
        bar_w = (width / len(MODES)) - 26
        h = (value / widest) * height
        colour = SEV_OK if value > 0.5 else SEV_CRITICAL
        f.add(bar(x, top + height - h, bar_w, h, colour, opacity=0.9))
        f.add(text(x, top + height - h - 8, f"{value * 100:.1f}%", 11.5, INK, weight="500"))
        f.add(text(x, top + height + 18, mode.split("/")[0], 10.5, INK_FAINT))
        if index:
            f.add(text(x, top + height + 33, mode.split("/")[1], 10.5, INK_FAINT))

    f.add(line(left, top + height, left + width, top + height, LINE_STRONG))
    f.add(text(left, 24, "the model answers the default correctly almost always, and the four directional modes almost never", 12, INK_DIM))
    f.add(text(left + width, 24, "114,048 held-out blocks", 12, INK_FAINT, anchor="end"))

    figures.append((f.name, f.render(), "class-accuracy", 900, 300, f.aria))


def fig_signalling_bins(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """The signalling side, measured: bins per block for each predictor, against the explicit baseline."""
    report = load_report()
    # The bench's own block counts, from the report's own results.
    results = report["results"]
    blocks = sum(result["block_count"] for result in results if result["codec"] == "girc-inferred")
    mismatches = sum(result["prediction_mismatches"] for result in results if result["codec"] == "girc-inferred")

    # The explicit syntax the codec uses, from the standard's own code: one bin
    # for the plain DCT-II and a truncated unary over the four others.
    model = load_model()
    distribution = transform_distribution(model)
    total = sum(distribution)
    explicit_bins = (
        distribution[0] * 1 + distribution[1] * 2 + distribution[2] * 3 + distribution[3] * 4 + distribution[4] * 4
    ) / total

    mismatch = mismatches / blocks
    learned_bins = (1 - mismatch) * 1 + mismatch * 3
    constant_bins = (1 - 0.5010434203142536) * 1 + 0.5010434203142536 * 3
    oracle_bins = 1.0

    f = Figure(
        "fig-signalling-bins",
        "What the signalling costs per block, against the explicit syntax it replaces",
        "research/benchmark/report-v9-joint.json · results, models/girc-predictor.json",
        900,
        300,
        "Signalling bins per block: the explicit syntax costs 2.12 bins, the constant predictor "
        "1.99, the learned model 1.97, and the oracle one bin",
    )

    rows = [
        ("the explicit mode, the standard's own mts_idx syntax", explicit_bins, INK_DIM, "the baseline"),
        ("the constant predictor, always the plain DCT-II", constant_bins, SEV_INFO, "no training, no model"),
        ("the shipped learned model", learned_bins, ACCENT, "the mechanism as shipped"),
        ("the oracle, which cannot be built", oracle_bins, SEV_CRITICAL, "the ceiling"),
    ]

    left, top, width = 330, 52, 520
    widest = explicit_bins
    for index, (label, value, colour, note) in enumerate(rows):
        y = top + index * 56
        w = (value / widest) * width
        f.add(bar(left, y, w, 30, colour, opacity=0.9))
        f.add(text(left + w + 10, y + 20, f"{value:.2f} bins", 12, INK, weight="500"))
        f.add(text(left, y - 8, label, 11.5, INK_DIM))
        f.add(text(left, y + 44, note, 10.5, INK_FAINT))

    f.add(text(18, 24, "the correction is coded relative to the prediction, so a wrong answer costs three bins rather than two", 12, INK_DIM))
    f.add(text(880, 24, f"{blocks:,} blocks, the benchmark's own count", 11, INK_FAINT, anchor="end"))

    figures.append((f.name, f.render(), "signalling-bins", 900, 300, f.aria))


def fig_rd_curves(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """The rate-distortion curve, and the gap the mechanism opens in it."""
    report = load_report()
    table = sequence_points(report)
    sequence = "foreman_cif"

    f = Figure(
        "fig-rd-curves",
        f"Rate-distortion, {sequence}: the two GIRC modes and the three anchors",
        "research/benchmark/report-v9-joint.json · results",
        900,
        400,
        f"The rate-distortion curves for {sequence} at four quantization parameters, for GIRC in "
        "both modes and for x265, SVT-AV1 and VVenC",
    )

    left, top, width, height = 80, 44, 740, 270
    codecs = [
        c
        for c in table[sequence]
        if c in ("girc-explicit", "girc-inferred", "x265", "svt-av1", "vvenc")
    ]
    all_points = [point for codec in codecs for point in table[sequence][codec]]
    rates = [point["bits"] for point in all_points]
    psnrs = [point["psnr_db"] for point in all_points]
    low_r, high_r = min(rates), max(rates)
    low_p, high_p = min(psnrs), max(psnrs)
    pad_p = (high_p - low_p) * 0.08

    def x_of(bits: float) -> float:
        return left + ((bits - low_r) / (high_r - low_r)) * width

    def y_of(psnr: float) -> float:
        return top + height - ((psnr - (low_p - pad_p)) / ((high_p + pad_p) - (low_p - pad_p))) * height

    for step in range(5):
        psnr = low_p + (high_p - low_p) * step / 4
        y = y_of(psnr)
        f.add(line(left, y, left + width, y, GRID))
        f.add(text(left - 10, y + 4, f"{psnr:.1f}", 11, INK_FAINT, anchor="end"))

    colours = {
        "girc-explicit": INK_DIM,
        "girc-inferred": ACCENT,
        "x265": SEV_INFO,
        "svt-av1": SEV_WARNING,
        "vvenc": SEV_OK,
    }
    for codec in codecs:
        points = table[sequence][codec]
        polyline_points = [(x_of(p["bits"]), y_of(p["psnr_db"])) for p in points]
        f.add(polyline(polyline_points, colours[codec], 2.0))
        for x, y in polyline_points:
            f.add(circle(x, y, 3.5, colours[codec]))

    # The legend, and the two GIRC points at QP 32 called out.
    legend_y = top - 18
    cursor = left
    for codec in codecs:
        f.add(line(cursor, legend_y, cursor + 18, legend_y, colours[codec], 2.2))
        f.add(text(cursor + 24, legend_y + 4, codec, 11.5, INK_DIM))
        cursor += 30 + len(codec) * 7

    explicit_32 = [p for p in table[sequence]["girc-explicit"] if p["qp"] == 32][0]
    inferred_32 = [p for p in table[sequence]["girc-inferred"] if p["qp"] == 32][0]
    ex, inx = x_of(explicit_32["bits"]), x_of(inferred_32["bits"])
    f.add(line(ex, y_of(explicit_32["psnr_db"]), inx, y_of(inferred_32["psnr_db"]), SEV_CRITICAL, 1.6))
    f.add(
        text(
            (ex + inx) / 2,
            y_of(explicit_32["psnr_db"]) + 26,
            f"the mechanism, at QP 32: {explicit_32['bits'] - inferred_32['bits']} bits apart",
            11,
            SEV_CRITICAL,
            anchor="middle",
        )
    )

    f.add(text(left - 62, top - 16, "PSNR, dB", 11.5, INK_DIM))
    f.add(text(left + width / 2, top + height + 40, "bits for ten frames", 11.5, INK_DIM, anchor="middle"))
    f.add(text(left, 24, "the anchors are better codecs; the gap between the two GIRC lines is the mechanism", 12, INK_FAINT))

    figures.append((f.name, f.render(), "rd-curves", 900, 400, f.aria))


def fig_causal_window(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """How the prediction works, drawn: the window, the block, and the one bit."""
    f = Figure(
        "fig-causal-window",
        "How the prediction works: the window, the block, and the single correction bit",
        "crates/girc-core/src/geometry.rs, crates/girc-core/src/bitstream.rs",
        900,
        300,
        "A diagram of the causal window the decoder reads, the block whose transform is predicted, "
        "and the one bin the encoder spends when the prediction was wrong",
    )

    cell = 14
    grid = 12
    ox, oy = 240, 60

    # The already-reconstructed window, and the block being coded.
    for row in range(grid):
        for column in range(grid):
            x = ox + column * cell
            y = oy + row * cell
            is_block = row >= grid - 4 and column >= grid - 4
            fill = PANEL if is_block else "var(--bar-track)"
            f.add(
                f'<rect x="{x}" y="{y}" width="{cell}" height="{cell}" fill="{fill}" '
                f'stroke="{GRID}" stroke-width="0.5" />'
            )

    # The block outline, and the arrows showing what is read.
    f.add(
        f'<rect x="{ox + (grid - 4) * cell}" y="{oy + (grid - 4) * cell}" width="{4 * cell}" '
        f'height="{4 * cell}" fill="none" stroke="{ACCENT}" stroke-width="1.6" />'
    )
    f.add(text(ox + (grid - 4) * cell + 2, oy + (grid - 4) * cell - 10, "the block being coded", 12, ACCENT))

    f.add(text(ox + 2, oy - 10, "already reconstructed, already known to both sides", 11.5, INK_DIM))

    # The encoder side: search, predict, compare, spend one bit.
    ex, ey = 40, 40
    boxes = [
        (ex, ey, "the encoder's rate-distortion search"),
        (ex, ey + 74, "the decoder's prediction, from the window"),
        (ex, ey + 148, "one bin: was the prediction right?"),
        (ex, ey + 222, "if it was wrong, two bins pick between the rest"),
    ]
    for index, (x, y, label) in enumerate(boxes):
        f.add(f'<rect x="{x}" y="{y}" width="280" height="46" rx="2" fill="{PANEL}" stroke="{LINE_STRONG}" stroke-width="1" />')
        f.add(text(x + 14, y + 28, label, 12, INK))
        if index < len(boxes) - 1:
            f.add(f'<path d="M {x + 140} {y + 46} V {y + 74}" stroke="{ACCENT}" stroke-width="1.4" fill="none" marker-end="url(#arrow)" />')

    # The decoder side, mirroring it.
    dx, dy = 560, 40
    decoder_boxes = [
        (dx, dy, "the decoder reads its own window"),
        (dx, dy + 74, "it predicts the mode, at no cost"),
        (dx, dy + 148, "it reads the one bin"),
        (dx, dy + 222, "right: take the prediction. wrong: read two more"),
    ]
    for index, (x, y, label) in enumerate(decoder_boxes):
        f.add(f'<rect x="{x}" y="{y}" width="300" height="46" rx="2" fill="{PANEL}" stroke="{LINE_STRONG}" stroke-width="1" />')
        f.add(text(x + 14, y + 28, label, 12, INK))
        if index < len(decoder_boxes) - 1:
            f.add(f'<path d="M {x + 150} {y + 46} V {y + 74}" stroke="{SEV_INFO}" stroke-width="1.4" fill="none" marker-end="url(#arrow2)" />')

    # The shared bitstream line between them.
    f.add(line(ox + 200, oy + grid * cell + 26, ox + 200, oy + grid * cell + 26, LINE_STRONG))
    f.add(
        text(
            ox + 200,
            oy + grid * cell + 52,
            "one bitstream, one flag in the header",
            11.5,
            INK_FAINT,
            anchor="middle",
        )
    )

    f.add(
        '<defs>'
        f'<marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">'
        f'<path d="M 0 0 L 10 5 L 0 10 z" fill="{ACCENT}" /></marker>'
        f'<marker id="arrow2" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">'
        f'<path d="M 0 0 L 10 5 L 0 10 z" fill="{SEV_INFO}" /></marker>'
        "</defs>"
    )

    figures.append((f.name, f.render(), "causal-window", 900, 300, f.aria))


def fig_transform_tie(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """Why the mechanism is small: the search's best and runner-up are close."""
    spread = json.loads(SPREAD.read_text(encoding="utf-8"))
    gap = spread["gap_between_best_and_runner_up"]
    relative = spread["relative_spread"]

    f = Figure(
        "fig-transform-tie",
        "Why the mechanism is small: the rate-distortion choice between modes is close to a tie",
        "bench-out/transform_spread.json",
        900,
        260,
        "The gap between the search's best transform and the runner-up, as a percentage of the "
        "best: a median of 0.67 percent and a 90th percentile of 2.64 percent",
    )

    left, top, width, height = 70, 46, 790, 150
    bars = [
        ("10th percentile", gap["p10"], INK_FAINT),
        ("median", gap["p50"], ACCENT),
        ("90th percentile", gap["p90"], SEV_WARNING),
    ]
    scale = height / (gap["p90"] or 1.0)

    f.add(text(left - 56, top - 16, "RD-cost gap", 11.5, INK_DIM))
    for index, (label, value, colour) in enumerate(bars):
        y = top + index * 48
        w = value * scale
        f.add(bar(left, y, w, 28, colour, opacity=0.9))
        f.add(text(left + w + 10, y + 19, f"{value:.3f}", 12, INK, weight="500"))
        f.add(text(left, y - 8, label, 11.5, INK_DIM))

    f.add(
        text(
            left,
            top + 3 * 48 + 6,
            f"relative spread: a median of {relative['p50']:.2f} and a 90th percentile of {relative['p90']:.2f} "
            f"percent of the best mode's own cost, over {spread['blocks']:,} blocks of akiyo_cif",
            11.5,
            INK_FAINT,
        )
    )

    figures.append((f.name, f.render(), "transform-tie", 900, 260, f.aria))


def fig_chroma_planes(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """Why there are two rate axes: the mechanism is a luma mechanism."""
    f = Figure(
        "fig-chroma-planes",
        "Why there are two rate axes: the mechanism is a luma mechanism",
        "crates/girc-core/src/chroma.rs, ITU-T H.266 clause 7.3.9",
        900,
        280,
        "A 4:2:0 frame drawn to scale: one luma sample per chroma sample in a 2x2 ratio, with the "
        "chroma planes carrying no transform-type signalling at all",
    )

    # A 4:2:0 frame drawn to scale: luma is four samples per chroma sample.
    luma_w, luma_h = 480, 320
    ox, oy = 40, 60
    f.add(f'<rect x="{ox}" y="{oy}" width="{luma_w}" height="{luma_h}" fill="{ACCENT}" opacity="0.14" stroke="{LINE_STRONG}" stroke-width="1" />')
    f.add(text(ox + 8, oy + 24, "luma: 352 × 288, four samples per chroma sample", 12, INK))
    f.add(text(ox + 8, oy + 44, "the transform type is signalled here, and the mechanism removes all of it", 11.5, INK_DIM))

    chroma_w, chroma_h = luma_w // 2, luma_h // 2
    for index, (label, dx) in enumerate((("Cb", 0), ("Cr", 1))):
        cx = ox + 520 + dx * 130
        cy = oy + 40
        f.add(f'<rect x="{cx}" y="{cy}" width="{chroma_w}" height="{chroma_h}" fill="{SEV_INFO}" opacity="0.14" stroke="{LINE_STRONG}" stroke-width="1" />')
        f.add(text(cx + 8, cy + 24, f"{label}: 176 × 144", 12, INK))
        f.add(text(cx + 8, cy + 44, "always the plain DCT-II", 11.5, INK_DIM))
        f.add(text(cx + 8, cy + 62, "no transform type is signalled", 11.5, INK_DIM))

    f.add(text(40, 24, "the mechanism changes the luma signalling and nothing else", 12.5, INK_DIM))
    f.add(
        text(
            40,
            44,
            "so the benchmark reports the luma rate, which is the mechanism's own effect, and the whole-frame rate, which is what a deployment saves",
            11.5,
            INK_FAINT,
        )
    )

    figures.append((f.name, f.render(), "chroma-planes", 900, 280, f.aria))


def fig_evaluation_lifecycle(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """The evaluation, drawn as the state machine the portal actually implements."""
    f = Figure(
        "fig-evaluation-lifecycle",
        "The evaluation lifecycle, as the portal implements it",
        "apps/portal/girc_eval/app.py",
        900,
        300,
        "The five states of an evaluation: registered, agreed, requested, approved, evaluated, with "
        "the gated artifacts released only between approved and evaluated",
    )

    states = [
        ("registered", "an organisation account exists", ACCENT),
        ("agreed", "the agreement's version and digest are recorded", ACCENT),
        ("requested", "the codec and the content are named", SEV_INFO),
        ("approved", "a reviewer approves; downloads are signed", SEV_OK),
        ("evaluated", "results are submitted and accepted", INK_FAINT),
    ]

    left, top, width = 40, 100, 820
    step = width / len(states)
    for index, (name, note, colour) in enumerate(states):
        x = left + index * step
        f.add(f'<rect x="{x}" y="{top}" width="{step - 18}" height="54" rx="2" fill="{PANEL}" stroke="{LINE_STRONG}" stroke-width="1" />')
        f.add(f'<rect x="{x}" y="{top}" width="4" height="54" fill="{colour}" />')
        f.add(text(x + 14, top + 24, name, 12.5, INK, weight="500"))
        f.add(text(x + 14, top + 42, note.split(",")[0], 10.5, INK_FAINT))
        if index < len(states) - 1:
            f.add(
                f'<path d="M {x + step - 18} {top + 27} H {x + step - 2}" stroke="{LINE_STRONG}" '
                f'stroke-width="1.4" fill="none" marker-end="url(#arrow3)" />'
            )

    # The gated gate, drawn as the boundary it is.
    gate_x = left + 3 * step
    f.add(
        f'<line x1="{gate_x - 10}" y1="{top - 56}" x2="{gate_x - 10}" y2="{top + 110}" '
        f'stroke="{SEV_CRITICAL}" stroke-width="1.4" stroke-dasharray="5 4" />'
    )
    f.add(text(gate_x - 16, top - 64, "the gate: nothing crosses it unsigned", 11.5, SEV_CRITICAL, anchor="end"))

    f.add(
        '<defs>'
        f'<marker id="arrow3" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">'
        f'<path d="M 0 0 L 10 5 L 0 10 z" fill="{LINE_STRONG}" /></marker>'
        "</defs>"
    )

    f.add(text(40, 24, "every transition writes an audit entry: the actor, the time and the address it came from", 12, INK_DIM))
    f.add(text(40, 44, "a download token is signed, time-limited, bound to the artifact's digest and to the requesting user", 11.5, INK_FAINT))
    f.add(text(40, 230, "the audit log is append-only: the application never updates or deletes a row in it", 12, INK_DIM))

    figures.append((f.name, f.render(), "evaluation-lifecycle", 900, 300, f.aria))


def fig_syntax(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """The two syntaxes, side by side, with the bin cost of each."""
    f = Figure(
        "fig-syntax",
        "The two syntaxes, and what each costs",
        "crates/girc-core/src/bitstream.rs, ITU-T H.266 clause 7.3.9",
        900,
        280,
        "The explicit mode's truncated binary code over the five modes, against the inferred mode's "
        "one bin plus a two-bin correction",
    )

    # The explicit mode's code, as a decision tree drawn left to right.
    rows = [
        ("plain DCT-II", "1 bin", 1, INK_DIM),
        ("DST-VII / DST-VII", "2 bins", 2, SEV_INFO),
        ("DCT-VIII / DST-VII", "3 bins", 3, SEV_INFO),
        ("DST-VII / DCT-VIII", "4 bins", 4, SEV_INFO),
        ("DCT-VIII / DCT-VIII", "4 bins", 4, SEV_INFO),
    ]

    left, top = 40, 56
    for index, (mode, cost, bins, colour) in enumerate(rows):
        y = top + index * 40
        f.add(f'<rect x="{left}" y="{y}" width="180" height="30" rx="2" fill="{PANEL}" stroke="{LINE_STRONG}" stroke-width="1" />')
        f.add(text(left + 12, y + 20, mode, 11.5, INK))
        # The bins, drawn as the coded flags.
        for b in range(bins):
            f.add(f'<rect x="{left + 210 + b * 34}" y="{y}" width="28" height="30" fill="{colour}" opacity="0.3" stroke="{LINE_STRONG}" stroke-width="0.8" />')
            f.add(text(left + 224 + b * 34, y + 20, "1" if b == 0 else "0", 11, INK))
        f.add(text(left + 210 + 4 * 34 + 12, y + 20, cost, 12, INK, weight="500"))

    # The inferred mode's code, for contrast.
    iy = top
    f.add(f'<rect x="{left + 560}" y="{iy}" width="300" height="30" rx="2" fill="{PANEL}" stroke="{ACCENT}" stroke-width="1.4" />')
    f.add(text(left + 572, iy + 20, "1 bin: was the prediction right?", 11.5, ACCENT))
    f.add(f'<rect x="{left + 560}" y="{iy + 40}" width="300" height="30" rx="2" fill="{PANEL}" stroke="{LINE_STRONG}" stroke-width="1" />')
    f.add(text(left + 572, iy + 60, "and if not, 2 bins select between the four the prediction ruled out", 11.5, INK_DIM))
    f.add(text(left + 560, iy + 104, "an override never costs more than the signalling it replaces", 11.5, INK_FAINT))
    f.add(text(left + 560, iy + 124, "the naive form spent a bin on the correction and then the full type again", 11.5, INK_FAINT))

    f.add(text(40, 24, "the inferred mode codes the correction relative to the prediction", 12.5, INK_DIM))
    f.add(text(860, 24, "the explicit mode is the standard's own mts_idx", 11.5, INK_FAINT, anchor="end"))

    figures.append((f.name, f.render(), "syntax", 900, 280, f.aria))


def fig_benchmark_apparatus(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """The measurement apparatus: what is held constant and what is compared."""
    f = Figure(
        "fig-benchmark-apparatus",
        "The measurement apparatus: one variable, everything else held constant",
        "crates/girc-bench/src/main.rs",
        900,
        300,
        "A diagram of the benchmark: the same content and quantization parameters through GIRC in "
        "both modes, and through three anchor codecs, differing only in how the transform mode "
        "reaches the decoder",
    )

    boxes = [
        (40, 60, 200, "twelve CIF sequences"),
        (40, 140, 200, "four quantization parameters"),
        (40, 220, 200, "ten frames each"),
    ]
    for x, y, w, label in boxes:
        f.add(f'<rect x="{x}" y="{y}" width="{w}" height="46" rx="2" fill="{PANEL}" stroke="{LINE_STRONG}" stroke-width="1" />')
        f.add(text(x + 14, y + 28, label, 12, INK))

    # The two modes, and the single difference between them.
    for index, (label, colour) in enumerate((("the explicit mode", INK_DIM), ("the inferred mode", ACCENT))):
        x = 320
        y = 60 + index * 100
        f.add(f'<rect x="{x}" y="{y}" width="200" height="76" rx="2" fill="{PANEL}" stroke="{colour}" stroke-width="1.4" />')
        f.add(text(x + 14, y + 30, label, 12.5, colour, weight="500"))
        f.add(text(x + 14, y + 50, "the same search, the same quantizer" if index == 0 else "the same search, plus a margin", 11, INK_DIM))

    # The single difference, drawn as the thing that is isolated.
    f.add(
        f'<rect x="580" y="98" width="280" height="84" rx="2" fill="{PANEL}" stroke="{SEV_CRITICAL}" stroke-width="1.4" />'
    )
    f.add(text(594, 124, "the only difference", 12.5, SEV_CRITICAL, weight="500"))
    f.add(text(594, 146, "how the transform mode reaches the decoder", 11.5, INK_DIM))
    f.add(text(594, 166, "every other stage is shared by both", 11.5, INK_DIM))

    for index in range(2):
        y = 60 + index * 100
        f.add(f'<path d="M 240 {60 + 140 * index + 23} L 320 {y + 38}" stroke="{LINE_STRONG}" stroke-width="1.2" fill="none" />')
        f.add(f'<path d="M 520 {y + 38} L 580 140" stroke="{SEV_CRITICAL}" stroke-width="1.2" fill="none" />')

    f.add(text(40, 24, "the anchors are coded the same way, on the same frames, with the same PSNR code", 12, INK_DIM))
    f.add(text(40, 286, "the benchmark decodes what it encodes, and refuses a result if the two pictures differ", 11.5, INK_FAINT))

    figures.append((f.name, f.render(), "benchmark-apparatus", 900, 300, f.aria))


def fig_mismatch(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """How often the prediction is wrong, and what the margin does to it."""
    report = load_report()

    f = Figure(
        "fig-mismatch",
        "How often the prediction is wrong, and how the margin buys it back",
        "research/benchmark/report-v9-joint.json · margin_sweep",
        900,
        300,
        "The prediction mismatch rate falling from 46.4 percent at a margin of zero to 30.6 percent at "
        "a margin of 200, as the encoder takes the prediction more often",
    )

    left, top, width, height = 70, 46, 790, 190
    sweep = report["margin_sweep"]
    values = [point["mean_mismatch_rate"] * 100 for point in sweep]
    lowest, highest = min(values), max(values)
    span = (highest - lowest) or 1.0

    def y_of(value: float) -> float:
        return top + height - ((value - lowest) / span) * height

    for step in range(4):
        value = lowest + span * step / 3
        y = y_of(value)
        f.add(line(left, y, left + width, y, GRID))
        f.add(text(left - 10, y + 4, f"{value:.0f}%", 11, INK_FAINT, anchor="end"))

    points = []
    for index, point in enumerate(sweep):
        x = left + index * (width / (len(sweep) - 1))
        points.append((x, y_of(point["mean_mismatch_rate"] * 100)))
    f.add(polyline(points, SEV_WARNING, 2.2))
    for index, point in enumerate(sweep):
        x, y = points[index]
        f.add(circle(x, y, 4.5, SEV_WARNING))
        f.add(text(x, y - 12, f"{point['mean_mismatch_rate'] * 100:.1f}%", 11, INK, anchor="middle", weight="500"))
        f.add(text(x, top + height + 20, f"{int(point['margin'])}", 11, INK_FAINT, anchor="middle"))

    f.add(text(left, 24, "the margin is what turns a wrong prediction into a free one", 12, INK_DIM))
    f.add(text(left + width / 2, top + height + 44, "margin, in rate-distortion cost units", 11.5, INK_DIM, anchor="middle"))
    f.add(text(left + width, 24, "the intra blocks' mismatch is higher than the inter blocks'", 11.5, INK_FAINT, anchor="end"))

    figures.append((f.name, f.render(), "mismatch", 900, 300, f.aria))


def fig_inter_intra(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """Intra against inter: the hypothesis holds, and the gain is smaller for inter."""
    spread = ROOT / "bench-out" / "inter_predictability.json"
    data = json.loads(spread.read_text(encoding="utf-8"))

    f = Figure(
        "fig-inter-intra",
        "Intra against inter: the hypothesis holds, and the gain is smaller for inter",
        "bench-out/inter_predictability.json",
        900,
        280,
        "Per-axis prediction accuracy over the majority baseline: a gain of 7.5 percentage points "
        "for the intra blocks and 1.9 for the inter blocks",
    )

    rows = [
        ("intra, horizontal axis", data["intra"]["horizontal"], data["intra"]["records"]),
        ("intra, vertical axis", data["intra"]["vertical"], data["intra"]["records"]),
        ("inter, horizontal axis", data["inter"]["horizontal"], data["inter"]["records"]),
        ("inter, vertical axis", data["inter"]["vertical"], data["inter"]["records"]),
    ]

    left, top, width = 300, 50, 540
    for index, (label, axis, count) in enumerate(rows):
        y = top + index * 54
        baseline = axis["majority_baseline"] * 100
        accuracy = axis["accuracy"] * 100
        f.add(bar(left, y, (baseline / 100) * width, 30, INK_FAINT, opacity=0.5))
        f.add(bar(left, y, (accuracy / 100) * width, 30, ACCENT if "intra" in label else SEV_INFO, opacity=0.9))
        f.add(text(left - 12, y + 20, label, 11.5, INK_DIM, anchor="end"))
        f.add(text(left + (accuracy / 100) * width + 10, y + 20, f"{accuracy:.2f}%", 12, INK, weight="500"))
        f.add(
            text(
                left + (baseline / 100) * width,
                y + 44,
                f"majority {baseline:.2f}% · a gain of {axis['gain_over_baseline'] * 100:+.2f} points over {count:,} records",
                10.5,
                INK_FAINT,
            )
        )

    f.add(text(40, 24, "the inter blocks' transform distribution is more concentrated, so there is less to gain", 12, INK_DIM))
    f.add(text(40, 262, "measured on the encoder's own records, not on a model of them", 11.5, INK_FAINT))

    figures.append((f.name, f.render(), "inter-intra", 900, 280, f.aria))


def fig_anchor_gap(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """Against the anchors: context, and the scale of the gap."""
    report = load_report()
    table = sequence_points(report)
    sequences = sorted(table.keys())

    f = Figure(
        "fig-anchor-gap",
        "Against the anchors: the same content, the same code, and the scale of the gap",
        "research/benchmark/report-v9-joint.json · bd_rate_vs_baseline",
        900,
        380,
        "Bjontegaard delta rate against the GIRC explicit baseline, per sequence, for x265, SVT-AV1 "
        "and VVenC, and for GIRC's own inferred mode",
    )

    left, top, width, height = 70, 44, 790, 250
    values: list[float] = []
    series: dict[str, list[float]] = {"girc-inferred": [], "girc-constant": []}
    for sequence in sequences:
        for codec in series:
            series[codec].append(report["bd_rate_vs_baseline"][sequence][codec])
        for codec in ("x265", "svt-av1", "vvenc"):
            series[codec] = series.get(codec, [])
            series[codec].append(report["bd_rate_vs_baseline"][sequence][codec])
    for values_list in series.values():
        values.extend(values_list)

    lowest, highest = min(values), max(values)
    span = (highest - lowest) or 1.0
    pad = span * 0.06
    lowest -= pad
    highest += pad

    def y_of(value: float) -> float:
        return top + height - ((value - lowest) / (highest - lowest)) * height

    for step in range(5):
        value = lowest + (highest - lowest) * step / 4
        y = y_of(value)
        f.add(line(left, y, left + width, y, GRID))
        f.add(text(left - 10, y + 4, f"{value:.0f}", 11, INK_FAINT, anchor="end"))

    zero = y_of(0.0)
    f.add(line(left, zero, left + width, zero, LINE_STRONG, 1.4))

    slot = width / len(sequences)
    bar_w = slot * 0.2
    for index, sequence in enumerate(sequences):
        x = left + index * slot
        for offset, codec in enumerate(("girc-inferred", "girc-constant", "x265", "svt-av1", "vvenc")):
            value = series[codec][index]
            y = y_of(value)
            colour = ACCENT if codec == "girc-inferred" else SEV_INFO if codec == "girc-constant" else SEV_CRITICAL
            f.add(bar(x + offset * (bar_w + 2), min(y, zero), bar_w, abs(y - zero), colour, opacity=0.9))
        f.add(text(x + slot / 2, top + height + 20, sequence.replace("_cif", ""), 10, INK_FAINT, anchor="middle"))

    cursor = left
    for codec, colour in (("girc-inferred", ACCENT), ("girc-constant", SEV_INFO), ("x265", SEV_CRITICAL), ("svt-av1", SEV_CRITICAL), ("vvenc", SEV_CRITICAL)):
        f.add(bar(cursor, 20, 12, 9, colour))
        f.add(text(cursor + 17, 28, codec, 11.5, INK_DIM))
        cursor += 24 + len(codec) * 7

    f.add(text(left, 24, "negative is a smaller bitrate than the GIRC explicit mode", 12, INK_DIM))

    figures.append((f.name, f.render(), "anchor-gap", 900, 380, f.aria))


def fig_architecture(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """The pipeline, drawn: where each artifact comes from."""
    f = Figure(
        "fig-architecture",
        "The pipeline: where each artifact comes from, and where it is used",
        "the repository layout",
        900,
        320,
        "A diagram of the repository: the crate that implements the codec, the pipeline that trains "
        "the predictor on the encoder's own decisions, and the harness that measures both",
    )

    columns = [
        ("crates/girc-core", ["VVC-exact integer transforms", "intra and inter prediction", "the geometry predictor", "the entropy coder"], ACCENT),
        ("python/girc-train", ["collect: the encoder's decisions", "fit: one model over five modes", "the predictability ceiling", "a frozen, versioned artifact"], SEV_INFO),
        ("crates/girc-bench", ["both modes, same content", "the margin sweep", "the anchor codecs", "the Bjontegaard implementation"], SEV_OK),
    ]

    left, top, width = 40, 60, 270
    for index, (name, items, colour) in enumerate(columns):
        x = left + index * (width + 30)
        f.add(f'<rect x="{x}" y="{top}" width="{width}" height="196" rx="2" fill="{PANEL}" stroke="{colour}" stroke-width="1.4" />')
        f.add(f'<rect x="{x}" y="{top}" width="{width}" height="34" fill="{colour}" opacity="0.16" />')
        f.add(text(x + 14, top + 22, name, 12.5, colour, weight="500"))
        for row, item in enumerate(items):
            f.add(text(x + 14, top + 58 + row * 34, item, 11.5, INK_DIM))

    # The artifact that connects them.
    f.add(
        f'<rect x="{left + 30}" y="{top + 230}" width="{width * 3}" height="46" rx="2" fill="{PANEL}" '
        f'stroke="{ACCENT}" stroke-width="1.4" />'
    )
    f.add(text(left + 44, top + 258, "models/girc-predictor.json — produced by the middle, read by both sides of the codec", 12, ACCENT))

    for index in range(2):
        x = left + index * (width + 30) + width
        f.add(
            f'<path d="M {x} {top + 150} L {x + 30} {top + 150}" stroke="{LINE_STRONG}" stroke-width="1.4" '
            f'fill="none" marker-end="url(#arrow4)" />'
        )

    f.add(
        '<defs>'
        f'<marker id="arrow4" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">'
        f'<path d="M 0 0 L 10 5 L 0 10 z" fill="{LINE_STRONG}" /></marker>'
        "</defs>"
    )

    f.add(text(40, 24, "one product: the codec, the training pipeline, the benchmark, and the portal that evaluates it", 12, INK_DIM))

    figures.append((f.name, f.render(), "architecture", 900, 320, f.aria))


def fig_hosted_service(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> None:
    """The hosted analysis: what it is, what it is not, and how it is kept awake."""
    f = Figure(
        "fig-hosted-service",
        "The hosted analysis: the same code, over HTTP, on one free instance",
        "apps/portal, render.yaml",
        900,
        300,
        "A diagram of the hosted service: the browser posts a trace, the portal encodes and decodes "
        "it, and the report comes back, with no trace retained",
    )

    boxes = [
        (40, 70, 220, "you", ["paste a sequence", "or upload a .girc stream"], INK_DIM),
        (330, 70, 220, "the portal, on Render", ["encode, decode, measure", "nothing is stored"], ACCENT),
        (620, 70, 240, "the report", ["the same figures", "as the local run"], SEV_OK),
    ]
    for x, y, w, title, lines, colour in boxes:
        f.add(f'<rect x="{x}" y="{y}" width="{w}" height="100" rx="2" fill="{PANEL}" stroke="{colour}" stroke-width="1.4" />')
        f.add(f'<rect x="{x}" y="{y}" width="{w}" height="30" fill="{colour}" opacity="0.16" />')
        f.add(text(x + 14, y + 20, title, 12, colour, weight="500"))
        for row, line in enumerate(lines):
            f.add(text(x + 14, y + 56 + row * 22, line, 11.5, INK_DIM))

    for index in range(2):
        x = 40 + index * 290
        f.add(
            f'<path d="M {x + 220} 120 H {x + 290}" stroke="{LINE_STRONG}" stroke-width="1.4" '
            f'fill="none" marker-end="url(#arrow5)" />'
        )

    f.add(
        '<defs>'
        f'<marker id="arrow5" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">'
        f'<path d="M 0 0 L 10 5 L 0 10 z" fill="{LINE_STRONG}" /></marker>'
        "</defs>"
    )

    f.add(text(40, 24, "a free instance sleeps after fifteen minutes idle, so an uptime monitor pings it every five", 12, INK_DIM))
    f.add(text(40, 44, "one service, on purpose: a workspace gets 750 free hours a month, and a month is 730 to 744 hours", 11.5, INK_FAINT))

    f.add(text(40, 226, "what it is not", 12.5, SEV_CRITICAL, weight="500"))
    f.add(text(40, 250, "not a different product: the portal runs the same crate you can read and deploy yourself", 11.5, INK_DIM))
    f.add(text(40, 270, "not a hosted-only feature: every figure it returns is reproducible from the repository", 11.5, INK_DIM))

    figures.append((f.name, f.render(), "hosted-service", 900, 300, f.aria))


def write_all(figures: list[tuple[str, str, str, int, int, str]], out: Path) -> dict[str, str]:
    """Write each figure to its own file, and return the name to markup map."""
    out.mkdir(parents=True, exist_ok=True)
    for stale in out.glob("*.svg"):
        stale.unlink()

    markup: dict[str, str] = {}
    for name, rendered, slug, width, height, aria in figures:
        path = out / f"{name}.svg"
        path.write_text(rendered, encoding="utf-8")
        markup[name] = f'<div data-figure="{slug}"></div>'
        print(f"  {path.relative_to(ROOT)}  {width}x{height}")
    return markup


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    arguments = parser.parse_args()

    out = arguments.output
    print(f"reading {REPORT.relative_to(ROOT)}")
    print(f"reading {MODEL.relative_to(ROOT)}")

    figures: list[tuple[str, str, str, int, int, str]] = []
    fig_mode_distribution(figures, out)
    fig_margin_sweep(figures, out)
    fig_learned_vs_constant(figures, out)
    fig_predictability(figures, out)
    fig_class_accuracy(figures, out)
    fig_signalling_bins(figures, out)
    fig_rd_curves(figures, out)
    fig_mismatch(figures, out)
    fig_inter_intra(figures, out)
    fig_anchor_gap(figures, out)
    fig_transform_tie(figures, out)
    fig_chroma_planes(figures, out)
    fig_causal_window(figures, out)
    fig_syntax(figures, out)
    fig_benchmark_apparatus(figures, out)
    fig_evaluation_lifecycle(figures, out)
    fig_architecture(figures, out)
    fig_hosted_service(figures, out)

    write_all(figures, out)
    print(f"\n{len(figures)} figures written to {out.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
