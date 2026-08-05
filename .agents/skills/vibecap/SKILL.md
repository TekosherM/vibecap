---
name: vibecap
description: Interactively capture screen, record videos/GIFs, and request human-in-the-loop developer annotations and voice notes for native apps, game engines, mobile emulators, and web UIs.
---

# Vibecap Studio Agent Skill 🎬

Use **Vibecap Studio** when pair-programming or vibe coding to capture screenshots, record high-FPS video/GIF clips of screen motion, and request visual annotations or voice feedback from the human developer.

**Docs:** `docs/USAGE.md` · `docs/MCP.md` · `docs/ARCHITECTURE.md`

---

## 🎯 When to use this skill

1. **Native & Mobile App Development**: Inspect Xcode iOS Simulator, Android Studio Emulator, Electron, Tauri, or Rust `egui` interfaces.
2. **Game & Animation Inspection**: Extract a 15-FPS animated GIF snippet around a specific timeline range (`start` to `end`) to check sprite animations, scrolling stuttering, or canvas physics.
3. **Human-in-the-Loop Feedback**: Ask the human to review media via the Feedback inbox (draw, voice note, text).

---

## 🛠 CLI & MCP

```bash
# Install once
cargo install --path .

# Desktop UI (annotation studio, editor, feedback inbox)
vibecap

# Headless screenshot → prints path under ~/Movies/Vibecap/
vibecap --screenshot

# MCP stdio server (preferred for agents)
vibecap --mcp
```

MCP client config (portable — no machine-local paths):

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

### MCP tools

| Tool | Purpose |
| :--- | :--- |
| `vibecap_capture` | Full-screen JPG (optional `app_name` focus) |
| `vibecap_record_video` | MP4 + motion GIF (`duration_secs`, max 600) |
| `vibecap_export_gif` | Timeline GIF (`video_path`, `start_time`, `end_time`) |
| `vibecap_start_live_inspection` | Rolling frames (`format`, `interval_secs`, `output_dir`) |
| `vibecap_get_live_frame` | Latest frame path + disk usage |
| `vibecap_stop_live_inspection` | Stop stream |
| `vibecap_set_budget` | Caps + tier (`eco` / `standard` / `intensive`) |
| `vibecap_get_spending` | Session spend vs caps |
| `vibecap_request_feedback` | Queue human review (`media_path`, `question`) |
| `vibecap_get_feedback` | Poll answer by `request_id` |

Annotation drawings (pen, arrows, step badges) happen in the **desktop app**, not in `vibecap_capture`.

GIF export example (shell):

```bash
ffmpeg -ss 00:00:10 -to 00:00:15 -i ~/Movies/Vibecap/latest_video.mp4 \
  -vf "fps=15,scale=800:-1:flags=lanczos" ~/Movies/Vibecap/motion_snippet.gif
```

---

## 💡 Best practices

- **Vision**: Read the returned image/GIF path with multi-modal vision.
- **Budget first**: Call `vibecap_set_budget` (prefer `eco`) before live inspection.
- **Feedback loop**: `request_feedback` → wait for human → `get_feedback`.
- **Audio**: If a sibling `.m4a` exists next to a screenshot, treat it as developer voice context.

## 🎛 Wardrobe (desktop)

Collapsible pro tools in the Edit tab: video frame-extract / mute / compress / rotate / speed; image crop / rotate / flip / resize / brightness / contrast / grayscale / blur.
