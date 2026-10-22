"""Classify a reference against the elements of the disclosure.

Anticipation is judged on the claims, not the description, so this
script reads the claims of each reference and records which elements of
the disclosure they disclose. It caches the fetched pages, so a re-run
is cheap and the rate limit is survivable.

The elements are:

E1  the transform is derived implicitly, without explicit signalling
E2  the derivation reads the geometry of a neighbourhood of reconstructed
    samples
E3  the horizontal and the vertical axis are predicted separately
E4  the encoder may take a less optimal transform, at a rate-distortion
    cost, to avoid signalling the correction
E5  the prediction comes from a trained or learned model
E6  a correction is signalled relative to a prediction
"""

from __future__ import annotations

import hashlib
import html as htmlmod
import json
import re
import sys
import time
import urllib.request
from pathlib import Path

CACHE = Path("/home/devansh/.hermes/cache/scratch/patents")
CACHE.mkdir(parents=True, exist_ok=True)

#: The elements, as they would appear in a claim.
ELEMENTS = {
    "E1_implicit": (
        r"implicit(?:ly)? (?:deriv|select|determin|obtain|indicat)|"
        r"without (?:any )?(?:explicit )?signal"
    ),
    "E2_geometry": (
        r"(?:gradient|orientation|edge (?:direction|orientation)|"
        r"neighbou?ring (?:block|sample|pixel))"
    ),
    "E3_per_axis": (
        r"(?:first|separate|independent|each).{0,80}(?:horizontal|vertical)"
        r".{0,140}(?:predict|train|model|estimat|determin)"
    ),
    "E4_margin": (
        r"(?:margin|sub-?optimal|second best|not the optimal|give up|sacrific)"
        r".{0,160}(?:distortion|rate|bit|cost)"
        r"|(?:distortion|rate|cost).{0,160}"
        r"(?:margin|give up|sub-?optimal|second best)"
    ),
    "E5_learned": (
        r"(?:neural network|machine learning|logistic regression|"
        r"trained|learned)"
    ),
    "E6_correction": (
        r"predict(?:ed|ion) (?:transform|mode)|mismatch|override|"
        r"differ(?:s|ent) from the predict"
    ),
}


def fetch(url: str, tries: int = 5) -> str:
    """Fetch a page, caching it, with a backoff for the rate limit."""
    key = hashlib.sha256(url.encode()).hexdigest()[:24]
    path = CACHE / f"{key}.html"
    if path.exists() and path.stat().st_size > 5000:
        return path.read_text(encoding="utf-8", errors="replace")
    last: Exception | None = None
    for attempt in range(tries):
        try:
            request = urllib.request.Request(
                url, headers={"User-Agent": "Mozilla/5.0 (X11; Linux x86_64)"}
            )
            with urllib.request.urlopen(request, timeout=90) as response:
                raw = response.read().decode("utf-8", "replace")
            path.write_text(raw, encoding="utf-8")
            return raw
        except Exception as error:  # noqa: BLE001
            last = error
            time.sleep(3 * (attempt + 1))
    raise last  # type: ignore[misc]


def claims_of(reference: str) -> list[str]:
    """The claim text of a reference, one entry per claim."""
    raw = fetch(f"https://patents.google.com/patent/{reference}/en")
    start = raw.find('itemprop="claims"')
    if start < 0:
        return []
    segment = raw[start : start + 250000]
    texts = re.findall(r'<div class="claim-text">(.*?)</div>', segment, re.S)
    claims = []
    for text in texts:
        text = htmlmod.unescape(re.sub(r"<[^>]+>", " ", text))
        text = re.sub(r"\s+", " ", text).strip()
        if text:
            claims.append(text)
    return claims


def title_of(raw: str) -> str:
    match = re.search(r'<meta name="DC.title" content="([^"]+)"', raw)
    return match.group(1) if match else ""


def main() -> int:
    references = sys.argv[1:]
    if not references:
        print("usage: classify_prior_art.py <reference>...", file=sys.stderr)
        return 2

    matrix: dict[str, dict] = {}
    for reference in references:
        try:
            claims = claims_of(reference)
        except Exception as error:  # noqa: BLE001
            print(f"{reference:20s} FAILED {error}")
            continue
        if not claims:
            print(f"{reference:20s} no claims section")
            continue
        joined = " ".join(claims)
        hits = {
            name: bool(re.search(pattern, joined, re.I))
            for name, pattern in ELEMENTS.items()
        }
        matrix[reference] = {
            "claims": len(claims),
            "claim_1": claims[0][:500],
            "elements": hits,
        }
        marks = " ".join(f"{('Y' if hits[k] else '.'):>13s}" for k in ELEMENTS)
        print(f"{reference:20s} {marks}   ({len(claims)} claims)")
        time.sleep(1.0)

    destination = Path("research/patent-package/prior_art_matrix.json")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(matrix, indent=2) + "\n", encoding="utf-8")
    print(f"\nwrote {destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
