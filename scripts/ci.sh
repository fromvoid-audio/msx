#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export VST3_SDK_DIR="${VST3_SDK_DIR:-$HOME/lib/vst3sdk}"
cargo fmt --all -- --check
cargo clippy --locked --release --all-targets -- -D warnings
cargo test --locked --release --lib
cargo build --locked --release
