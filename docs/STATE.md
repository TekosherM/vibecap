# Current state (read this on resume)

Last updated: 2026-08-23. Source of truth is `master` on GitHub, not a chat transcript.

## Where we are

| | |
| :--- | :--- |
| Tag | [v0.3.0](https://github.com/TekosherM/vibecap/releases/tag/v0.3.0) |
| Tip | `master` — web HTTP studio + `vibecap_job` |
| Connector that works | **CLI** `record start` / `--screenshot` / `record stop` (no MCP). Linux = ffmpeg x11grab. |
| Lumen Cart connector | Open web studio tab + `vibecap_job` |
| Connector that often fails | Native `vibecap --mcp` in Cursor / Grok Bot dynamic-tool harnesses (tools never appear) |

## Agent recipe (signed-in desktop / Chrome flow)

```
vibecap record start --output-dir ./frames --display "$DISPLAY"
vibecap --screenshot --output-dir ./frames
vibecap record stop
```

See `docs/AGENTS.md`. Do **not** use `GET /api/agent/still` without `display`/`output_dir` — that is the demo shutter.

## Agent recipe (Lumen Cart evidence)

```
GET  /api/agent/hooks
POST /api/agent/call  {"tool":"vibecap_job"}
GET  /api/agent/still/{id}.jpg
GET  /api/agent/clip/{id}.webm
```

Job records, walks Lumen Cart (coupon 422 → tax 500 → pay 402, 3 stills), ingests frontend/backend/database/logs, stops, packs. Clip is persisted (`captures.clip_url`) and survives reload.

## Done (do not redo)

- Unbounded record + snap-while-REC
- Walk / coupon / tax / pay + auto stills
- `vibecap_job` one-shot
- WebM persist + Pack Download JSON / stills / clip
- Job re-runnable (resets checkout)
- Screen/camera banner: pixels only; JSON still taps Lumen Cart
- Native `--help` / crate 0.3.0: if MCP never attaches, use the web studio
- CI: Linux `libxdo-dev`, web typecheck, MCP smoke accepts 0.3.x
- Native binaries on v0.3.0 (macOS arm/intel, Linux, Windows)

## Still true (not bugs to “fix” by pretending)

- Native MCP is two processes. Do not rewrite it into the GUI in a drive-by.
- JSON hooks bind to **Lumen Cart**, not a random shared Chrome tab.
- Auth is off. Rows unowned. Neon if `DATABASE_URL`, else PGLite.

## Save rule for this repo

Commit and push to `master` **before** the turn ends. Chat is not durable. If a turn dies, clone `TekosherM/vibecap` @ `master` and continue from this file.
