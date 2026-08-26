#!/usr/bin/env bash
# Nuclear clean + reinstall of Vibecap on macOS.
# Unregisters ALL Vibecap.app copies (including the repo/dev tree), then installs
# only /Applications/Vibecap.app and points ~/.cargo/bin/vibecap at it.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="/Applications/Vibecap.app"
CARGO_BIN="${CARGO_HOME:-$HOME/.cargo}/bin/vibecap"
LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"

echo "== 1) Stop every Vibecap process =="
killall Vibecap 2>/dev/null || true
killall vibecap 2>/dev/null || true
# Catch stubborn MCP children
pkill -f '/[Vv]ibecap' 2>/dev/null || true
sleep 1
if pgrep -lf '[Vv]ibecap' >/dev/null 2>&1; then
  echo "Still running — force kill:"
  pgrep -lf '[Vv]ibecap' || true
  pkill -9 -f '[Vv]ibecap' 2>/dev/null || true
  sleep 1
fi
echo "Processes left: $(pgrep -lf '[Vv]ibecap' || echo none)"

echo ""
echo "== 2) Unregister Launch Services (all known copies) =="
if [[ -x "$LSREGISTER" ]]; then
  while IFS= read -r path; do
    [[ -z "$path" ]] && continue
    echo "  unregister: $path"
    "$LSREGISTER" -u "$path" 2>/dev/null || true
  done < <(mdfind "kMDItemCFBundleIdentifier == 'app.vibecap.studio'" 2>/dev/null || true)
  for path in \
    "$DEST" \
    "$HOME/Applications/Vibecap.app" \
    "$ROOT/Vibecap.app" \
    "/Volumes/2T/Dev/vibecap/Vibecap.app"
  do
    echo "  unregister: $path"
    "$LSREGISTER" -u "$path" 2>/dev/null || true
  done
else
  echo "  (lsregister not found — skip)"
fi

echo ""
echo "== 3) Remove installed apps =="
rm -rf "$DEST"
rm -rf "$HOME/Applications/Vibecap.app"
echo "  removed $DEST (and ~/Applications copy if any)"

echo ""
echo "== 4) Neutralize repo/dev Vibecap.app (keep as packaging shell only) =="
# Do NOT delete Info.plist/Resources — install script needs the template.
# Remove executable so Spotlight/TCC never launches the dev copy.
rm -f "$ROOT/Vibecap.app/Contents/MacOS/vibecap"
# Extra belt: hide from Finder as runnable app if empty MacOS
if [[ -d "$ROOT/Vibecap.app/Contents/MacOS" ]]; then
  rmdir "$ROOT/Vibecap.app/Contents/MacOS" 2>/dev/null || true
  mkdir -p "$ROOT/Vibecap.app/Contents/MacOS"
fi
echo "  stripped $ROOT/Vibecap.app/Contents/MacOS/vibecap"

echo ""
echo "== 5) Unlink CLI / cargo binary =="
if [[ -L "$CARGO_BIN" || -f "$CARGO_BIN" ]]; then
  rm -f "$CARGO_BIN"
  echo "  removed $CARGO_BIN"
else
  echo "  no $CARGO_BIN"
fi
# Other common places
for extra in /usr/local/bin/vibecap "$HOME/bin/vibecap"; do
  if [[ -e "$extra" ]]; then
    rm -f "$extra"
    echo "  removed $extra"
  fi
done

echo ""
echo "== 6) Build release + install ONLY to /Applications =="
cd "$ROOT"
if [[ ! -x target/release/vibecap ]]; then
  cargo build --release
fi
# Force rebuild if source is newer than binary is handled by cargo; always rebuild for clean test:
cargo build --release
VIBECAP_CARGO_INSTALL=0 bash "$ROOT/scripts/install_macos_app.sh"

echo ""
echo "== 7) Verify single identity =="
echo "  apps (mdfind):"
mdfind "kMDItemCFBundleIdentifier == 'app.vibecap.studio'" 2>/dev/null || true
echo "  CLI:"
ls -la "$CARGO_BIN" 2>&1 || true
echo "  codesign:"
codesign -dv "$DEST" 2>&1 | head -6 || true

echo ""
echo "============================================================"
echo " MANUAL (required — macOS will not do this for us):"
echo "============================================================"
echo "1. Open Screen Recording settings:"
echo "   open 'x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture'"
echo ""
echo "2. Remove/disable EVERY old Vibecap row (and Terminal if you no longer need CLI-only capture)."
echo "3. Enable ONLY the remaining Vibecap (aperture icon / from Applications)."
echo "4. Quit System Settings."
echo ""
echo "5. Launch the clean app:"
echo "   open -a Vibecap"
echo ""
echo "6. Smoke test (another app must be frontmost — e.g. Safari, then click Screenshot in Vibecap):"
echo "   # or after granting permission:"
echo "   sleep 2 && open -a Safari && sleep 1 && vibecap --screenshot && ls -lt ~/Movies/Vibecap 2>/dev/null | head -5"
echo "============================================================"
