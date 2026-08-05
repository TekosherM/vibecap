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
```

## Smoke script (`scripts/smoke_mcp.sh`)

Exercises:

1. `initialize` → server info `vibecap` / `0.1.0`
2. `tools/list` → exactly the 10 documented tools
3. `vibecap_set_budget` / `vibecap_get_spending`
4. `vibecap_request_feedback` / `vibecap_get_feedback` (pending path)
5. `ping`

Optional (macOS + Screen Recording permission):

```bash
SMOKE_CAPTURE=1 ./scripts/smoke_mcp.sh
# also runs vibecap --screenshot and vibecap_export_gif on a tiny synthetic clip
```

## Manual GUI checklist

Run once before a release:

| # | Action | Pass if |
| ---: | :--- | :--- |
| 1 | Launch `vibecap` | Window opens, Capture tab usable |
| 2 | Screenshot from UI | Image lands in Annotation Studio |
| 3 | Draw pen + step badge + save | File under `~/Movies/Vibecap/` |
| 4 | Record ~3s fullscreen | MP4 appears in Library |
| 5 | Edit → export GIF range | GIF plays, ~15 FPS |
| 6 | Settings → set budget caps | Caps visible; MCP `get_spending` matches |
| 7 | MCP `request_feedback` while app open | Feedback tab shows request |
| 8 | Answer feedback + poll MCP | Agent receives text |

## What we do **not** automate yet

- Pixel-perfect annotation rendering
- Global hotkey registration on CI runners
- Cross-platform capture (not implemented)

## Regression tips

- Prefer fixing real ffmpeg exit codes (jobs already surface stderr tails in the UI).
- Budget file corruption should fail closed — delete `~/.config/vibecap/budget.json` to reset.
- Keep docs and `tools/list` in sync when adding MCP tools.
