#!/bin/bash
# Builds and installs /Applications/AI Launcher.app
set -euo pipefail
cd "$(dirname "$0")"
VERSION="1.1"
APP="/Applications/AI Launcher.app"
OUT="build/AI Launcher.app"

swift build -c release
rm -rf build && mkdir -p "$OUT/Contents/MacOS" "$OUT/Contents/Resources"
cp "$(swift build -c release --show-bin-path)/AILauncher" "$OUT/Contents/MacOS/AI Launcher"
strip -S -x "$OUT/Contents/MacOS/AI Launcher"  # drops debug paths (they include the builder's home folder)
"$OUT/Contents/MacOS/AI Launcher" --write-app-icon "$OUT/Contents/Resources/AppIcon.icns"

cat > "$OUT/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>AI Launcher</string>
  <key>CFBundleIdentifier</key><string>local.ailauncher</string>
  <key>CFBundleName</key><string>AI Launcher</string>
  <key>CFBundleDisplayName</key><string>AI Launcher</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$(git rev-list --count HEAD 2>/dev/null || echo 1)</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>Run separate signed-in copies of your AI apps side by side.</string>
</dict></plist>
EOF

# Same certificate as the profiles, so Open at Login and other approvals survive rebuilds.
SIGNER="-"; security find-certificate -c "AI Launcher Local" >/dev/null 2>&1 && SIGNER="AI Launcher Local"
# Release builds are ad-hoc signed: a personal certificate would ship its details (e.g. email) to everyone.
[ "${1:-}" = "--release" ] && SIGNER="-"
codesign --force --sign "$SIGNER" "$OUT"
if [ "${1:-}" = "--release" ]; then
  ditto -c -k --keepParent "$OUT" "build/AI-Launcher-$VERSION.zip"
  echo "Built build/AI-Launcher-$VERSION.zip"; exit 0
fi

pkill -x "AI Launcher" 2>/dev/null && sleep 1 || true
rm -rf "$APP" && mv "$OUT" "$APP"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP"
echo "Installed $APP (signed: $SIGNER)"
