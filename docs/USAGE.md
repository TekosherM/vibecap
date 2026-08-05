# Usage Guide

## Prerequisites

| Requirement | Why |
| :--- | :--- |
| **macOS** (today) | Capture uses `screencapture` / `open -a` |
| **ffmpeg** on `PATH` | GIF export, filmstrip, wardrobe video tools |
| **Screen Recording** permission | System Settings → Privacy & Security → Screen Recording |

Install ffmpeg (Homebrew): `brew install ffmpeg`

## Install

```bash
cargo install --path .
# binary: ~/.cargo/bin/vibecap
```

Or from a clone without installing:

```bash
cargo run --release -- [FLAGS]
```

## CLI

| Command | What it does |
| :--- | :--- |
| `vibecap` | Launch the desktop UI |
| `vibecap --mcp` | Stdio MCP server for AI agents |
| `vibecap --screenshot` | Headless full-screen capture; prints output path |
| `vibecap --version` | Print version |
| `vibecap --help` | Help |

Headless capture example:

```bash
OUT=$(vibecap --screenshot)
echo "Saved: $OUT"
```

Default save root: `~/Movies/Vibecap/`

## Desktop app

### Tabs

| Tab | Purpose |
| :--- | :--- |
| **Capture** | Fullscreen / region / window recording; FPS & audio toggles |
| **Library** | Browse saved media; reveal in Finder |
| **Edit** | Trim, GIF range export, filmstrip; wardrobe advanced tools |
| **Feedback** | Agent feedback inbox — answer questions on media |
| **Settings** | Save dir, agent budget supervision |

### Annotation Studio

Triggered after a screenshot from the UI (or when annotating a feedback request):

| Tool | Use |
| :--- | :--- |
| Pen / Arrow / Rect / Highlight | Visual mark-up |
| Text | Labels |
| Blur | Redact sensitive regions |
| Step badge | Numbered steps `[1]` `[2]` … |
| Voice note | Optional `.m4a` beside the image |
| Text note | Optional `.txt` context for agents |

Save exports annotated JPG (+ optional `.txt` / `.m4a`) into the media folder.

### Video editor (Edit tab)

- Set **Start** / **End** timestamps (`HH:MM:SS`)
- **Trim** (stream copy, non-destructive)
- **Export GIF** (15 FPS Lanczos, scale width 800)
- **Extract audio**
- **Wardrobe → Advanced Video**: frame extract, mute, compress (CRF 28), rotate, speed
- **Wardrobe → Image Editor**: crop, rotate, flip, resize, brightness/contrast, grayscale, blur

### Hotkeys

| Shortcut | Action |
| :--- | :--- |
| **Ctrl + Shift + 2** | Start / stop recording |

## Agent workflows

### A. One-shot screenshot for vision

```bash
vibecap --screenshot
# or via MCP: vibecap_capture
```

### B. Motion analysis

1. Record with UI or `vibecap_record_video`
2. Export a range with Edit tab or `vibecap_export_gif`
3. Feed the GIF to the agent’s vision model

### C. Human-in-the-loop feedback

1. Agent: `vibecap_request_feedback(media_path, question)` → `request_id`
2. Human: open Vibecap → **Feedback** tab → answer (text / voice / annotate)
3. Agent: poll `vibecap_get_feedback(request_id)`

### D. Live inspection (budget-aware)

1. Optional: `vibecap_set_budget` (frames / MB / minutes + tier)
2. `vibecap_start_live_inspection` (format: `jpg` | `gif` | `mp4`)
3. Poll `vibecap_get_live_frame` + `vibecap_get_spending`
4. `vibecap_stop_live_inspection` when done

See [MCP.md](MCP.md) for full tool schemas.

## Paths

| Path | Role |
| :--- | :--- |
| `~/Movies/Vibecap/` | Screenshots, videos, GIFs |
| `~/Movies/Vibecap/live/` | Live-inspection stream frames |
| `~/.config/vibecap/budget.json` | Agent budget caps |
| `~/.config/vibecap/feedback/` | Request / response inbox |
| `.vibecap_temp/` | Optional local temp (gitignored) |

## Troubleshooting

| Symptom | Check |
| :--- | :--- |
| Blank / failed screenshots | Screen Recording permission for Terminal / IDE / `vibecap` |
| GIF / trim fails | `ffmpeg -version` works; input path exists |
| MCP client cannot start server | Use `vibecap` on `PATH`, not a hard-coded machine path |
| Budget always exhausted | Settings → Agent Session & Budget, or delete `~/.config/vibecap/budget.json` |
| Feedback never answers | Desktop app must be open (or polled later after human replies) |
