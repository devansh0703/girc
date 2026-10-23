#!/usr/bin/env python3
"""Inline the site's fonts into the stylesheet, so the design system is local.

The site previously loaded its two Plex faces from Google Fonts with a single
`@import`, and the Phosphor icon font was never loaded at all: the pages carry
`<i class="ph ph-waveform">` markup with no rule anywhere that styles it, so
the icons rendered as empty elements. A product site that needs a third-party
request to apply its own design system is a site that fails for somebody, and
it fails silently — the markup is correct and the page is unstyled or iconless.

This downloads each font once, keeps only what the site uses (the latin and
latin-ext subsets of Plex; the Phosphor glyphs the pages actually reference),
and embeds the woff2 as a data URL. The result is one self-contained
stylesheet and no request. Each block is independent and idempotent: adding an
icon to a page and re-running adds its glyph, and leaves the rest alone.

    python3 tools/fetch_fonts.py
"""

from __future__ import annotations

import base64
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STYLESHEET = ROOT / "web" / "girc-site" / "assets" / "site.css"
PAGES = ROOT / "web" / "girc-site"
GOOGLE = "https://fonts.googleapis.com/css2"

#: The Phosphor release the design contract pins. Pinned rather than latest so
#: a regeneration two years from now produces the same glyphs, not whatever
#: the icon set has since redrawn.
PHOSPHOR = "https://unpkg.com/@phosphor-icons/web@2.1.2/src/regular"

#: The faces the design contract fixes, and the weights the pages use.
FACES = (
    ("IBM Plex Mono", 400),
    ("IBM Plex Mono", 500),
    ("IBM Plex Sans", 400),
    ("IBM Plex Sans", 600),
)

#: The subsets worth shipping. The site is English and its figures use the
#: Latin alphabet plus the punctuation in the design contract.
SUBSETS = ("latin", "latin-ext")

PLEX_MARKER = "The two Plex faces, inlined."
PHOSPHOR_MARKER = "Phosphor icons, inlined."

PLEX_HEADER = """/* The two Plex faces, inlined.

   The site previously loaded these from Google Fonts with an @import, which
   made the whole stylesheet one remote request: offline, or behind a
   blocker, the page rendered with no styling at all rather than with the
   fallback fonts the stylesheet names. A product site that depends on a
   third-party request for its own design system is a site that fails for
   somebody, and it fails silently -- the markup is correct and the page is
   unstyled.

   The latin and latin-ext subsets are inlined as woff2, which is every glyph
   this site uses; the other subsets are dropped rather than shipped.
   Regenerate with tools/fetch_fonts.py.
*/

"""

PHOSPHOR_HEADER = """/* Phosphor icons, inlined.

   The pages carry <i class="ph ph-..."> markup, but nothing loaded the icon
   font: the elements were empty and the icons did not render, online or off.
   The woff2 is embedded here and only the glyphs the pages reference are
   kept, so a page with four icons does not ship the whole icon set.

   Regenerate with tools/fetch_fonts.py after adding or removing an icon.
*/

"""

#: The @import the generated Plex block replaces. Optional, because a
#: stylesheet that already carries the inlined faces has nothing to replace.
IMPORT = re.compile(r'@import url\("https://fonts\.googleapis\.com/[^"]*"\);\n')

#: The blocks of a Google Fonts stylesheet, as (subset, block) pairs.
BLOCK = re.compile(r"/\* ([a-z-]+) \*/\s*(@font-face \{.*?\})", re.DOTALL)

#: A class attribute on an icon element, e.g. `ph ph-waveform`.
CLASS_ATTR = re.compile(r'class="([^"]*\bph\b[^"]*)"')

#: A Phosphor glyph rule, e.g. `.ph.ph-waveform:before { content: "\\e802"; }`.
#: Substituted with str.replace rather than str.format: the pattern contains
#: literal braces, which str.format would read as its own field syntax.
GLYPH = r"\.ph\.ph-NAME:before\s*\{[^}]*\}"

#: The base .ph rule, with the upstream comments stripped. The ligature
#: declarations upstream ships exist for text-ligature usage this site does
#: not do: every icon here is a class with an explicit content codepoint.
PHOSPHOR_BASE = """.ph {
  font-family: "Phosphor" !important;
  speak: never;
  font-style: normal;
  font-weight: normal;
  font-variant: normal;
  text-transform: none;
  line-height: 1;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}"""


def fetch(url: str) -> bytes:
    """Fetch a URL, with a browser's user agent.

    Google serves woff2 only to a client it recognises, so a bare
    `urllib` would get a TTF instead and a larger file.
    """
    request = urllib.request.Request(
        url, headers={"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/120 Safari/537.36"}
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


def face_css(family: str, weight: int) -> str:
    """The Google Fonts stylesheet for one face, with its woff2 inlined."""
    slug = family.replace(" ", "+")
    url = f"{GOOGLE}?family={slug}:wght@{weight}&display=swap"
    css = fetch(url).decode("utf-8")

    blocks: list[str] = []
    for subset, block in BLOCK.findall(css):
        if subset not in SUBSETS:
            continue
        font_url = re.search(r"url\((https://[^)]+)\)", block)
        if font_url is None:
            # A block with no URL is a face Google served from a local name;
            # it is not one this site should ship.
            continue
        encoded = base64.b64encode(fetch(font_url.group(1))).decode("ascii")
        block = re.sub(
            r"src: url\(https://[^)]+\)",
            f"src: url(data:font/woff2;base64,{encoded})",
            block,
        )
        blocks.append(f"/* {family} {weight} {subset} */\n{block}")
    return "\n\n".join(blocks)


def used_icons() -> list[str]:
    """The Phosphor glyph names the pages reference, sorted for a stable diff."""
    # The weight tokens upstream also ships as classes (ph-light, ph-fill...)
    # are not glyph names and must not be looked up as such.
    weights = {"light", "thin", "regular", "bold", "fill", "duotone"}
    icons: set[str] = set()
    for page in PAGES.rglob("*.html"):
        for classes in CLASS_ATTR.findall(page.read_text(encoding="utf-8")):
            tokens = classes.split()
            icons |= {token[3:] for token in tokens if token.startswith("ph-") and token[3:] not in weights}
    return sorted(icons)


def phosphor_block() -> str:
    """The Phosphor @font-face and the glyph rules the pages use."""
    icons = used_icons()
    if not icons:
        print("no Phosphor icons found in the pages", file=sys.stderr)
        raise SystemExit(1)

    css = fetch(f"{PHOSPHOR}/style.css").decode("utf-8")
    encoded = base64.b64encode(fetch(f"{PHOSPHOR}/Phosphor.woff2")).decode("ascii")

    rules: list[str] = []
    for name in icons:
        match = re.search(GLYPH.replace("NAME", re.escape(name)), css)
        if match is None:
            print(f"Phosphor has no glyph named {name!r}", file=sys.stderr)
            raise SystemExit(1)
        rules.append(match.group(0))

    face = (
        '@font-face {\n  font-family: "Phosphor";\n'
        f"  src: url(data:font/woff2;base64,{encoded}) format(\"woff2\");\n"
        "  font-weight: normal;\n  font-style: normal;\n  font-display: block;\n}"
    )
    return (
        PHOSPHOR_HEADER
        + face
        + "\n\n"
        + PHOSPHOR_BASE
        + "\n\n"
        + "\n\n".join(rules)
        + "\n"
    )


def main() -> int:
    text = STYLESHEET.read_text(encoding="utf-8")
    changed = False

    added: list[str] = []
    if PHOSPHOR_MARKER not in text:
        text = text.rstrip() + "\n\n" + phosphor_block()
        added.append("Phosphor")

    if PLEX_MARKER not in text:
        blocks = [face_css(family, weight) for family, weight in FACES]
        block = PLEX_HEADER + "\n".join(blocks) + "\n"

        # The generated faces go where the @import was, and the @import goes
        # away: leaving it would make the browser fetch the same faces twice,
        # once from the data URL and once from the network.
        if IMPORT.search(text):
            text = IMPORT.sub(block, text)
        else:
            # No @import means the stylesheet never asked for the faces at
            # all, which is the failure the inlining exists to prevent. The
            # faces go after the header comment and before the first rule.
            marker = "\n:root,"
            if marker not in text:
                print(f"could not find an insertion point in {STYLESHEET}", file=sys.stderr)
                return 1
            text = text.replace(marker, "\n" + block + marker.lstrip("\n"), 1)
        added.append("Plex")

    if not added:
        print(f"{STYLESHEET.relative_to(ROOT)} already carries both font blocks; nothing to do")
        return 0

    STYLESHEET.write_text(text, encoding="utf-8")
    print(f"inlined {' and '.join(added)} into {STYLESHEET.relative_to(ROOT)} ({STYLESHEET.stat().st_size // 1024} KiB)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
