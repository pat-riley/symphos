#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cargo_bin="${CARGO:-$HOME/.cargo/bin/cargo}"
local_bin="${XDG_BIN_HOME:-$HOME/.local/bin}"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"

"$cargo_bin" build --release --manifest-path "$project_dir/Cargo.toml"
install -Dm755 "$project_dir/target/release/symphos" "$local_bin/symphos"
install -Dm644 \
  "$project_dir/assets/io.github.riley.Symphos.desktop" \
  "$data_home/applications/io.github.riley.Symphos.desktop"
install -Dm644 \
  "$project_dir/assets/io.github.riley.Symphos.svg" \
  "$data_home/icons/hicolor/scalable/apps/io.github.riley.Symphos.svg"
install -Dm644 \
  "$project_dir/assets/io.github.riley.Symphos.metainfo.xml" \
  "$data_home/metainfo/io.github.riley.Symphos.metainfo.xml"

command -v update-desktop-database >/dev/null && update-desktop-database "$data_home/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f -t "$data_home/icons/hicolor" || true

echo "Symphos is installed for this user. Launch it from the Omarchy app menu or run: symphos"
