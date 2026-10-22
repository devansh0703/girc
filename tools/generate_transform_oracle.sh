#!/usr/bin/env bash
# Regenerate the VVC transform reference vectors used by girc-core's tests.
#
# The vectors come from Fraunhofer HHI's VVenC transform kernels, compiled
# into a small oracle (`tools/transform_oracle.c`). The reference
# implementation is the oracle: girc-core's transform must reproduce it
# bit-exactly.
#
# Usage: tools/generate_transform_oracle.sh [path/to/vvenc/source] [out.rs]
set -euo pipefail

REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
VVENC=${1:-$REPO/data/tools/vvenc-src}
OUT=${2:-$REPO/crates/girc-core/src/transform_oracle_vectors.rs}

if [ ! -d "$VVENC/source/Lib/CommonLib" ]; then
  echo "VVenC source not found at $VVENC" >&2
  echo "Clone it with: git clone --depth 1 --branch v1.14.0 https://github.com/fraunhoferhhi/vvenc.git $VVENC" >&2
  exit 1
fi

mkdir -p "$REPO/build"
g++ -O2 -std=c++17 \
  -I"$VVENC/source/Lib" \
  -I"$VVENC/source/Lib/CommonLib" \
  -I"$VVENC/include" \
  -I"$VVENC/build" \
  "$REPO/tools/transform_oracle.c" \
  "$VVENC/source/Lib/CommonLib/TrQuant_EMT.cpp" \
  "$VVENC/source/Lib/CommonLib/RomTr.cpp" \
  -o "$REPO/build/transform_oracle"

"$REPO/build/transform_oracle" > "$OUT"
echo "wrote $OUT"
