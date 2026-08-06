---
name: vibecap
description: Interactively capture screen, record videos/GIFs, and request human-in-the-loop developer annotations and voice notes for native apps, game engines, mobile emulators, and web UIs.
---

# Vibecap Studio Agent Skill 🎬

Use **Vibecap Studio** when pair-programming or vibe coding to capture screenshots, record high-FPS video/GIF clips of screen motion, and request visual annotations or voice feedback from the human developer.

**Docs:** `docs/USAGE.md` · `docs/MCP.md` · `docs/FEEDBACK_USE_CASES.md` · `docs/ARCHITECTURE.md`

---

## 🎯 When to use this skill

1. **Native & Mobile App Development**: Inspect Xcode iOS Simulator, Android Studio Emulator, Electron, Tauri, or Rust `egui` interfaces.
2. **Game & Animation Inspection**: Extract a 15-FPS animated GIF snippet around a specific timeline range (`start` to `end`) to check sprite animations, scrolling stuttering, or canvas physics.
3. **Human-in-the-Loop Feedback**: Ask the human to review media (or decide without media) via the **🤖 Agent Inbox** — text, choice chips, voice, image mark-up.

---

## 🛠 CLI & MCP

```bash
# Install once
cargo install --path .

# Desktop UI (annotation studio, editor, agent inbox) + menu bar tray
vibecap
# vibecap --hidden   # start in tray only
# vibecap --no-tray  # close quits (no tray)

# Headless screenshot → prints path under media dir
vibecap --screenshot

# MCP stdio server (preferred for agents). Multiple --mcp processes OK.
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
| `vibecap_request_feedback` | Queue human question (`question` required; `media_path` optional) |
| `vibecap_get_feedback` | **Poll** answer by `request_id` |
| `vibecap_list_feedback` | List pending/closed (recover ids after restart) |
| `vibecap_cancel_feedback` | Abandon a pending request |

Annotation drawings (pen, arrows, step badges) happen in the **desktop app** (Mark up), not in `vibecap_capture`.

---

## 🧑 Human-in-the-loop (critical)

**Answers are never auto-pushed into your chat.** Vibecap writes files; you must poll.

### Required loop

```
1. vibecap_request_feedback(question=…, media_path?=…, options?=…, priority?=…, agent_label?=…, preferred_reply?=…, context?=…)
   → save request_id from the response
2. Poll vibecap_get_feedback(request_id) every 2–5 seconds
   until line starts with ✅ / 🚫 / ⏭ (not ⏳ status=pending)
3. If annotated_media is set and text is empty → open that PNG with vision
4. If you lose the id → vibecap_list_feedback(status=pending)
5. If the human is gone / question is stale → vibecap_cancel_feedback(request_id)
```

### `request_feedback` fields

| Arg | Required | Notes |
| :--- | :---: | :--- |
| `question` | ✅ | What you need decided |
| `media_path` | | Image/GIF/video path; **omit** for permission / pure choice |
| `options` | | Array of ≤8 chips, e.g. `["approve","reject"]` |
| `preferred_reply` | | `any` · `text` · `annotate` · `voice` · `choice` |
| `priority` | | `low` · `normal` · `high` (inbox sort) |
| `agent_label` | | e.g. `codex`, `claude`, `grok` — shown in inbox |
| `context` | | Extra notes, before/after paths, constraints |

### Interpreting answers

| Signal | Meaning |
| :--- | :--- |
| `status=pending` | Keep polling |
| `choice: approve` | Human tapped a chip |
| `text: "…"` | Freeform reply |
| `annotated_media: …` | **Primary answer may be the drawing** — empty text is OK |
| `voice_note: …` | Audio path next to captures |
| `status=dismissed` | Human closed without answering |
| `status=cancelled` | You (or another agent) cancelled |

### 30 use-case map

See **`docs/FEEDBACK_USE_CASES.md`** for the full table (docs QA, bug mark-up, A/B, destructive permission, live-session unblock, i18n overflow, secret redaction, batch prune, etc.). Summary categories:

1. **Visual QA** — README, brand, dark mode, mobile layout  
2. **Localization of issues** — annotate bug, spacing, overflow  
3. **Motion** — GIF/video jank, onboarding flow  
4. **Copy & a11y** — modal text, contrast  
5. **Decisions without media** — allow/deny, pick approach  
6. **Security** — PII/secret redaction before share  
7. **Session control** — live “what is this?”, agent stuck  

---

## 💡 Best practices

- **Vision**: Read returned image/GIF/annotated paths with multi-modal vision.
- **Budget first**: Call `vibecap_set_budget` (prefer `eco`) before live inspection.
- **Feedback**: Always poll; set `agent_label` when multiple agents share one human.
- **Prefer chips** for binary decisions so the human one-taps.
- **Prefer annotate** when pointing at pixels beats paragraphs.
- **Audio**: If a sibling `.m4a` exists next to a screenshot, treat it as developer voice context.

## 🎛 Wardrobe (desktop)

**Clip** rail stage: filmstrip preview, trim/GIF/audio, plus frame-extract / mute / compress / rotate / speed. **Still** rail stage: live preview first, then crop / rotate / flip / resize / brightness / contrast / grayscale / blur.
