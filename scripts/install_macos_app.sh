#!/usr/bin/env bash
# Build (if needed) and install Vibecap.app into /Applications.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

BIN_SRC="${VIBECAP_BIN:-$ROOT/target/release/vibecap}"
if [[ ! -x "$BIN_SRC" ]]; then
  echo "Building release binary…"
  cargo build --release
fi

# Also refresh cargo bin for CLI / MCP
if command -v cargo >/dev/null 2>&1; then
  cargo install --path . --force --quiet 2>/dev/null || cargo install --path . --force
fi

APP_SRC="$ROOT/Vibecap.app"
MACOS_DIR="$APP_SRC/Contents/MacOS"
RES_DIR="$APP_SRC/Contents/Resources"
mkdir -p "$MACOS_DIR" "$RES_DIR"
cp -f "$BIN_SRC" "$MACOS_DIR/vibecap"
chmod +x "$MACOS_DIR/vibecap"

# Bundle brand icon (icns for Finder/Dock when app is not running)
if [[ -f "$ROOT/docs/brand/AppIcon.icns" ]]; then
  cp -f "$ROOT/docs/brand/AppIcon.icns" "$RES_DIR/AppIcon.icns"
elif [[ -d "$ROOT/docs/brand/AppIcon.iconset" ]] && command -v iconutil >/dev/null 2>&1; then
  iconutil -c icns "$ROOT/docs/brand/AppIcon.iconset" -o "$RES_DIR/AppIcon.icns"
fi
if [[ -f "$ROOT/docs/brand/app-icon-1024.png" ]]; then
  cp -f "$ROOT/docs/brand/app-icon-1024.png" "$RES_DIR/AppIcon.png"
elif [[ -f "$ROOT/assets/app_icon.png" ]]; then
  cp -f "$ROOT/assets/app_icon.png" "$RES_DIR/AppIcon.png"
fi

# Prefer user Applications if /Applications is not writable
DEST="/Applications/Vibecap.app"
if [[ ! -w /Applications ]]; then
  DEST="$HOME/Applications/Vibecap.app"
  mkdir -p "$HOME/Applications"
fi

echo "Installing → $DEST"
rm -rf "$DEST"
cp -R "$APP_SRC" "$DEST"

# Clear quarantine so Gatekeeper is less noisy for local builds
xattr -dr com.apple.quarantine "$DEST" 2>/dev/null || true

# Touch Info.plist + drop icon caches so Dock/Finder pick up the new icns
touch "$DEST/Contents/Info.plist" 2>/dev/null || true
rm -rf "$HOME/Library/Caches/com.apple.iconservices.store" 2>/dev/null || true
# Best-effort: refresh Dock icon registration without killing user session hard
if command -v /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister >/dev/null 2>&1; then
  /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
    -f "$DEST" 2>/dev/null || true
fi

# Ad-hoc sign so macOS TCC can list "Vibecap" under Screen Recording
if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "$DEST" 2>/dev/null \
    && echo "Ad-hoc codesigned (required for Screen Recording permission entry)." \
    || echo "Warning: codesign failed — Screen Recording may not attach cleanly."
fi

echo "Done."
echo "  App:  $DEST"
echo "  CLI:  $(command -v vibecap 2>/dev/null || echo ~/.cargo/bin/vibecap)"
echo "  Icon: $DEST/Contents/Resources/AppIcon.icns"
echo ""
echo "IMPORTANT (macOS capture):"
echo "  System Settings → Privacy & Security → Screen Recording"
echo "  → enable Vibecap (and Terminal if you use CLI)"
echo "  → fully quit Vibecap (tray Quit) and reopen"
echo ""
echo "If Dock still shows the old icon: Quit Vibecap, then:"
echo "  killall Dock"
echo ""
echo "Open with: open \"$DEST\""
echo "Or Spotlight: Vibecap"
