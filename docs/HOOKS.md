# Hook plan — when to collect what

The studio and the agent share one plan (`GET /api/agent/hooks` / `vibecap_hooks`). Neither should guess.

## Mediums

| Medium | Available when | Use for |
| :--- | :--- | :--- |
| **JPEG** | Studio tab attached (demo, screen, or camera) | Pixels as they are right now |
| **WebM** | Same. Unbounded record — no duration | Multi-step flow until the UI settles |
| **JSON** | Always (server-side) | DOM, console, HTTP, shell, database, session logs |

Screen / camera only change the **visual** medium. DOM, console, HTTP, and DB taps bind to the **instrumented subject**, not a random browser tab.

## When → hook → medium

| What’s firing | Hook | Tool | Skip when |
| :--- | :--- | :--- | :--- |
| Wrong total / label / clipped CTA | DOM | `vibecap_ingest_frontend` | Server-only 500 with no UI change |
| Uncaught, React warn, failed fetch in console | Browser console | `vibecap_ingest_frontend` | Quiet console + visual-only bug — a still is enough |
| Status ≥ 400, body contradicts UI | HTTP traces | `vibecap_ingest_backend` | UI-only copy with 200s |
| Stack trace, process crash, throwing line | Shell / runtime | `vibecap_ingest_backend` | No server log for this repro |
| Wrong stock / price / row | Database | `vibecap_ingest_database` | Bug never touches persisted state |
| Need a timeline of collections | Session logs | `vibecap_ingest_logs` | First minute — collect layers first |
| Need pixels now / failure frame | Still | `vibecap_snapshot` (or `capture`) | Need motion — use record |
| Cart → pay → error, timing, race | Video | `record_start` … `record_stop` | Single static screen — one still is cheaper |
| Don’t want to choose | Pack | `vibecap_bug_pack` | — |

## Typical checkout job

```
hooks            → see 2 console errors, HTTP 402, tax 500, stock 0, 4 DOM issues
record_start     → camera on, no duration
snapshot         → each failure frame (JPEG inline)
ingest_frontend  → DOM + console
ingest_backend   → HTTP + compose
ingest_database  → catalog_items (LM-15 stock 0)
record_stop      → poster + clip in Media
bug_pack         → one JSON
```

Inbox is optional. Do not poll it unless you asked a human a question.
