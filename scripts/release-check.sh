#!/usr/bin/env bash
set -euo pipefail

./scripts/check.sh

if ! command -v cargo-deny >/dev/null 2>&1; then
  echo "cargo-deny is required for release checks." >&2
  echo "Install it with: cargo install cargo-deny --locked" >&2
  exit 1
fi

cargo deny check bans licenses sources

echo "Checking advisories. Reviewed maintenance advisories are pinned in deny.toml."
cargo deny check advisories

cargo metadata --locked --format-version 1 >/dev/null
./scripts/package-check.sh

echo "Automated release checks passed."
echo "Complete docs/platform-smoke.md on physical target systems before release."
