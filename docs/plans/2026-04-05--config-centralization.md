# Config Centralization

**Date:** 2026-04-05

## Problem

After the last 4 commits added change-viewer, git integration, and syntax highlighting,
several categories of hardcoded values are scattered across component files:

- **Keybindings** duplicated in 3 component files (file_list, file_viewer, peek_overlay)
- **Behavior constants** (scroll amount, tick rate, line length limits) scattered across app.rs and components
- **Language support** hardcoded with no way to enable/disable languages

## Solution

Three targeted changes, no new crates.

### 1. Keybindings module (`crates/view/src/keybindings.rs`)

Centralize all key → action mappings as predicate functions. Components call
`keybindings::is_move_up(key)` instead of matching `KeyCode::Up | KeyCode::Char('k')` directly.

### 2. Config module (`crates/view/src/config.rs`)

Centralize behavior constants:

| Constant | Value | Currently in |
|----------|-------|-------------|
| `TICK_RATE` | 16ms | `app.rs` |
| `PAGE_SCROLL_LINES` | 20 | `file_viewer.rs`, `peek_overlay.rs` |
| `MAX_LINE_LENGTH` | 500 | `file_viewer.rs` |
| `BINARY_DETECTION_LIMIT` | 8192 | `app.rs` |
| `FILE_LIST_WIDTH_PERCENT` | 30 | `app.rs` |
| `FILE_VIEWER_WIDTH_PERCENT` | 70 | `app.rs` |
| `POPUP_WIDTH_PERCENT` | 70 | `peek_overlay.rs` |
| `POPUP_HEIGHT_PERCENT` | 80 | `peek_overlay.rs` |

### 3. Config-gated language support

- `TreeSitter::with_languages(enabled: HashSet<String>)` constructor
- When a language isn't in the enabled set, `highlight()` returns `UnsupportedLanguage` (app falls back to plain text)
- Config file at `~/.config/codepeek/config.toml` controls enabled languages
- Default when no config file: all 19 languages enabled
- New dependencies: `serde`, `toml`, `dirs` (in `apps/tui` only)

### Config file format

```toml
[languages]
enabled = [
    "rust", "python", "javascript", "typescript",
    "go", "c", "cpp", "java", "ruby",
    "toml", "json", "yaml", "bash",
    "css", "html", "lua", "markdown",
]
```

## What we're NOT doing

- No config crate (premature — config stays in modules until user-facing config grows)
- No theme extraction (already centralized in theme.rs)
- No UI string extraction (presentation labels, not user config)
- No runtime dynamic loading of tree-sitter grammars (requires unsafe)
- No feature flags for languages (developer-only, doesn't help end users)
