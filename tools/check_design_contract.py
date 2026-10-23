#!/usr/bin/env python3
"""Check the site against the Wire Terminal design contract.

The contract is written down in the site's own stylesheet. A violation is a
rule that has drifted from the design, and the cheapest way to stop the drift
is to fail the build when it happens.

Run: python3 tools/check_design_contract.py
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SITE = ROOT / "web" / "girc-site"
STYLESHEET = SITE / "assets" / "site.css"

# Each rule is a pattern that must NOT appear in a stylesheet or a page. The
# reason is carried alongside so a failure explains itself.
BANNED: list[tuple[str, str, str]] = [
    (
        "gradient",
        r"gradient\(",
        "the contract bans gradients; elevation comes from borders and contrast",
    ),
    (
        "glassmorphism",
        r"backdrop-filter|backdrop-blur",
        "the contract bans glassmorphism",
    ),
    (
        "uppercase",
        r"text-transform:\s*uppercase",
        "the contract bans an all-caps mono kicker above a heading",
    ),
    (
        "coloured left border",
        r"border-left:\s*\d+px",
        "the contract bans coloured left borders as an elevation cue",
    ),
    (
        "pulsing dot",
        r"@keyframes\s+(pulse|blink)",
        "the contract bans pulsing dots",
    ),
    (
        "soft shadow",
        r"box-shadow:\s*(?!none)[^;]*",
        "the contract bans soft shadows; elevation comes from borders",
    ),
    (
        "oversized radius",
        r"border-radius:\s*(?:[5-9]|[1-9][0-9]+)px",
        "the contract fixes the radius at 3px",
    ),
    (
        "emoji icon",
        r"[\U0001F300-\U0001FAFF]",
        "the contract uses Phosphor icons, not emoji",
    ),
    (
        "lucide",
        r"lucide",
        "the contract uses Phosphor icons, not Lucide",
    ),
    (
        "inter or geist font",
        r"['\"]?(Inter|Geist|Space Grotesk)['\"]?\s*,",
        "the contract uses IBM Plex Mono and IBM Plex Sans only",
    ),
    (
        "dark-by-default document",
        r"<html[^>]*data-theme=['\"]dark",
        "light is the default theme; dark is applied by the inline script after "
        "the reader's stored choice and the OS preference are read",
    ),
    (
        "hardcoded theme colour outside the palette",
        r"(?:fill|stroke|background|color):\s*#(?:[0-9a-f]{6})",
        "colours come from the custom properties in site.css, so that both "
        "palettes stay in one place",
    ),
    (
        "a chart with no accessible name",
        r"<svg(?![^>]*aria-label)[^>]*>",
        "every chart carries an aria-label saying what it shows",
    ),
]

# The colours the contract bans outright.
BANNED_COLOURS = ["#8b5cf6", "#14b8a6", "#3b82f6", "#6366f1", "#06b6d4", "#0ea5e9"]

# The places a hex literal is legitimate: the palette that defines it, and the
# inline SVG favicon, where a data URL has to carry its own colour.
ALLOWED = {STYLESHEET}


def collect_files() -> list[pathlib.Path]:
    """Every page and every stylesheet, at any depth.

    Recursive on purpose: the site has pages in subdirectories and figures in
    a subdirectory of assets, and a non-recursive glob checked four files out of
    twenty. It reported clean while most of the site was unchecked.
    """
    files: list[pathlib.Path] = []
    for pattern in ("**/*.css", "**/*.html"):
        files.extend(sorted(SITE.glob(pattern)))
    return [f for f in files if f.is_file()]


def inline_svg(line: str) -> bool:
    """Whether a line is a data-URL favicon, which must carry its own colour."""
    return "data:image/svg+xml" in line


def main() -> int:
    failures: list[str] = []
    checked = 0

    files = collect_files()
    if not files:
        print(f"DESIGN CONTRACT: no files found under {SITE}; the paths moved")
        return 1

    for path in files:
        rel = path.relative_to(ROOT)
        text = path.read_text(encoding="utf-8")
        lines = text.split("\n")
        checked += 1

        for name, pattern, reason in BANNED:
            for index, line in enumerate(lines, 1):
                # The palette file is where a hex literal is the definition
                # rather than an escape from the tokens.
                if name == "hardcoded theme colour outside the palette" and path in ALLOWED:
                    continue
                # The generated figures carry their own fill attributes, and
                # the generator is the one place a colour is a value rather
                # than a token reference.
                if "assets/figures" in str(rel):
                    continue
                # A data-URL favicon is a self-contained SVG and has to carry
                # its own colours; it is not part of the page's palette.
                if inline_svg(line):
                    continue
                if re.search(pattern, line, re.IGNORECASE):
                    failures.append(
                        f"{rel}:{index}: the {name} rule was broken. {reason}.\n"
                        f"    {line.strip()[:100]}"
                    )

        for colour in BANNED_COLOURS:
            for index, line in enumerate(lines, 1):
                if colour.lower() in line.lower():
                    failures.append(
                        f"{rel}:{index}: the colour {colour} is banned by the contract.\n"
                        f"    {line.strip()[:100]}"
                    )

    # The fonts and the icon set are loaded once, in the stylesheet every page
    # uses, so the rule is checked where they are declared rather than repeated
    # on every page.
    if not STYLESHEET.exists():
        failures.append("no stylesheet at assets/site.css; the design system has nowhere to live")
    else:
        text = STYLESHEET.read_text(encoding="utf-8")
        if "IBM Plex" not in text:
            failures.append("assets/site.css: the stylesheet does not declare the two Plex faces")
        if "--radius: 3px" not in text:
            failures.append("assets/site.css: the contract fixes the radius at 3px")
        if "data-theme='light'" not in text and 'data-theme="light"' not in text:
            failures.append("assets/site.css: the contract makes light the default theme")

    if failures:
        print(f"DESIGN CONTRACT: {len(failures)} violation(s) in {checked} file(s)\n")
        for failure in failures:
            print(f"  {failure}")
        return 1

    print(f"DESIGN CONTRACT: clean across {checked} file(s), the design system in one stylesheet")
    return 0


if __name__ == "__main__":
    sys.exit(main())
