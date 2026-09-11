#!/usr/bin/env bash
# Portable MCP stdio entry — no machine-specific mcp.json required.
# Cursor / Claude / Grok: point command at this script (see .cursor/mcp.json).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ -n "${VIBECAP_BIN:-}" && -x "$VIBECAP_BIN" ]]; then
  exec "$VIBECAP_BIN" --mcp
fi
for cand in "$ROOT/target/release/vibecap" "$ROOT/target/debug/vibecap"; do
  if [[ -x "$cand" ]]; then
    exec "$cand" --mcp
  fi
done
if command -v vibecap >/dev/null 2>&1; then
  exec vibecap --mcp
fi
echo "vibecap not found. cargo build --release (or cargo install --path .) then retry." >&2
echo "CLI without MCP: vibecap record start --output-dir DIR --display \"\$DISPLAY\"" >&2
exit 1
