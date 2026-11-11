#!/bin/bash
# Builds and installs /Applications/AIGENT.app
set -euo pipefail
cd "$(dirname "$0")"
VERSION="1.1"
APP="/Applications/AIGENT.app"
OUT="build/AIGENT.app"

swift build -c release
rm -rf build && mkdir -p "$OUT/Contents/MacOS" "$OUT/Contents/Resources"
cp "$(swift build -c release --show-bin-path)/AIGENT" "$OUT/Contents/MacOS/AIGENT"
strip -S -x "$OUT/Contents/MacOS/AIGENT"  # drops debug paths (they include the builder's home folder)
"$OUT/Contents/MacOS/AIGENT" --write-app-icon "$OUT/Contents/Resources/AppIcon.icns"

cat > "$OUT/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>AIGENT</string>
  <key>CFBundleIdentifier</key><string>local.aigent</string>
  <key>CFBundleName</key><string>AIGENT</string>
  <key>CFBundleDisplayName</key><string>AIGENT</string>
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
  # Drag-to-install disk image: the app plus an Applications shortcut.
  mkdir -p build/dmg && mv "$OUT" build/dmg/ && ln -s /Applications build/dmg/Applications
  hdiutil create -volname "AIGENT" -srcfolder build/dmg -fs HFS+ -format UDZO -ov "build/AIGENT-$VERSION.dmg" >/dev/null
  echo "Built build/AIGENT-$VERSION.dmg"; exit 0
fi

pkill -x "AIGENT" 2>/dev/null && sleep 1 || true
# Pre-rename install: quit it and move it to the Trash.
if [ -d "/Applications/AI Launcher.app" ]; then
  pkill -x "AI Launcher" 2>/dev/null && sleep 1 || true
  defaults export local.ailauncher - 2>/dev/null | defaults import local.aigent - 2>/dev/null || true  # keep favorites/settings
  mv "/Applications/AI Launcher.app" ~/.Trash/"AI Launcher $(date +%s).app"
fi
rm -rf "$APP" && mv "$OUT" "$APP"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP"
echo "Installed $APP (signed: $SIGNER)"
