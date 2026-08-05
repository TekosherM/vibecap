# Platform support

Vibecap keeps one CLI/MCP surface across OSes. Capture backends are selected at compile time via `src/platform/`.

## Status

| Capability | macOS | Windows | Linux |
| :--- | :--- | :--- | :--- |
| Desktop UI (egui) | ✅ | ✅ (build) | ✅ (build) |
| Headless screenshot | ✅ `screencapture` | ✅ `ffmpeg gdigrab` | ✅ grim / import / `x11grab` |
| Fixed-duration video | ✅ `screencapture -v` | ✅ `ffmpeg gdigrab` | ✅ `ffmpeg x11grab` |
| Live inspection | ✅ | ✅ (via ffmpeg) | ✅ (X11; Wayland via grim for jpg) |
| GIF export / wardrobe | ✅ ffmpeg | ✅ ffmpeg | ✅ ffmpeg |
| App focus (`app_name`) | ✅ `open -a` | ⚠️ best-effort `start` | ⚠️ wmctrl / gtk-launch |
| System audio in GUI record | ✅ avfoundation | ⚠️ limited (set `VIBECAP_AUDIO_DEVICE`) | ⚠️ pulse default |
| Pause recording | ✅ SIGSTOP | ❌ no-op | ✅ SIGSTOP |
| Reveal in file manager | ✅ Finder | ✅ Explorer | ✅ parent folder |

**macOS** is the primary day-to-day target. Windows/Linux compile and use ffmpeg-based capture; interactive region pick and rich audio still lean on macOS quality.

## Paths

| Role | Resolution |
| :--- | :--- |
| Media | `dirs::video_dir()/Vibecap` (else `~/Movies/Vibecap` or `~/Vibecap`) |
| Live frames | `{media}/live` |
| Config / budget / feedback | `dirs::config_dir()/vibecap` |

## Dependencies per OS

| OS | Required | Optional |
| :--- | :--- | :--- |
| macOS | Screen Recording permission, ffmpeg for GIF/editor | — |
| Windows | ffmpeg on `PATH` | chocolatey ffmpeg |
| Linux | ffmpeg; X11 session for x11grab | `grim` (Wayland stills), `wmctrl`, `xdpyinfo` |

Environment knobs:

- `VIBECAP_SCREEN_SIZE` — Linux capture size when xdpyinfo is missing (`1920x1080` default)
- `VIBECAP_AUDIO_DEVICE` — Windows DirectShow audio device name for voice notes

## Backend label

```bash
vibecap --help   # prints “Capture backend: …”
```

Implementation lives under `src/platform/` (`paths`, `capture`, `shell`, `process`).
