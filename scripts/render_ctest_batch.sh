#!/usr/bin/env bash
set -euo pipefail

PKG_CONFIG_PATH="/home/wylliam/dev/oidn-2.3.3.x86_64.linux/lib/pkgconfig"
BASE_IN="blender_scene/export"
BASE_OUT="blender_scene/export/render_ctest_batch"

for i in $(seq 1 250); do
  id=$(printf "%05d" "$i")
  input="${BASE_IN}/ctest${id}.json"
  output="${BASE_OUT}/ctest${id}_noisy.exr"

  echo "Rendering ${input} -> ${output}"
  PKG_CONFIG_PATH="${PKG_CONFIG_PATH}" cargo run --release --example=render --features oidn -- \
    -i "${input}" \
    -o "${output}" \
    -n 32 \
    -a scenes/sah.json \
    -s 0.12
done
