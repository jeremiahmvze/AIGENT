#!/bin/bash
# Builds AIGENT.exe and its installer in ../build/. Works on macOS/Linux (cross-compiling) and in Git Bash on Windows.
# Needs: rustup, mingw-w64 (not on Windows), NSIS (makensis). On macOS: brew install mingw-w64 makensis
set -euo pipefail
cd "$(dirname "$0")"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
TARGET=x86_64-pc-windows-gnu
[ "$(uname -o 2>/dev/null)" = "Msys" ] && TARGET=x86_64-pc-windows-msvc

cargo build --release --target "$TARGET"
mkdir -p ../build/windows
cp "target/$TARGET/release/AIGENT.exe" ../build/windows/
# makensis crashes (bad_alloc) when no locale is set.
LC_ALL="${LC_ALL:-en_US.UTF-8}" makensis -V2 -DVERSION="$VERSION" installer.nsi
echo "Built build/windows/AIGENT.exe and build/AIGENT-Setup-$VERSION.exe"
