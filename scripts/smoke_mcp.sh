#!/usr/bin/env bash
# Lightweight MCP/CLI smoke test for Vibecap.
# Does not require a display. Optional capture tests: SMOKE_CAPTURE=1
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

echo "== CLI =="
HELP_OUT="$("$BIN" --help)"
echo "$HELP_OUT" | grep -q -- '--mcp' && ok "help lists --mcp" || bad "help --mcp" "$HELP_OUT"
echo "$HELP_OUT" | grep -q -- '--screenshot' && ok "help lists --screenshot" || bad "help --screenshot" "$HELP_OUT"
VER_OUT="$("$BIN" --version)"
echo "$VER_OUT" | grep -qE 'vibecap [0-9]+\.[0-9]+\.[0-9]+' && ok "version prints" || bad "version" "$VER_OUT"

echo "== MCP protocol =="
# Multi-request session over one stdio process
MCP_IN="$TMP/mcp_in.jsonl"
MCP_OUT="$TMP/mcp_out.jsonl"
cat >"$MCP_IN" <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"0.0.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"ping"}
{"jsonrpc":"2.0","id":3,"method":"tools/list"}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"vibecap_set_budget","arguments":{"max_frames":10,"max_mb":50,"max_minutes":5,"analysis_tier":"eco"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"vibecap_get_spending","arguments":{}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"vibecap_request_feedback","arguments":{"media_path":"/tmp/vibecap_smoke_missing.jpg","question":"Smoke test — ignore"}}}
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"vibecap_get_feedback","arguments":{"request_id":"does-not-exist-yet"}}}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"vibecap_list_feedback","arguments":{"status":"all"}}}
{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"vibecap_cancel_feedback","arguments":{"request_id":"does-not-exist-yet"}}}
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"vibecap_request_feedback","arguments":{"question":"Smoke text-only decision?","options":["allow","deny"],"priority":"low","agent_label":"smoke","preferred_reply":"choice"}}}
{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"vibecap_stop_live_inspection","arguments":{}}}
{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"vibecap_list_apps","arguments":{}}}
{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"vibecap_set_retro","arguments":{"enabled":false}}}
{"jsonrpc":"2.0","id":14,"method":"tools/call","params":{"name":"vibecap_save_retro","arguments":{}}}
EOF

# Run MCP; close stdin after requests so the server can exit when EOF is hit
# (server may block on stdin forever — give it a timeout via background + kill)
"$BIN" --mcp <"$MCP_IN" >"$MCP_OUT" 2>"$TMP/mcp_err.log" &
MCP_PID=$!
# Wait for responses (max ~8s)
for _ in $(seq 1 40); do
  if [[ -s "$MCP_OUT" ]] && [[ $(wc -l <"$MCP_OUT" | tr -d ' ') -ge 12 ]]; then
    break
  fi
  sleep 0.2
done
# Stop server if still running
kill "$MCP_PID" 2>/dev/null || true
wait "$MCP_PID" 2>/dev/null || true

RESP="$(cat "$MCP_OUT" 2>/dev/null || true)"
if [[ -z "$RESP" ]]; then
  bad "MCP produced no stdout" "$(cat "$TMP/mcp_err.log" 2>/dev/null || true)"
else
  echo "$RESP" | grep -q '"name":"vibecap"' && ok "initialize serverInfo.name" || bad "initialize name" "$RESP"
  echo "$RESP" | grep -q 'vibecap_capture' && ok "tools/list has capture" || bad "tools capture" "$RESP"
  echo "$RESP" | grep -q 'vibecap_record_video' && ok "tools/list has record_video" || bad "tools record" "$RESP"
  echo "$RESP" | grep -q 'vibecap_export_gif' && ok "tools/list has export_gif" || bad "tools gif" "$RESP"
  echo "$RESP" | grep -q 'vibecap_start_live_inspection' && ok "tools/list has live start" || bad "tools live" "$RESP"
  echo "$RESP" | grep -q 'vibecap_get_live_frame' && ok "tools/list has live frame" || bad "tools live frame" "$RESP"
  echo "$RESP" | grep -q 'vibecap_stop_live_inspection' && ok "tools/list has live stop" || bad "tools live stop" "$RESP"
  echo "$RESP" | grep -q 'vibecap_set_budget' && ok "tools/list has set_budget" || bad "tools budget" "$RESP"
  echo "$RESP" | grep -q 'vibecap_get_spending' && ok "tools/list has get_spending" || bad "tools spending" "$RESP"
  echo "$RESP" | grep -q 'vibecap_request_feedback' && ok "tools/list has request_feedback" || bad "tools req fb" "$RESP"
  echo "$RESP" | grep -q 'vibecap_get_feedback' && ok "tools/list has get_feedback" || bad "tools get fb" "$RESP"
  echo "$RESP" | grep -q 'vibecap_list_feedback' && ok "tools/list has list_feedback" || bad "tools list fb" "$RESP"
  echo "$RESP" | grep -q 'vibecap_cancel_feedback' && ok "tools/list has cancel_feedback" || bad "tools cancel fb" "$RESP"
  echo "$RESP" | grep -q 'vibecap_list_apps' && ok "tools/list has list_apps" || bad "tools list_apps" "$RESP"
  echo "$RESP" | grep -q 'vibecap_set_retro' && ok "tools/list has set_retro" || bad "tools set_retro" "$RESP"
  echo "$RESP" | grep -q 'vibecap_save_retro' && ok "tools/list has save_retro" || bad "tools save_retro" "$RESP"
  echo "$RESP" | grep -q 'vibecap_bug_report' && ok "tools/list has bug_report" || bad "tools bug_report" "$RESP"
  TOOL_COUNT=$(echo "$RESP" | tr ',' '\n' | grep -c '"name":"vibecap_' || true)
  if [[ "$TOOL_COUNT" -ge 16 ]]; then
    ok "tools/list reports ≥16 vibecap_* tools ($TOOL_COUNT)"
  else
    bad "tool count" "found $TOOL_COUNT"
  fi
  echo "$RESP" | grep -q 'BUDGET\|budget\|eco\|tier\|spending\|frames' && ok "get_spending/set_budget responded" || bad "budget response" "$RESP"
  echo "$RESP" | grep -q 'request_id\|feedback\|pending\|not found\|No feedback\|⏳\|✅\|Human\|does not exist\|Unknown request' && ok "feedback tools responded" || bad "feedback response" "$RESP"
  echo "$RESP" | grep -q 'Feedback inbox\|No feedback requests\|text-only\|poll required' && ok "list/text-only feedback paths" || bad "list/text-only" "$RESP"
fi

if [[ "${SMOKE_CAPTURE:-0}" == "1" ]]; then
  echo "== Optional capture (SMOKE_CAPTURE=1) =="
  if OUT="$("$BIN" --screenshot 2>"$TMP/shot_err")"; then
    if [[ -f "$OUT" ]]; then
      ok "CLI --screenshot wrote $OUT"
    else
      bad "screenshot path missing" "$OUT"
    fi
  else
    bad "CLI --screenshot" "$(cat "$TMP/shot_err")"
  fi

  # Tiny synthetic video + export_gif via MCP
  SYN="$TMP/clip.mp4"
  if ffmpeg -y -f lavfi -i color=c=orange:s=320x240:d=2 -pix_fmt yuv420p "$SYN" >/dev/null 2>&1; then
    GIF_REQ="$TMP/gif_req.jsonl"
    GIF_OUT="$TMP/gif_out.jsonl"
    cat >"$GIF_REQ" <<EOF
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"vibecap_export_gif","arguments":{"video_path":"$SYN","start_time":"00:00:00","end_time":"00:00:01"}}}
EOF
    "$BIN" --mcp <"$GIF_REQ" >"$GIF_OUT" 2>/dev/null &
    GPID=$!
    sleep 3
    kill "$GPID" 2>/dev/null || true
    wait "$GPID" 2>/dev/null || true
    if grep -q 'Exported timeline GIF\|clip.gif' "$GIF_OUT" 2>/dev/null; then
      ok "export_gif on synthetic clip"
    else
      bad "export_gif" "$(cat "$GIF_OUT" 2>/dev/null | head -c 400)"
    fi
  else
    bad "ffmpeg synthetic clip" "ffmpeg lavfi failed"
  fi
else
  echo "== Optional capture skipped (set SMOKE_CAPTURE=1 to enable) =="
fi

echo
echo "Results: $PASS passed, $FAIL failed"
if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi
echo "All smoke checks passed."
