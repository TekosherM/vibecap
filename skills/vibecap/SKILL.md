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
POST /api/agent/call   { "tool": "vibecap_record_start" }
POST /api/agent/call   { "tool": "vibecap_subject_pay" }   # walk checkout — 402
POST /api/agent/call   { "tool": "vibecap_snapshot" }      # while REC is on
POST /api/agent/call   { "tool": "vibecap_record_stop" }
POST /api/agent/call   { "tool": "vibecap_bug_pack" }
GET  /api/agent/still/{id}.jpg
GET  /api/agent/help
```

`record_start` has **no duration**. `subject_pay` walks Lumen Cart (tax throw + Stripe 402) without stopping REC. Snap the failure frame, then stop.

Capture tools need the studio tab open (`studio.attached`). Poll `GET /api/agent/result/{id}`.

### When to hook

| Signal | Tool | Medium |
| :--- | :--- | :--- |
| Pixels / layout / wrong UI copy | `snapshot` or `capture` | JPEG |
| Multi-step until it settles | `record_start` → `subject_pay` → `record_stop` | WebM + JPEG |
| Console / DOM | `vibecap_ingest_frontend` | JSON |
| 4xx/5xx / stack / shell | `vibecap_ingest_backend` | JSON |
| Wrong stock / price / row | `vibecap_ingest_database` | JSON |
| Don’t want to choose | `vibecap_bug_pack` | all |


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
