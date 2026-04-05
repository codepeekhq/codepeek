# codepeek-view

Presentation layer and UI components for codepeek, built on ratatui.

## Scope

- `App`: main application state machine, event loop, focus management
- `FileList`: changed files list with selection, badges, and rename display
- `FileViewer`: syntax-highlighted source with line numbers, gutter marks, and inline diff toggle
- `PeekOverlay`: floating overlay showing HEAD content of deleted files
- `StatusBar`: context-sensitive key binding hints
- Theme module: colors, styles, and gutter marks for all change kinds
- Depends on `codepeek-core` for domain types and traits (receives implementations via `Box<dyn ChangeDetector>` / `Box<dyn SyntaxHighlighter>`)

## Not in scope

- Domain logic, types, or trait definitions -> `codepeek-core`
- Git implementation -> `codepeek-git`
- Syntax highlighting implementation -> `codepeek-syntax`
- Application bootstrap or terminal lifecycle -> `apps/tui`
