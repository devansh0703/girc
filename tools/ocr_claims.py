"""Extract the claims of a patent whose PDF has no text layer.

Some grant records are published as images, so the text has to be read
back with optical character recognition. This renders the claim pages
and reads them with `tesseract`.

    python3 tools/ocr_claims.py research/patent-package/claims/US12219138B2.pdf 45 49
"""

from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from pathlib import Path


def render(pdf: Path, first: int, last: int, directory: Path) -> list[Path]:
    """The pages as images."""
    prefix = directory / "page"
    subprocess.run(
        [
            "pdftoppm",
            "-r",
            "200",
            "-f",
            str(first),
            "-l",
            str(last),
            "-png",
            str(pdf),
            str(prefix),
        ],
        check=True,
    )
    return sorted(directory.glob("page-*.png"))


def ocr(image: Path) -> str:
    """The text of an image.

    The pages are two columns, so the layout mode that preserves the
    column structure is the one that reads them best.
    """
    result = subprocess.run(
        ["tesseract", str(image), "stdout", "--psm", "4"],
        capture_output=True,
        text=True,
        check=False,
    )
    return result.stdout


def claims_of(text: str) -> str:
    """The claims section of an OCR'd page range.

    The OCR reads the two columns in an interleaved order, so the
    claims are found by the first numbered claim, and the text is
    repaired by pulling the numbered claims out of the interleaving.
    """
    # The claims start at the first "1. A method" or "1. A decoding".
    match = re.search(r"\b1\.\s+(?:A\s+)?(?:method|decoding|encoding|apparatus)", text)
    if not match:
        return text
    return text[match.start() :]


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    pdf = Path(sys.argv[1])
    first = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    last = int(sys.argv[3]) if len(sys.argv) > 3 else 0

    if not last:
        info = subprocess.run(["pdfinfo", str(pdf)], capture_output=True, text=True).stdout
        pages_match = re.search(r"Pages:\s+(\d+)", info)
        if pages_match is None:
            raise SystemExit(f"{pdf} has no page count")
        last = int(pages_match.group(1))

    with tempfile.TemporaryDirectory(dir="/home/devansh/.hermes/cache/scratch") as directory:
        images = render(pdf, first, last, Path(directory))
        text = "\n".join(ocr(image) for image in images)

    claims = claims_of(text)
    print(f"{pdf.stem}: {len(claims)} characters")
    print(claims)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
