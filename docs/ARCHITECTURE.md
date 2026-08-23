# Architecture

Two products, one repo:

| | Native | Web (`web/`) |
| :--- | :--- | :--- |
| Process | One Rust binary: UI + MCP | TanStack Start HTTP studio |
| Agent attach | `vibecap --mcp` (stdio) | **The open tab is the connector** |
| State | Files under `~/.config/vibecap` | Unowned Postgres (Neon / PGLite) |
| Output | `{Videos}/Vibecap` | Pack JSON + JPEG/WebM downloads |

The native crate stays **one binary**. No service mesh. MCP is stdio only.

---

## Native process modes

```text
vibecap                 →  eframe/egui desktop app
vibecap --mcp            →  stdio JSON-RPC MCP server (no window)
vibecap --screenshot     →  one-shot headless capture, then exit
```

Mode is chosen in `main()` before any UI init.

## Native layers

```text
┌────────────────────────────────────────────┐
│  Presentation                              │
│  eframe / egui tabs: Capture · Library ·   │
│  Edit · Feedback · Settings · Annotation   │
└─────────────────┬──────────────────────────┘
                  │
┌─────────────────▼──────────────────────────┐
│  Capture & media jobs                      │
│  macOS: screencapture, open -a             │
│  ffmpeg: record (avfoundation), GIF, trim, │
│  wardrobe transforms (background threads)  │
└─────────────────┬──────────────────────────┘
                  │
┌─────────────────▼──────────────────────────┐
│  Shared agent state (~/.config/vibecap)    │
│  budget.json · feedback requests/responses │
│  atomic write (tmp + rename)               │
└─────────────────┬──────────────────────────┘
                  │
┌─────────────────▼──────────────────────────┐
│  MCP stdio loop                            │
│  tools/list · tools/call · initialize      │
│  live inspection thread + budget gates     │
└────────────────────────────────────────────┘
```

## Design choices (lightweight)

| Choice | Rationale |
| :--- | :--- |
| Single `main.rs` binary | Fast to ship; agents install one tool |
| Shell out to `ffmpeg` / `screencapture` | No heavy media crates; OS-quality capture |
| File-based budget/feedback | App and MCP share state without IPC |
| egui 0.28 | Small immediate-mode UI, no web stack |
| No network server | MCP over stdio only — local, private |

## Dependencies (runtime)

| Crate | Role |
| :--- | :--- |
| `eframe` / `egui` / `egui_extras` | Desktop UI |
| `image` | Annotation / image wardrobe |
| `arboard` | Clipboard |
| `rfd` | File dialogs |
| `global-hotkey` | Ctrl+Shift+2 |
| `crossbeam-channel` | UI ↔ worker jobs |
| `serde` / `serde_json` | MCP + config |
| `chrono` | Timestamps |

External binaries: **ffmpeg**, **screencapture** (macOS).

---

## Web studio (`web/`)

HTTP evidence studio for agents that cannot attach stdio MCP.

```text
Browser (Safelight UI)
  shutter / sources / pack / media / still / inbox / agent
        │
        ├─ capture-engine   demo | screen | camera
        │                   unbounded MediaRecorder + JPEG snaps
        │
        └─ POST /api/agent/call
              capture tools  → command queue → studio tab executes
              JSON tools     → ingest frontend / backend / db / logs
              bug_pack       → one JSON + stills
```

- Capture tools need the tab heartbeating (`studio.attached`).
- JSON hooks bind to the **instrumented subject**, not a random screen share.
- Stills: inline `data_url` + `GET /api/agent/still/{id}.jpg`. Not `~/Movies`.

See [WEB.md](WEB.md) and [HOOKS.md](HOOKS.md).

## Platform module

```text
src/platform/
  mod.rs      # re-exports + capture_backend_label()
  paths.rs    # dirs::video_dir / config_dir
  capture.rs  # screenshot, record, live, voice, gif
  shell.rs    # focus_app, open_path, reveal_in_file_manager
  process.rs  # SIGSTOP/CONT (Unix)
```

macOS uses `screencapture` + avfoundation; Windows `gdigrab`; Linux `x11grab`/grim. See [PLATFORMS.md](PLATFORMS.md).

## Binary size

Release build is on the order of **~14 MB**. Size is dominated by egui/eframe, not a web runtime.
