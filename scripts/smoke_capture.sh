#!/usr/bin/env bash
# Functional capture smoke — requires display + Screen Recording permission (macOS).
# Run before release/commit when capture paths change.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

BIN="${VIBECAP_BIN:-$ROOT/target/release/vibecap}"
if [[ ! -x "$BIN" ]]; then
  echo "Building release binary…"
  cargo build --release
fi

PASS=0
FAIL=0
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

ok() { PASS=$((PASS + 1)); echo "  ✓ $1"; }
bad() { FAIL=$((FAIL + 1)); echo "  ✗ $1"; echo "    $2"; }

echo "== Capture functional smoke =="
echo "Binary: $BIN"
echo "Host: $(uname -s) $(uname -m)"

# 1) CLI full-screen screenshot (caller output dir)
echo "-- CLI --screenshot --output-dir --"
SHOT_DIR="$TMP/frames"
mkdir -p "$SHOT_DIR"
if OUT="$("$BIN" --screenshot --output-dir "$SHOT_DIR" --display "${DISPLAY:-:0}" 2>"$TMP/shot_err")"; then
  if [[ -f "$OUT" ]]; then
    BYTES=$(wc -c <"$OUT" | tr -d ' ')
    if [[ "$BYTES" -gt 20000 ]]; then
      ok "CLI screenshot wrote $OUT ($BYTES bytes)"
    else
      bad "CLI screenshot too small (likely blank/black)" "$OUT is $BYTES bytes"
    fi
  else
    bad "CLI screenshot path missing" "$OUT / $(cat "$TMP/shot_err")"
  fi
else
  bad "CLI --screenshot failed" "$(cat "$TMP/shot_err")"
fi

# 2) Focus Finder, then MCP capture (should not be empty desktop of nothing)
if [[ "$(uname -s)" == "Darwin" ]]; then
  echo "-- Focus Finder + MCP capture --"
  open -a Finder || true
  sleep 0.6
  MCP_IN="$TMP/mcp_in.jsonl"
  MCP_OUT="$TMP/mcp_out.jsonl"
  cat >"$MCP_IN" <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"capture-smoke","version":"0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"vibecap_capture","arguments":{"app_name":"Finder"}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"vibecap_list_apps","arguments":{}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"vibecap_set_retro","arguments":{"enabled":false}}}
EOF
  "$BIN" --mcp <"$MCP_IN" >"$MCP_OUT" 2>"$TMP/mcp_err.log" &
  MPID=$!
  for _ in $(seq 1 40); do
    if [[ -s "$MCP_OUT" ]] && [[ $(wc -l <"$MCP_OUT" | tr -d ' ') -ge 3 ]]; then
      break
    fi
    sleep 0.25
  done
  kill "$MPID" 2>/dev/null || true
  wait "$MPID" 2>/dev/null || true
  RESP="$(cat "$MCP_OUT" 2>/dev/null || true)"
  if echo "$RESP" | grep -q 'Captured screenshot successfully'; then
    # Extract path after "to "
    PATH_CAP=$(echo "$RESP" | tr '"' '\n' | grep -E 'screenshot_.*\.jpg|Movies/Vibecap|Vibecap/' | head -1 || true)
    # Parse from text field more carefully
    SHOT=$(echo "$RESP" | sed -n 's/.*Captured screenshot successfully to \([^"\\]*\).*/\1/p' | head -1)
    if [[ -n "$SHOT" && -f "$SHOT" ]]; then
      BYTES=$(wc -c <"$SHOT" | tr -d ' ')
      if [[ "$BYTES" -gt 20000 ]]; then
        ok "MCP capture with app_name=Finder ($BYTES bytes)"
      else
        bad "MCP capture too small" "$SHOT is $BYTES bytes"
      fi
    else
      # Still pass list if capture text present but path parse failed — check media dir newest
      NEWEST=$(ls -t "$HOME/Movies/Vibecap"/screenshot_*.jpg 2>/dev/null | head -1 || true)
      if [[ -n "$NEWEST" ]]; then
        BYTES=$(wc -c <"$NEWEST" | tr -d ' ')
        AGE=$(( $(date +%s) - $(stat -f%m "$NEWEST") ))
        if [[ "$AGE" -lt 30 && "$BYTES" -gt 20000 ]]; then
          ok "MCP capture (newest media shot $BYTES bytes, age ${AGE}s)"
        else
          bad "MCP capture path parse / freshness" "shot=$SHOT newest=$NEWEST bytes=$BYTES age=$AGE resp=$(echo "$RESP" | head -c 200)"
        fi
      else
        bad "MCP capture no file" "$RESP"
      fi
    fi
  else
    bad "MCP capture response" "$RESP / $(cat "$TMP/mcp_err.log" 2>/dev/null | head -c 300)"
  fi

  if echo "$RESP" | grep -qiE 'Finder|Running apps'; then
    ok "MCP list_apps responded"
  else
    bad "MCP list_apps" "$RESP"
  fi
fi

# 3) Unit + protocol smoke (always)
echo "-- cargo test + protocol smoke --"
if cargo test --bin vibecap --quiet 2>"$TMP/test_err"; then
  ok "cargo test --bin vibecap"
else
  bad "cargo test" "$(cat "$TMP/test_err" | tail -20)"
fi
if SMOKE_CAPTURE=0 ./scripts/smoke_mcp.sh >"$TMP/proto.out" 2>&1; then
  ok "protocol smoke_mcp.sh"
else
  bad "protocol smoke" "$(tail -20 "$TMP/proto.out")"
fi

# 4) Design gates
if [[ "$(wc -l < src/main.rs | tr -d ' ')" -lt 2800 ]]; then
  ok "main.rs line budget (<2800)"
else
  bad "main.rs lines" "$(wc -l < src/main.rs)"
fi
if ! grep -rn 'Color32::from_rgb' src --include='*.rs' | grep -v theme.rs >/dev/null; then
  ok "G4 theme tokens"
else
  bad "G4 raw Color32 outside theme" "$(grep -rn 'Color32::from_rgb' src --include='*.rs' | grep -v theme.rs)"
fi

echo ""
echo "Results: $PASS passed, $FAIL failed"
if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi
echo "All capture functional checks passed."
echo "Manual still required: GUI Fullscreen with Safari/Finder open (see docs/TESTING.md)."
