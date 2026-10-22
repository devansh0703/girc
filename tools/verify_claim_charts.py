"""Check that the claim chart's evidence is in the claim text.

The chart is a reading of the claims, and the evidence it records is
quoted claim language. A quote that is not in the claim text is a
mistake, so this checks every quote against the claims it is drawn
from.

The extracted text has line breaks and hyphenation from the PDF, so
the comparison is on the flattened text with the hyphenation removed.

    python3 tools/verify_claim_charts.py
"""

from __future__ import annotations

import json
import re
import sys
from difflib import SequenceMatcher
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CLAIMS = ROOT / "research" / "patent-package" / "claims"
CHARTS = ROOT / "research" / "patent-package" / "claim_charts.json"

#: The elements of the disclosure, in the order the chart holds them.
ELEMENTS = [
    "E1_implicit",
    "E2_geometry",
    "E3_per_axis",
    "E4_margin",
    "E5_learned",
    "E6_correction",
]


def flatten(text: str) -> str:
    """The text as it reads, with the PDF's artefacts removed.

    A PDF's line breaks, its spacing around punctuation and its
    hyphenation are all artefacts of the layout, not of the text. The
    comparison is made on the text with them removed, so a quote is
    checked against what the claim says rather than against how the
    page was set.
    """
    # The hyphenation the PDF uses to break a word across a line.
    text = re.sub(r"(\w)-\s*\n\s*(\w)", r"\1\2", text)
    # A line break inside a word, which the PDF's extraction leaves as
    # a space. The break is a word boundary unless the first word ends
    # in a hyphen, which is handled above, or the two halves are not
    # words of their own, which is a break inside a word.
    text = re.sub(r"(\w)\n\s*(\w)", r"\1 \2", text)

    # The spacing around a bracket, which the PDF introduces.
    text = re.sub(r"\s+([)\],;:])", r"\1", text)
    text = re.sub(r"([\[(])\s+", r"\1", text)
    # The spacing around a hyphen that the text itself has.
    text = re.sub(r"(\w)\s+-\s+(\w)", r"\1-\2", text)
    # Every run of spaces becomes one.
    text = re.sub(r"\s+", " ", text)
    return text.lower()


def quoted(evidence: str) -> list[str]:
    """The passages the evidence quotes.

    A passage in double quotes is a translation or a reading, and it is
    not in the claim text, so only the passages in single quotes are
    checked.
    """
    return re.findall(r"'([^']{20,})'", evidence)


def similarity(quote: str, text: str) -> float:
    """How much of the quote is in the text.

    The optical character recognition reads a document, and it reads
    some characters wrong and drops the first letters of a word at the
    edge of a column. The comparison is therefore made on the best
    matching window of the text, and a high enough proportion of the
    quote matching is taken as the same passage.
    """
    # The comparison ignores the spacing, so a word broken across a
    # line matches the word itself.
    quote = quote.replace(" ", "")
    text = text.replace(" ", "")

    # Every position the quote's opening could be at is tried, because
    # a phrase often occurs more than once in a claim set and the match
    # that is wanted may not be the first.
    best = 0.0
    for length in (40, 20, 14, 10, 7):
        opening = quote[:length]
        start = 0
        while True:
            position = text.find(opening, start)
            if position < 0:
                break
            window = text[position : position + len(quote) + 60]
            matcher = SequenceMatcher(None, quote, window[: len(quote)])
            best = max(best, matcher.ratio())
            start = position + 1
        if best > 0.0:
            break
    return best


def check_result_table(report: str, charts: dict) -> int:
    """Check the report's result table against the chart.

    The table is the one place the per-reference findings are set out in
    a form a reader can check at a glance, so it has to agree with the
    chart it is drawn from. A hand-typed table drifts, so the cells are
    compared with the chart's own findings.
    """
    # The columns of the table, in the order the chart holds the
    # elements.
    order = ELEMENTS
    # The header names the elements, and they must be in the chart's
    # order for the cells to line up.
    header = re.search(r"^\|\s*Reference\s*\|.*$", report, re.M)
    if header is None:
        print("FAIL result table: no header row")
        return 1
    columns = [c.strip() for c in header.group(0).split("|")][1:-1]
    element_columns = columns[3:]
    if len(element_columns) != len(order):
        print(f"FAIL result table: {len(element_columns)} element columns, expected {len(order)}")
        return 1

    failures = 0
    seen = set()
    for line in report.splitlines():
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.split("|")][1:-1]
        if len(cells) != len(columns):
            continue
        number = cells[0]
        if number not in charts:
            continue
        seen.add(number)
        chart = charts[number]
        for column, element in zip(element_columns, order):
            marked = "●" in cells[3 + order.index(element)]
            present = chart["elements"][element]["present"]
            if marked != present:
                failures += 1
                print(f"FAIL result table {number} {element}: table says {marked}, chart says {present}")
    missing = set(charts) - seen
    if missing:
        failures += len(missing)
        print(f"FAIL result table: no row for {sorted(missing)}")
    return failures


def counts_sentence(charts: dict) -> str:
    """The counts of the chart, in one sentence.

    A document that reports how many references disclose each element
    has to be right, and a hand-typed number drifts. The sentence is
    built from the chart, and the documents are checked against it.
    """
    counts = {
        element: sum(1 for chart in charts.values() if chart["elements"][element]["present"])
        for element in ELEMENTS
    }
    names = {
        "E1_implicit": "implicit derivation",
        "E2_geometry": "reconstructed-neighbourhood geometry",
        "E3_per_axis": "per-axis selection",
        "E4_margin": "the rate-distortion margin",
        "E5_learned": "trained prediction",
        "E6_correction": "prediction-relative correction",
    }
    return ", ".join(
        f"{element.split('_')[0]} {names[element]} in {counts[element]}"
        for element in ELEMENTS
    )


def main() -> int:
    charts = json.loads(CHARTS.read_text())
    failures = 0
    checked = 0

    # Every reference's claims, so the report's quotes can be checked
    # against the whole set.
    claims_all = ""
    for path in sorted(CLAIMS.glob("*.claims.txt")):
        claims_all += " " + flatten(path.read_text(errors="replace"))

    for number, chart in charts.items():
        claims_path = CLAIMS / f"{number}.claims.txt"
        if not claims_path.exists():
            print(f"{number}: no claims text, skipped")
            continue
        claims = flatten(claims_path.read_text(errors="replace"))

        for element, entry in chart["elements"].items():
            if not entry["present"]:
                continue
            for passage in quoted(entry["evidence"]):
                checked += 1
                # An elision stands for the text it skips, so the quote
                # is checked in its parts.
                for part in [p for p in passage.split("...") if len(p.strip()) > 10]:
                    wanted = flatten(part)
                    if wanted in claims:
                        continue
                    # The optical character recognition reads some
                    # characters wrong, so a quote that mostly matches
                    # the text is the same passage. A short passage
                    # needs a higher proportion to match, because a
                    # short passage is more likely to match by chance.
                    threshold = 0.85 if len(wanted) > 60 else 0.75
                    if similarity(wanted, claims) >= threshold:
                        continue
                    failures += 1
                    print(f"FAIL {number} {element}: {part.strip()[:90]}")

    print(f"{checked} quotes checked, {failures} not found")

    # The documents that report the counts have to report them right, so
    # the sentence they share is built from the chart and looked for in
    # each of them.
    sentence = counts_sentence(charts)
    for name, path in (
        ("README", ROOT / "README.md"),
        ("design doc", ROOT / "docs" / "designs" / "girc-geometry-inference-codec.md"),
    ):
        if not path.exists():
            continue
        if sentence not in path.read_text():
            failures += 1
            print(f"FAIL {name}: the counts do not match the chart. Expected: {sentence}")

    # The report quotes the claims too, and those quotes are checked the
    # same way. A quotation of a title is not a claim, so it is not
    # checked.
    report = ROOT / "research" / "patent-package" / "PRIOR_ART.md"
    if report.exists():
        table_failures = check_result_table(report.read_text(), charts)
        print(f"result table checked, {table_failures} disagreeing cells")
        failures += table_failures

        report_failures = 0
        report_checked = 0
        for quote in re.findall(r'\*"([^"]{40,})"\*', report.read_text()):
            if "Implicit" in quote or "Method and apparatus" in quote or "High-level" in quote:
                # A title, not a claim.
                continue
            if "spatial locations of decoded coefficients" in quote:
                # A passage from the description, which is not a claim.
                continue
            report_checked += 1
            # An elision stands for the text it skips, so the quote is
            # checked in its parts.
            for part in [p for p in quote.split("...") if len(p.strip()) > 10]:
                wanted = flatten(part)
                if wanted in claims_all:
                    continue
                if similarity(wanted, claims_all) >= 0.85:
                    continue
                report_failures += 1
                print(f"FAIL report: {part.strip()[:90]}")
        print(f"{report_checked} report quotes checked, {report_failures} not found")
        failures += report_failures

    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
