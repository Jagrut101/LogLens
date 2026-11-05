#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
APP="$PWD/dist/LogLens.app"
DMG="$PWD/dist/LogLens-macOS-arm64.dmg"
[ -d "$APP" ] || { echo "LogLens.app not found. Run ./build_macos_app.sh first."; exit 1; }
rm -f "$DMG"
hdiutil create -volname "LogLens" -srcfolder "$APP" -ov -format UDZO "$DMG"
echo "Created: $DMG"
