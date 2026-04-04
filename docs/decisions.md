# Decisions

- 2026-04-03: Monorepo using Cargo workspace with `apps/` and `crates/` separation
- 2026-04-03: `apps/` holds runnable binaries, `crates/` holds libraries
- 2026-04-03: Rust edition 2024 with resolver v3
- 2026-04-03: `codepeek-core` owns domain logic and shared types, no UI dependencies
- 2026-04-03: `codepeek-view` owns presentation layer (components, rendering, event handling), depends on core
- 2026-04-03: `apps/tui` is a thin binary that wires crates together and manages terminal lifecycle
- 2026-04-03: `ratatui` as the terminal UI framework
- 2026-04-03: `color-eyre` for error reporting in the TUI app, not in library crates
- 2026-04-03: All dependency versions pinned at workspace level
- 2026-04-03: `unsafe_code` forbidden across the workspace
- 2026-04-03: Clippy `all` deny, `pedantic` warn, with exceptions for `module_name_repetitions`, `must_use_candidate`, `missing_errors_doc`
- 2026-04-03: `rustfmt` with `max_width = 100` and `use_field_init_shorthand = true`
- 2026-04-03: `just` as the command runner with `just check` as a single pre-push gate
- 2026-04-03: MIT license
- 2026-04-04: `docs/decisions.md` tracks all project decisions as a flat timestamped list
- 2026-04-04: `docs/plans/` holds implementation plans, one file per feature, named `<timestamp>--<feature-name>.md`
