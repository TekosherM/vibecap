# Vibecap Studio 🎬

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg)](docs/PLATFORMS.md)
[![CI](https://github.com/TekosherM/vibecap/actions/workflows/ci.yml/badge.svg)](https://github.com/TekosherM/vibecap/actions/workflows/ci.yml)

**High-performance native screen recorder, annotation studio, and AI agent visual inspection sidecar.**

Vibecap Studio is a lightweight pure-Rust desktop app (`eframe` / `egui`) with an interactive recorder, screenshot annotation studio, timeline GIF exporter, developer voice-note capture, and a Model Context Protocol (MCP) server for AI coding agents (Cursor, Claude Desktop, Gemini, Windsurf, and more).

---

## 🌟 Features

- **🎥 High-performance screen recording** — full-screen, window, or region with optional system/mic audio and 30/60 FPS controls.
- **📸 Auto-focusing screenshot & app window capture** — focuses the target window so captures stay clean (macOS: `open -a` / `screencapture`).
- **🎨 Annotation & feedback studio**
  - Freehand pen, arrows, rectangles, highlights
  - Custom text annotations
  - Blur / pixelate for sensitive regions
  - Numbered step badges (`[1]`, `[2]`, `[3]`) for tutorials & bug repros
  - One-click copy to system clipboard
- **🎙 Voice notes** — attach `.m4a` audio and text notes next to screenshots for agents.
- **🎞 Video editor & timeline GIF export**
  - Background filmstrip via `ffmpeg`
  - Non-destructive trim (`-c copy`)
  - Range GIF export: pick start/end timestamps for smooth 15-FPS Lanczos GIFs
- **🤖 MCP server & agent skill** — human-in-the-loop pair programming with coding agents
- **💰 Agent budget controls** — `vibecap_set_budget` / `vibecap_get_spending` with live caps (frames / MB / minutes + eco/standard/intensive tiers)
- **💬 Feedback inbox** — agents submit media + questions; humans reply in-app; agents poll for answers
- **🎛 Wardrobe menus** — collapsible pro tools (frame extract, mute, compress, rotate, speed; full image editor)

---

## 🆚 Vibecap vs browser MCPs

| Workspace | Browser MCPs | Vibecap Studio |
| :--- | :--- | :--- |
| **Web DOM** | ✅ | ✅ |
| **Native desktop apps** | ❌ | ✅ (Tauri, egui, Electron, Qt, SwiftUI, …) |
| **Mobile simulators** | ❌ | ✅ (iOS Simulator, Android Emulator) |
| **Game / WebGL canvases** | ⚠️ often fails | ✅ |
| **Human voice & drawing** | ❌ | ✅ |
| **Motion / animation analysis** | ⚠️ stills only | ✅ timeline GIF export |

---

## 🏗 Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     Vibecap Studio                          │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────┐  │
│  │ Capture UI   │  │ Annotation   │  │ Video Editor /    │  │
│  │ (eframe/egui)│  │ Studio       │  │ GIF Timeline      │  │
│  └──────┬───────┘  └──────┬───────┘  └─────────┬─────────┘  │
│         │                 │                    │            │
│         └─────────────────┼────────────────────┘            │
│                           ▼                                 │
│              Media store (~/Movies/Vibecap)                 │
│                           ▲                                 │
│  ┌────────────────────────┴──────────────────────────────┐  │
│  │  MCP Server (--mcp)  ·  Budget  ·  Feedback inbox     │  │
│  │  stdio JSON-RPC · shared ~/.config/vibecap state      │  │
│  └────────────────────────┬──────────────────────────────┘  │
└───────────────────────────┼─────────────────────────────────┘
                            │
              AI agents (Cursor, Claude, Gemini, …)
```

**MCP tools (10):** `capture`, `record_video`, `export_gif`, `start_live_inspection`, `get_live_frame`, `stop_live_inspection`, `set_budget`, `get_spending`, `request_feedback`, `get_feedback`.

---

## 🚀 Install & run

### Prerequisites

- [Rust](https://rustup.rs/) (edition 2021)
- [ffmpeg](https://ffmpeg.org/) on `PATH` (GIF export, filmstrip, wardrobe video tools)
- **macOS** Screen Recording permission for the terminal / app (System Settings → Privacy & Security)

### Prebuilt binaries

Download the latest release for your platform from  
**https://github.com/TekosherM/vibecap/releases**

```bash
# Example: macOS Apple Silicon
tar -xzf vibecap-aarch64-apple-darwin.tar.gz
sudo mv vibecap-aarch64-apple-darwin/vibecap /usr/local/bin/
vibecap --help
```

| Asset | Platform |
| :--- | :--- |
| `vibecap-aarch64-apple-darwin.tar.gz` | macOS Apple Silicon |
| `vibecap-x86_64-apple-darwin.tar.gz` | macOS Intel |
| `vibecap-x86_64-unknown-linux-gnu.tar.gz` | Linux x86_64 |
| `vibecap-x86_64-pc-windows-msvc.zip` | Windows x86_64 |

### Build from source

```bash
git clone https://github.com/TekosherM/vibecap.git
cd vibecap

# Install binary to ~/.cargo/bin/vibecap
cargo install --path .

# Or run without installing
cargo run --release
```

### Desktop app (system tray)

```bash
vibecap              # window + menu bar / tray icon
vibecap --hidden     # start in tray only
vibecap --no-tray    # classic window; close quits
```

Closing the window **hides to the tray** (does not quit). Use **Quit Vibecap** from the tray menu to exit. Tray actions: Show, Hide, Screenshot, Feedback inbox, Quit.

### Headless screenshot

```bash
vibecap --screenshot   # prints path under media dir
```

Annotation (pen, arrows, step badges) is in the **desktop UI**, not this CLI flag.

### MCP server mode (multi-instance OK)

```bash
vibecap --mcp
```

Run **one MCP process per agent/client** alongside the human GUI — no single-instance lock. Live inspection uses per-process session folders; budget/feedback are shared.

### Verify install

```bash
./scripts/smoke_mcp.sh              # CLI + MCP tools (no GUI)
SMOKE_CAPTURE=1 ./scripts/smoke_mcp.sh   # also tries screenshot + GIF export
```

---

## 📚 Documentation

| Doc | Contents |
| :--- | :--- |
| [docs/USAGE.md](docs/USAGE.md) | CLI, desktop tabs, agent workflows, troubleshooting |
| [docs/MCP.md](docs/MCP.md) | All 10 MCP tools, config, disk state |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Single-binary design, deps, size |
| [docs/PLATFORMS.md](docs/PLATFORMS.md) | macOS / Windows / Linux backends |
| [docs/TESTING.md](docs/TESTING.md) | Smoke script + manual GUI checklist |
| [CONTRIBUTING.md](CONTRIBUTING.md) | PR hygiene, keep it lightweight |
| [skills/vibecap/SKILL.md](skills/vibecap/SKILL.md) | Agent skill definition |

---

## 🤖 AI agent & MCP setup

### MCP config

Add Vibecap to Cursor (`~/.cursor/mcp.json`), Claude Desktop, Gemini, or any MCP client:

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

Use the installed binary (`vibecap` on `PATH`). Avoid hardcoding machine-local `cargo --manifest-path` absolute paths in shared configs.

### Agent skill

Canonical skill files live in this repo:

| Location | Purpose |
| :--- | :--- |
| `skills/vibecap/SKILL.md` | Canonical skill definition |
| `.agents/skills/vibecap/SKILL.md` | Workspace skill (kept in sync) |

Copy or symlink into your agent skill directory as needed (e.g. `~/.agents/skills/vibecap/`).

### Example agent capture flow (macOS)

```bash
open -a "iTerm" && sleep 0.4 && screencapture -t jpg ~/Movies/Vibecap/capture.jpg
```

---

## ⌨ Hotkeys

| Shortcut | Action |
| :--- | :--- |
| **Ctrl + Shift + 2** | Start / stop screen recording |

---

## 🌍 Platform support

| Platform | Capture backend | Status |
| :--- | :--- | :--- |
| **macOS** | `screencapture` + ffmpeg | ✅ Primary |
| **Windows** | ffmpeg `gdigrab` | ✅ Builds; install ffmpeg |
| **Linux** | ffmpeg `x11grab` / grim | ✅ Builds; X11 or grim |

Details: [docs/PLATFORMS.md](docs/PLATFORMS.md). Paths use `dirs` (`video_dir` / `config_dir`).

---

## 📁 Default media paths

| Path | Use |
| :--- | :--- |
| `{Videos}/Vibecap/` (or `~/Movies/Vibecap`) | Screenshots, videos, GIFs |
| `{media}/live/` | Live inspection stream frames |
| `.vibecap_temp/` | Local temp (gitignored) |
| `{config}/vibecap/` | Agent budget + feedback inbox state |

---

## 🛠 Development

```bash
# Debug build
cargo build

# Release
cargo build --release

# MCP smoke (stdio; leave running while a client connects)
cargo run --release -- --mcp
```

---

## 📄 License

Licensed under either of:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT license](LICENSE-MIT)

at your option.

---

## 🗺 Roadmap

1. ~~Housekeeping & path sanitization~~ · ~~Open-source metadata & dual license~~
2. ~~Docs + headless smoke tests (CLI/MCP)~~
3. ~~Cross-platform capture abstraction (`src/platform`, dirs/open, Win/Linux ffmpeg)~~
4. ~~CI matrix (macOS smoke + Win/Linux build)~~
5. ~~GitHub Release binaries (tag `v*`)~~
6. Richer Windows audio & Wayland capture; optional installers
