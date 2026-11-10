#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo test -p guic --no-default-features --locked --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
RUSTDOCFLAGS="-D warnings" cargo doc -p guic-components --no-default-features --no-deps --locked
cargo run -p xtask --locked -- schema --check
