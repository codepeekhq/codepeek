# codepeek-syntax

Tree-sitter syntax highlighting for codepeek.

## Scope

- `TreeSitter`: implements `codepeek_core::SyntaxHighlighter` using tree-sitter-highlight
- `Noop`: fallback highlighter that returns lines without syntax spans
- Language detection from file extensions (19 extensions mapped to 17 grammars)
- Bundled grammars: Rust, Python, JavaScript, JSX, TypeScript, TSX, Go, C, C++, Java, Ruby, TOML, JSON, Bash, CSS, HTML, YAML, Lua, Markdown
- Maps tree-sitter capture names to `HighlightKind` via exact match and prefix fallback

## Not in scope

- Domain types or trait definitions -> `codepeek-core`
- Git operations -> `codepeek-git`
- UI rendering -> `codepeek-view`
