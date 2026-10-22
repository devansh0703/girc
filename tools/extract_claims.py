"""Extract the claims of a patent publication from its PDF.

Anticipation is judged on the claims, not the description, so the
analysis needs the claim text of every reference. A patent PDF is
usually two columns, and the claims sit in one of them, so the words
are read with their positions and only the claims column is kept.

    python3 tools/extract_claims.py research/patent-package/claims/US11297348B2.pdf
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

#: A word, with where it sits on the page.
WORD = re.compile(
    r'<word xMin="(?P<xmin>[\d.]+)" yMin="(?P<ymin>[\d.]+)"'
    r' xMax="(?P<xmax>[\d.]+)" yMax="(?P<ymax>[\d.]+)">(?P<text>[^<]*)</word>'
)

#: A numbered claim, which is how the claims column is recognised.
CLAIM_START = re.compile(
    r"^\d+\.\s*(?:A\s*|The\s*)?(?:method|decoding|encoding|apparatus|computer|system|"
    r"media|storage|non-transitory)",
    re.I,
)


def page_words(pdf: Path, page: int) -> list[dict]:
    """The words of a page, with their positions."""
    result = subprocess.run(
        [
            "pdftotext",
            "-bbox-layout",
            "-f",
            str(page),
            "-l",
            str(page),
            str(pdf),
            "-",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    found = []
    for match in WORD.finditer(result.stdout):
        found.append(
            {
                "xmin": float(match.group("xmin")),
                "ymin": float(match.group("ymin")),
                "xmax": float(match.group("xmax")),
                "ymax": float(match.group("ymax")),
                "text": match.group("text"),
            }
        )
    return found


def page_number(pdf: Path) -> int:
    info = subprocess.run(["pdfinfo", str(pdf)], capture_output=True, text=True).stdout
    match = re.search(r"Pages:\s+(\d+)", info)
    if match is None:
        raise SystemExit(f"{pdf} has no page count")
    return int(match.group(1))


def claims_column(pdf: Path, pages: int) -> tuple[float, float] | None:
    """The x range of the column the claims are in.

    The claims are recognised by their numbered claims, and the column
    they are in is the column those words are in. The number and the
    first words of the claim are separate words in the PDF, so the
    sequence is what is matched.
    """
    for page in range(1, pages + 1):
        words = page_words(pdf, page)
        for index, word in enumerate(words):
            if not re.fullmatch(r"\d+\.", word["text"]):
                continue
            following = [w["text"] for w in words[index + 1 : index + 6]]
            if not following:
                continue
            joined = " ".join(following)
            if not re.match(
                r"^(?:A\s+)?(?:method|decoding|encoding|apparatus|computer|system|"
                r"media|storage|non-transitory)",
                joined,
                re.I,
            ):
                continue
            run = words[index : index + 12]
            left = word["xmin"]
            # The column runs from the claim's text to the right margin,
            # which the widest word on the page gives.
            right = max(w["xmax"] for w in words)
            return (left, right)
    return None


def column_text(words: list[dict], left: float, right: float) -> str:
    """The text of one column, line by line."""
    selected = [w for w in words if left - 2 <= w["xmin"] <= right + 2]
    if not selected:
        return ""
    selected.sort(key=lambda w: (round(w["ymin"], 1), w["xmin"]))
    lines = []
    current: list[dict] = []
    current_y = None
    for word in selected:
        if current_y is None or abs(word["ymin"] - current_y) < 3.0:
            current.append(word)
            current_y = word["ymin"] if current_y is None else current_y
        else:
            lines.append(" ".join(w["text"] for w in current))
            current = [word]
            current_y = word["ymin"]
    if current:
        lines.append(" ".join(w["text"] for w in current))
    return "\n".join(lines)


def claims_of(pdf: Path) -> str:
    """The claims of a patent, as text.

    The claims start at the last "What is claimed", and run to the end
    of the document. Some documents have a trailing section after the
    claims, so the search is for the last marker rather than the first.
    """
    pages = page_number(pdf)
    column = claims_column(pdf, pages)
    if column is None:
        return ""
    left, right = column

    text = "\n".join(
        column_text(page_words(pdf, page), left, right) for page in range(1, pages + 1)
    )

    markers = [
        r"What is claimed is\s*:",
        r"What is claimed\s*:",
        r"^\s*Claims\s*$",
        r"^\s*We claim:",
    ]
    starts = []
    for marker in markers:
        for match in re.finditer(marker, text, re.M | re.I):
            starts.append(match.start())
    if not starts:
        return ""
    return text[max(starts) :]


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    for argument in sys.argv[1:]:
        pdf = Path(argument)
        claims = claims_of(pdf)
        # The claims go next to the PDF, so the claim chart can read them.
        destination = pdf.with_suffix("").with_suffix(".claims.txt")
        destination.write_text(claims)
        print(f"{pdf.stem}: {len(claims)} characters of claims -> {destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
