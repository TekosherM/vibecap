# Contributing

Thanks for helping with Vibecap. Keep the project **comprehensive but lightweight**: one binary, clear docs, no unnecessary crates or services.

## Try first (with your agent)

The most useful contributions start from real use:

1. Install and leave `vibecap` running in the tray.  
2. Wire `vibecap --mcp` (and/or the skill under `skills/vibecap/`) into **your** agent harness.  
3. Use capture + Inbox for visual feedback while coding.  
4. Open an [issue](https://github.com/TekosherM/vibecap/issues) with OS, harness name, and what felt broken or missing.

Screenshots or short GIFs taken **with Vibecap** are ideal repros.

## Setup

```bash
git clone https://github.com/TekosherM/vibecap.git
cd vibecap
cargo build --release
./scripts/smoke_mcp.sh
```

Needs: Rust (edition 2021), ffmpeg, macOS Screen Recording permission for capture tests.

## Before you open a PR

1. `cargo build --release`
2. `./scripts/smoke_mcp.sh`
3. Update docs if you change CLI flags, MCP tools, or paths:
   - `README.md` (entry)
   - `docs/USAGE.md`, `docs/MCP.md`, `docs/ARCHITECTURE.md`
   - `skills/vibecap/SKILL.md` **and** `.agents/skills/vibecap/SKILL.md` (keep identical)

## Scope guidelines

| Do | Avoid |
| :--- | :--- |
| Small, focused PRs | Drive-by refactors across the whole UI |
| Shell out to ffmpeg / OS tools when enough | Pulling large media frameworks “just because” |
| File-based state under `~/.config/vibecap` | Network services for local agent IPC |
| Document new MCP tools in `docs/MCP.md` | Undocumented tool surface |

## Code layout

| Path | Role |
| :--- | :--- |
| `src/main.rs` | eframe shell, tray, hotkeys, orchestration |
| `src/app/` | Budget, feedback, library, MCP, recording, retro, session |
| `src/ui/` | Safelight tabs, theme, components |
| `src/platform/` | Capture, ffmpeg resolve, notify, shell/OS |

## License

Contributions are dual-licensed under MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`).
