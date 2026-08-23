# Web Studio (HTTP connector)

The native app speaks **stdio MCP** (`vibecap --mcp`). The web studio speaks **HTTP**. Agents that cannot attach a local MCP process should use this.

**This running studio tab is the connector.** Do not look for `vibecap --mcp`, `~/Movies/Vibecap`, or `~/Vibecap`.

## Surfaces

| Stage | What it is |
| :--- | :--- |
| Shutter | Live subject (demo / screen / camera). Unbounded record. Snap while REC is on. |
| Sources | Hook plan — firing vs quiet, JPEG / WebM / JSON availability, collect per layer. |
| Pack | One JSON bundle. Download JSON + stills. |
| Media | Library. Download JPEG or WebM per card. |
| Still | Annotate a JPEG. |
| Inbox | Optional human questions. Skip unless you need a person. |
| Agent | Live connector: capture-only recipe, hook plan, HTTP sketch. |
| Settings | Graphite / Paper theme, budget tier, new session. |

## Agent HTTP

| Method | Path | Purpose |
| :--- | :--- | :--- |
| GET | `/api/agent/help` | Short capture-only skill |
| GET | `/api/agent/hooks` | Live plan: signals, medium, next tools |
| GET | `/api/agent/status` | Studio heartbeat + plan + tool list |
| POST | `/api/agent/call` | `{ "tool", "args"? }` |
| GET | `/api/agent/result/{id}` | Poll a capture command |
| GET | `/api/agent/media` | Stills with `file` + `data_url` |
| GET | `/api/agent/still/{id}.jpg` | Raw JPEG |

Capture tools (`record_start`, `snapshot`, `record_stop`, `capture`) enqueue until the studio tab executes them. Status `studio.attached` is true while the tab is heartbeating.

JSON tools (`hooks`, `ingest_*`, `bug_pack`, budget, inbox) run immediately on the server.

## Capture-only (most jobs)

1. `GET /api/agent/hooks`
2. `vibecap_job` — record → walk (coupon 422, tax 500, pay 402, **3 stills**) → ingest FE/BE/DB/logs → stop → pack.

Granular: `record_start`, `subject_walk`, `subject_coupon` / `tax` / `pay`, `snapshot`, `record_stop`, `bug_pack`.


`vibecap_record_video` still exists. `duration_secs` is optional; omit it for unbounded, then `record_stop`.

## Output

| Kind | Where |
| :--- | :--- |
| JPEG stills | Tool `data_url`, `GET /api/agent/still/{id}.jpg`, Media → Download JPEG |
| WebM clips | Media → Download clip (session blob). Poster JPEG is stored. |
| Evidence pack | Pack → Download JSON / Download stills |

**Not** `~/Movies/Vibecap` or `~/Vibecap`. Those paths are the native app.

## Database

Auth is **off**. Rows are unowned.

- Preview: embedded PGLite
- Production: `DATABASE_URL` (Neon)

See [HOOKS.md](HOOKS.md) for when to collect each layer.


Subject HTTP: `GET /api/cart`, `GET /api/tax` (500), `POST /api/checkout` (402).
