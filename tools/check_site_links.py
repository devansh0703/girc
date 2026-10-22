"""Check that every page in the site links to a page that exists.

The check runs against the assembled site, because that is what is
deployed: the two source trees are copied into one directory, with the
evaluation site's pages under `evaluate/`, and a link that is correct
in the source tree can be wrong in the assembled one.

    python3 tools/check_site_links.py
"""

from __future__ import annotations

import re
import shutil
import sys
import tempfile
from pathlib import Path


def assemble(root: Path, site: Path) -> Path:
    """Assemble the site the way the deployment does."""
    assembled = root / "site"
    shutil.copytree(site / "girc-site", assembled)
    shutil.copytree(site / "eval-site", assembled / "evaluate")
    # The portal's address is substituted when the site is assembled,
    # so it is substituted here too, or the check would look for a
    # page at a path that only exists in the deployment.
    for page in assembled.rglob("*.html"):
        text = page.read_text(encoding="utf-8")
        text = text.replace("{{PORTAL_URL}}", "https://portal.invalid")
        page.write_text(text, encoding="utf-8")
    return assembled


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory(prefix="girc-site-check-") as temporary:
        assembled = assemble(Path(temporary), root / "web")

        failed = False
        for page in sorted(assembled.rglob("*.html")):
            text = page.read_text(encoding="utf-8")
            for target in re.findall(r'href="([^"#]+)"', text):
                if target.startswith(("http", "mailto:", "/")):
                    continue
                # A link with a query or a fragment is checked on its
                # path alone.
                path = target.split("?")[0].split("#")[0]
                if not path:
                    continue
                resolved = (page.parent / path).resolve()
                if not resolved.exists():
                    print(f"{page.relative_to(assembled)}: broken link to {target}")
                    failed = True

        if failed:
            return 1
        print(f"every link in the assembled site resolves")
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
