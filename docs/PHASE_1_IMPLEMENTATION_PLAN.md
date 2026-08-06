# Phase 1 implementation plan — Safelight chrome, lightweight-safe

**Status:** Active plan (aligned 2026-08-06)  
**North star:** `docs/design-mocks/safelight-studio-mock.html` + `docs/design-mocks/screens/*.png`  
**Product bar:** single Rust binary, capture/MCP core untouched, chrome only  
**Consensus:** mocks = language, not a 100-item feature backlog. **Pay the refactor first.**

Related: [DESIGN_REVAMP_PROPOSAL.md](DESIGN_REVAMP_PROPOSAL.md) · [MARKETING_PLAN.md](MARKETING_PLAN.md) · [ARCHITECTURE.md](ARCHITECTURE.md)

---

## Lightweight bar (measurable gates)

Check these every PR that touches UI. Fail CI / reject the PR if violated.

| Gate | Rule | Today (approx.) |
| ---: | :--- | :--- |
| **G1 Deps** | Direct `Cargo.toml` deps ≤ **current + 2 tiny crates** for two phases | **14** direct deps (eframe/egui + ffmpeg-CLI pattern; no media frameworks) |
| **G2 main.rs** | Line count of `src/main.rs` may **only decrease** (or stay flat on pure docs) | **~1757** (was ~4145 → 3032 → 1757) |
| **G3 Binary** | Stripped release binary **&lt; 20 MB** (CI check) | **~14 MB** |
| **G4 Tokens** | No raw `Color32::from_rgb(...)` outside `src/ui/theme.rs` (`TRANSPARENT` allowed) | ✅ Clean — only `theme.rs` + rare `TRANSPARENT` |
| **G5 Core freeze** | No capture/MCP behavior change in Phase 1a; MCP tool surface unchanged | 12 tools, platform in `src/platform/` |

**egui reality (set expectations now):**

| Mock effect | Honest egui translation |
| :--- | :--- |
| Backdrop blur (Shutter / toast) | Translucent fills (`Color32::from_rgba_premultiplied`) |
| Spring motion | 2–3 time-based animations (REC pulse, toast fade) — no spring physics crate |
| Fancy vector SVG set | Prefer **pre-rendered PNGs @1x/2x** via `egui_extras` (already a dep); keep painted icons as fallback |
| Pixel-perfect HTML mock | Same language, approximated effects — not a clone |

---

## Fidelity ladder

1. **Chrome parity** — colors, rail, type, empty states, toasts, stage names  
2. **Interaction parity** — capture → library badge → annotate → inbox reply still works end-to-end  
3. **Never full widget parity** — no retro-on-by-default, no CapCut timeline, no OCR/ASR in Phase 1  

---

## Phase structure (refined)

```text
1a  EXTRACTION GATE     → opens chrome work
1b  TOKEN ENFORCEMENT   → light theme becomes one table
1c  CHROME VERTICAL     → rail + Shutter + toasts match mock language
1d  CHEAP HIGH-TRUST    → palette, session restore, density, undo toasts
--- later ---
2   Inbox conversation restyle + post-capture toast actions
3   Wizard + Capture HUD family (region loupe, countdown bubble)
4   Optional spikes (retro buffer OFF by default, …)
```

### What already landed (partial 1c)

Do **not** re-do from scratch; finish extraction around it.

| Piece | Path | Notes |
| :--- | :--- | :--- |
| Theme tokens + Graphite apply | `src/ui/theme.rs` | Accent-for-live; 8pt; type helpers |
| Painted icons | `src/ui/icons.rs` | Temporary; PNG kit optional later |
| Loop rail, Shutter strip, toast card, empty_state | `src/ui/components.rs` | Works; still called from fat `main.rs` |
| Shell wiring | `src/main.rs` ~2000+ | SidePanel rail + Capture shutter |

**1a is still required:** chrome inside 4k-line `main.rs` will stay slow and merge-painful.

---

## Phase 1a — Extraction gate (zero deps, zero behavior)

**Goal:** `main.rs` shrinks; UI paint paths live under `src/ui/`; app state + MCP stay reachable.

### Target module map

```text
src/
  main.rs                 # CLI, eframe entry, thin App impl: update() orchestrates only
  tray_ui.rs              # unchanged
  platform/               # capture/paths/process — FROZEN this phase
  ui/
    mod.rs
    theme.rs              # tokens + apply_graphite / apply_light (light later)
    icons.rs              # paint or PNG loaders
    components.rs         # Button/Chip/Toast/Card/Badge primitives (grow here)
    shell.rs              # Loop rail + stage header + status strip stub
    capture_tab.rs        # Shutter + target options + capture tab body
    library_tab.rs        # library list UI only
    edit_tab.rs           # filmstrip + wardrobe UI only
    inbox_tab.rs          # feedback inbox UI only
    settings_tab.rs       # settings + budget UI only
    annotation.rs         # annotation studio overlay UI
  app/                    # optional if needed for clean compile
    state.rs              # VibecapApp struct + Default
    feedback.rs           # FeedbackRequest/Response + disk IO
    budget.rs             # budget load/save + live_usage_snapshot
    recording.rs          # arm/stop/cancel/spawn
    library.rs            # refresh_library, delete, reveal
    mcp.rs                # run_mcp_server (move from main)
```

**Pragmatic order of moves** (each step compiles + smoke green):

| Step | Move | Acceptance |
| ---: | :--- | :--- |
| A1 | Move MCP server + budget/feedback disk helpers → `app/mcp.rs`, `app/budget.rs`, `app/feedback.rs` | ✅ Done — smoke 20/20; `main.rs` 4145→3032 |
| A2 | Recording helpers + library scan → `app/recording.rs`, `app/library.rs`, `app/paths.rs` | ✅ Done |
| A3 | Extract `show_annotation` → `ui/annotation.rs` | ⏳ Optional later |
| A4 | Extract tab match arms → `ui/*_tab.rs` | ✅ Done — capture/library/edit/inbox/settings |
| A5 | Extract Loop rail host → `ui/shell.rs` | ⏳ Rail already in components; host still in main `update` |
| A6 | Confirm `main.rs` &lt; **2500** lines (stretch &lt; **1800**) | ✅ **~1757** |

**Rule:** extract with `git mv` / cut-paste only. No new features in 1a PRs.

### Ownership boundary

| Layer | Owns | Must not own |
| :--- | :--- | :--- |
| `platform/` | OS capture, paths, process signals | UI colors, layout |
| `app/` (or impl blocks) | State, IO, ffmpeg jobs, MCP | Layout pixels |
| `ui/` | Paint, layout, tokens | Spawn ffmpeg, write feedback JSON |
| `tray_ui/` | Tray menu | Full app state |

---

## Phase 1b — Token enforcement

1. Grep `Color32::from_rgb` outside `theme.rs` → replace with `theme::*`  
2. Add `theme::Density { Comfortable, Compact }` multiplier (1.0 / 0.85) for spacing  
3. Stub `apply_light_theme()` table (can be incomplete until Phase 2–3)  
4. Document token names next to mock CSS vars if present in HTML  

**Acceptance:** `rg 'Color32::from_rgb' src --glob '!**/theme.rs'` → empty (or listed exceptions with `// theme-allow`).

### Status (done 2026-08-06)

| Item | Result |
| :--- | :--- |
| Raw RGB outside theme | ✅ none (allow: `Color32::TRANSPARENT` only) |
| Semantic tokens | SUCCESS / WARN / DANGER / INFO + ANNO_* + overlays |
| `danger_pulse(t)` | REC pill animation |
| `Density` enum | Ready for 1d wiring (not UI-toggled yet) |
| `apply_light_theme` | Stubbed; not exposed in Settings yet |
| Smoke | 20/20 |

Check: `grep -rn 'Color32::from_rgb' src --include='*.rs' | grep -v theme.rs`

---

## Phase 1c — Chrome vertical slice (mock language)

Match **screen 1** language first (shell + library + Shutter + live REC).

| Item | Mock ref | Implementation note |
| :--- | :--- | :--- |
| Graphite canvas, paper primaries | All screens | `theme.rs` |
| Loop rail + pending badge + REC live | Screen 1 | `shell.rs` / existing `loop_rail` |
| Loop-position badges on cards | Screen 1 | Small chip on library rows: Capture/Review/… derived from path/status |
| Shutter dock + hotkey hints | Screen 1 / 5 | Existing `shutter_strip`; translucent fill not blur |
| Status strip | Screen 1 | storage / budget / ffmpeg / MCP stub / agent activity — **read-only chips**, no new backend |
| Severity toasts | Screen 5 post-capture | Existing `show_toast_card` |
| Empty states | Media / Inbox | Existing `empty_state` |

**Out of 1c:** conversation inbox redesign (→ Phase 2), wizard (→ Phase 3), region loupe (→ Phase 3).

### Status (done 2026-08-06)

| Item | Result |
| :--- | :--- |
| `LoopPosition` heuristic | `app/library.rs` — Capture / Review / Annotate / Ask / Answered |
| Library card badges | `loop_position_badge` on each row |
| Status strip | bottom of main content: storage · tier · ffmpeg · inbox · REC |
| Shutter glass | `SURFACE_GLASS` translucent fill |
| Smoke / G4 | still green |

---

## Phase 1d — Cheap high-trust wins (pull into 1–2)

All chrome-cost, no heavy deps:

| Win | Est. size | Notes |
| :--- | :--- | :--- |
| **⌘K / Ctrl+K palette** | ~250–400 lines | Filtered action list → same handlers as buttons/hotkeys |
| **Session restore** | ~100–150 lines | Serialize geometry, tab, last `edit_file`, density → `config/session.json` |
| **Density toggle** | ~30 lines | Token multiplier in theme |
| **Undo toast for prune/delete** | ~80–120 lines | Keep last deleted paths in trash staging for N seconds |

### Status (done 2026-08-06)

| Item | Result |
| :--- | :--- |
| Command palette | `ui/palette.rs` — ⌘K / Ctrl+K, header button, filter + Enter |
| Session | `app/session.rs` → `session.json` (tab, edit file, density, filter) |
| Density | Settings + palette toggle; spacing uses `density.sp` in header |
| Undo delete | Soft-stage to `undo_trash/`; **Z** within ~12s restores |
| Smoke / G4 / binary | green · ~14 MB |

---

## Explicitly deferred (not Phase 1)

| Item | Why | Later policy |
| :--- | :--- | :--- |
| Retro buffer | Policy + memory; not infeasible | Spike: **off by default**, 60s @ ~2 fps, hard cap ~200 MB |
| Real timeline + playhead | Heavy UI + media | Keep filmstrip + HH:MM:SS |
| ASR transcript / waveform | New crates / services | Voice path only |
| OCR redaction | Heavy | Manual blur |
| Full light parity every pane | After tokens solid | Phase 3 |
| Backdrop blur / spring physics | egui limits | Approximate |
| `resvg` / new icon framework | Dep bloat | PNG via `egui_extras` |

---

## CI checks to add (lightweight)

```bash
# Binary size (adjust path for CI artifact)
test $(stat -f%z target/release/vibecap 2>/dev/null || stat -c%s target/release/vibecap) -lt 20971520

# main.rs only shrinks — enforce in PR script vs base branch
# Token leak (warn or fail after 1b complete)
# rg 'Color32::from_rgb' src --glob '!**/theme.rs' && exit 1
```

Smoke remains: `./scripts/smoke_mcp.sh` (MCP freeze).

---

## Suggested PR slices (mergeable)

| PR | Title | Gate |
| ---: | :--- | :--- |
| 1 | `refactor: extract MCP + budget + feedback from main` | G2↓, G5, smoke |
| 2 | `refactor: extract recording + library helpers` | G2↓, smoke + manual rec |
| 3 | `refactor: move tab UIs into src/ui/*_tab.rs` | G2↓, visual check |
| 4 | `ui: enforce theme tokens (no raw colors)` | G4 |
| 5 | `ui: status strip + loop badges on library cards` | mock screen 1 |
| 6 | `ui: command palette + session restore + density` | 1d |
| 7 | `ci: binary size + main.rs line budget` | G1–G3 |

---

## One sentence

**The revamp’s real cost isn’t the new UI — it’s the refactor that makes the new UI cheap; pay that first (1a), then chrome (1b–1d), without growing deps or the binary.**

---

## Done when Phase 1 is “done”

- [ ] `main.rs` substantially smaller; tab UI outside it  
- [ ] Gates G1–G5 hold  
- [ ] Dark shell recognisable vs mock screen 1 (language, not pixels)  
- [ ] Shutter + rail + toasts + empty states stable  
- [ ] At least one of: palette / session restore / undo-toast  
- [ ] MCP smoke green; capture still works on macOS  

**Not required for Phase 1 done:** conversation inbox mock, wizard, light theme complete, retro buffer.

---

## Phase 2 — Inbox conversation + post-capture (done 2026-08-06)

| Item | Status |
| :--- | :--- |
| Post-capture toast (Annotate / Copy / Reveal / ✕) | ✅ no auto-open annotation |
| Conversation inbox list + detail composer | ✅ agent dots, priority chips, media preview, choice chips |
| Full mock 2-pane Inbox | ✅ left thread list (~280px) + right conversation pane |
| Wizard / Capture HUD loupe | → Phase 3 |

## Phase 3 — Wizard + Capture HUD + light theme (done 2026-08-06)

| Item | Status |
| :--- | :--- |
| First-run wizard (4 steps) | ✅ welcome · save dir · budget tier · shortcuts; skip; Settings replay |
| `wizard_done` in session.json | ✅ new installs only (old sessions migrate skip) |
| Capture HUD W×H | ✅ `src/ui/capture_hud.rs` |
| Region thirds grid + handles | ✅ rule-of-thirds + corner/edge handles |
| Light theme parity | ✅ dual-palette functions; Settings + palette toggle; session `theme` |
| Cursor loupe (chrome) | ✅ ring + crosshair + coords; no continuous pixel sample (lightweight) |
| Arrow nudge + last region | ✅ ←↑↓→ / ⇧10px · Enter confirm · ghost + `session.json` |
| Retro buffer spike | ✅ `src/app/retro.rs` — off by default, 2 fps, 15/30/60s, 200 MB cap, GIF dump |
| Record countdown bubble | ✅ 0/3/5s · Esc cancel · Capture HUD family |
| Bug report pack | ✅ screenshot + retro GIF one-shot (palette / settings / capture) |
| Window picker | ✅ running apps combo + focus-before-capture |
| MCP retro/apps/bug | ✅ 16 tools: list_apps, set_retro, save_retro, bug_report |

---
