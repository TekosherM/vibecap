# Testing

Vibecap is a GUI + OS-capture tool. Automated coverage focuses on **headless surfaces** (CLI + MCP) that CI and agents can run without a display server session for every path.

## Quick checks

```bash
# Compile
cargo build --release

# CLI surface
./target/release/vibecap --help
./target/release/vibecap --version

# MCP protocol smoke (no screen capture required for budget/feedback tools)
./scripts/smoke_mcp.sh

# Phase 1 design gates (lightweight)
test "$(wc -l < src/main.rs)" -lt 2500
# G4: no raw RGB outside theme (TRANSPARENT ok)
! grep -rn 'Color32::from_rgb' src --include='*.rs' | grep -v theme.rs
```

## Smoke script (`scripts/smoke_mcp.sh`)

Exercises:

1. `initialize` → server info `vibecap` / `0.1.0`
2. `tools/list` → ≥12 documented tools (incl. list/cancel feedback)
3. `vibecap_set_budget` / `vibecap_get_spending`
4. `vibecap_request_feedback` / `get_feedback` / `list_feedback` / `cancel_feedback`
5. Text-only `request_feedback` (no media) with options
6. `ping`

Optional (macOS + Screen Recording permission):

```bash
SMOKE_CAPTURE=1 ./scripts/smoke_mcp.sh
# also runs vibecap --screenshot and vibecap_export_gif on a tiny synthetic clip

# Pre-commit capture functional (CLI shot size + MCP Finder focus + gates)
./scripts/smoke_capture.sh
```

## Manual GUI checklist

Run once before a release:

| # | Action | Pass if |
| ---: | :--- | :--- |
| 1 | Launch `vibecap` | Window opens, Capture tab usable |
| 2 | Open Safari/Finder, click Vibecap, Fullscreen **Screenshot** | Shot shows that app (not empty desktop); Capture tip lists last front app |
| 3 | Window target → pick app → Screenshot | App focused then captured |
| 4 | Screenshot from UI | Image lands via post-capture toast (Annotate optional) |
| 3 | Draw pen + step badge + save | File under `~/Movies/Vibecap/` |
| 4 | Record ~3s fullscreen | MP4 appears in Library |
| 5 | Edit → export GIF range | GIF plays, ~15 FPS |
| 6 | Settings → set budget caps | Caps visible; MCP `get_spending` matches |
| 7 | MCP `request_feedback` while app open | **🤖 Agent Inbox** shows request (priority / options / agent label) |
| 8 | Answer with text / choice / mark-up + poll MCP | Agent sees text, choice, and/or annotated path |
| 9 | Text-only high-priority decision | Inbox shows without media; chips one-tap |
| 10 | Annotate-only (empty text) + poll | `annotated_media` path; agent opens with vision |
| 11 | Dismiss + agent cancel | `status=dismissed` / `cancelled` on poll |

## What we do **not** automate yet

- Pixel-perfect annotation rendering
- Global hotkey registration on CI runners
- Cross-platform capture (not implemented)

## Regression tips

- Prefer fixing real ffmpeg exit codes (jobs already surface stderr tails in the UI).
- Budget file corruption should fail closed — delete `~/.config/vibecap/budget.json` to reset.
- Keep docs and `tools/list` in sync when adding MCP tools.
