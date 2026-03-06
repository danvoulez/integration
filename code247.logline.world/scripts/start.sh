#!/usr/bin/env bash
set -euo pipefail

target_dir="${CARGO_TARGET_DIR:-target/code247-runtime}"
export CARGO_TARGET_DIR="$target_dir"

cargo build --release
"./${target_dir}/release/dual-agents-rust"
