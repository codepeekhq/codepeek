# Change Viewer v1

## Summary

A read-only change viewer that shows uncommitted changes (staged + unstaged vs HEAD) with tree-sitter syntax highlighting and gutter marks on changed lines.

### Core loop

1. Launch codepeek
2. See a list of uncommitted changed files, sorted by filesystem mtime (most recent first)
3. Navigate the list, select a file
4. See the full file with tree-sitter syntax highlighting and gutter marks on changed lines
5. Optionally toggle a diff view for a specific block
6. Deleted files appear in the list with a peek overlay showing old content
7. `q` to quit

### Out of scope for v1

- Staging/unstaging
- Committing
- Editor jumping
- Claude Code session launching
- Issue/fix tagging
- Search within files

---

## Architecture

### Crate structure

```
crates/
  core/       domain types + traits (no external deps beyond std + thiserror)
  git/        git2-based change detection and diff computation
  syntax/     tree-sitter syntax highlighting
  view/       ratatui components, rendering, event handling
apps/
  tui/        thin binary: init terminal, wire crates, run event loop
```

### Dependency graph

```
apps/tui ──> codepeek-view ──> codepeek-core
         |
         |-> codepeek-git ───> codepeek-core
         |
         |-> codepeek-syntax -> codepeek-core
```

Key constraint: **`codepeek-view` depends only on `codepeek-core`, never on `git` or `syntax` directly.** The TUI app injects concrete implementations via traits defined in `core`. This keeps the view layer decoupled and testable without git repos or tree-sitter parsers.

### Why this split

| Crate | Responsibility | Key dependency |
|-------|---------------|----------------|
| `core` | Shared domain types, trait definitions, error types | `thiserror` only |
| `git` | Detect changed files, compute line-level diffs | `git2` |
| `syntax` | Parse source code, produce highlighted spans | `tree-sitter`, `tree-sitter-highlight`, grammar crates |
| `view` | Ratatui components, layout, event handling, rendering | `ratatui`, `crossterm` |
| `apps/tui` | Terminal lifecycle, wire crates together, panic hooks | `color-eyre`, all workspace crates |

This follows NLM guidance: core at the bottom of the dependency tree for maximum stability, implementation crates depend on core, app wires everything.

---

## Core crate (`codepeek-core`)

### Domain types

```rust
// change.rs
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
}

pub struct FileChange {
    pub path: PathBuf,
    pub kind: ChangeKind,
    pub mtime: SystemTime, // filesystem mtime, used for ordering
}
```

```rust
// diff.rs
pub enum LineChange {
    Added,
    Removed,
    Modified,
}

pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

pub struct DiffLine {
    pub kind: LineChange,
    pub content: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
}

/// Set of line numbers that have changes, for gutter marking.
/// Derived from DiffHunks but optimized for per-line lookup.
pub struct ChangeMap {
    pub added: HashSet<u32>,    // lines that are new
    pub modified: HashSet<u32>, // lines that differ from HEAD
    pub deleted: Vec<u32>,      // line numbers where deletions occurred (between lines)
}
```

```rust
// highlight.rs
pub enum HighlightKind {
    Keyword,
    Function,
    Type,
    String,
    Comment,
    Number,
    Operator,
    Variable,
    Punctuation,
    // ... extend as needed, map from tree-sitter capture names
}

pub struct HighlightSpan {
    pub start: usize,  // byte offset within line
    pub end: usize,    // byte offset within line
    pub kind: HighlightKind,
}

pub struct HighlightedLine {
    pub content: String,
    pub spans: Vec<HighlightSpan>,
}
```

### Traits

```rust
// traits.rs
pub trait ChangeDetector: Send + Sync {
    /// List all uncommitted changes, both staged and unstaged.
    fn detect_changes(&self) -> Result<Vec<FileChange>, ChangeError>;

    /// Compute line-level diff for a specific file against HEAD.
    fn compute_diff(&self, path: &Path) -> Result<Vec<DiffHunk>, ChangeError>;

    /// Read file content at HEAD (for deleted files / old version).
    fn read_at_head(&self, path: &Path) -> Result<String, ChangeError>;
}

pub trait SyntaxHighlighter: Send + Sync {
    /// Highlight source code, returning one HighlightedLine per line.
    fn highlight(&self, source: &str, path: &Path) -> Result<Vec<HighlightedLine>, SyntaxError>;
}
```

### Error types

```rust
// error.rs — using thiserror
#[derive(Debug, thiserror::Error)]
pub enum ChangeError {
    #[error("repository not found at {path}")]
    RepoNotFound { path: PathBuf },

    #[error("failed to read file status")]
    StatusFailed(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("failed to compute diff for {path}")]
    DiffFailed {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("file not found at HEAD: {path}")]
    FileNotInHead { path: PathBuf },
}

#[derive(Debug, thiserror::Error)]
pub enum SyntaxError {
    #[error("unsupported language for {path}")]
    UnsupportedLanguage { path: PathBuf },

    #[error("failed to parse {path}")]
    ParseFailed {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
```

### Module layout

```
crates/core/src/
  lib.rs           re-exports all public types and traits
  change.rs        FileChange, ChangeKind
  diff.rs          DiffHunk, DiffLine, LineChange, ChangeMap
  highlight.rs     HighlightSpan, HighlightedLine, HighlightKind
  traits.rs        ChangeDetector, SyntaxHighlighter
  error.rs         ChangeError, SyntaxError
```

---

## Git crate (`codepeek-git`)

### Responsibility

Implements `ChangeDetector` using `git2`. Encapsulates all libgit2 interactions.

### Key dependency choice: `git2` over `gix`

`git2` (v0.20.x) is chosen over `gix` because:
- Line-level diff API is mature and complete (`diff_index_to_workdir`, `diff_tree_to_index`)
- `statuses()` API directly maps to our needs
- `gix` diff subsystem is still experimental and not at full git parity
- `git2` is battle-tested in production (cargo, etc.)

Trade-off: C dependency (libgit2). Acceptable for a desktop TUI.

### Implementation sketch

```rust
pub struct GitChangeDetector {
    repo: git2::Repository,
}

impl GitChangeDetector {
    pub fn open(path: &Path) -> Result<Self, ChangeError> {
        let repo = Repository::discover(path)
            .map_err(|_| ChangeError::RepoNotFound { path: path.to_path_buf() })?;
        Ok(Self { repo })
    }
}

impl ChangeDetector for GitChangeDetector {
    fn detect_changes(&self) -> Result<Vec<FileChange>> {
        // 1. repo.statuses(None) to get all changed files
        // 2. For each entry, map git2::Status flags to ChangeKind:
        //    - INDEX_NEW | WT_NEW → Added
        //    - INDEX_MODIFIED | WT_MODIFIED → Modified
        //    - INDEX_DELETED | WT_DELETED → Deleted
        //    - INDEX_RENAMED → Renamed
        // 3. For each file, stat the filesystem to get mtime
        // 4. Return sorted by mtime descending
    }

    fn compute_diff(&self, path: &Path) -> Result<Vec<DiffHunk>> {
        // 1. Get HEAD tree
        // 2. diff_tree_to_workdir_with_index() for combined staged+unstaged
        // 3. Filter to the specific file path
        // 4. Walk hunks and lines, map to DiffHunk/DiffLine types
    }

    fn read_at_head(&self, path: &Path) -> Result<String> {
        // 1. Get HEAD tree
        // 2. Find blob at path
        // 3. Return content as string
    }
}
```

### Module layout

```
crates/git/src/
  lib.rs           re-exports GitChangeDetector
  detector.rs      ChangeDetector implementation
  error.rs         internal error mapping (git2::Error → ChangeError)
```

### Dependencies

```toml
[dependencies]
codepeek-core.workspace = true
git2 = "0.20.4"   # pin exact
```

---

## Syntax crate (`codepeek-syntax`)

### Responsibility

Implements `SyntaxHighlighter` using tree-sitter. Handles language detection, grammar loading, highlight query execution, and mapping capture names to `HighlightKind`.

### Key dependency choices

- **`tree-sitter`** (v0.26.x) + **`tree-sitter-highlight`** for the highlight pipeline
- **`tree-sitter-language-pack`** for polyglot grammar support (248+ languages with on-demand loading) instead of individual grammar crates — dramatically reduces maintenance burden
- Language detection by file extension (simple map), no magic/heuristics for v1

### Implementation sketch

```rust
pub struct TreeSitterHighlighter {
    // Cache of HighlightConfiguration per language, built lazily
    configs: HashMap<String, HighlightConfiguration>,
    highlight_names: Vec<String>,
}

impl TreeSitterHighlighter {
    pub fn new() -> Self {
        // Define the highlight names we care about, mapped to HighlightKind
        let highlight_names = vec![
            "keyword", "function", "function.builtin",
            "type", "type.builtin", "string", "comment",
            "number", "operator", "variable", "variable.builtin",
            "punctuation.bracket", "punctuation.delimiter",
            "constant", "constant.builtin", "property",
            "tag", "attribute",
        ];
        Self {
            configs: HashMap::new(),
            highlight_names,
        }
    }

    fn get_config(&mut self, lang_name: &str) -> Result<&HighlightConfiguration> {
        // Lazily load from tree-sitter-language-pack
        // Each config is created once per language and cached
    }

    fn detect_language(path: &Path) -> Option<String> {
        // Map file extension to tree-sitter language name
        // .rs → "rust", .js → "javascript", .py → "python", etc.
    }

    fn map_highlight(index: usize, names: &[String]) -> HighlightKind {
        // Map capture name to HighlightKind enum
        // "keyword" → HighlightKind::Keyword, etc.
    }
}

impl SyntaxHighlighter for TreeSitterHighlighter {
    fn highlight(&self, source: &str, path: &Path) -> Result<Vec<HighlightedLine>> {
        // 1. Detect language from file extension
        // 2. Get or create HighlightConfiguration for that language
        // 3. Run highlighter.highlight(&config, source.as_bytes(), None, |_| None)
        // 4. Walk HighlightEvents, split by newlines into HighlightedLines
        // 5. Map each HighlightEvent::HighlightStart to a HighlightSpan with kind
        // 6. Return Vec<HighlightedLine>
        //
        // Note: tree-sitter-highlight returns byte offsets, so we must
        // track current line and convert byte offsets to line-relative offsets.
    }
}
```

### Caching strategy

- `HighlightConfiguration` per language: created once, reused across files
- Highlight results are **not** cached at this layer — the view layer decides what to cache (e.g., only visible lines). This keeps the syntax crate stateless per call.

### Module layout

```
crates/syntax/src/
  lib.rs              re-exports TreeSitterHighlighter
  highlighter.rs      SyntaxHighlighter implementation
  languages.rs        language detection (extension → name map)
  mapping.rs          capture name → HighlightKind mapping
```

### Dependencies

```toml
[dependencies]
codepeek-core.workspace = true
tree-sitter = "0.26.8"              # pin exact
tree-sitter-highlight = "0.25.3"    # pin exact — verify latest at impl time
tree-sitter-language-pack = "0.6.0" # pin exact — verify latest at impl time
```

---

## View crate (`codepeek-view`)

### Responsibility

All ratatui components, layout, event handling, and rendering. Depends only on `codepeek-core` types and traits — never on `git2` or `tree-sitter`.

### Architecture pattern: Component architecture (NLM-recommended)

Each component encapsulates:
- Its own **state**
- Its own **event handling** (returns messages/actions)
- Its own **rendering** logic

Top-level `App` routes events to the focused component.

### State model

```rust
// app.rs
pub enum Focus {
    FileList,
    FileViewer,
}

pub struct App {
    focus: Focus,
    file_list: FileList,
    file_viewer: FileViewer,
    change_detector: Box<dyn ChangeDetector>,
    highlighter: Box<dyn SyntaxHighlighter>,
    should_quit: bool,
}
```

```rust
// components/file_list.rs
pub struct FileList {
    files: Vec<FileChange>,
    selected: usize,
    scroll_offset: usize,
}
```

```rust
// components/file_viewer.rs
pub struct FileViewer {
    lines: Vec<HighlightedLine>,   // highlighted source
    change_map: ChangeMap,          // which lines changed
    scroll_offset: usize,
    file_path: Option<PathBuf>,
    show_diff: bool,                // diff overlay toggle
    diff_hunks: Vec<DiffHunk>,      // loaded when file selected
}
```

### Event flow

```
Terminal Event (crossterm)
  → App.handle_event()
    → match self.focus:
        FileList  → FileList.handle_event() → Action
        FileViewer → FileViewer.handle_event() → Action
    → App.dispatch(action)
        Action::Quit → set should_quit
        Action::SelectFile(idx) → load file, highlight, compute diff, switch focus
        Action::Back → switch focus to FileList
        Action::ToggleDiff → toggle diff overlay
        Action::ScrollUp/Down → delegate to focused component
        Action::PeekDeleted(idx) → show overlay with HEAD content
```

### Action enum

```rust
pub enum Action {
    Quit,
    SelectFile(usize),
    Back,
    ToggleDiff,
    ScrollUp(u16),
    ScrollDown(u16),
    PeekDeleted(usize),
    DismissPeek,
    Noop,
}
```

### Layout

```
Two-panel layout:

┌──────────────────────────────────────────────────────────┐
│ ┌─ Changed Files ──────┐ ┌─ file.rs ──────────────────┐ │
│ │ ▌ M src/main.rs      │ │   1  use std::io;          │ │
│ │   M src/lib.rs       │ │   2                         │ │
│ │   A src/new_file.rs  │ │ ▎ 3  fn main() {           │ │
│ │   D old_module.rs    │ │ ▎ 4      let x = 42;       │ │
│ │                      │ │   5      println!("{x}");   │ │
│ │                      │ │ ▎ 6      do_thing();        │ │
│ │                      │ │   7  }                      │ │
│ │                      │ │                              │ │
│ └──────────────────────┘ └──────────────────────────────┘ │
│ q: quit  Enter: open  Esc: back  d: toggle diff          │
└──────────────────────────────────────────────────────────┘

Legend:
  ▌ = selected item in file list
  ▎ = gutter mark for changed line (colored bar)
  M/A/D = Modified/Added/Deleted badge
```

File list panel:
- Relative file paths
- Change kind badge (M/A/D/R) with color coding
- Selected item highlighted
- Sorted by mtime (most recent first)
- Scrollable if many files

File viewer panel:
- Line numbers in left gutter
- Change markers (colored bar) next to changed lines — added=green, modified=yellow, deleted=red marker between lines
- Full tree-sitter syntax highlighting
- Scrollable

### Deleted file peek

When a deleted file is selected:
- The file viewer shows a floating overlay (popup) with the old file content from HEAD
- Tree-sitter highlighted
- Dimmed or marked to indicate "this is the deleted version"
- Press `Esc` to dismiss

### Diff toggle

When viewing a file, press `d` to toggle diff mode:
- Changed lines expand to show the full hunk (old + new lines, unified diff style)
- Press `d` again to collapse back to the annotated file view
- This is a per-file toggle, not global

### Rendering rules (from NLM research)

- **No heap allocations in render methods.** Pre-compute all display strings in state/update, not in render.
- **Event queue draining.** Poll with 16ms timeout, then drain all pending events with `Duration::ZERO` poll. (Already implemented in current code.)
- **Pure view functions.** Render methods must not mutate state — only read.

### Module layout

```
crates/view/src/
  lib.rs                  re-exports App
  app.rs                  App struct, top-level event routing, layout
  action.rs               Action enum
  theme.rs                color scheme, style definitions
  components.rs           re-exports components
  components/
    file_list.rs          FileList component
    file_viewer.rs        FileViewer component (highlighting + gutter)
    diff_overlay.rs       Diff toggle rendering
    peek_overlay.rs       Deleted file peek popup
    status_bar.rs         Bottom status bar with keybindings
```

### Dependencies

```toml
[dependencies]
codepeek-core.workspace = true
ratatui.workspace = true
crossterm.workspace = true
```

---

## TUI app (`apps/tui`)

### Responsibility

Thin binary. Initialize terminal, create concrete implementations, wire them into the view, run the event loop, restore terminal on exit.

### Implementation

```rust
fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let repo_path = std::env::current_dir()?;
    let detector = GitChangeDetector::open(&repo_path)?;
    let highlighter = TreeSitterHighlighter::new();

    let terminal = ratatui::init();
    let result = App::new(
        Box::new(detector),
        Box::new(highlighter),
    ).run(terminal);
    ratatui::restore();

    result?;
    Ok(())
}
```

That's it. All logic lives in the crates.

### Dependencies

```toml
[dependencies]
codepeek-core.workspace = true
codepeek-view.workspace = true
codepeek-git.workspace = true
codepeek-syntax.workspace = true
color-eyre.workspace = true
ratatui.workspace = true
```

---

## Workspace dependencies (root Cargo.toml additions)

```toml
[workspace.dependencies]
# Internal crates
codepeek-core = { path = "crates/core" }
codepeek-view = { path = "crates/view" }
codepeek-git = { path = "crates/git" }
codepeek-syntax = { path = "crates/syntax" }

# TUI
ratatui = "0.30.0"
crossterm = "0.29.0"     # verify — ratatui 0.30 may re-export

# Git
git2 = "0.20.4"

# Syntax highlighting
tree-sitter = "0.26.8"
tree-sitter-highlight = "0.25.3"
tree-sitter-language-pack = "0.6.0"

# Error handling
color-eyre = "0.6.5"
thiserror = "2.0.12"

# NOTE: verify all versions are latest at implementation time.
# These are known-good as of 2026-04-04.
```

---

## Implementation milestones

Each milestone produces a working, runnable state. You can `just check` after every milestone and `just run` from milestone 4 onward. Milestones are designed so each one builds visibly on the last — you'll see progress every step.

---

### Milestone 1: Crate scaffolding

**You'll see:** All new crates exist and compile. No new functionality yet.

**What to build:**
1. Create `crates/git/Cargo.toml` and `crates/git/src/lib.rs` (empty, just a comment)
2. Create `crates/syntax/Cargo.toml` and `crates/syntax/src/lib.rs` (empty)
3. Add `crates/git` and `crates/syntax` to `[workspace.members]` in root `Cargo.toml`
4. Add all new workspace dependencies (git2, tree-sitter, thiserror, etc.) to root `Cargo.toml`
5. Wire dependency references in each crate's `Cargo.toml` using `.workspace = true`

**Verify:** `just check` passes. All 5 workspace members compile.

**Crates touched:** root `Cargo.toml`, `crates/git`, `crates/syntax`

---

### Milestone 2: Core domain types and traits

**You'll see:** Core crate has all the types the rest of the system will use. Tests prove they work.

**What to build:**
1. `crates/core/src/change.rs` — `FileChange`, `ChangeKind`
2. `crates/core/src/diff.rs` — `DiffHunk`, `DiffLine`, `LineChange`, `ChangeMap` + `ChangeMap::from_hunks()` constructor
3. `crates/core/src/highlight.rs` — `HighlightSpan`, `HighlightedLine`, `HighlightKind`
4. `crates/core/src/traits.rs` — `ChangeDetector`, `SyntaxHighlighter` trait definitions
5. `crates/core/src/error.rs` — `ChangeError`, `SyntaxError` (using `thiserror`)
6. `crates/core/src/lib.rs` — re-export everything
7. Unit tests: `ChangeMap::from_hunks()` correctly maps hunks to line sets, `ChangeKind` display, error formatting

**Verify:** `just check` passes. `just test` runs core tests.

**Crates touched:** `crates/core`

---

### Milestone 3: Git change detection

**You'll see:** The git crate can list uncommitted changed files from a real repo, sorted by mtime.

**What to build:**
1. `crates/git/src/detector.rs` — `GitChangeDetector` struct, `open()` constructor
2. Implement `ChangeDetector::detect_changes()`:
   - Call `repo.statuses(None)` to get all changed files
   - Map `git2::Status` flags → `ChangeKind`
   - Stat each file for filesystem mtime
   - Return sorted by mtime descending
3. `crates/git/src/lib.rs` — re-export `GitChangeDetector`
4. Integration test: init a temp git repo, create/modify/delete files, verify `detect_changes()` output

**Verify:** `just check` passes. `just test` runs git integration tests. You can also verify manually:
```rust
// Quick manual check (not shipped, just for dev confidence)
let detector = GitChangeDetector::open(&PathBuf::from(".")).unwrap();
let changes = detector.detect_changes().unwrap();
for c in &changes { println!("{:?} {:?}", c.kind, c.path); }
```

**Crates touched:** `crates/git`

---

### Milestone 4: File list in TUI

**You'll see:** Launch codepeek, see a full-screen list of changed files. Navigate with arrow keys. `q` quits.

**What to build:**
1. `crates/syntax/src/lib.rs` — stub `NoopHighlighter` that returns unhighlighted lines (implements `SyntaxHighlighter`). Just enough so the app compiles.
2. `crates/view/src/action.rs` — `Action` enum (Quit, SelectFile, ScrollUp, ScrollDown, Noop for now)
3. `crates/view/src/theme.rs` — basic color constants (change kind colors, selection highlight, borders)
4. `crates/view/src/components/file_list.rs` — `FileList` component:
   - State: `files: Vec<FileChange>`, `selected: usize`, `scroll_offset: usize`
   - Render: show relative paths with change kind badge (M/A/D/R), colored
   - Events: Up/Down arrows move selection, handle scroll when list overflows
   - Returns `Action::SelectFile(idx)` on Enter
5. `crates/view/src/components/status_bar.rs` — `StatusBar` component: renders keybinding hints at the bottom
6. `crates/view/src/components.rs` — module declarations, re-exports
7. Refactor `crates/view/src/app.rs`:
   - `App::new()` accepts `Box<dyn ChangeDetector>` + `Box<dyn SyntaxHighlighter>`
   - On init: call `detect_changes()`, populate `FileList`
   - Route keyboard events to `FileList`
   - Layout: `FileList` fills the screen, `StatusBar` at the bottom
8. Update `crates/view/src/lib.rs` — re-exports
9. Update `apps/tui/src/main.rs` — create `GitChangeDetector`, `NoopHighlighter`, pass to `App`

**Verify:** `just run` in the codepeek repo (make some uncommitted changes first). You see a list of files, sorted by last edit. Arrow keys move the selection. `q` quits cleanly.

**Crates touched:** `crates/syntax` (stub), `crates/view` (major), `apps/tui`

---

### Milestone 5: Two-panel layout + raw file content

**You'll see:** Press Enter on a file, see its raw content (no highlighting) in the right panel. Esc goes back.

**What to build:**
1. `crates/view/src/components/file_viewer.rs` — `FileViewer` component:
   - State: `lines: Vec<String>`, `scroll_offset: usize`, `file_path: Option<PathBuf>`
   - Render: line numbers in left gutter, raw text content, scrollable
   - Events: Up/Down/PageUp/PageDown scroll, Esc returns `Action::Back`
2. Add `Focus` enum to `app.rs` (`FileList` | `FileViewer`)
3. Update `App` layout: split horizontal — file list (30% left), file viewer (70% right)
4. Update `App` event routing:
   - When focus is `FileList` and `Action::SelectFile(idx)` fires: read file content from disk, populate `FileViewer`, switch focus
   - When focus is `FileViewer` and `Action::Back` fires: switch focus back to `FileList`
   - Route keyboard events to whichever component has focus
5. Update `StatusBar` to show context-appropriate keybindings based on current focus
6. File list stays visible and shows the selected file highlighted even when focus is on viewer

**Verify:** `just run`. Navigate to a file, press Enter. Right panel shows the file content with line numbers. Scroll through it. Press Esc, you're back on the file list. The selected file is still highlighted.

**Crates touched:** `crates/view`

---

### Milestone 6: Tree-sitter syntax highlighting

**You'll see:** File content now has full syntax colors. This is the moment it starts looking like a real tool.

**What to build:**
1. `crates/syntax/src/languages.rs` — file extension → tree-sitter language name mapping
2. `crates/syntax/src/mapping.rs` — tree-sitter capture name → `HighlightKind` mapping
3. `crates/syntax/src/highlighter.rs` — `TreeSitterHighlighter`:
   - Lazy-load `HighlightConfiguration` per language from `tree-sitter-language-pack`
   - `highlight()` runs the tree-sitter highlight pipeline
   - Walk `HighlightEvent` iterator, split by newlines, produce `Vec<HighlightedLine>`
   - Cache configurations (not results) for reuse across files
4. Replace `NoopHighlighter` with `TreeSitterHighlighter` in `apps/tui/src/main.rs`
5. Update `FileViewer` to accept `Vec<HighlightedLine>` instead of `Vec<String>`
6. Update `FileViewer` render: for each line, iterate spans and apply `Style` from `theme.rs` based on `HighlightKind`
7. Extend `theme.rs` with highlight kind → ratatui `Style` color mapping (keyword=purple, string=green, comment=gray, etc.)
8. Update `App`: when a file is selected, call `highlighter.highlight()` and pass results to `FileViewer`

**Verify:** `just run`. Open a `.rs` file — keywords are purple, strings are green, comments are gray. Open a `.toml` file — keys and values are colored. Open an unknown extension — falls back to plain text (no crash).

**Crates touched:** `crates/syntax` (major), `crates/view`, `apps/tui`

---

### Milestone 7: Gutter change marks

**You'll see:** Changed lines have colored bars in the gutter. At a glance you can see what was touched.

**What to build:**
1. Implement `ChangeDetector::compute_diff()` in `crates/git/src/detector.rs`:
   - Use `diff_tree_to_workdir_with_index()` for combined staged+unstaged diff
   - Filter to the requested file path
   - Walk hunks and lines, map to `DiffHunk`/`DiffLine`
2. Integration test: create a temp repo, make known changes, verify diff output matches expected hunks
3. Update `FileViewer` state: add `change_map: ChangeMap`
4. Update `App`: when file is selected, also call `compute_diff()`, build `ChangeMap::from_hunks()`, pass to `FileViewer`
5. Update `FileViewer` render:
   - Between line number and content, render a thin gutter column (1-2 chars wide)
   - If line number is in `change_map.added` → green `▎`
   - If line number is in `change_map.modified` → yellow `▎`
   - If line number is in `change_map.deleted` → red `▁` (between-line marker)
   - Unchanged lines get an empty gutter
6. For newly added files (ChangeKind::Added): mark all lines as added (entire file is new)

**Verify:** `just run`. Make some changes to a file (add lines, modify lines, delete lines). Open it in codepeek. Green bars appear next to new lines, yellow next to modified lines, red markers where lines were deleted. Open a brand new file — entire gutter is green.

**Crates touched:** `crates/git`, `crates/view`

---

### Milestone 8: Diff toggle

**You'll see:** Press `d` while viewing a file, and changed sections expand to show the actual diff (old vs new lines). Press `d` again to collapse.

**What to build:**
1. Add `DiffHunk` data to `FileViewer` state (already have it from milestone 7, just store it)
2. Add `show_diff: bool` toggle to `FileViewer`
3. Add `Action::ToggleDiff` triggered by `d` key
4. `crates/view/src/components/diff_overlay.rs` — diff rendering logic:
   - When `show_diff` is true, render changed regions differently:
   - For each hunk, show removed lines (red background, prefixed with `-`) and added lines (green background, prefixed with `+`)
   - Unchanged lines render normally
   - When `show_diff` is false, render the normal annotated view (gutter marks only)
5. Update `FileViewer` render to delegate to diff overlay when toggled
6. Update `StatusBar` to show `d: diff` hint when viewing a file

**Verify:** `just run`. Open a modified file. See gutter marks. Press `d`. Changed sections expand to show the full diff with red/green lines. Press `d` again. Back to the clean annotated view.

**Crates touched:** `crates/view`

---

### Milestone 9: Deleted file peek

**You'll see:** Deleted files appear in the list. Select one, see a floating popup with the old file content from HEAD.

**What to build:**
1. Implement `ChangeDetector::read_at_head()` in `crates/git/src/detector.rs`:
   - Get HEAD tree → find blob at path → return content as String
2. Integration test: delete a file in temp repo, verify `read_at_head()` returns the old content
3. `crates/view/src/components/peek_overlay.rs` — `PeekOverlay` component:
   - Floating centered popup (70% width, 80% height) with border
   - Title: "Deleted: path/to/file" with dimmed style
   - Content: old file content with tree-sitter highlighting
   - Scrollable
   - Esc dismisses
4. Update `App`:
   - When `SelectFile` fires on a deleted file → call `read_at_head()`, highlight content, show `PeekOverlay`
   - Add `Action::PeekDeleted` and `Action::DismissPeek`
   - When peek is visible, route events to `PeekOverlay`
5. Visual treatment: deleted files in file list get a strikethrough or dimmed style + `D` badge

**Verify:** `just run`. Delete a tracked file (`git rm` or just `rm`). It appears in the file list with a `D` badge. Press Enter. A floating popup shows the old file content, syntax highlighted. Scroll through it. Press Esc. Back to the file list.

**Crates touched:** `crates/git`, `crates/view`

---

### Milestone 10: Polish and edge cases

**You'll see:** Everything works smoothly. Edge cases handled. Ready for daily use.

**What to build:**
1. **Renamed files:** show both old and new paths in file list (e.g., `old.rs → new.rs`), open new path in viewer
2. **Binary files:** detect binary content, show "Binary file, N bytes" placeholder instead of trying to render/highlight
3. **Empty files:** handle gracefully (show empty viewer, no crash)
4. **Very long lines:** truncate or horizontal scroll (decide at impl time)
5. **Large files:** if performance is an issue, only highlight the visible region + 50-line buffer above/below
6. **Consistent theme:** audit all colors across components, ensure they work on both dark and light terminals
7. **Error handling:** if git operations fail mid-use (e.g., repo disappears), show error message instead of crashing
8. **Refresh:** consider re-scanning for changes periodically or on a keybinding (`r` to refresh)
9. Run `just check` — all fmt, lint, test gates pass cleanly

**Verify:** Use codepeek on real projects for a day. Try edge cases: binary files, empty files, huge files, repos with hundreds of changes, renamed files. Everything works or fails gracefully.

**Crates touched:** `crates/view`, `crates/git`, `apps/tui`

---

### Milestone summary

| # | Name | What you'll see | Key crates |
|---|------|----------------|------------|
| 1 | Crate scaffolding | Everything compiles | root, git, syntax |
| 2 | Core domain types | Types + tests exist | core |
| 3 | Git change detection | File list from real repo | git |
| 4 | File list in TUI | Navigate changed files | view, tui |
| 5 | Two-panel layout | Open file, see raw content | view |
| 6 | Tree-sitter highlighting | Syntax colors | syntax, view |
| 7 | Gutter change marks | Green/yellow/red bars | git, view |
| 8 | Diff toggle | `d` expands inline diff | view |
| 9 | Deleted file peek | Floating popup with old content | git, view |
| 10 | Polish | Edge cases, daily-driver ready | all |

**First runnable TUI:** Milestone 4
**First "this looks real":** Milestone 6
**Feature-complete v1:** Milestone 9
**Ship-ready:** Milestone 10

---

## Open decisions (to resolve during implementation)

1. **`crossterm` version:** ratatui 0.30 may re-export crossterm or use it internally. Check if view needs an explicit crossterm dependency or gets it transitively.

2. **`tree-sitter-language-pack` vs individual grammar crates:** The language pack is convenient but may be heavy. If binary size is a concern, switch to individual grammar crates for the most common languages (rust, js, ts, python, go, c, cpp, java, toml, yaml, json, markdown).

3. **Visible-region highlighting optimization:** For large files, highlighting the entire file is wasteful. Phase 2 can start with full-file highlighting, but if performance is an issue, switch to visible-region-only highlighting with a buffer zone. This is a performance optimization, not an architecture change.

4. **Theme:** Need to define a color mapping from `HighlightKind` to ratatui `Style`. Start with a sensible default (dark terminal theme). Can be made configurable later.

5. **`ChangeDetector` mutability:** `tree-sitter-highlight`'s `Highlighter::highlight()` takes `&mut self`. The `SyntaxHighlighter` trait currently takes `&self`. We may need interior mutability (`RefCell` or `Mutex`) or change the trait signature. Resolve at implementation time.
