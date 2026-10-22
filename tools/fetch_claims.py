"""Build the claim charts for the prior-art analysis.

Anticipation is judged on the claims, not the description, so the
analysis needs the claim text of every reference. This script fetches
each publication's page to find its PDF, extracts the claims from the
PDF, and records which elements of the disclosure the claims disclose.

The results are cached, so a re-run is cheap.

    python3 tools/fetch_claims.py --all
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / "research" / "patent-package" / "claims"
CACHE.mkdir(parents=True, exist_ok=True)

#: The publications the analysis covers, with the title each one carries.
PUBLICATIONS = {
    "US11297348B2": "Implicit transform settings for coding a block of pixels (MediaTek)",
    "EP3977732B1": "Implicit multiple transform selection (InterDigital)",
    "US11785254B2": "Method and apparatus for transform type determination (Qualcomm)",
    "HK40058354A": "Method and apparatus for transform type determination (Qualcomm)",
    "US11134275B2": "Implicit transform type signalling (Qualcomm)",
    "AU2023202827B2": "Implicit transform type signalling (Qualcomm)",
    "US20230086769A1": "Implicit transform type signalling (Qualcomm)",
    "US12219138B2": "Method and apparatus for implicit transform selection (InterDigital)",
    "US12143605B2": "Method and apparatus for transform type determination (Samsung)",
    "HK40117489A": "Method and apparatus for transform type determination (Samsung)",
    "WO2026122348A1": "Method and apparatus for transform type determination (Samsung)",
    "EP4661397A1": "Method and apparatus for transform type determination (Samsung)",
    "US11363302B2": "Neural post-filter for video coding (MediaTek), not transform art",
}

#: The elements of the disclosure, as they would appear in a claim.
#:
#: E1  the transform is derived implicitly, without explicit signalling
#: E2  the derivation reads the geometry of a neighbourhood of reconstructed
#:     samples
#: E3  the horizontal and the vertical axis are predicted separately
#: E4  the encoder may take a less optimal transform, at a rate-distortion
#:     cost, to avoid signalling the correction
#: E5  the prediction comes from a trained or learned model
#: E6  a correction is signalled relative to a prediction
ELEMENTS = {
    "E1_implicit": (
        r"implicit(?:ly)?\s+(?:deriv|select|determin|obtain|indicat|signal)"
        r"|without\s+(?:any\s+)?(?:explicit\s+)?signal"
        r"|no\s+explicit\s+signal"
    ),
    "E2_geometry": (
        r"\bgradient"
        r"|edge\s+(?:direction|orientation)"
        r"|orientation\s+of\s+(?:the\s+)?(?:edge|gradient)"
        r"|neighbou?ring\s+(?:block|sample|pixel)s?"
        r"|prediction\s+neighbou?rhood"
    ),
    "E3_per_axis": (
        r"(?:separate|independent|each|first|horizontal)"
        r".{0,120}(?:horizontal|vertical)"
        r".{0,160}(?:predict|train|model|estimat|determin)"
    ),
    "E4_margin": (
        r"(?:margin|sub-?optimal|second[\s-]best|not\s+the\s+optimal|give\s+up|sacrific)"
        r".{0,200}(?:distortion|rate|bit|cost|quality)"
        r"|(?:distortion|rate|cost|quality)"
        r".{0,200}(?:margin|give\s+up|sub-?optimal|second[\s-]best)"
    ),
    "E5_learned": (
        r"neural\s+network|machine\s+learning|logistic\s+regression"
        r"|\btrain(?:ed|ing)\b|\blearn(?:ed|ing)\b|gradient\s+boost"
    ),
    "E6_correction": (
        r"predict(?:ed|ion)\s+(?:transform|mode|type)"
        r"|\bmismatch\b|\boverride\b"
        r"|differ(?:s|ent)\s+from\s+the\s+predict"
        r"|correct(?:ion|ive)"
    ),
}


def page_for(number: str) -> str:
    return f"https://patents.google.com/patent/{number}/en"


def pdf_link(page: str) -> str | None:
    """The PDF link on a Google Patents page."""
    match = re.search(r"https://patentimages\.storage\.googleapis\.com/\S+?\.pdf", page)
    return match.group(0) if match else None


def download(url: str, destination: Path) -> bool:
    if destination.exists() and destination.stat().st_size > 5000:
        return True
    request = urllib.request.Request(
        url,
        headers={
            "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
            "(KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
            "Accept": "*/*",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            data = response.read()
    except Exception as error:  # noqa: BLE001 - the caller reports it
        print(f"  download failed: {error}", file=sys.stderr)
        return False
    if len(data) < 5000:
        print("  download too small, probably a block page", file=sys.stderr)
        return False
    destination.write_bytes(data)
    return True


def pdf_text(path: Path) -> str:
    """The text of a PDF."""
    result = subprocess.run(
        ["pdftotext", "-layout", "-nopgbrk", str(path), "-"],
        capture_output=True,
        text=True,
        check=False,
    )
    return result.stdout


def claims_from(text: str) -> str:
    """The claims section of a patent's text.

    The claims start at the last "What is claimed", and run to the end
    of the document. Some documents have a trailing section after the
    claims, so the search is for the last marker rather than the first.
    """
    markers = [
        r"What is claimed is:",
        r"What is claimed:",
        r"^\s*Claims\s*$",
        r"^\s*CLAIMS\s*$",
        r"^\s*We claim:",
    ]
    starts = []
    for marker in markers:
        for match in re.finditer(marker, text, re.M | re.I):
            starts.append(match.start())
    if not starts:
        return ""
    return text[max(starts) :]


def classify(claims: str) -> dict:
    """Which elements of the disclosure the claims disclose."""
    flat = re.sub(r"\s+", " ", claims)
    found = {}
    for name, pattern in ELEMENTS.items():
        match = re.search(pattern, flat, re.I)
        found[name] = {
            "present": bool(match),
            "evidence": flat[max(0, match.start() - 140) : match.end() + 140].strip()
            if match
            else "",
        }
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("publications", nargs="*", help="publication numbers")
    parser.add_argument("--all", action="store_true", help="every known publication")
    parser.add_argument(
        "--out",
        default=str(ROOT / "research" / "patent-package" / "claim_charts.json"),
        help="where to write the claim chart",
    )
    args = parser.parse_args()

    wanted = list(PUBLICATIONS) if args.all else args.publications
    if not wanted:
        parser.error("give publication numbers or --all")

    results = {}
    for number in wanted:
        print(f"{number}:")
        pdf = CACHE / f"{number}.pdf"
        text_path = CACHE / f"{number}.txt"

        if not pdf.exists() or pdf.stat().st_size < 5000:
            # The page gives the PDF's address, which is a hash and so
            # cannot be derived.
            from hermes_tools import web_extract

            fetched = web_extract([page_for(number)], char_limit=8000)
            entry = fetched["results"][0]
            link = pdf_link(entry.get("content") or "")
            if not link:
                results[number] = {
                    "title": PUBLICATIONS.get(number, ""),
                    "error": "the page gave no PDF link",
                }
                print("  no PDF link")
                continue
            if not download(link, pdf):
                results[number] = {
                    "title": PUBLICATIONS.get(number, ""),
                    "error": "the PDF could not be downloaded",
                }
                print("  download failed")
                continue
            time.sleep(2)

        if not text_path.exists():
            text_path.write_text(pdf_text(pdf))

        text = text_path.read_text(errors="replace")
        claims = claims_from(text)
        results[number] = {
            "title": PUBLICATIONS.get(number, ""),
            "pdf": str(pdf.relative_to(ROOT)),
            "characters": len(claims),
            "elements": classify(claims) if claims else {},
            "claims": claims[:8000],
        }
        present = [k for k, v in results[number].get("elements", {}).items() if v["present"]]
        print(f"  {len(claims)} characters of claims; elements present: {present}")

    Path(args.out).write_text(json.dumps(results, indent=2))
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
