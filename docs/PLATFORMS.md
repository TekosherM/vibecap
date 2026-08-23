# Platform support

Native capture backends live under `src/platform/`. The **web studio** (`web/`) is HTTP and runs wherever Node does — see [WEB.md](WEB.md).

## Status

| Capability | macOS | Windows | Linux |
| :--- | :--- | :--- | :--- |
| Desktop UI (egui) | ✅ | ✅ (build) | ✅ (build) |
| Headless screenshot | ✅ `screencapture` | ✅ `ffmpeg gdigrab` | ✅ **ffmpeg x11grab** (agent backend); grim only if x11grab fails and no display was named |
| Fixed-duration / unbounded video | ✅ `screencapture -v` / ffmpeg | ✅ `ffmpeg gdigrab` | ✅ **ffmpeg x11grab** |
| Live inspection | ✅ | ✅ (via ffmpeg) | ✅ (X11; Wayland stills: grim fallback) |
| GIF export / wardrobe | ✅ ffmpeg | ✅ ffmpeg | ✅ ffmpeg |
| App focus (`app_name`) | ✅ `open -a` | ⚠️ best-effort `start` | ⚠️ wmctrl / gtk-launch |
| System audio in GUI record | ✅ avfoundation | ⚠️ limited (set `VIBECAP_AUDIO_DEVICE`) | ⚠️ pulse default |
| Pause recording | ✅ SIGSTOP | ❌ no-op | ✅ SIGSTOP |
| Reveal in file manager | ✅ Finder | ✅ Explorer | ✅ parent folder |

**macOS** is the primary day-to-day target. Windows/Linux compile and use ffmpeg-based capture; interactive region pick and rich audio still lean on macOS quality.

## Paths

| Role | Resolution |
| :--- | :--- |
| Media | `--output-dir` or `dirs::video_dir()/Vibecap` (else `~/Movies/Vibecap` on macOS, else `~/Vibecap`). Print with `vibecap --paths`. |
| Live frames | `{media}/live` |
| Config / budget / feedback | `dirs::config_dir()/vibecap` |

## Dependencies per OS

| OS | Required | Optional |
| :--- | :--- | :--- |
| macOS | Screen Recording permission, ffmpeg for GIF/editor | — |
| Windows | ffmpeg on `PATH` | chocolatey ffmpeg |
| Linux | **ffmpeg** (x11grab is the supported agent backend); X11/`DISPLAY`; `libxdo-dev` to **link** the desktop binary | `wmctrl` / `xdotool` (window crop), `xdpyinfo`, `grim` (Wayland still fallback) |

Environment knobs:

- `DISPLAY` / `VIBECAP_DISPLAY` — X11 display to grab (`:0`, `:1`)
- `VIBECAP_OUTPUT_DIR` — default output when `--output-dir` is omitted
- `VIBECAP_SCREEN_SIZE` — Linux capture size when xdpyinfo is missing (`1920x1080` default)
- `VIBECAP_AUDIO_DEVICE` — Windows DirectShow audio device name for voice notes
- `VIBECAP_FFMPEG` — absolute ffmpeg path
- `VIBECAP_BIN` — absolute vibecap path (MCP wrapper + web studio native capturer)

Web studio HTTP with `display` / `window` / `output_dir` shells out to the same CLI.

## Backend label

```bash
vibecap --help   # prints “Capture backend: …”
```

Implementation lives under `src/platform/` (`paths`, `capture`, `shell`, `process`).
