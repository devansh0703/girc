#!/usr/bin/env python3
"""Assemble the site: inline the figures, check the contract, check the links.

The site's pages carry a ``{{FIGURE:name}}`` token where a figure belongs.
This script reads each SVG from ``assets/figures/`` and substitutes its markup,
so a page cannot carry a figure the generator did not produce and a figure
cannot exist that no page carries.

Three checks run here, because they are the checks the pages cannot do on
themselves:

* every ``{{FIGURE:name}}`` token resolves to a generated SVG, and every
  generated SVG is carried by a page -- a figure nobody shows is dead weight;
* no page carries a placeholder, an invented address, or a dark default;
* every page carries at least two figures, which is the site's own rule.

    python3 tools/assemble_site.py
"""

from __future__ import annotations

import argparse
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SITE = ROOT / "web" / "girc-site"
FIGURES = SITE / "assets" / "figures"
TOKEN = re.compile(r"\{\{FIGURE:([a-z0-9-]+)\}\}")

#: The portal's address on Render. A repository setting, not a secret.
PORTAL_URL = "https://girc-portal.onrender.com"

#: Words and patterns that must not appear on a product site. Each carries the
#: reason, so a failure explains itself.
BANNED: list[tuple[str, str]] = [
    (r"example\.com", "a placeholder domain"),
    (r"example\.org", "a placeholder domain"),
    (r"@example\.", "a placeholder address"),
    (r"\.invalid", "a reserved placeholder domain"),
    (r"lorem ipsum", "placeholder prose"),
    (r"coming soon", "a page that promises rather than shows"),
    (r"TODO|FIXME", "an unfinished marker"),
    (r'<html[^>]*data-theme="dark"', "a dark default; light is the default theme"),
]

#: The pages that must name the real contact detail.
CONTACT_PAGES = ("index.html", "about.html", "service.html", "evaluate/index.html")
CONTACT = "devanshraulo@gmail.com"


def substitute(text: str, figures: dict[str, str]) -> tuple[str, list[str]]:
    """Replace every figure token, and report the ones that did not resolve."""
    missing: list[str] = []

    def replace(match: re.Match[str]) -> str:
        name = match.group(1)
        if name not in figures:
            missing.append(name)
            return f"<!-- no figure named {name} -->"
        return figures[name]

    return TOKEN.sub(replace, text), missing


def check_pages(assembled: Path, figures: dict[str, str]) -> list[str]:
    """Run every page-level check. Returns the failures."""
    failures: list[str] = []
    pages = sorted(assembled.rglob("*.html"))
    carried: set[str] = set()

    for page in pages:
        rel = page.relative_to(assembled)
        text = page.read_text(encoding="utf-8")

        found = re.findall(r'<figure class="chart"', text)
        if len(found) < 2:
            failures.append(
                f"{rel}: carries {len(found)} figure(s); every page carries at least two"
            )

        # A token that survived substitution is a figure that was not
        # generated, which the substitution already reported.
        for name in TOKEN.findall(text):
            failures.append(f"{rel}: carries an unresolved figure token {name}")

        for pattern, reason in BANNED:
            # A filename is not a page's content: TODOS.md is a real document
            # in this repository, and a link to it is not an unfinished marker
            # in the page.
            if pattern in (r"TODO|FIXME",):
                if re.search(pattern, text, re.IGNORECASE) and not re.search(
                    r'href="[^"]*TODOS\.md"', text
                ):
                    failures.append(f"{rel}: carries {reason}")
                continue
            if re.search(pattern, text, re.IGNORECASE):
                failures.append(f"{rel}: carries {reason}")

        if page.name in CONTACT_PAGES or str(rel) in CONTACT_PAGES:
            if CONTACT not in text:
                failures.append(f"{rel}: does not name the real contact address")

    # Which figures are carried is counted from the figure captions, because a
    # substituted page holds the figure's markup rather than its filename.
    for name, markup in figures.items():
        caption = re.search(r"<span>(.*?)</span>", markup, re.DOTALL)
        if caption and any(
            caption.group(1) in page.read_text(encoding="utf-8") for page in pages
        ):
            carried.add(name)

    for name in sorted(figures):
        if name not in carried:
            failures.append(f"assets/figures/{name}.svg is generated and carried by no page")

    print(f"  {len(pages)} page(s) checked, {len(figures)} figure(s) generated, {len(carried)} carried")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-only", action="store_true", help="check without writing")
    arguments = parser.parse_args()

    if not FIGURES.is_dir():
        print(f"no figures at {FIGURES}; run tools/generate_figures.py first", file=sys.stderr)
        return 1

    figures: dict[str, str] = {
        path.stem: path.read_text(encoding="utf-8") for path in sorted(FIGURES.glob("*.svg"))
    }

    if arguments.check_only:
        failures: list[str] = []
        for page in sorted(SITE.rglob("*.html")):
            _, missing = substitute(page.read_text(encoding="utf-8"), figures)
            for name in missing:
                failures.append(f"{page.relative_to(ROOT)}: no figure named {name}")
        failures.extend(check_pages(SITE, figures))
        if failures:
            print("ASSEMBLY: failed")
            for failure in failures:
                print(f"  {failure}")
            return 1
        print("ASSEMBLY: clean")
        return 0

    assembled = ROOT / "build" / "site"
    if assembled.exists():
        shutil.rmtree(assembled)
    assembled.mkdir(parents=True)

    failures: list[str] = []
    for path in sorted(SITE.rglob("*")):
        if path.is_dir():
            continue
        relative = path.relative_to(SITE)
        target = assembled / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if path.suffix == ".html":
            text, missing = substitute(path.read_text(encoding="utf-8"), figures)
            for name in missing:
                failures.append(f"{relative}: no figure named {name}")
            target.write_text(text, encoding="utf-8")
        else:
            shutil.copy2(path, target)

    # The page checks run against the assembled output only: the source pages
    # carry tokens where the figures go, and the assembled tree is what is
    # deployed. Running them against the source first would report every page
    # as figureless.
    #
    # The portal's address is substituted after the figures, because a page
    # that links to the portal is checked on the address it actually carries.
    for page in assembled.rglob("*.html"):
        text = page.read_text(encoding="utf-8")
        if "{{PORTAL_URL}}" in text:
            page.write_text(text.replace("{{PORTAL_URL}}", PORTAL_URL), encoding="utf-8")

    failures.extend(check_pages(assembled, figures))

    if failures:
        print("ASSEMBLY: failed")
        for failure in failures:
            print(f"  {failure}")
        return 1

    print(f"ASSEMBLY: clean, {len(list(assembled.rglob('*.html')))} page(s) in build/site")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
