---
name: vibecap
description: Capture stills and unbounded video of the screen the agent is driving. Prefer the CLI when MCP is not attached. Web studio HTTP can use the same native capturer if display/output_dir is set.
---

# Capture-only agent

Install once, then start → still → stop. **Do not** use the web studio demo shutter for a real Chrome window.

```bash
cargo install --path .          # or ./target/release/vibecap
# Linux: ffmpeg on PATH; echo $DISPLAY  (backend = ffmpeg x11grab)

OUT=/workspace/search-review/run4/frames
mkdir -p "$OUT"

vibecap record start --output-dir "$OUT" --display "$DISPLAY"
# drive the signed-in flow (unbounded — hours are fine)
vibecap --screenshot --output-dir "$OUT" --display "$DISPLAY"
# optional crop: --window "Chrome"
vibecap record stop             # MP4 in $OUT; add --gif for a companion GIF
```

| | |
| :--- | :--- |
| Files | `--output-dir` (required for agent jobs). Default if omitted: `vibecap --paths` |
| Display | `--display` / `$DISPLAY` / `VIBECAP_DISPLAY` |
| Window | `--window` / `--app` (Linux x11grab crop when geometry is found) |
| MCP | `./scripts/vibecap-mcp.sh` via `.cursor/mcp.json` — `record_start` / `capture` / `record_stop` |
| No MCP | This CLI. Cursor / Grok Bot dynamic tools often never list MCP tools. |
| HTTP | `POST /api/agent/call` with `args.display` or `args.output_dir` uses the **same** capturer. Omit them and you get the Lumen Cart shutter. |

`vibecap --help` and `docs/AGENTS.md` match this recipe.

---

## Two connectors (do not mix blindly)

| | Native CLI / MCP | Web studio |
| :--- | :--- | :--- |
| Attach | `vibecap --mcp` **or** the CLI above (no mcp.json needed) | Open tab **or** HTTP native args |
| Real screen | Yes — x11grab / screencapture / gdigrab | Only if `display` / `window` / `output_dir` is set |
| Lumen Cart pack | — | `vibecap_job` (demo subject + JSON hooks) |
| Docs | `docs/AGENTS.md` `docs/MCP.md` | `docs/WEB.md` `docs/HOOKS.md` |

## Web HTTP (Lumen Cart evidence)

```
GET  /api/agent/hooks
POST /api/agent/call   { "tool": "vibecap_job" }
GET  /api/agent/still/{id}.jpg
```

`vibecap_job` walks the **demo** checkout. JSON hooks always bind to Lumen Cart.

## Native MCP (if the harness actually lists tools)

```json
{ "mcpServers": { "vibecap": { "command": "./scripts/vibecap-mcp.sh" } } }
```

`vibecap_record_start` → drive → `vibecap_capture` → `vibecap_record_stop`.
`vibecap_record_video` without `duration_secs` is start (unbounded). With `duration_secs` (max 600) it is a short clip.

## HITL (optional)

Inbox / `request_feedback` + poll only when you need a human. Skip for capture-only jobs.
Full tool list: `docs/MCP.md`.
