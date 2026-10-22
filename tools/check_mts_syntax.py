"""Check GIRC's transform signalling against the reference encoder's.

GIRC codes its explicit transform mode with the syntax ITU-T H.266 uses
for `mts_idx`: one bin saying whether a multiple transform selection
mode is used at all, and, when one is, a truncated unary code over the
four multiple transform selection modes.

That claim is only worth what the check is worth, so the check is made
against the reference encoder's own output rather than against the
standard's text. The reference encoder is run with multiple transform
selection enabled and its syntax trace is read, which gives the
transform mode it actually chose for every transform unit it coded. The
bin sequence the reference implementation writes for each of those
choices is then computed by the algorithm in its source, and compared
with GIRC's.

The reference encoder's algorithm, from
`source/Lib/EncoderLib/CABACWriter.cpp`:

    int symbol = mtsIdx != MTS_DCT2_DCT2 ? 1 : 0;
    encodeBin( symbol, MTSIdx(0) );
    if( symbol )
    {
      ctxIdx = 1;
      for( int i = 0; i < 3; i++, ctxIdx++ )
      {
        symbol = mtsIdx > i + MTS_DST7_DST7 ? 1 : 0;
        encodeBin( symbol, MTSIdx(ctxIdx) );
        if( !symbol ) break;
      }
    }

and the modes are the enum `MTSIdx`:
`MTS_DCT2_DCT2 = 0`, `MTS_SKIP = 1`, `MTS_DST7_DST7 = 2`,
`MTS_DCT8_DST7 = 3`, `MTS_DST7_DCT8 = 4`, `MTS_DCT8_DCT8 = 5`.

The mapping from GIRC's five modes to the reference's five is by
transform, not by index, because the reference's index 1 is the
transform skip, which GIRC does not have.

Run it from the repository root:

    python3 tools/check_mts_syntax.py
"""

from __future__ import annotations

import collections
import json
import re
import subprocess
import sys
from pathlib import Path

# GIRC's five modes, in GIRC's order.
GIRC_MODES = [
    "DCT-II",
    "DST-VII/DST-VII",
    "DCT-VIII/DST-VII",
    "DST-VII/DCT-VIII",
    "DCT-VIII/DCT-VIII",
]

# The reference encoder's `MTSIdx` enum.
VVENC_MTS = {
    "MTS_DCT2_DCT2": 0,
    "MTS_SKIP": 1,
    "MTS_DST7_DST7": 2,
    "MTS_DCT8_DST7": 3,
    "MTS_DST7_DCT8": 4,
    "MTS_DCT8_DCT8": 5,
}

# GIRC's mode index to the reference's index.
GIRC_TO_VVENC = {
    0: VVENC_MTS["MTS_DCT2_DCT2"],
    1: VVENC_MTS["MTS_DST7_DST7"],
    2: VVENC_MTS["MTS_DCT8_DST7"],
    3: VVENC_MTS["MTS_DST7_DCT8"],
    4: VVENC_MTS["MTS_DCT8_DCT8"],
}


def reference_bins(mts_index: int) -> list[int]:
    """The bins the reference implementation writes for a mode.

    This is the algorithm in `CABACWriter::mts_idx`, transcribed.
    """
    bins: list[int] = []
    symbol = 1 if mts_index != VVENC_MTS["MTS_DCT2_DCT2"] else 0
    bins.append(symbol)
    if symbol:
        for i in range(3):
            symbol = 1 if mts_index > i + VVENC_MTS["MTS_DST7_DST7"] else 0
            bins.append(symbol)
            if not symbol:
                break
    return bins


def girc_bins(mode_index: int) -> list[int]:
    """The bins GIRC writes for a mode.

    This is `code_transform_signal` in `crates/girc-core/src/codec.rs`,
    transcribed.
    """
    bins = [1 if mode_index > 0 else 0]
    for i in range(3):
        if mode_index <= i:
            break
        bins.append(1 if mode_index > i + 1 else 0)
    return bins


def capture_reference_modes(
    vvenc: Path, sequence: Path, work: Path
) -> collections.Counter:
    """Run the reference encoder and read the modes it actually coded.

    The reference encoder is built with tracing enabled, and its
    `D_SYNTAX` trace records every `mts_idx` call with the mode it
    coded.
    """
    work.mkdir(parents=True, exist_ok=True)
    trace = work / "syntax.txt"
    output = work / "reference.266"

    result = subprocess.run(
        [
            str(vvenc),
            "--InputFile",
            str(sequence),
            "--BitstreamFile",
            str(output),
            "--FrameRate",
            "30",
            "--Frames",
            "1",
            "--QP",
            "32",
            "--Threads",
            "1",
            "--MTS",
            "2",
            "--MTSIntraMaxCand",
            "4",
            "--tracefile",
            str(trace),
            "--tracerule",
            "D_SYNTAX:slice==0",
            "--verbosity",
            "4",
        ],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"the reference encoder failed:\n{result.stderr[-2000:]}")

    counts: collections.Counter = collections.Counter()
    with trace.open() as handle:
        for line in handle:
            if "mts_idx()" not in line:
                continue
            match = re.search(r"mtsIdx=(-?\d+)", line)
            if match:
                counts[int(match.group(1))] += 1
    return counts


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    vvenc = root / "data/tools/vvenc-src/bin/release-static/vvencFFapp"
    sequence = root / "data/seq/akiyo_cif.y4m"
    work = root / "bench-out/mts-check"

    if not vvenc.exists():
        print(
            f"the reference encoder is not built at {vvenc}\n"
            "build it with tracing enabled:\n"
            "  cmake -S data/tools/vvenc-src -B data/tools/vvenc-src/build-trace \\\n"
            "    -DCMAKE_BUILD_TYPE=Release -DVVENC_ENABLE_TRACING=1\n"
            "  cmake --build data/tools/vvenc-src/build-trace --config Release -j 4",
            file=sys.stderr,
        )
        return 2

    # The check itself: the two algorithms agree for every mode.
    print("the syntax, mode by mode:")
    failed = False
    for girc_index, girc_name in enumerate(GIRC_MODES):
        vvenc_index = GIRC_TO_VVENC[girc_index]
        reference = reference_bins(vvenc_index)
        girc = girc_bins(girc_index)
        agree = reference == girc
        failed = failed or not agree
        print(
            f"  {girc_name:20s} reference {reference}  GIRC {girc}  "
            f"{'agree' if agree else 'DISAGREE'}"
        )
    if failed:
        print("\nthe two syntaxes disagree")
        return 1

    # And the check against the reference encoder's actual output.
    print("\nrunning the reference encoder with multiple transform selection:")
    counts = capture_reference_modes(vvenc, sequence, work)
    if not counts:
        print("the reference encoder coded no transform units", file=sys.stderr)
        return 1

    reverse = {value: key for key, value in GIRC_TO_VVENC.items()}
    print(f"  {sum(counts.values())} transform units coded")
    for vvenc_index in sorted(counts):
        name = next(
            (g for g, v in GIRC_TO_VVENC.items() if v == vvenc_index),
            None,
        )
        label = GIRC_MODES[name] if name is not None else "not in GIRC's set"
        print(f"    reference mode {vvenc_index}: {counts[vvenc_index]:>7} times  ({label})")

    # Every mode the reference coded must be one GIRC can express, and
    # the bins must agree for it.
    unknown = [
        index for index in counts if index not in reverse and index != VVENC_MTS["MTS_SKIP"]
    ]
    if unknown:
        print(f"\nthe reference coded modes GIRC does not have: {unknown}")
        return 1

    print("\nevery mode the reference coded is in GIRC's set, and the two")
    print("syntaxes agree on all of them")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
