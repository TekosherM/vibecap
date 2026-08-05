# Contributing

Thanks for helping with Vibecap. Keep the project **comprehensive but lightweight**: one binary, clear docs, no unnecessary crates or services.

## Setup

```bash
git clone https://github.com/TekosherM/vibecap.git
cd vibecap
cargo build --release
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

Everything ships from `src/main.rs` for now. Prefer small helper functions near related state rather than a premature multi-crate split. Platform abstraction (Phase 3) should land as clear modules when Windows/Linux backends are real.

## License

Contributions are dual-licensed under MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`).
