#!/usr/bin/env bash
set -euo pipefail

minimal=false
if [[ "${1:-}" == "--minimal" ]]; then
  minimal=true
  shift
fi

if [[ "$#" -ne 0 ]]; then
  echo "Usage: $0 [--minimal]" >&2
  exit 2
fi

if command -v apt-get >/dev/null 2>&1; then
  packages=(
    build-essential
    libasound2-dev
    libfontconfig1-dev
    libgl1-mesa-dev
    libssl-dev
    libwayland-dev
    libx11-dev
    libx11-xcb-dev
    libxcb-render0-dev
    libxcb-shape0-dev
    libxcb-xfixes0-dev
    libxkbcommon-dev
    libxkbcommon-x11-dev
  )

  if [[ "$minimal" == false ]]; then
    packages+=(
      libgtk-3-dev
      libwebkit2gtk-4.1-dev
    )
  fi

  sudo apt-get update
  sudo apt-get install -y "${packages[@]}"
else
  echo "Unsupported package manager. Please install GPUI dependencies manually."
fi
