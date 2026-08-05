# Architecture

Vibecap is intentionally **one binary**: desktop UI + MCP server in the same crate (`src/main.rs`). No service mesh, no database, no cloud.

## Process modes

```text
vibecap                 →  eframe/egui desktop app
vibecap --mcp            →  stdio JSON-RPC MCP server (no window)
vibecap --screenshot     →  one-shot headless capture, then exit
```

Mode is chosen in `main()` before any UI init.

## Layers

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

## Platform note

Capture paths are **macOS-first** today (`screencapture`, `open -a`, avfoundation). Windows/Linux backends are planned (see README roadmap) behind the same CLI/MCP surface.

## Binary size

Release build is on the order of **~14 MB** (single Mach-O). Size is dominated by egui/eframe, not a web runtime.
