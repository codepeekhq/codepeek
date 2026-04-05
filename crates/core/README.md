# codepeek-core

Domain logic and shared types for the codepeek ecosystem.

## Scope

- Domain models: `FileChange`, `ChangeKind`, `DiffHunk`, `DiffLine`, `ChangeMap`
- Syntax types: `HighlightKind`, `HighlightSpan`, `HighlightedLine`
- Trait definitions: `ChangeDetector` (git operations), `SyntaxHighlighter` (parsing)
- Error types: `ChangeError`, `SyntaxError` (via thiserror)
- No dependencies on UI frameworks, git libraries, or tree-sitter

## Not in scope

- Git implementation -> `codepeek-git`
- Syntax highlighting implementation -> `codepeek-syntax`
- Rendering or terminal UI -> `codepeek-view`
- Application bootstrap or wiring -> `apps/tui`
