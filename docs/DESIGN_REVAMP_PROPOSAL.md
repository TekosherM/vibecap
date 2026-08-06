# Design revamp proposal — Safelight Studio / “the Loop”

> Source: external proposal (Qwen). Saved for product/design review.  
> Status: **proposal only** — not an approved implementation plan.  
> Related: [PHASE_1_IMPLEMENTATION_PLAN.md](PHASE_1_IMPLEMENTATION_PLAN.md) (active engineering plan) · [MARKETING_PLAN.md](MARKETING_PLAN.md) · [FEEDBACK_USE_CASES.md](FEEDBACK_USE_CASES.md) · [ARCHITECTURE.md](ARCHITECTURE.md) · [design-mocks/](design-mocks/)

---

## The concept: “Safelight Studio — the Loop”

Vibecap’s product isn’t five tabs — it’s the room where a human and an agent look at evidence together. The real flow is a **loop**: capture → review → annotate → ask → answer, and today’s tool-tab layout (Capture / Library / Editor / Feedback / Settings) makes users re-assemble that loop by hand every pass. The revamp reorganizes the app around the loop, and lifts the consumer-grade polish your current screens already show — dark surfaces, one vivid accent, rounded cards, wizard-style progressive disclosure, chip grids, confident CTAs — into a real design system that every pane obeys.

### Five signature moves

1. **Safelight design language.** Neutral graphite canvas instead of the espresso brown; the brand accent reserved exclusively for live states (recording, agent waiting, pending inbox) so REC actually pops; semantic success/warn/danger colors; vector icons instead of emoji; a type scale and 8pt grid; true light-theme parity.
2. **From tabs to the Loop rail.** A left rail shows the loop’s stages; every still, clip, GIF, and request carries a small “loop position” badge so you always know where it lives in the flow.
3. **The Shutter bar.** A persistent capture dock — screenshot / record / GIF / inspect one click or hotkey away — with the menu bar as its ambient twin, and the region selector, countdown, and recording pill unified into one Capture HUD family.
4. **Wizard-grade first run.** The step-wizard pattern from progressive disclosure (progress dots, one question per screen, chip grids, skip link) extended to role, budget tier, permissions, and MCP auto-config.
5. **Inbox as conversation.** Feedback requests become chat-like threads with media cards, choice chips, and voice bubbles — answering an agent should feel like replying to a colleague, not processing a queue.

### Suggested phases

| Phase | Scope | Intent |
| ---: | :--- | :--- |
| **1** | Design system + shell + toasts/icons | Cheap, huge perceived jump |
| **2** | Capture HUD + annotation studio | Core capture UX |
| **3** | Timeline editor + conversation inbox | Depth for power users + HITL |
| **4** | a11y, i18n, platform parity | Trust & reach |

---

## 100 improvements

### 1 · Design language & theming

1. Token-based design system (color/spacing/radius/elevation/type) replacing hard-coded `Color32` literals like the `#141008` espresso canvas and `#f59e0b` amber.
2. Reserve the accent exclusively for live states (recording, agent waiting, pending inbox); give ordinary primary actions a calmer brass/paper-white.
3. Unify the split accent (teal CTAs in onboarding screens vs amber headings in the shell) into one brand accent plus a semantic success/warn/danger scale.
4. Replace emoji glyphs (🎥 ✂) with a tintable vector icon set that renders identically on macOS, Windows, and Linux.
5. Type scale (12/13/15/18/24/32) replacing the mix of `.size(26)` headings and ad-hoc `.strong()` calls.
6. 8pt spacing grid; retire manual centering hacks like `add_space(width/2 − 180)` that break at narrow window widths.
7. True WCAG-AA light theme plus a neutral Graphite dark alongside today’s espresso; persist the choice and optionally follow the OS.
8. Elevation tiers (6/10/14 radius) instead of uniform 10px rounding on buttons, panels, and HUD alike.
9. Motion system: 120–180 ms ease-out transitions, shutter flash on capture, pulse ring while recording arms, spring-in on inbox badges, with a reduced-motion toggle.
10. Compact severity-tinted toast cards replacing the full-width amber toast bar; brand kit with logo variants, tray icon states, and splash art consistent with the onboarding screens.

### 2 · App shell & navigation

11. Left icon rail with badges (pending inbox count, live rec timer) replacing the top emoji tab bar.
12. ⌘K command palette covering every action, including wardrobe tools and jump-to-request.
13. Full keyboard navigation: ⌘1–5 sections, arrows walk Library rows, Space selects, Enter opens, Esc exits annotation; “?” cheatsheet overlay.
14. Persistent status strip: storage used, budget spend, ffmpeg health, MCP connections, and a live agent-activity chip fed by `live_usage_snapshot`.
15. Responsive split view (list + detail) on wide windows, single pane on narrow.
16. Deep links `vibecap://open` and `vibecap://feedback/<id>` from browsers and agent chats.
17. Pop-out windows for the annotation studio and editor, reusing the egui viewport already used for the recorder pill.
18. Session restore: window geometry, active tab, last editor file, and draft annotations.
19. Undo toasts (“Pruned 12 items — Undo”) and modal confirms that state exactly what bulk deletes remove.
20. Density toggle (Comfortable/Compact) for 13″ laptops vs 4K displays, plus a live-filtering Settings search field.

### 3 · Onboarding & first run

21. First-launch wizard: progress dots, one question per screen, chip grids, big rounded CTAs, skip link on every step.
22. “How will you use Vibecap?” chip step (solo dev / pair with agents / QA / design review) that tunes defaults.
23. Budget-tier step with plain-language cards (Eco/Standard/Intensive), estimated disk cost, and sample outputs side by side.
24. MCP auto-detect step: find installed agent clients and offer one-click install of the `vibecap --mcp` entry.
25. Permissions checklist (screen recording, microphone, files) with live status pills, ffmpeg detection with a brew-install hint, and a test capture.
26. 60-second interactive tour of the Shutter bar, Library, and Inbox using bundled sample media.
27. End the wizard with “import your first capture” so users land on a populated Library.
28. Resumable wizard, plus a post-onboarding checklist card until first capture, annotation, and reply are done.
29. Designed empty states with a film-strip motif and one primary action, replacing bare gray labels.
30. Update checker against GitHub Releases with an in-app “what’s new” sheet rendered from `CHANGELOG.md`.

### 4 · Capture experience

31. Persistent Shutter bar dock plus floating mini-mode when the window is hidden; region selector, countdown, and recording pill unified into one Capture HUD family.
32. Region select with live W×H readout, thirds grid, cursor loupe, arrow-key nudge, and remembered last region.
33. Window picker presenting a live window list with app icons (`CGWindowList` on macOS) instead of focus-and-capture tricks.
34. Countdown/flash options (0/3/5 s) with a big-number overlay and skip, plus shutter-sound toggle.
35. **Retro buffer:** “capture the last N seconds” from a low-FPS ring buffer — the single biggest win for bug hunting.
36. First-class pause/resume (distinct pill state, tray timer styling) and a marker hotkey that drops chapter points for the timeline.
37. Scrolling capture (stitched screenshots) for long pages, and a display picker for multi-monitor setups.
38. Live-inspection HUD showing FPS, frame count, and disk burn rate; audio level meter whenever “include audio” is on.
39. Presets: “bug report” (screenshot + last-3 s GIF + system info), “demo clip”, “still”; per-target presets remembering FPS, audio, and region.
40. Editable naming tokens (`{app}-{date}-{seq}`) with live preview; post-capture shutter toast with Annotate/Copy/Edit/Reveal instead of an automatic annotation takeover; `.txt` sidecar summaries and OCR text-region capture for agent context.

### 5 · Media library

41. Grid/filmstrip view grouped by day with hover-scrub previews; lazy thumbnails with a disk cache and virtualized lists instead of per-row `file://` loads.
42. Preview pane with metadata (dimensions, duration, FPS) and inline voice-note playback; spacebar Quick Look overlay with frame stepping.
43. Filters (type, source app, agent label, date range) with saved chips; sort controls; search across filenames, notes, and feedback text.
44. Batch select (shift/⌘) for prune, export, tag, and share; drag out to Finder and drag in to import.
45. Tags, stars, and pins — settable by agents through a new MCP arg — with smart collections.
46. Hash-based duplicate detection and one-click cleanup for near-identical live-inspection frames.
47. Per-category storage bars reusing dir-size helpers, with “free up space” suggestions that always move to trash with Undo.
48. Infinite scroll with a sticky “showing X of Y” header replacing “Show more (N hidden)”.
49. Share menu: copy to clipboard, reveal in Finder, drag-and-drop out of the window.
50. Date-grouped sections (Today / Yesterday / Earlier) instead of one flat paginated list.

### 6 · Editor & export

51. Real timeline: draggable trim handles on a filmstrip with a time ruler; recording markers with “export between markers”; HH:MM:SS kept as precision secondary input.
52. In-app playhead preview with speed control, frame stepping (←/→), and loop-region.
53. Non-destructive adjustment stack (trim/rotate/speed/crop) as reorderable chips in a right inspector, with per-step reset and a before/after slider.
54. Image crop via drag handles on the preview rather than four numeric text fields.
55. GIF export dialog with looping live preview, FPS/width/quality sliders, live estimated file size, and palette optimization.
56. One-click presets: Discord ≤8 MB, Slack, README 480p, 3 s bug clip; lossless MP4 option.
57. Export queue with per-job progress and plain-language retry on ffmpeg failure; batch queue across selected Library items.
58. Optional burned-in caption track generated from feedback text; audio extraction (m4a/mp3/wav) with attach-to-reply.
59. Export history with one-click re-run using identical settings.
60. “Send to Inbox”: attach the current edit as a media card in a new feedback request.

### 7 · Annotation studio

61. Focused canvas with a floating tool palette instead of a whole-window takeover; Esc returns you where you were; zoom/pan with fit/100% toggles.
62. Semantic pen colors (red = bug, amber = question, green = approve, blue = note) with smooth strokes and width/opacity sliders.
63. Arrow, rectangle, ellipse, and step-badge tools with edge snapping, shift-constrain, and auto-incrementing badges that renumber on drag.
64. Text callouts with a floating input at the click point, auto-contrast chip background, and drag-to-reposition.
65. Redact mode: drag-to-blur with remembered strength plus regex/OCR suggestions for emails, URLs, and secrets; before/after slider to verify before sharing.
66. Layers panel (show/hide/reorder/delete) with multi-step undo and redo via a small history slider.
67. Time-pinned annotations for video/GIF that track playback.
68. Annotations saved as project files so originals are never mutated; explicit “save as new” vs overwrite; flattened PNG export.
69. Templates: “bug report” (step badges + red arrows) and “design review” (spacing rulers).
70. Agent-label avatars on strokes created from agent requests; auto-attach the annotated result to the reply being composed.

### 8 · Human-in-the-loop inbox

71. Conversation-style request cards: agent identity dot with consistent per-agent colors, priority chip, age timer, and inline media thumbnail.
72. All four reply types as first-class buttons (chips, text, voice, annotated image) with a composer offering markdown-lite preview and a chip builder.
73. Voice-note recorder with waveform, re-record, and transcription when available.
74. Priority lanes with pinning and snooze-with-reminder; easy restore from Closed; age-based color escalation plus a system notification when the window is hidden.
75. “Agent last polled Ns ago” and gentle waiting-time nudges.
76. Bulk triage with j/k navigation and a/r to approve/reject; bulk answer for similar low-priority chips.
77. Quick-reply from the tray and a global hotkey without opening the window.
78. Searchable history of every answered request and its media.
79. Saved reply snippets insertable in one click.
80. Delegate/forward a request to another agent with a note via MCP; deep links open the exact request.

### 9 · Tray, hotkeys & ambient presence

81. Tray icon states: idle, recording with live timer, agent-waiting dot, pending-inbox count, error.
82. Menu-bar pill becomes a live menu (pause/stop/cancel) and a macOS popover controller; consistent tooltips on Windows/Linux trays.
83. Global hotkeys — capture ⇧C, record ⇧R, inbox ⌘I, palette ⌘K — with editable bindings and conflict detection.
84. Do-not-disturb mode that queues notifications; notification-center inline reply chips for high priority.
85. Drag-and-drop files onto the tray icon to import them into the Library.
86. Tray menu shows the current budget tier and today’s spend at a glance.
87. Close-to-tray by default with a “quit instead” setting (macOS convention).
88. Quick-capture countdown bubble that works while the window is hidden.
89. Sound design: subtle shutter tick, record blips, feedback chime — each individually toggleable.
90. Multi-display awareness: capture display picker; tray on the active display’s menu bar.

### 10 · Settings, budget, trust, access & platform

91. Sectioned, searchable Settings sidebar; budget editor with sliders and inline validation instead of parse-error toasts.
92. Budget dashboard: spend-vs-cap donut and sparkline with a per-agent breakdown and forecast line.
93. Privacy panel showing exactly what is captured and where; one-click purge with confirm + undo window; privacy-safe “export debug bundle” (logs + settings, no media) for GitHub issues.
94. Profiles/workspaces (e.g. “client A”, “open source”) with separate media dirs and budgets; export/import settings as JSON.
95. Extract all UI strings into a catalog; ship English + Swedish.
96. Accessibility pass: focus-visible rings, font scaling, high-contrast variant, reduced motion, and real labels for icon-only buttons.
97. Split the large `main.rs` into per-tab modules plus a small design-system component layer (`Button` / `Toast` / `Chip` / `Card` / `Timeline`) — the enabler for everything above.
98. UI snapshot/golden-image tests in CI so the visual revamp cannot regress silently.
99. Platform parity: PipeWire/Wayland capture and Windows WASAPI loopback audio; inline banner with a copyable fix when ffmpeg is missing.
100. Surface budget auto-stops as human-visible notifications, not just MCP-side messages; MCP parity dashboard showing connected clients, last call per tool, and a “script it” panel with copyable CLI equivalents for every GUI action.

---

## Through-line (from proposal)

Items **1–10** and **97–98** are the foundation — a token-driven design system plus modular components turns the current prototype-grade chrome into a studio-grade product without touching the capture/MCP core that already works well.

Natural next step if approved: **Phase 1 in code** — graphite theme, icon rail, toast cards, and the type/spacing scale.

---

## Builder assessment (repo-local review)

This section is **not** from the original proposal. It evaluates fit for Vibecap’s current product strategy (lightweight OSS, MCP-first, single Rust binary).

### What’s strong (keep)

| Idea | Why it fits Vibecap |
|---|---|
| **Loop framing** | Matches real agent + human workflow better than generic “Capture / Library / Editor” |
| **Safelight accent for live only** | REC / pending inbox are the product’s emotional peak — currently drowned in amber-everywhere |
| **Shutter bar + Capture HUD family** | Unifies region / countdown / recording pill; aligns with recent record-arming work |
| **Inbox as conversation** | Directly amplifies the 30 feedback use cases and marketing “HITL” story |
| **MCP auto-detect on first run** | Highest activation ROI for MCP market / PH install path |
| **Retro buffer (#35)** | Genuine differentiator for agent bug hunts; rare in lightweight tools |
| **Modularize main.rs (#97)** | Required before any serious UI revamp; not optional |

### What’s high-risk / defer

| Idea | Risk |
|---|---|
| Full non-destructive editor stack (#51–59) | Competes with CapCut/Descript; huge surface; not the wedge |
| OCR + regex secret detect (#65, #40) | Heavy deps, false positives, privacy narrative complexity |
| i18n EN+SV (#95) early | Before string freeze and shell stabilize = rework |
| Golden UI screenshots in CI (#98) | Flaky on headless Linux; better after design system exists |
| PipeWire/WASAPI perfection (#99) | Platform rabbit hole; keep “macOS primary” honest for launch |
| Deep links + multi-agent delegate (#16, #80) | Protocol design before demand is proven |
| Full command palette + density + light theme + motion all at once | Scope explosion in egui |

### Tension with “lightweight”

Marketing sells **one binary, MCP eyes, HITL**. A 100-item studio OS can:

- Delay PH / MCP listing polish  
- Inflate binary size and maintenance  
- Blur the message into “yet another screen recorder”

**Recommendation:** treat this doc as a **backlog map**, not a sprint plan. Ship a **thin Loop shell** that makes the existing capture + MCP + inbox feel intentional.

### Suggested cut for Phase 1 (2–4 focused sessions)

Only these, in order:

1. **#97** — Split `main.rs` into modules (`ui/theme`, `ui/shell`, `capture`, `library`, `edit`, `inbox`, `settings`) without behavior change  
2. **#1, #2, #5, #6** — Tokens: Graphite dark, accent-for-live, type scale, 8pt spacing  
3. **#4, #10** — Vector icons (or a small embedded set) + severity toasts  
4. **#11** — Left rail with badges (loop stages mapped to current tabs — no new features)  
5. **#29** — Designed empty states for Library / Inbox  
6. **#31 (minimal)** — Shutter strip on Capture that reuses existing screenshot/record actions + links to floating REC bar  

**Explicitly not Phase 1:** timeline rewrite, OCR, i18n, light theme, retro buffer, command palette, pop-outs.

### Suggested Phase 2 (product wedge)

1. Post-capture toast: Annotate / Copy / Edit / Reveal (don’t always force annotation)  
2. Conversation-style inbox cards (agent label, priority, media thumb, chips) — **#71–72**  
3. Permissions + ffmpeg + MCP one-click first-run — **#24–25**  
4. Retro buffer spike — **#35** (if technical spike is cheap on macOS)

### Decision checklist before implementing Phase 1

- [ ] Approve name: keep **Vibecap** in market; “Safelight / the Loop” as internal design codename only (avoid rebrand pre-PH)  
- [ ] Confirm Graphite dark as default (espresso → graphite is a brand shift; update marketing screenshots)  
- [ ] Cap Phase 1 to design system + shell only — no new capture backends  
- [ ] Keep MCP tool surface stable during UI revamp (marketing/MCP listings depend on it)

### Mapping to current tabs (Loop rail)

| Loop stage | Current tab | Rail label idea |
|---|---|---|
| Capture | Capture | Shutter |
| Review | Library | Media |
| Annotate / Edit | Edit (+ annotation mode) | Studio |
| Ask / Answer | Agent Inbox | Inbox |
| Configure | Settings | Settings |

No need for six new destinations — **rename + reframe** is enough for Phase 1.

---

## Status

| Item | State |
|---|---|
| Proposal saved | ✅ this file |
| Phase 1 approved | ✅ 2026-08-06 |
| Phase 1 implementation | ✅ Graphite theme · Loop rail · Shutter strip · toast cards · empty states · `src/ui/*` module |
| Phase 2+ | ⏳ not started |

### Phase 1 landed (code)

- `src/ui/theme.rs` — Graphite tokens, accent-for-live, 8pt grid, type scale helpers  
- `src/ui/icons.rs` — vector icon set (no emoji in rail)  
- `src/ui/components.rs` — Loop rail, Shutter strip, severity toasts, empty states  
- Shell: left rail (Shutter / Media / Studio / Inbox / Settings) + stage header  
- Capture: Shutter dock replaces dual centered buttons  
- Library / Inbox: designed empty states  
- Toast: bottom-right severity card (not full-width amber bar)
