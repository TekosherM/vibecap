---
name: vibecap
description: Capture stills and unbounded video, and collect frontend / backend / database / log evidence. Use the web HTTP studio when MCP is not attached; use vibecap --mcp for the native desktop app.
---

# Vibecap — capture-only

Two connectors. Pick one. Do not mix paths.

| | Native (desktop) | Web studio |
| :--- | :--- | :--- |
| Attach | Keep `vibecap` running, then `vibecap --mcp` | **The open studio tab is the connector.** No `--mcp`. |
| Output | `{Videos}/Vibecap` or `~/Movies/Vibecap` | Pack + Media downloads. JPEG inline. **Not** `~/Movies`. |
| Docs | `docs/MCP.md` `docs/USAGE.md` | `docs/WEB.md` `docs/HOOKS.md` |

---

## Web HTTP (use this if MCP never attached)

```
GET  /api/agent/hooks
POST /api/agent/call   { "tool": "vibecap_job" }
GET  /api/agent/still/{id}.jpg
GET  /api/agent/clip/{id}.webm
GET  /api/agent/help
```

`vibecap_job` is the whole checkout evidence run: record (no duration), walk (coupon 422, tax 500, pay 402, 3 stills), ingest frontend/backend/database/logs, stop, pack. JPEG inline. Not ~/Movies.

Capture tools need the studio tab open (`studio.attached`). Poll `GET /api/agent/result/{id}`.

### When to hook

| Signal | Tool | Medium |
| :--- | :--- | :--- |
| Pixels / layout / wrong UI copy | `snapshot` or `capture` | JPEG |
| Full checkout evidence | `vibecap_job` | WebM + 3 JPEGs + JSON pack |
| Console / DOM | `vibecap_ingest_frontend` | JSON |
| 4xx/5xx / stack / shell | `vibecap_ingest_backend` | JSON |
| Wrong stock / price / row | `vibecap_ingest_database` | JSON |
| Don’t want to choose | `vibecap_bug_pack` | all |
| Screen / camera | pixels only | JSON still taps Lumen Cart |

Inbox / annotate / poll loops are **optional**. Skip them unless you need a human.

---

## Native MCP

```bash
vibecap              # GUI + tray — leave running
vibecap --mcp        # stdio sidecar
```

```json
{ "mcpServers": { "vibecap": { "command": "vibecap", "args": ["--mcp"] } } }
```

Capture-only here too: `vibecap_capture` (still), `vibecap_record_video` (`duration_secs` required on native), then read the printed path under the media dir.

HITL (`request_feedback` + poll) is optional. Full tool list: `docs/MCP.md`.
