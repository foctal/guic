#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

independent_packages=(
  guic-macros
  guic-tokens
  guic-assets
  guic-webview
)

dependent_packages=(
  guic-core
  guic-icons
  guic-components
  guic-charts
  guic-editor
  guic-terminal
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

for package in "${independent_packages[@]}"; do
  echo "Checking package metadata and contents: ${package}"
  check_package_license "${package}"
  cargo package --locked --allow-dirty -p "${package}"
done

for package in "${dependent_packages[@]}"; do
  echo "Checking pre-publication package contents: ${package}"
  check_package_license "${package}"
done

echo "Independent package archives and all package file lists passed."
echo "Dependent archives require publishing prerequisites in docs/publication.md order."
