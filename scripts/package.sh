#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Run prepare-vendor.py and build-driver.sh once before packaging. This script
# uses cached build dependencies and makes no network requests.
[ "$(uname -m)" = arm64 ] || { echo 'This package currently targets Apple Silicon.' >&2; exit 1; }
[ -f vendor/runtime/bin/ntfs-3g ] || { echo 'Run scripts/prepare-vendor.py and scripts/build-driver.sh first.' >&2; exit 1; }
npm run build
cargo build --offline --release --locked --workspace
python3 scripts/package.py
