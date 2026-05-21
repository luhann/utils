#!/usr/bin/env sh
set -eu

RUNS="${1:-500}"
WARMUP="${2:-100}"

if ! command -v hyperfine >/dev/null 2>&1; then
  echo "hyperfine is not installed."
  echo "Install it, then rerun this script."
  exit 1
fi

cargo build --release

echo "$(pwd)"

hyperfine \
  --warmup "$WARMUP" \
  --runs "$RUNS" \
  --shell=none \
  'target/release/writeback'
