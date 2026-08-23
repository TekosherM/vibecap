# Contributing

Thanks for helping with Vibecap. Keep it **comprehensive but lightweight**.

Two surfaces:

| Surface | Stay lightweight by |
| :--- | :--- |
| Native (`src/`) | One Rust binary. No extra crates or cloud for local MCP. |
| Web (`web/`) | HTTP studio. Unowned DB rows. Auth off. Stills in the pack, not a home folder. |

## Try first (with your agent)

The most useful contributions start from real use:

**Native**

1. Leave `vibecap` running in the tray.
2. Wire `vibecap --mcp` and/or `skills/vibecap/` into your harness.
3. Capture + optional Inbox while coding.

**Web** (when MCP never attaches)

1. `cd web && npm install && npm run dev` — leave the tab open. **That tab is the connector.**
2. Capture-only: hooks → `record_start` → `snapshot` → `record_stop` → `bug_pack`.
3. Do not look in `~/Movies/Vibecap`.

Open an [issue](https://github.com/TekosherM/vibecap/issues) with OS, harness, and what broke. Visual repros taken with Vibecap are ideal.

## Setup

```bash
git clone https://github.com/TekosherM/vibecap.git
cd vibecap
cargo build --release
./scripts/smoke_mcp.sh

cd web && npm install && npm run typecheck
```

Native needs: Rust (edition 2021), ffmpeg, macOS Screen Recording for capture tests.

## Before you open a PR

**Native**

1. `cargo build --release`
2. `./scripts/smoke_mcp.sh`
3. If you change CLI flags, MCP tools, or paths: `README.md`, `docs/USAGE.md`, `docs/MCP.md`, `docs/ARCHITECTURE.md`, and both skill copies (`skills/vibecap/SKILL.md` **and** `.agents/skills/vibecap/SKILL.md`).

**Web**

1. `cd web && npm run typecheck`
2. If you change agent tools, hooks, or output: `docs/WEB.md`, `docs/HOOKS.md`, `web/README.md`, and both skill copies.

## Scope guidelines

| Do | Avoid |
| :--- | :--- |
| Small, focused PRs | Drive-by refactors across the whole UI |
| Shell out to ffmpeg / OS tools when enough | Pulling large media frameworks “just because” |
| File-based state under `~/.config/vibecap` (native) | Network services for local MCP |
| Pack / Media downloads (web) | Writing stills to `~/Movies` or `~/Vibecap` from the web studio |
| Document new tools in `docs/MCP.md` or `docs/WEB.md` | Undocumented tool surface |

## Code layout

| Path | Role |
| :--- | :--- |
| `src/main.rs` | Native eframe shell, tray, hotkeys |
| `src/app/` | Budget, feedback, library, MCP, recording |
| `src/ui/` | Safelight tabs, theme |
| `src/platform/` | Capture, ffmpeg, notify, OS |
| `web/` | HTTP evidence studio |
| `docs/WEB.md` · `docs/HOOKS.md` | Agent HTTP + when to collect each layer |

## License

Contributions are dual-licensed under MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`).
