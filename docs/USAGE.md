# Usage Guide

## Prerequisites

| Requirement | Why |
| :--- | :--- |
| **OS** | macOS primary; Windows/Linux via ffmpeg (see [PLATFORMS.md](PLATFORMS.md)) |
| **ffmpeg** installed | Screen record, GIF export, filmstrip (GUI also searches Homebrew paths; override with `VIBECAP_FFMPEG`) |
| **Screen Recording** permission (macOS) | System Settings → Privacy & Security → Screen Recording |

Install ffmpeg (Homebrew): `brew install ffmpeg`

## Install

### CLI / MCP (terminal)

```bash
cargo install --path .
# binary: ~/.cargo/bin/vibecap
```

That only installs the command-line binary. It does **not** put an icon in **Applications**.

### macOS app (Finder / Spotlight / Launchpad)

```bash
./scripts/install_macos_app.sh
# → /Applications/Vibecap.app  (or ~/Applications if /Applications is not writable)
open -a Vibecap
```

Or from a clone without installing:

```bash
cargo run --release -- [FLAGS]
```

## CLI

| Command | What it does |
| :--- | :--- |
| `vibecap` | Launch the desktop UI (system tray / menu bar icon) |
| `vibecap --mcp` | Stdio MCP server for AI agents (multiple processes OK) |
| `vibecap --screenshot` | Headless full-screen capture; prints output path |
| `vibecap --hidden` | Start GUI hidden in the tray |
| `vibecap --no-tray` | No tray; window close quits the app |
| `vibecap --version` | Print version |
| `vibecap --help` | Help |

### Multi-instance (human + agents)

Vibecap is designed so **several processes can run at once**:

| Process | Role |
| :--- | :--- |
| `vibecap` (GUI) | Human annotation, feedback inbox, budget supervision, tray |
| `vibecap --mcp` (agent A) | Cursor / Claude / etc. MCP client #1 |
| `vibecap --mcp` (agent B) | Another client or second workspace |

- There is **no single-instance lock**.
- Each MCP process writes live frames under `…/live/session-<pid>/`.
- Budget + feedback stay **shared** via the config directory so the human sees all agent requests.
- Window close **hides to tray** (Quit from the tray menu fully exits).
- **S** / **R** in the focused app take a screenshot or toggle recording; **Ctrl+Shift+3** / **Ctrl+Shift+2** work globally (including from the tray).
- While recording, the tray shows a live **● mm:ss** timer and the menu item becomes **Stop Recording**.
- Screen recording is **video-only by default** (audio off until you enable it).

Headless capture example:

```bash
OUT=$(vibecap --screenshot)
echo "Saved: $OUT"
```

Default save root: platform Videos folder `/Vibecap` (often `~/Movies/Vibecap` on macOS).

## Desktop app

### Tabs

| Tab | Purpose |
| :--- | :--- |
| **Capture** | Fullscreen / region / **window picker**; FPS & audio; retro status when enabled |
| **Library** | Browse saved media; reveal in Finder |
| **Edit** | Trim, GIF range export, filmstrip; wardrobe advanced tools |
| **Feedback** | Agent feedback inbox — answer questions on media |
| **Settings** | Save dir, agent budget, theme, **retro buffer** (off by default) |

### Retro buffer (optional)

Rolling low-FPS screen capture so you can dump “what just happened” after a bug:

1. **Settings → Recording → Enable retro buffer** (stays **off** until you turn it on).
2. Pick a window: **15s / 30s / 60s** (~2 fps, hard-capped at ~200 MB under `~/.config/vibecap/retro_buffer/`).
3. When something breaks, hit **Save last as GIF** (Settings, Capture strip, or ⌘K → “Save retro buffer as GIF”).

Disable anytime — frames are cleared. **Restarting the app keeps frames** (until you disable or Clear). Agents can `vibecap_set_retro` (starts capture in the MCP process) then `vibecap_save_retro`. Requires Screen Recording permission + ffmpeg for the GIF export.

### Record countdown

**Settings → Recording → Countdown:** Off / 3s / 5s. Shows a big-number bubble before ffmpeg starts; **Esc** cancels. Applies to full-screen and region record.

### Bug report pack

One shot for agent bug hunts:

- ⌘K → **Bug report pack**, or Settings / Capture **Bug pack**
- Saves a still (`bug_….jpg`) plus a retro GIF when the buffer has frames  
- Enable retro buffer first if you want the “what just happened” clip

### Window target

1. Capture → **Window**
2. Pick a running app from the combo (↻ refreshes) or type a name  
3. Screenshot / Record focuses that app first, then captures  

Agents: `vibecap_list_apps` then `vibecap_capture` / `vibecap_bug_report` with `app_name`.

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

### Clip studio (video)

Dedicated Loop rail stage for motion media:

- Preview filmstrip first, then range controls
- Set **Start** / **End** timestamps (`HH:MM:SS`)
- **Trim** (stream copy, non-destructive)
- **Export GIF** (15 FPS Lanczos, scale width 800)
- **Extract audio**
- **More tools**: frame extract, mute, compress (CRF 28), rotate, speed

### Still studio (image)

Dedicated Loop rail stage for screenshots and photos:

- Live preview canvas first, then adjust controls
- Crop, rotate, flip, resize, brightness/contrast, grayscale, blur
- **Save edited image** writes a new file beside the source

From **Media**: open video/GIF with **Clip**, screenshots with **Still**.

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
2. Export a range from **Clip** or `vibecap_export_gif`
3. Feed the GIF to the agent’s vision model

### C. Human-in-the-loop feedback

1. Agent: `vibecap_request_feedback(question, media_path?, options?, …)` → `request_id`
2. **Vibecap notifies you** (even if hidden to the menu bar):
   - macOS notification (“Vibecap · agent…”) with sound  
   - Menu bar title becomes **Inbox** / **Inbox N**  
   - Dock bounce · in-app toast · window opens on **Inbox** with the question selected  
3. Human: answer with text / choice chips / voice / **Mark up**
4. Agent: **poll** `vibecap_get_feedback(request_id)` every few seconds (answers are file-based, not pushed into chat)
5. Optional: `vibecap_list_feedback` · `vibecap_cancel_feedback`

Keep **Vibecap running in the tray** while coding with agents — that is the live link.

See [FEEDBACK_USE_CASES.md](FEEDBACK_USE_CASES.md) for 30 supported scenarios.

### D. Live inspection (budget-aware)

1. Optional: `vibecap_set_budget` (frames / MB / minutes + tier)
2. `vibecap_start_live_inspection` (format: `jpg` | `gif` | `mp4`)
3. Poll `vibecap_get_live_frame` + `vibecap_get_spending`
4. `vibecap_stop_live_inspection` when done

See [MCP.md](MCP.md) for full tool schemas.

## Paths

| Path | Role |
| :--- | :--- |
| `{Videos}/Vibecap/` | Screenshots, videos, GIFs |
| `{media}/live/` | Live-inspection stream frames |
| `{config}/vibecap/budget.json` | Agent budget caps |
| `{config}/vibecap/feedback/` | Request / response inbox |
| `.vibecap_temp/` | Optional local temp (gitignored) |

## Troubleshooting

| Symptom | Check |
| :--- | :--- |
| Blank / failed screenshots | Screen Recording permission for Terminal / IDE / `vibecap` |
| GIF / trim fails | `ffmpeg -version` works; input path exists |
| MCP client cannot start server | Use `vibecap` on `PATH`, not a hard-coded machine path |
| Budget always exhausted | Settings → Agent Session & Budget, or delete `~/.config/vibecap/budget.json` |
| Feedback never answers | Desktop app must be open (or polled later after human replies) |
