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

# Optional: also install a cargo-managed binary. Default OFF — we symlink
# ~/.cargo/bin/vibecap → the app binary below so GUI + MCP share one TCC identity.
# Set VIBECAP_CARGO_INSTALL=1 only if you explicitly want a separate cargo copy.
if [[ "${VIBECAP_CARGO_INSTALL:-0}" == "1" ]] && command -v cargo >/dev/null 2>&1; then
  echo "VIBECAP_CARGO_INSTALL=1 → cargo install --path . --force …"
  cargo install --path . --force --quiet 2>/dev/null || cargo install --path . --force
fi

APP_SRC="$ROOT/Vibecap.app"
MACOS_DIR="$APP_SRC/Contents/MacOS"
RES_DIR="$APP_SRC/Contents/Resources"
mkdir -p "$MACOS_DIR" "$RES_DIR"
# Ensure Info.plist exists (packaging template must not ship empty)
if [[ ! -s "$APP_SRC/Contents/Info.plist" ]]; then
  if git -C "$ROOT" show HEAD:Vibecap.app/Contents/Info.plist >"$APP_SRC/Contents/Info.plist" 2>/dev/null; then
    echo "Restored Info.plist from git."
  else
    echo "ERROR: Vibecap.app/Contents/Info.plist missing — cannot install." >&2
    exit 1
  fi
fi
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

# Repo Vibecap.app is only a packaging template — strip its executable so
# Launch Services / Screen Recording never treat the dev tree as a second app.
if [[ -f "$APP_SRC/Contents/MacOS/vibecap" ]]; then
  rm -f "$APP_SRC/Contents/MacOS/vibecap"
fi

# Clear quarantine so Gatekeeper is less noisy for local builds
xattr -dr com.apple.quarantine "$DEST" 2>/dev/null || true

# Touch Info.plist + drop icon caches so Dock/Finder pick up the new icns
touch "$DEST/Contents/Info.plist" 2>/dev/null || true
rm -rf "$HOME/Library/Caches/com.apple.iconservices.store" 2>/dev/null || true

LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -x "$LSREGISTER" ]]; then
  # Drop every known Vibecap.app path so TCC only sees /Applications.
  for stale in \
    "$APP_SRC" \
    "$HOME/Applications/Vibecap.app" \
    "/Applications/Vibecap.app"
  do
    "$LSREGISTER" -u "$stale" 2>/dev/null || true
  done
  "$LSREGISTER" -f "$DEST" 2>/dev/null || true
fi

# Ad-hoc sign the *bundle* so TCC lists "Vibecap" (bundle id app.vibecap.studio).
# Note: each new binary hash can require re-toggling Screen Recording once after upgrades.
if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - --identifier "app.vibecap.studio" "$DEST" 2>/dev/null \
    && echo "Ad-hoc codesigned as app.vibecap.studio (Screen Recording identity)." \
    || echo "Warning: codesign failed — Screen Recording may not attach cleanly."
fi

# Point cargo/MCP CLI at the SAME binary as the app → one Screen Recording entry.
# (cargo install writes a separate unsigned binary, which becomes a second TCC client.)
CARGO_BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
if [[ -d "$CARGO_BIN_DIR" ]]; then
  ln -sfn "$DEST/Contents/MacOS/vibecap" "$CARGO_BIN_DIR/vibecap"
  echo "CLI linked → $DEST/Contents/MacOS/vibecap (shared TCC with the app)"
fi

echo "Done."
echo "  App:  $DEST"
echo "  CLI:  $CARGO_BIN_DIR/vibecap  (symlink → app binary)"
echo "  Icon: $DEST/Contents/Resources/AppIcon.icns"
echo ""
echo "IMPORTANT (macOS Screen Recording — read if captures are bare desktop):"
echo "  1. Fully Quit Vibecap (tray → Quit Vibecap). Quit stray: killall vibecap"
echo "  2. System Settings → Privacy & Security → Screen Recording"
echo "  3. Enable ONLY “Vibecap” (the app). Remove/disable duplicate Vibecap entries."
echo "  4. open \"$DEST\"   — do not use a separate cargo-built binary for GUI"
echo "  Agents using vibecap --mcp now share the same binary/permission as the app."
echo ""
echo "If Dock still shows the old icon: Quit Vibecap, then: killall Dock"
echo "Open with: open \"$DEST\"  or Spotlight: Vibecap"
