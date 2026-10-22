#!/usr/bin/env python3
"""Cross-check the VVC transform matrices in girc-core against FFmpeg.

`girc-core`'s tables are generated from Fraunhofer HHI VVenC
(`source/Lib/CommonLib/RomTr.cpp`, Clear BSD). FFmpeg's
`libavcodec/vvc/data.c` (LGPL-2.1+) encodes the same ITU-T H.266 tables
independently. This script expands FFmpeg's macros and compares every entry
against the committed Rust tables, so a transcription or regeneration error
cannot slip through.

Usage: python3 tools/verify_transform_tables.py <ffmpeg_vvc_data.c> <girc tables.rs>
"""

import json
import re
import sys


def ffmpeg_macros(text):
    """Extract every DEFINE_*_MATRIX macro.

    Returns {name: (parameter_names, row_cell_templates)}. The cell templates
    reference the macro's parameters by name (``a``, ``-b``), which bind
    positionally to the values supplied at the call site.
    """
    macros = {}
    pattern = r"#define\s+(DEFINE_\w+_MATRIX)\(([^)]*)\)\s*\\\n(.*?)(?=\n\n)"
    for match in re.finditer(pattern, text, re.S):
        name, params, body = match.group(1), match.group(2), match.group(3)
        rows = [
            [cell.strip() for cell in row.group(1).split(",")]
            for row in re.finditer(r"\{([^{}]*)\}", body)
        ]
        macros[name] = ([p.strip() for p in params.split(",")], rows)
    return macros


def expand(macros, name, values):
    """Substitute a macro's parameters into its row templates.

    The macro's parameter names bind positionally to ``values``; a cell may
    reference a parameter directly (``a``) or negated (``-b``).
    """
    params, rows = macros[name]
    args = [v.strip() for v in values.split(",") if v.strip()]
    if len(args) != len(params):
        raise ValueError(f"{name}: {len(args)} values for {len(params)} parameters")
    lookup = {p: args[i] for i, p in enumerate(params)}

    out = []
    for row in rows:
        cells = []
        for cell in row:
            if cell in lookup:
                cells.append(lookup[cell])
            elif cell.startswith("-") and cell[1:] in lookup:
                cells.append("-" + lookup[cell[1:]])
            else:
                # A literal cell (e.g. the 0 in the DCT-VIII 4-point matrix).
                try:
                    int(cell)
                except ValueError:
                    raise ValueError(f"{name}: unresolved cell {cell!r}") from None
                cells.append(cell)
        out.append(cells)
    return out


def ffmpeg_tables(text, macros):
    """Return {(kind, size): matrix} for every VVC transform table FFmpeg ships.

    FFmpeg keeps DCT-II in `libavcodec/vvc/dct.c` and the DCT-VIII / DST-VII
    matrices in `libavcodec/vvc/data.c`, so this covers the two families whose
    tables are independently encoded from ITU-T H.266.
    """
    tables = {}
    for kind in ("dct8", "dst7"):
        for size in (4, 8, 16, 32):
            decl = re.search(
                r"const int8_t ff_vvc_%s_%dx%d\[\d+\]\[\d+\]\s*=\s*"
                r"DEFINE_%s_P%d_MATRIX\s*\(([^)]*)\);"
                % (kind, size, size, kind.upper(), size),
                text,
            )
            if not decl:
                continue
            tables[(kind, size)] = expand(
                macros, f"DEFINE_{kind.upper()}_P{size}_MATRIX", decl.group(1)
            )
    return tables


def rust_tables(text):
    """Return {(kind, size): matrix} parsed out of the generated Rust file."""
    tables = {}
    pattern = r"static (DCT2|DCT8|DST7)(\d+): \[\[i32; \d+\]; \d+\] = \[(.*?)\];"
    for match in re.finditer(pattern, text, re.S):
        kind = {"DCT2": "dct2", "DCT8": "dct8", "DST7": "dst7"}[match.group(1)]
        size = int(match.group(2))
        tables[(kind, size)] = [
            [int(cell.strip()) for cell in row.group(1).split(",") if cell.strip()]
            for row in re.finditer(r"\[([^\]]*)\]", match.group(3))
        ]
    return tables


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    ff_path, rs_path = sys.argv[1], sys.argv[2]
    ff_text = open(ff_path, encoding="utf-8", errors="replace").read()
    rs_text = open(rs_path, encoding="utf-8", errors="replace").read()

    ff = ffmpeg_tables(ff_text, ffmpeg_macros(ff_text))
    rs = rust_tables(rs_text)

    report = {}
    ok = True
    for key in sorted(ff):
        a, b = ff[key], rs.get(key)
        label = f"{key[0].upper()}{key[1]}"
        if b is None:
            report[label] = "MISSING in girc-core"
            ok = False
            continue
        if len(a) != len(b) or any(
            len(ra) != len(rb) or any(int(x) != y for x, y in zip(ra, rb))
            for ra, rb in zip(a, b)
        ):
            report[label] = "MISMATCH"
            report[label + "_ffmpeg"] = a
            report[label + "_girc"] = b
            ok = False
        else:
            report[label] = f"match ({len(a)}x{len(a[0])})"

    print(json.dumps(report, indent=1))
    if not ok:
        print("FAIL: transform tables disagree with FFmpeg")
        return 1
    print(f"OK: all {len(ff)} VVC transform tables agree with FFmpeg")
    return 0


if __name__ == "__main__":
    sys.exit(main())
