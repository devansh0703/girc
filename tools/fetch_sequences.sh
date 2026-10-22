#!/usr/bin/env bash
# Fetch the test sequences the benchmark uses.
#
# The sequences are the Xiph.Org CIF test set, distributed under a
# permissive licence that permits redistribution. They are not in the
# repository because of their size; this script fetches them and the
# reproducibility manifest records their checksums.
#
# Usage: tools/fetch_sequences.sh [output-dir]
set -euo pipefail

REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
DEST=${1:-$REPO/data/seq}
BASE=https://media.xiph.org/video/derf/y4m

SEQUENCES=(
  akiyo_cif
  bus_cif
  city_cif
  coastguard_cif
  container_cif
  crew_cif
  deadline_cif
  flower_cif
  foreman_cif
  highway_cif
  mobile_cif
  stefan_cif
)

mkdir -p "$DEST"

for name in "${SEQUENCES[@]}"; do
  target="$DEST/$name.y4m"
  if [ -f "$target" ]; then
    echo "have $name"
    continue
  fi
  echo "fetching $name"
  curl -fsSL -o "$target.part" "$BASE/$name.y4m"
  # A truncated or error response would poison the benchmark, so the
  # header is checked before the file is accepted.
  if ! head -c 6 "$target.part" | grep -q YUV4MPEG2; then
    rm -f "$target.part"
    echo "not a Y4M file: $name" >&2
    exit 1
  fi
  mv "$target.part" "$target"
done

echo "--- checksums ---"
cd "$DEST"
sha256sum *.y4m
