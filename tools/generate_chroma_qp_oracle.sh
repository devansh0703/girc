#!/usr/bin/env bash
# Regenerate the chroma QP mapping reference vectors used by girc-core's
# tests.
#
# The vectors come from Fraunhofer HHI's VVenC, compiled into a small
# oracle (`tools/chroma_qp_oracle.c`). The reference implementation is the
# oracle: girc-core's chroma QP mapping must reproduce it exactly.
#
# Usage: tools/generate_chroma_qp_oracle.sh [path/to/vvenc/source] [out.rs]
set -euo pipefail

REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
VVENC=${1:-$REPO/data/tools/vvenc-src}
OUT=${2:-$REPO/crates/girc-core/src/chroma_qp_oracle_vectors.rs}

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
  "$REPO/tools/chroma_qp_oracle.c" \
  "$VVENC/lib/release-static/libvvenc.a" \
  -lpthread -ldl \
  -o "$REPO/build/chroma_qp_oracle"

"$REPO/build/chroma_qp_oracle" > "$OUT"

# The oracle emits eight values to a line; rustfmt reflows the array to
# the width limit. Format the generated file so the checked-in state is
# what the generator produces, which is what makes a regeneration come
# out byte-identical rather than drifting on line breaks.
rustfmt --edition 2021 "$OUT"

echo "wrote $OUT"
