# Vibecap Web Studio

HTTP evidence studio for agents. Same Safelight Graphite + amber brand as the native app.

This running tab **is** the connector for Lumen Cart evidence. For a real Chrome/desktop flow, pass `display` / `output_dir` on capture tools (same x11grab CLI) or use `vibecap record start` — see [docs/AGENTS.md](../docs/AGENTS.md).

## What it is

- Unbounded record (no duration). Snap JPEGs while video rolls. Stop when the UI settles.
- Live **hook plan**: DOM, console, HTTP, shell, database, logs — what’s firing, which medium to use.
- One **bug pack**: stills + frontend + backend + database + logs as JSON.
- Downloads from **Media** (JPEG / WebM) and **Pack** (JSON + stills). Nothing is written to `~/Movies/Vibecap`.

Auth is off. Session rows are unowned. Production uses `DATABASE_URL` (Neon); preview uses PGLite.

## Run

```bash
cd web
npm install
npm run dev
```

Then open the studio and leave it up so capture tools can run.

## Agent loop (most jobs)

```
GET  /api/agent/hooks
POST /api/agent/call           { "tool": "vibecap_job" }
GET  /api/agent/still/{id}.jpg
GET  /api/agent/media
GET  /api/agent/help
```

`vibecap_job` records, walks Lumen Cart (coupon 422, tax 500, pay 402, 3 stills), ingests frontend/backend/database/logs, stops, and packs. Granular tools still exist.

Capture tools enqueue while the studio tab is open. Poll `GET /api/agent/result/{id}`.

JSON ingest tools (`vibecap_ingest_*`, `vibecap_hooks`, `vibecap_bug_pack`) run on the server and do not need the tab.

## When to hook

| What’s firing | Tool | Medium |
| :--- | :--- | :--- |
| Wrong UI copy / layout | `vibecap_snapshot` + `ingest_frontend` | JPEG + JSON |
| `console.error` / React warn | `vibecap_ingest_frontend` | JSON |
| 4xx/5xx / stack / compose | `vibecap_ingest_backend` | JSON |
| Wrong stock / price / row | `vibecap_ingest_database` | JSON |
| Multi-step until it settles | `record_start` → snap → `record_stop` | WebM + JPEG |
| Don’t want to choose | `vibecap_bug_pack` | all of the above |

Screen / camera only change the visual medium. DOM, console, HTTP, and DB taps bind to the instrumented subject.

Inbox / annotate / poll loops are optional.

Full write-up: [docs/WEB.md](../docs/WEB.md) · [docs/HOOKS.md](../docs/HOOKS.md) · [skills/vibecap/SKILL.md](../skills/vibecap/SKILL.md)
