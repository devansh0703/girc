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

#: A block of text, with where it sits on the page.
BLOCK = re.compile(
    r'<block xMin="(?P<xmin>[\d.]+)" yMin="(?P<ymin>[\d.]+)"'
    r' xMax="(?P<xmax>[\d.]+)" yMax="(?P<ymax>[\d.]+)">(?P<body>.*?)</block>',
    re.S,
)
BLOCK_WORD = re.compile(r"<word[^>]*>(?P<text>[^<]*)</word>")

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


def page_blocks(pdf: Path, page: int) -> list[dict]:
    """The blocks of a page, with the text of each.

    A block is a line of one column, so the blocks give the columns
    directly where the words' positions do not.
    """
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
    for match in BLOCK.finditer(result.stdout):
        words = [w.group("text") for w in WORD.finditer(match.group("body"))]
        found.append(
            {
                "xmin": float(match.group("xmin")),
                "ymin": float(match.group("ymin")),
                "xmax": float(match.group("xmax")),
                "text": " ".join(words),
            }
        )
    return found


def page_number(pdf: Path) -> int:
    info = subprocess.run(["pdfinfo", str(pdf)], capture_output=True, text=True).stdout
    match = re.search(r"Pages:\s+(\d+)", info)
    if match is None:
        raise SystemExit(f"{pdf} has no page count")
    return int(match.group(1))


def gutter(words: list[dict]) -> float | None:
    """The x of the gutter between the two columns of a page.

    The gutter is the widest band of x that no word's middle falls in,
    which is where the two columns of a patent page meet.
    """
    if not words:
        return None
    middles = [(w["xmin"] + w["xmax"]) / 2 for w in words]
    low = min(middles)
    high = max(middles)
    bucket = 4.0
    occupied = {int(m // bucket) for m in middles}

    best = None
    index = int(low // bucket)
    while index <= int(high // bucket):
        if index not in occupied:
            start = index
            while index + 1 not in occupied and index + 1 <= int(high // bucket):
                index += 1
            span = (index - start) * bucket
            if best is None or span > best:
                best = span
                best_start = start
        index += 1

    if best is None or best < 12.0:
        return None
    return (best_start + best / bucket) * bucket


def claims_column(pdf: Path, pages: int) -> tuple[float, float] | None:
    """The x range of the column the claims are in.

    The claims are recognised by their numbered claims, and the column
    they are in is the column those words are in. The number and the
    first words of the claim are separate words in the PDF, so the
    sequence is what is matched.
    """
    for page in range(1, pages + 1):
        words = page_words(pdf, page)
        split = gutter(words)
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
            if split is None:
                # A single-column page: the claims run to the margin.
                return (word["xmin"], max(w["xmax"] for w in words))
            # The claims are in the column the claim's number is in.
            if word["xmin"] < split:
                return (0.0, split)
            return (split, max(w["xmax"] for w in words))
    return None


def claims_column_from_blocks(pdf: Path, pages: int) -> tuple[float, float] | None:
    """The x range of the claims column, from the page's blocks.

    Some PDFs have a layout that the word positions do not separate
    into columns, but whose blocks do.
    """
    for page in range(1, pages + 1):
        blocks = page_blocks(pdf, page)
        if not blocks:
            continue
        for index, block in enumerate(blocks):
            # The block may hold the claim's number on its own, or the
            # number and the claim's first words together.
            text = block["text"]
            if not re.match(r"^\d+\.\s", text):
                continue
            following = [
                other["text"]
                for other in blocks[index + 1 : index + 6]
                if abs(other["xmin"] - block["xmin"]) < 20
            ]
            candidate = text + " " + " ".join(following)
            if not re.search(
                r"\d+\.\s*(?:A|An|The)?\s*(?:method|decoding|encoding|apparatus|"
                r"computer|system|media|storage)",
                candidate,
                re.I,
            ):
                continue
            # The column runs to the gutter. The gutter is the gap in
            # the blocks' starting positions that no block crosses, so
            # it is the widest gap in the middle of the page.
            starts = sorted({round(b["xmin"], 1) for b in blocks})
            best = None
            for position in range(1, len(starts)):
                gap = starts[position] - starts[position - 1]
                if gap < 10.0:
                    continue
                # The gutter is between the two halves of the page.
                middle = (starts[position - 1] + starts[position]) / 2
                if middle < 0.4 * max(starts) or middle > 0.6 * max(starts):
                    continue
                if best is None or gap > best[0]:
                    best = (gap, starts[position - 1])
            if best is None:
                right = max(b["xmax"] for b in blocks)
            else:
                right = best[1]
            return (block["xmin"], right)
    return None


def block_text(blocks: list[dict], left: float, right: float) -> str:
    """The text of the claims column, in reading order."""
    selected = [b for b in blocks if left - 2 <= b["xmin"] < right]
    if not selected:
        return ""
    selected.sort(key=lambda b: (round(b["ymin"], 1), b["xmin"]))
    return "\n".join(b["text"] for b in selected)


def column_text(words: list[dict], left: float, right: float) -> str:
    """The text of one column, line by line."""
    selected = [w for w in words if left <= (w["xmin"] + w["xmax"]) / 2 < right]
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

    # The block route is tried first, because a block is a line of one
    # column and the blocks separate the columns where the words'
    # positions do not.
    block_column = claims_column_from_blocks(pdf, pages)
    word_column = claims_column(pdf, pages)
    if block_column is not None and word_column is None:
        left, right = block_column
        text = "\n".join(
            block_text(page_blocks(pdf, page), left, right) for page in range(1, pages + 1)
        )
    elif word_column is not None:
        left, right = word_column
        text = "\n".join(
            column_text(page_words(pdf, page), left, right) for page in range(1, pages + 1)
        )
    elif block_column is not None:
        left, right = block_column
        text = "\n".join(
            block_text(page_blocks(pdf, page), left, right) for page in range(1, pages + 1)
        )
    else:
        return ""

    markers = [
        r"What is claimed is\s*:",
        r"What is claimed\s*:",
        r"^\s*Claims\s*$",
        r"^\s*We claim:",
        # Some patents have no marker at all, and the claims start at the
        # first numbered claim.
        r"(?m)^\s*1\.\s+(?:A|An|The)\s+(?:method|decoding|encoding|apparatus|computer|system)\b",
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
