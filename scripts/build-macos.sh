#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "Build the macOS app on a Mac with Xcode command line tools installed." >&2
    exit 1
fi
if ! command -v cargo >/dev/null 2>&1 && [[ -x "$HOME/.cargo/bin/cargo" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi
export MACOSX_DEPLOYMENT_TARGET=14.2
cargo build --release --locked
app="$PWD/target/macos/Symphos.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/symphos "$app/Contents/MacOS/symphos"
mkdir -p "$app/Contents/Resources/Licenses"
cp THIRD_PARTY_NOTICES.md "$app/Contents/Resources/"
cp vendor/projectm/licenses/*.txt vendor/projectm/COPYING vendor/projectm/LICENSE.txt "$app/Contents/Resources/Licenses/"
cp assets/macos/Info.plist "$app/Contents/Info.plist"
version=$(cargo pkgid --offline | sed 's/.*#//; s/.*@//')
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $version" "$app/Contents/Info.plist"
# Ad-hoc signing is for this local developer build. Public downloads need a
# Developer ID identity and notarization; this is not a distributable release.
codesign --force --deep --sign "${SYMPHOS_SIGN_IDENTITY:--}" "$app"
codesign --verify --deep --strict "$app"
echo "Built $app"
echo "Launch with: open \"$app\""
