#!/usr/bin/env bash
# Put the encoders the benchmark anchors against on the PATH.
#
# The benchmark compares GIRC against x265, SVT-AV1 and VVenC, and it
# finds them by name. x265 and SVT-AV1 come from the system packages,
# and VVenC is built from source into this repository, so its binary is
# not on the PATH by default and the benchmark reports it as
# unavailable.
#
# Sourcing this script makes all three findable:
#
#     source tools/benchmark_env.sh
#     girc-bench --sequence data/seq/akiyo_cif.y4m --qps 27
#
# The script does not install anything. It only says where the tools
# are, and checks that they are there.

set -u

_girc_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

_vvenc_bin="${_girc_root}/data/tools/vvenc-src/bin/release-static"
if [ -d "${_vvenc_bin}" ]; then
    case ":${PATH}:" in
        *":${_vvenc_bin}:"*) ;;
        *) PATH="${_vvenc_bin}:${PATH}" ;;
    esac
fi

export PATH

# The tools the benchmark anchors against, and what happens without
# them. The benchmark reports an unavailable anchor rather than
# failing, so a missing tool is visible in the report's tool versions.
for _tool in x265 ffmpeg vvencapp; do
    if command -v "${_tool}" >/dev/null 2>&1; then
        printf '%-10s %s\n' "${_tool}" "$(command -v "${_tool}")"
    else
        printf '%-10s %s\n' "${_tool}" "MISSING: the benchmark will report it as unavailable"
    fi
done

unset _girc_root _vvenc_bin _tool
