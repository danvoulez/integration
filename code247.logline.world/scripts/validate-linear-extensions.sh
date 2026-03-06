#!/usr/bin/env bash
set -euo pipefail

export CODE247_REPO_ROOT="${CODE247_REPO_ROOT:-.}"
export CODE247_MANIFEST_PATH="${CODE247_MANIFEST_PATH:-.code247/workspace.manifest.json}"
export CODE247_MANIFEST_SCHEMA_PATH="${CODE247_MANIFEST_SCHEMA_PATH:-schemas/code247-linear-runtime.extensions.schema.json}"
export CODE247_MANIFEST_REQUIRED="${CODE247_MANIFEST_REQUIRED:-true}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target/code247-manifest-check}"

cargo run --quiet --bin validate_manifest
