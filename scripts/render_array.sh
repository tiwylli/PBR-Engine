#!/bin/bash
#SBATCH --job-name=ctest-render
#SBATCH --array=1-250
#SBATCH --time=00:10:00
#SBATCH --account=def-agruson
#SBATCH --mem=0
#SBATCH --cpus-per-task=192
#SBATCH --output=slurm-%x-%A_%a.out

set -euo pipefail

# shellcheck disable=SC1090
[ -f "$HOME/.zhrsc" ] && source "$HOME/.zhrsc"

module load cuda
module load rust/1.91.0

if [ -f "$HOME/.cargo/env" ]; then
  # Ensure cargo from rustup is on PATH when the batch environment is minimal.
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found even after loading rust/1.91.0; please load the appropriate Rust module." >&2
  exit 1
fi

PROJECT_DIR="/home/tiwylli/projects/def-agruson/tiwylli/CPPnt"
cd "$PROJECT_DIR"

SCENE_DIR="$PROJECT_DIR/blender_scene/export"
OUT_DIR="$SCENE_DIR/render_array"
mkdir -p "$OUT_DIR"

export OPTIX_CACHE_PATH="${SLURM_TMPDIR:-/tmp}"
export OPTIX_CACHE_PATH="${SLURM_TMPDIR:-/tmp}"
export OIDN_DIR="/home/tiwylli/projects/def-agruson/tiwylli/oidn"
export LD_LIBRARY_PATH="$OIDN_DIR/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PKG_CONFIG_PATH="$OIDN_DIR/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"


TASK_ID="${SLURM_ARRAY_TASK_ID:-1}"
PADDED_ID="$(printf "%05d" "$TASK_ID")"


OUT_FILE="${OUT_DIR}/ctest${PADDED_ID}.exr"
IN_FILE="${SCENE_DIR}/ctest${PADDED_ID}.json"

PKG_CONFIG_PATH="/home/tiwylli/projects/def-agruson/tiwylli/oidn" cargo run --release --example=render --features oidn -- -i "$IN_FILE" -o "$OUT_FILE" -n 64 -a scenes/sah.json -s 0.125