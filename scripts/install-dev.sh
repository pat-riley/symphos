#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cargo_bin="${CARGO:-$HOME/.cargo/bin/cargo}"
local_bin="${XDG_BIN_HOME:-$HOME/.local/bin}"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
build_target_dir="${CARGO_TARGET_DIR:-$project_dir/target}"

cd "$project_dir"
export CARGO_TARGET_DIR="$build_target_dir"
"$cargo_bin" fmt --all -- --check
"$cargo_bin" clippy --all-targets --locked -- -D warnings
"$cargo_bin" test --all-targets --locked
"$cargo_bin" build --release --locked

# Check the artifact before touching the launcher target. --version is headless.
built_version="$("$build_target_dir/release/symphos" --version)"
mkdir -p "$local_bin"
staged_binary="$(mktemp "$local_bin/.symphos.XXXXXX")"
trap 'rm -f -- "$staged_binary"' EXIT
install -m755 "$build_target_dir/release/symphos" "$staged_binary"
cmp --silent "$build_target_dir/release/symphos" "$staged_binary"
# Atomic replacement also allows an already-running instance to exit normally.
mv -f -- "$staged_binary" "$local_bin/symphos"
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

installed_version="$("$local_bin/symphos" --version)"
test "$built_version" = "$installed_version"
echo "Installed and verified: $installed_version"
echo "Binary: $local_bin/symphos"
echo "Close any running Symphos window and reopen it from your app launcher."
