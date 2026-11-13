#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

packages=(
  guic-packager-utils
  guic-packager
  guic-packager-updater
  guic-packager-resource-resolver
  guic-build
  cargo-guic
  guic-macros
  guic-tokens
  guic-assets
  guic-webview
  guic-core
  guic-native
  guic-icons
  guic-components
  guic-charts
  guic-editor
  guic
)

check_package_license() {
  local package="$1"
  local package_files

  package_files="$(cargo package --locked --allow-dirty --no-verify --list -p "${package}")"
  if ! grep -qx "LICENSE" <<<"${package_files}"; then
    echo "Package ${package} does not include LICENSE." >&2
    return 1
  fi
}

arguments=()
for package in "${packages[@]}"; do
  echo "Checking package metadata and contents: ${package}"
  check_package_license "${package}"
  arguments+=(-p "${package}")
done

# Cargo's temporary registry resolves dependencies between unpublished archives.
cargo package --locked --allow-dirty "${arguments[@]}"

echo "All public package archives and file lists passed verification."
