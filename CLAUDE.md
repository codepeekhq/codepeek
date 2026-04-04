# CLAUDE.md

## Commands

- Use `just` for all commands, not raw `cargo`
- `just check` runs fmt, lint, and test - run this before committing
- `just run` to launch the TUI
- `just test` to run tests

## Architecture

- Monorepo: `apps/` for binaries, `crates/` for libraries
- Dependency direction: `apps/tui` -> `codepeek-view` -> `codepeek-core`
- `codepeek-core`: domain logic and shared types, MUST NOT depend on UI frameworks
- `codepeek-view`: presentation layer (ratatui components, rendering, event handling)
- `apps/tui`: thin binary, only wires crates together and manages terminal lifecycle
- New code goes in the appropriate crate based on these boundaries, not in the app

## Code style

- No `unsafe` code, it is forbidden at workspace level
- Pin all dependency versions exactly, no `^` or `~` ranges
- Workspace-level dependency declarations in root `Cargo.toml`, crates reference with `.workspace = true`

## Documentation

- `docs/decisions.md` tracks project decisions as a timestamped bulleted list
- `docs/plans/` holds implementation plans, one file per feature, named `<timestamp>--<feature-name>.md`
- When making architectural or technical decisions, add them to `docs/decisions.md`
- Before starting a non-trivial feature, create a plan in `docs/plans/` and align with the user before coding
