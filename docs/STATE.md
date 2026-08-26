# Current state (read this on resume)

Last updated: 2026-08-26. Source of truth is `master` on GitHub, not a chat transcript.

## Where we are

| | |
| :--- | :--- |
| Tag | [v0.3.0](https://github.com/TekosherM/vibecap/releases/tag/v0.3.0) |
| Tip | `master` — web HTTP studio + `vibecap_job` + agent record start/stop |
| Connector that works | **CLI** `record start` / `--screenshot` / `record stop`. Linux = ffmpeg x11grab. |
| Lumen Cart connector | Open web studio tab + `vibecap_job` |
| MCP tools (when harness surfaces them) | `vibecap_record_start/stop/status`, `vibecap_record_video`, `vibecap_capture`, `vibecap_export_gif`, live-inspection + budget + feedback tools |
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
- Agent record path fixed: start/stop, still-to-dir, real Linux screen (PR #22)
- MCP: `vibecap_record_start/stop/status` + `vibecap_capture` with display/window/output_dir; `record_stop` returns persisted clip_path
- Web HTTP studio shells to the same CLI when `display`/`output_dir` args are passed
- Desktop UI polish pass 2026-08-25: library heading de-accented, capture live-stats row, inbox auto-select respects explicit picks, ⌘I opens Inbox, import rot pruned (cargo check clean of unused imports)
- GUI screenshot focus hardening 2026-08-26: bare-desktop shots refused with guidance when no focus target; `focus_app` verifies + retries + reports failure
- Agent recorder detach 2026-08-26: `record start` returns immediately, ffmpeg in own process group, stderr → `.ffmpeg.log`; honest crash status; absolute output paths (stop/status cwd-independent). Verified end-to-end on macOS: piped-shell start returns in ~140ms, mp4 finalizes, GIF companion works
- Known gaps (do not rediscover): `record stop --gif` is a synchronous full-clip transcode — can exceed agent timeouts on long clips; `--window` focuses but does not crop on macOS (crop is Linux-only)
- CI: Linux `libxdo-dev`, web typecheck, MCP smoke accepts 0.3.x
- Native binaries on v0.3.0 (macOS arm/intel, Linux, Windows)

## Still true (not bugs to “fix” by pretending)

- Native MCP is two processes. Do not rewrite it into the GUI in a drive-by.
- JSON hooks bind to **Lumen Cart**, not a random shared Chrome tab.
- Auth is off. Rows unowned. Neon if `DATABASE_URL`, else PGLite.

## Save rule for this repo

Commit and push to `master` **before** the turn ends. Chat is not durable. If a turn dies, clone `TekosherM/vibecap` @ `master` and continue from this file.
