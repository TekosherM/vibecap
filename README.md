# Vibecap Studio 🎬

**High-Performance Native Screen Recorder, Annotation Studio & AI Agent Visual Inspection Sidecar.**

Vibecap Studio is a lightweight, pure-Rust native desktop application built with `eframe`/`egui`. It provides an interactive screen recorder, screenshot annotation studio, timeline-range GIF exporter, developer voice-note recorder, and a Model Context Protocol (MCP) server for AI coding agents (Antigravity, Cursor, Claude Desktop, Windsurf).

---

## 🌟 Features

- **🎥 High-Performance Screen Recording**: Full-screen, window, or region selection with optional system/microphone audio and 30/60 FPS controls.
- **📸 Auto-Focusing Screenshot & App Window Capture**: Automatically focuses target windows (`open -a AppName` or `screencapture -l`) to guarantee clean, un-obscured captures.
- **🎨 Rich Annotation & Feedback Studio**:
  - ✏ Freehand Pen, ➡ Arrows, 🔲 Rectangles, 🖍 Highlights
  - 🔤 Custom Text Annotations
  - 💧 Blur / Pixelate Confidential Information Box
  - 🔢 Incrementing Numbered Step Badges (`[1]`, `[2]`, `[3]`) for tutorial & bug reproduction flows
  - 📋 One-Click Copy to System Clipboard
- **🎙 Developer Voice Note Audio Feedback**: Record accompanying `.m4a` audio voice feedback and attach written context notes alongside screenshots for AI agents.
- **🎞 Interactive Video Editor & Timeline Range GIF Exporter**:
  - Background thumbnail filmstrip extraction via `ffmpeg`.
  - Non-destructive fast video trimming (`-c copy`).
  - Range-based GIF Exporter: Select exact `Start` and `End` timestamps (`00:00:10` to `00:00:15`) to generate smooth 15-FPS Lanczos GIFs for analyzing UI animations and scrolling motion.
- **🤖 Built-in MCP Server & Agent Skill**: Integrates directly with AI Coding Agents for human-in-the-loop pair programming.
- **💰 Agent Budget & Spending Controls**: Agents set their own analysis budget (`vibecap_set_budget`: frame/MB/minute caps + eco/standard/intensive tiers) and report spending (`vibecap_get_spending`) — because intensive frame analysis can be expensive. Caps are enforced live (auto-stop + refusal) and are human-supervised in Settings.
- **💬 Agent Feedback Inbox**: Agents submit pics/GIFs/videos with a question (`vibecap_request_feedback`); the human answers live in-session or later from the app; agents poll `vibecap_get_feedback`.
- **🎛 Wardrobe Menus — Optional Pro Tools**: collapsible advanced panels keep the base UI clean — video frame-extract/mute/compress/rotate/speed, and a full image editor (crop, rotate, flip, resize, brightness/contrast, grayscale, blur).

---

## 🆚 Vibecap Studio vs. Browser MCPs

| Workspace / Environment | Browser MCPs | Vibecap Studio |
| :--- | :--- | :--- |
| **Web DOM** | ✅ Supported | ✅ Supported |
| **Native Desktop Apps** | ❌ Impossible | ✅ Native macOS, Tauri, Rust `egui`, Electron, Qt, SwiftUI |
| **Mobile Simulators** | ❌ Impossible | ✅ Xcode iOS Simulator & Android Studio Emulator |
| **Game Canvas Engines** | ❌ Fails (No DOM nodes in WebGL) | ✅ Unity, Unreal Engine, Bevy, WebGL 3D Canvas |
| **Human Voice & Drawing** | ❌ None | ✅ Voice notes, freehand drawing, step badges |
| **Animation Motion Analysis** | ⚠️ Static images only | ✅ Timeline Range GIF Exporter (15 FPS Lanczos) |

---

## 🚀 Installation & Build

### 1. Build and Install Binary to PATH
```bash
git clone https://github.com/your-username/vibecap.git
cd vibecap

# Install 'vibecap' binary globally into ~/.cargo/bin/vibecap
cargo install --path .
```

### 2. Run Vibecap Desktop App
```bash
vibecap
```

---

## 🤖 AI Agent & MCP Setup

### MCP Server Config (`mcp_config.json`)
Add Vibecap to your Antigravity, Cursor, or Claude Desktop MCP configuration:
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

### Agent Skill Installation
The skill file is installed at:
- **Global**: `~/.gemini/config/skills/vibecap/SKILL.md`
- **Workspace**: `.agents/skills/vibecap/SKILL.md`

AI agents can trigger window focus & screenshot captures via:
```bash
open -a "iTerm" && sleep 0.4 && screencapture -t jpg ~/Movies/Vibecap/capture.jpg
```

---

## ⌨ Hotkeys

- **Ctrl + Shift + 2**: Global shortcut to Start / Stop Screen Recording.

---

## 📄 License
MIT License.
