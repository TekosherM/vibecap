# MCP Server Reference

Vibecap exposes a **stdio JSON-RPC** Model Context Protocol server for AI coding agents.

## Start the server

```bash
vibecap --mcp
```

### Client config example

```json
{
  "mcpServers": {
    "vibecap": {
      "command": "vibecap",
      "args": ["--mcp"]
    }
  }
}
```

Use the installed binary. Prefer this over `cargo run --manifest-path /absolute/...` so configs stay portable.

| Client | Typical config path |
| :--- | :--- |
| Cursor | `~/.cursor/mcp.json` |
| Claude Desktop | `claude_desktop_config.json` |
| Gemini / Antigravity | MCP config under `~/.gemini/` |

## Protocol

- Transport: **stdio** (one JSON object per line)
- Protocol version advertised: `2024-11-05`
- Methods: `initialize`, `ping`, `tools/list`, `tools/call`, `notifications/initialized`

## Tools (10)

### Capture & media

| Tool | Args | Behavior |
| :--- | :--- | :--- |
| **`vibecap_capture`** | `app_name?` | Full-screen JPG → `~/Movies/Vibecap/screenshot_<ts>.jpg`. Optional focus via `open -a`. Does **not** open the annotation UI (use desktop app for drawings). |
| **`vibecap_record_video`** | `app_name?`, `duration_secs?` (default 5, max 600) | Records MP4 + companion motion GIF. Honors budget caps. |
| **`vibecap_export_gif`** | `video_path`, `start_time`, `end_time` | Timeline GIF via ffmpeg (`fps=15,scale=800:-1:flags=lanczos`). |

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

### Human feedback

| Tool | Args | Behavior |
| :--- | :--- | :--- |
| **`vibecap_request_feedback`** | `media_path`, `question` | Writes request under `~/.config/vibecap/feedback/requests/`; returns `request_id`. |
| **`vibecap_get_feedback`** | `request_id` | Pending until human answers in the app; then text + optional annotated image / voice paths. |

## State on disk

```text
~/.config/vibecap/
  budget.json
  feedback/
    requests/<id>.json
    responses/<id>.json

~/Movies/Vibecap/
  screenshot_*.jpg
  video_*.mp4
  video_*_clip.gif
  live/                  # live inspection frames
```

App UI and MCP share the same budget + feedback directories.

## Smoke test

```bash
./scripts/smoke_mcp.sh
```

See [TESTING.md](TESTING.md).

## Skill file

Canonical agent skill: `skills/vibecap/SKILL.md` (synced to `.agents/skills/vibecap/SKILL.md`).
