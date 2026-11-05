#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

APP_NAME="LogLens"
BUNDLE_ID="com.loglens.desktop"
TARGET="aarch64-apple-darwin"
BIN_NAME="loglens-replica"
DIST="$PWD/dist"
APP="$DIST/$APP_NAME.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"

echo "==> Checking Rust toolchain"
command -v cargo >/dev/null || { echo "cargo not found. Run: source \"$HOME/.cargo/env\""; exit 1; }
rustup target add "$TARGET" >/dev/null

echo "==> Building optimized Apple-silicon release"
cargo build --release --target "$TARGET"

BIN="$PWD/target/$TARGET/release/$BIN_NAME"
[ -f "$BIN" ] || { echo "Build completed but binary not found: $BIN"; exit 1; }

rm -rf "$APP"
mkdir -p "$MACOS" "$RESOURCES/assets" "$RESOURCES/templates" "$RESOURCES/examples"
cp "$BIN" "$MACOS/$APP_NAME"
chmod +x "$MACOS/$APP_NAME"
cp -R assets/. "$RESOURCES/assets/" 2>/dev/null || true
cp -R templates/. "$RESOURCES/templates/" 2>/dev/null || true
cp -R examples/. "$RESOURCES/examples/" 2>/dev/null || true

cat > "$CONTENTS/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>$APP_NAME</string>
    <key>CFBundleDisplayName</key><string>$APP_NAME</string>
    <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
    <key>CFBundleVersion</key><string>0.54.42</string>
    <key>CFBundleShortVersionString</key><string>0.54.42</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleExecutable</key><string>$APP_NAME</string>
    <key>LSMinimumSystemVersion</key><string>12.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
</dict>
</plist>
PLIST

# Ad-hoc signing prevents some local bundle warnings. This is not Developer-ID notarization.
if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true
fi

echo
echo "Created: $APP"
echo "Open with: open \"$APP\""
echo "Install locally with: cp -R \"$APP\" /Applications/"
