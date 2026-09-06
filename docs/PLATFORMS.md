# Platform support

Native capture backends live under `src/platform/`. The **web studio** (`web/`) is HTTP and runs wherever Node does — see [WEB.md](WEB.md).

## Status

| Capability | macOS | Windows | Linux |
| :--- | :--- | :--- | :--- |
| Desktop UI (egui) | ✅ | ✅ | ✅ |
| Headless screenshot | ✅ `screencapture` (`-l` window id when `--window`) | ✅ `ffmpeg gdigrab` (desktop crop; HWND fallback) | ✅ **ffmpeg x11grab** (agent backend); grim only if x11grab fails and no display was named |
| Fixed-duration / unbounded video | ✅ `screencapture -v` / ffmpeg | ✅ `ffmpeg gdigrab` | ✅ **ffmpeg x11grab** |
| Live inspection | ✅ | ✅ (via ffmpeg) | ✅ (X11; Wayland stills: grim fallback) |
| GIF export / wardrobe | ✅ ffmpeg | ✅ ffmpeg | ✅ ffmpeg |
| App focus (`app_name`) | ✅ AppKit / `open -a` | ✅ PowerShell exact then fuzzy (never `start`) | ⚠️ wmctrl / gtk-launch |
| Window crop | ⚠️ stills: `screencapture -l`; record focuses then display | ✅ gdigrab offsets (GPU-safe); HWND if unfocused | ⚠️ wmctrl / xdotool; loud miss in `--paths` |
| System audio in GUI record | ✅ avfoundation | ⚠️ dshow input / `VIBECAP_AUDIO_DEVICE` (switch warns if none) | ⚠️ pulse default |
| Pause recording | ✅ SIGSTOP | ❌ hidden (no-op) | ✅ SIGSTOP |
| Reveal in file manager | ✅ Finder | ✅ Explorer | ✅ parent folder |

Windows is a first-class capture target (gdigrab + dedicated overlay + REC bar). macOS stills can crop a window; macOS *recordings* still focus then capture the display. Interactive region pick on Windows/Linux freezes a snap first (transparent overlays do not composite).

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
| Windows | ffmpeg (`winget install Gyan.FFmpeg`) | chocolatey ffmpeg |
| Linux | **ffmpeg** (x11grab is the supported agent backend); X11/`DISPLAY`; `libxdo-dev` to **link** the desktop binary | `wmctrl` / `xdotool` (window crop), `xdpyinfo`, `grim` (Wayland still fallback) |

Environment knobs:

- `DISPLAY` / `VIBECAP_DISPLAY` — X11 display to grab (`:0`, `:1`)
- `VIBECAP_OUTPUT_DIR` — default output when `--output-dir` is omitted
- `VIBECAP_SCREEN_SIZE` — Linux capture size when xdpyinfo is missing (`1920x1080` default)
- `VIBECAP_AUDIO_DEVICE` — Windows DirectShow *audio input* name for record / voice notes
- `VIBECAP_FFMPEG` — absolute ffmpeg path
- `VIBECAP_BIN` — absolute vibecap path (MCP wrapper + web studio native capturer)
- `VIBECAP_OPEN_FEEDBACK` — set by `vibecap://feedback/<id>` so the GUI selects that Inbox thread

`vibecap doctor` prints backend, ffmpeg path, monitors, GUI stdio, and window-crop hints.

**Windows GUI stdio:** release builds use `windows_subsystem = "windows"`. ffmpeg must not inherit the GUI’s null stdin/stdout (`Could not open file`). All capture jobs go through `platform::run_ffmpeg` (stdin/stdout null, stderr piped).

**Windows hide-for-capture:** never `Visible(false)` — that destroys the region overlay and REC bar. Park off-screen (`src/app/capture_flow.rs`).

Web studio HTTP with `display` / `window` / `output_dir` shells out to the same CLI.

## Backend label

```bash
vibecap --help   # prints “Capture backend: …”
```

Implementation lives under `src/platform/` (`paths`, `capture`, `shell`, `process`).
