# codepeek (TUI)

The terminal user interface binary. This is the main entry point for codepeek.

## Scope

- Application bootstrap and terminal lifecycle (ratatui init/restore)
- Wiring together crates: `codepeek-git` (change detection), `codepeek-syntax` (highlighting), `codepeek-view` (UI)
- Error reporting via color-eyre

## Not in scope

- Domain logic or shared types -> `codepeek-core`
- Git operations -> `codepeek-git`
- Syntax highlighting -> `codepeek-syntax`
- UI components, rendering, and layout -> `codepeek-view`
