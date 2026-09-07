#!/usr/bin/env bash
set -euo pipefail

local_bin="${XDG_BIN_HOME:-$HOME/.local/bin}"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"

targets=(
  "$local_bin/symphos"
  "$data_home/applications/io.github.riley.Symphos.desktop"
  "$data_home/icons/hicolor/scalable/apps/io.github.riley.Symphos.svg"
  "$data_home/metainfo/io.github.riley.Symphos.metainfo.xml"
)

for target in "${targets[@]}"; do
  if [[ -f "$target" ]]; then
    rm -- "$target"
  fi
done

command -v update-desktop-database >/dev/null && update-desktop-database "$data_home/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f -t "$data_home/icons/hicolor" || true

echo "Removed the user-local Symphos installation. The repository and build files were not changed."
