---
name: vibecap
description: Interactively capture screen, record videos/GIFs, and request human-in-the-loop developer annotations and voice notes for native apps, game engines, mobile emulators, and web UIs.
---

# Vibecap Studio Agent Skill 🎬

Use **Vibecap Studio** when pair-programming or vibe coding to capture screenshots, record high-FPS video/GIF clips of screen motion, and request visual annotations or voice feedback from the human developer.

---

## 🎯 When to use this skill

1. **Native & Mobile App Development**: Inspect Xcode iOS Simulator, Android Studio Emulator, Electron, Tauri, or Rust `egui` interfaces.
2. **Game & Animation Inspection**: Extract a 15-FPS animated GIF snippet around a specific timeline range (`start` to `end`) to check sprite animations, scrolling stuttering, or canvas physics.
3. **Human-in-the-Loop Feedback**: Trigger an interactive screenshot capture where the developer can draw red arrows, drop step badges (`[1]`, `[2]`, `[3]`), or record a voice note (`.m4a`).

---

## 🛠 Available Commands & Usage

### 1. Trigger Screenshot & Developer Annotation
Launch Vibecap to take a screenshot and open the Annotation Studio for the developer to provide visual feedback:
```bash
cargo run --release -- --screenshot
```
*Output*: Saves the annotated image to `~/Movies/Vibecap/screenshot_<timestamp>.jpg` alongside an optional text note (`.txt`) and audio voice note (`.m4a`).

### 2. Focus Target App & Capture Crisp Window
To capture a specific app (e.g. `iTerm2`, `Vibecap`, `Chrome`, `Xcode`) without the IDE covering it:
```bash
open -a "iTerm" && sleep 0.4 && screencapture -t jpg ~/Movies/Vibecap/iterm2_capture.jpg
```

### 3. Export High-FPS GIF Snippet around a Time Window
To analyze motion glitches (e.g. scrolling, page transitions, character jumps) between timestamps:
```bash
ffmpeg -ss 00:00:10 -to 00:00:15 -i ~/Movies/Vibecap/latest_video.mp4 -vf "fps=15,scale=800:-1:flags=lanczos" ~/Movies/Vibecap/motion_snippet.gif
```

### 4. Model Context Protocol (MCP) Mode
To run Vibecap as a standard stdio MCP Server for Cursor, Antigravity, or Claude Desktop:
```bash
cargo run --release -- --mcp
```
Add to `mcp_config.json`:
```json
{
  "mcpServers": {
    "vibecap": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "/Volumes/2T/Dev/vibecap/Cargo.toml", "--release", "--", "--mcp"]
    }
  }
}
```

---

## 💡 Best Practices for AI Agents

- **Multi-modal Vision**: Read the captured `.jpg` file using multi-modal vision to parse the developer's red drawing strokes, text annotations, and step badges.
- **Audio Context**: If a matching `.m4a` voice note exists alongside the screenshot, transcribe or process the voice feedback to understand the user's verbal requirements.

---

## 💰 Agent Budget & Spending Controls (MCP)

Intensive frame analysis can be expensive. Agents control their own spending, and the human can supervise it in the app (Settings → Agent Session & Budget):

- **`vibecap_set_budget`**: `max_frames`, `max_mb`, `max_minutes` (0 = unlimited) + `analysis_tier` (`eco` = jpg @ ≥5s, cheapest · `standard` = gif @ ~3s · `intensive` = gif/mp4 @ 1s, richest but expensive).
- **`vibecap_get_spending`**: frames, MB, elapsed minutes, caps, tier, and `within budget` / `BUDGET EXHAUSTED` status.
- Enforcement is live: streams auto-stop at caps and new streams are refused while exhausted.

## 💬 Human Feedback Loop (MCP)

- **`vibecap_request_feedback(media_path, question)`** → the request lands in the Vibecap app 💬 Feedback Inbox. The human can answer live in-session or any time after you submit.
- **`vibecap_get_feedback(request_id)`** → poll for the answer (⏳ pending until the human responds).

## 🎛 Wardrobe Menus (Optional Pro Tools)

Advanced features live in collapsible "wardrobe" menus so the base UI stays clean:
- **🎛 Advanced Video Tools** (Video Editor tab): extract frame, remove audio, compress (CRF 28), rotate 90°/180°/270°, apply speed change.
- **🖼 Image Editor** (Video Editor tab): crop, rotate, flip, resize %, brightness/contrast, grayscale, blur for pics.
