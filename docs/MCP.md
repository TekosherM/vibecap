# MCP Server Reference

Vibecap native exposes a **stdio JSON-RPC** Model Context Protocol server.

If your harness **never lists** these tools (common on Cursor / Grok Bot dynamic-tool
runtimes), **do not wait for MCP**. Use the CLI in [AGENTS.md](AGENTS.md):

```bash
vibecap record start --output-dir ./frames --display "$DISPLAY"
vibecap --screenshot --output-dir ./frames
vibecap record stop
```

Web HTTP can call the **same capturer** when `args.display` / `output_dir` is set
([WEB.md](WEB.md)). `vibecap_job` is the Lumen Cart demo pack, not a random Chrome tab.

## Start the native server

```bash
./scripts/vibecap-mcp.sh
# or: vibecap --mcp
```

### Portable client config

Checked in: [`.cursor/mcp.json`](../.cursor/mcp.json) and [`mcp.json`](../mcp.json).

```json
{
  "mcpServers": {
    "vibecap": {
      "command": "./scripts/vibecap-mcp.sh"
    }
  }
}
```

The script finds `target/release/vibecap`, `VIBECAP_BIN`, or `vibecap` on `PATH`.
No machine-specific absolute path.

| Client | Typical config path |
| :--- | :--- |
| Cursor | project `.cursor/mcp.json` (this repo) or `~/.cursor/mcp.json` |
| Claude Desktop | `claude_desktop_config.json` |
| Gemini / Antigravity | MCP config under `~/.gemini/` |

## Protocol

- Transport: **stdio** (one JSON object per line)
- Protocol version advertised: `2024-11-05`
- Methods: `initialize`, `ping`, `tools/list`, `tools/call`, `notifications/initialized`

## Tools (19)

### Capture & media

| Tool | Args | Behavior |
| :--- | :--- | :--- |
| **`vibecap_capture`** | `output_dir?`, `display?`, `window?`, `app_name?` | Still of the **named display / window** (Linux: ffmpeg x11grab). JPEG → `output_dir` or the default from `vibecap --paths`. |
| **`vibecap_record_start`** | `output_dir?`, `display?`, `window?`, `gif?` | Start **unbounded** MP4. Drive the flow; then `record_stop`. |
| **`vibecap_record_stop`** | `gif?` | Stop and finalize the MP4 (optional companion GIF). |
| **`vibecap_record_status`** | — | Live? pid, elapsed, path. |
| **`vibecap_record_video`** | `duration_secs?`, `output_dir?`, `display?`, `window?`, `gif?` | **Omit** `duration_secs` → same as start (unbounded). Set it (max 600) for a short clip + GIF. |
| **`vibecap_export_gif`** | `video_path`, `start_time`, `end_time` | Timeline GIF via ffmpeg (`fps=15,scale=800:-1:flags=lanczos`). |
| **`vibecap_list_apps`** | — | Running app names for window focus / `app_name` args. |
| **`vibecap_bug_report`** | `app_name?` | Still + retro GIF when frames exist (one-shot bug pack). |

### Retro buffer (shared with desktop app)

Off by default. Frames live under config `retro_buffer/`.

| Tool | Args | Behavior |
| :--- | :--- | :--- |
| **`vibecap_set_retro`** | `enabled` (bool) | Writes `retro.json` (desktop reloads ≤~2s). **Also** starts/stops a capturer in this MCP process so headless agents get frames. Disable clears frames. |
| **`vibecap_save_retro`** | — | Export ring buffer JPG frames → media `retro_<ts>.gif`. |

**Contract:** Prefer `set_retro enabled=true` before the user reproduces a bug; wait a few seconds; then `save_retro` or `bug_report`. Frames survive GUI restart (not wiped on quit); explicit disable clears them.

### Live inspection

| Tool | Args | Behavior |
| :--- | :--- | :--- |
| **`vibecap_start_live_inspection`** | `app_name?`, `format?` (`gif`/`jpg`/`mp4`), `interval_secs?`, `output_dir?` | Background rolling frames. Defaults from analysis tier when args omitted. |
| **`vibecap_get_live_frame`** | — | Latest frame path + disk usage for the session. |
| **`vibecap_stop_live_inspection`** | — | Stop stream; report storage summary. |

### Budget (cost control)

| Tool | Args | Behavior |
| :--- | :--- | :--- |
| **`vibecap_set_budget`** | `max_frames?`, `max_mb?`, `max_minutes?`, `analysis_tier?` | Caps (0 = unlimited). Shared with app Settings. |
| **`vibecap_get_spending`** | — | Frames / MB / minutes vs caps, tier, exhausted status. |

**Tiers** (drive live defaults when format/interval omitted):

| Tier | Default format | Default interval | Intent |
| :--- | :--- | :--- | :--- |
| `eco` | `jpg` | ≥ 5s | Cheapest vision |
| `standard` | `gif` | ~3s | Balanced |
| `intensive` | gif/mp4 | 1s | Richest, costly |

Enforcement is live: streams auto-stop at caps; new streams/recordings are refused when exhausted. Corrupt `budget.json` fails closed.

### Human feedback (poll-based)

**Answers are not pushed into the agent chat.** The human replies in the app **🤖 Agent Inbox**; agents must poll.

| Tool | Args | Behavior |
| :--- | :--- | :--- |
| **`vibecap_request_feedback`** | `question` (required), `media_path?`, `options?`, `priority?`, `agent_label?`, `preferred_reply?`, `context?` | Writes request under config `feedback/requests/`; returns `request_id`. `media_path` optional for decision-only questions. |
| **`vibecap_get_feedback`** | `request_id` | `pending` until answered / cancelled / dismissed. Then: text, `choice`, annotated image, and/or voice path. Empty text + annotated path is valid (vision the PNG). |
| **`vibecap_list_feedback`** | `status?` (`all` / `pending` / `closed`) | Recover inbox after restart; max 50 lines. |
| **`vibecap_cancel_feedback`** | `request_id` | Agent abandons a pending request (`status=cancelled`). |

Full use-case matrix: [FEEDBACK_USE_CASES.md](FEEDBACK_USE_CASES.md).

## State on disk

```text
~/.config/vibecap/
  budget.json
  feedback/
    requests/<id>.json
    responses/<id>.json

{output_dir or default from `vibecap --paths`}/
  screenshot_*.jpg
  video_*.mp4
  video_*_clip.gif
  .vibecap-record.json   # live unbounded session breadcrumb
  live/                  # live inspection frames
```

Default media dir is **one** path: `dirs::video_dir()/Vibecap`, else `~/Movies/Vibecap` (macOS), else `~/Vibecap`. Agents should pass `output_dir`.

App UI and MCP share the same budget + feedback directories.

## Smoke test

```bash
./scripts/smoke_mcp.sh
```

See [TESTING.md](TESTING.md).

## Skill file

Canonical agent skill: `skills/vibecap/SKILL.md` (synced to `.agents/skills/vibecap/SKILL.md`).
