# Multi-View Homepage

> **Revision history:** v1 drafted 2026-04-07. v2 revised 2026-04-08 after a deep review against the rust-style-guide notebook (see Architectural decisions section for what changed and why).

## Summary

Codepeek today is a single-purpose change viewer: launch → land on the file list → open a file → see it. There is no real homepage, no concept of "another view", and no scaffolding for the broader vision (sessions, search, tags, branches, todos).

This plan introduces a **multi-view architecture** for codepeek with:

- An **activity-feed Home view** as the new landing screen — selectable entries that contextually open the right view (a session entry opens Sessions, a commit opens Branches, a tag jumps to file:line).
- A **`View` enum** (closed-set static dispatch) so the `App` can host any number of screens without dynamic dispatch overhead, while preserving exhaustive-match safety.
- **Single-letter cross-view navigation** (`h`/`c`/`s`/`/`/`t`/`b`/`T`) plus a **command palette overlay** (`:` or `Ctrl+P`) for everything else.
- Six new top-level views beyond the existing Changes view: **Home, Sessions, Search, Tags, Branches/Log, TODO/FIXME inbox**.
- **Non-blocking background data loading** via `std::sync::mpsc` channels and `std::thread::spawn`. Every view that does I/O ships in a `Loading` → `Ready` lifecycle and never blocks the render loop.

The existing zen-mode aesthetic (single-view, centered, transparent root) is preserved — every new view is its own zen experience, no split panels, no tab strip chrome.

### Core loop after this change

1. Launch codepeek
2. Land on **Home** — activity feed showing recent edits, sessions, commits, tags, todos with stats in the title
3. `j/k` through the activity feed; `Enter` opens whatever the entry points to
4. From any view: `h` Home, `c` Changes, `s` Sessions, `/` Search, `t` Tags, `b` Branches, `T` Todos, `:` Command palette, `Esc` back, `q` quit
5. Slow data sources (sessions, todos, search, branches) load in a background worker — the spinner animates while data streams in, the UI never freezes

### Out of scope for this plan

- **Launching new Claude Code sessions** (e.g. spawning zellij panes with `claude --resume`). Sessions view is read-only listing for v1. Spawning is its own future plan.
- **Editor jumping** (helix/neovim launching). Belongs in a future plan.
- **AI cost / token tracker** — distinct enough to deserve its own plan.
- **MCP servers status view** — distinct enough to deserve its own plan.
- **LSP diagnostics view** — needs an LSP client, way too heavy for this plan.
- **Pinned files** — punted; revisit after Tags ships and we know whether the distinction is needed.
- **`tokio` / async runtime.** Background work uses `std::thread::spawn` + `std::sync::mpsc`, not async/await. Reasoning: the render loop is single-threaded by nature and the I/O patterns we need (one-shot scans, JSONL reads, git2 calls) don't benefit from a runtime. Adding tokio is its own architectural decision.
- **Theme/colorscheme switching, custom keybindings UI, settings view.** Not in this plan.

---

## Architecture

### Crate structure (additions)

```
crates/
  core/         (extended)  new types (Tag, Todo, SessionInfo, CommitInfo,
                            BranchInfo, ActivityEntry), newtype IDs (TagId,
                            SessionId, CommitSha), input structs (NewTag),
                            new traits (SessionStore, TagStore, TodoScanner,
                            FileSearcher, CommitLog)
  git/          (extended)  CommitLog impl on GitChangeDetector
  syntax/       (unchanged)
  view/         (heavy ext) View enum, Router, ViewId, views.rs + views/,
                            background loading infrastructure (LoadState<T>),
                            CommandPalette overlay, hierarchical Action enum,
                            App refactor
  sessions/     (NEW)       Claude Code session discovery & metadata
  search/       (NEW)       File-name and TODO scanning over a workdir
  store/        (NEW)       JSON-backed persistent storage (tags)
apps/
  tui/          (extended)  wires the new crates, builds the Router, owns
                            background worker spawning
```

### Dependency graph

```
apps/tui ──> codepeek-view ──> codepeek-core
         ├─> codepeek-git ────> codepeek-core
         ├─> codepeek-syntax ─> codepeek-core
         ├─> codepeek-sessions > codepeek-core
         ├─> codepeek-search ─> codepeek-core
         └─> codepeek-store ──> codepeek-core
```

Key constraint (unchanged from change-viewer plan): **`codepeek-view` depends only on `codepeek-core`, never on `git` / `syntax` / `sessions` / `search` / `store` directly.** All implementations are injected through traits defined in `core`. This keeps the view layer testable with stub data and prevents circular complexity.

### Why three new crates instead of one or zero

This decision was challenged in the v2 review and explicitly validated by the rust-style-guide notebook against *Rust Design Patterns* §3.3.2 "Prefer Small Crates" and the Ratatui workspace's own `ratatui-core` / `ratatui-widgets` / backend split.

Each new crate has a single, distinct external dependency surface:

| Crate | Concern | External deps |
|-------|---------|---------------|
| `codepeek-sessions` | Read Claude Code session JSONLs | `serde_json`, `dirs` |
| `codepeek-search` | File-name and content scanning | `ignore`, `regex` |
| `codepeek-store` | Atomic JSON-backed persistent storage | `serde_json`, `dirs`, `tempfile` |

Folding them into a `codepeek-data` junk-drawer would erase the dependency-isolation and parallel-compilation benefits. Folding them into the binary would block the view layer from depending on their traits via core. Three crates matches the existing pattern (`core` / `git` / `syntax` / `view`).

We do **not** add a new crate for branches/log. That's git-domain — it extends `codepeek-git`.

### Single-threaded design — `Rc` over `Arc`, no `Send + Sync` bounds

Codepeek runs a single render+event loop on the main thread. The only multi-threaded code introduced by this plan is **short-lived background worker threads** (see "Background data loading" below). Those workers don't share trait objects with the main thread — they hand back owned `T` values via `mpsc::channel`.

Therefore:

- **Shared store ownership uses `Rc<dyn Trait>`, not `Arc<dyn Trait>`.** Multiple views can hold a clone of the same `Rc<dyn TagStore>` without atomic refcount overhead. (`Box<dyn Trait>` is wrong here because exclusive ownership prevents sharing.)
- **Interior mutability uses `Rc<RefCell<dyn Trait>>`, not `Arc<Mutex<dyn Trait>>`.** Specifically: `SyntaxHighlighter` takes `&mut self` (tree-sitter requires it) and is held as `Rc<RefCell<dyn SyntaxHighlighter>>`.
- **None of the new traits have `: Send + Sync` bounds.** `TagStore`, `SessionStore`, `TodoScanner`, `FileSearcher`, `CommitLog` are all `pub trait Foo { … }` — no thread-safety constraint.
- **Worker threads consume owned values, not trait objects.** When SessionsView spawns a background scan, the worker is a `move` closure that owns a fresh `ClaudeSessionStore` value (cheap to construct), not a clone of the view's `Rc`. This sidesteps `Send` requirements entirely.

There's one wrinkle: the existing `ChangeDetector` and `SyntaxHighlighter` traits in `codepeek-core` are currently declared `Send + Sync`. Those bounds were added speculatively in the original change-viewer plan. They're harmless today (the `git2::Repository` inside `GitChangeDetector` is wrapped in a `Mutex` to satisfy `Sync`) but they don't pull their weight. **This plan removes those bounds in M2** and replaces the `Mutex` inside `GitChangeDetector` with a `RefCell`. If a future plan needs to ship work to a worker thread, it can construct a fresh detector inside the worker the same way SessionsView does.

The notebook is explicit on this: Clippy lints `arc_with_non_send_sync`, `rc_mutex`, and `mutex_atomic` all flag the multi-threaded-primitive-in-single-threaded-code anti-pattern. We'd be tripping all three with the v1 design.

### Static dispatch for views — `enum View`, not `Box<dyn View>`

The set of top-level views is **closed and known at compile time**: Home, Changes, Sessions, Search, Tags, Branches, Todos, FileViewer (the contextual file-display view). New views require a code change. There's no plugin system, no runtime registration.

For closed sets, the idiomatic Rust choice is an enum, not a trait object:

```rust
pub enum View {
    Home(HomeView),
    Changes(ChangesView),
    Sessions(SessionsView),
    Search(SearchView),
    Tags(TagsView),
    Branches(BranchesView),
    Todos(TodosView),
    FileViewer(FileViewerView),
}
```

Benefits over `Box<dyn View>`:

- **Exhaustive matches.** The compiler forces every match on `View` to handle every variant — when we add a future `Pins` view, the compiler tells us exactly which match arms need updating. (Aligns with the Rust style guide's "Prefer exhaustive matches" rule.)
- **No heap allocation on navigation.** Each view enum carries its concrete type inline; navigation is `self.current = View::Sessions(SessionsView::new(…))`, no `Box::new`.
- **Compiler inlining.** `match` over an enum lets the compiler inline render and update logic per-variant. Trait object dispatch goes through a vtable.
- **No object-safety constraints.** We can have associated constants, generic methods, etc. on individual View variants without worrying about whether the trait is dyn-compatible.

The cost is that the enum file lists every variant (small) and every method delegates via `match` (boilerplate). For seven variants and seven methods this is ~50 lines of plumbing in `views.rs`. The `enum_dispatch` crate exists to generate this automatically; we don't use it for v1 because the manual `match` is clear and adds no dependency.

The notebook noted that `Box<dyn View>` is *acceptable* for ~6 top-level views — vtable lookups are nanoseconds and the Component Architecture is friendly to dynamic dispatch — so this is a "more idiomatic" call, not a "fixing a bug" call.

**In contrast:** the injected stores (`TagStore`, `SessionStore`, etc.) are still `Rc<dyn Trait>`. The store set really is open — a test stub, a JSON impl, a hypothetical future SQLite impl all need to coexist. Trait objects are right for that side of the boundary.

### Background data loading — `mpsc` channels and worker threads

Walking 100+ session JSONL files, scanning a workdir for TODO comments, or asking `git2` for the recent commit log can each take 100–500ms on a real repo. The render loop has a 16ms budget per frame (for 60fps); blocking on these calls inside `View::on_enter` would freeze the UI.

**The pattern:** every view that does I/O has a `LoadState<T>` field rather than holding `T` directly:

```rust
// crates/view/src/loading.rs
pub enum LoadState<T> {
    /// View was just constructed; no load attempted yet.
    Idle,
    /// Worker thread is fetching data; receiver waits for the result.
    Loading {
        rx: mpsc::Receiver<Result<T, String>>,
        spinner_tick: u8,
    },
    /// Data is loaded and ready to render.
    Ready(T),
    /// Worker failed; the error string is shown via ErrorBar.
    Failed(String),
}
```

**Lifecycle:**

1. `View::on_enter` is called when the user navigates to the view. It transitions `LoadState::Idle` → `LoadState::Loading { rx }` and `std::thread::spawn`s a worker that owns whatever it needs (a fresh store value, or a `Sender` clone). The worker calls the slow API and sends back a `Result<T, String>` over the channel.
2. The main loop calls `View::poll_loading(&mut self)` once per tick (between `draw` and `handle_events`). `poll_loading` does a non-blocking `rx.try_recv()`. If a result has arrived, transition to `Ready(t)` or `Failed(msg)`. If still pending, increment `spinner_tick` so the spinner animates.
3. `View::render` checks `LoadState`:
   - `Idle` → render an empty placeholder ("Press `r` to load" or auto-trigger from `on_enter`)
   - `Loading` → render a centered spinner with view title
   - `Ready(t)` → render normally using `t`
   - `Failed(msg)` → render via `ErrorBar`

**Why `std::thread::spawn` and not `tokio::spawn`?**

- No new dependency, no runtime to set up
- The work is one-shot per view enter, not a long-lived async task
- The worker only needs to send one message back; we don't need futures composition
- `std::sync::mpsc::Receiver::try_recv` is exactly the non-blocking poll we need
- Adding `tokio` would force every other crate's traits into `async fn` and balloon the plan

**Worker code shape (illustrative):**

```rust
// inside SessionsView::on_enter
let (tx, rx) = mpsc::channel();
let repo = self.repo_root.clone();
std::thread::spawn(move || {
    let result = ClaudeSessionStore::discover()
        .and_then(|store| store.list_sessions(&repo))
        .map_err(|e| e.to_string());
    let _ = tx.send(result);  // ignore SendError if view was dropped
});
self.state = LoadState::Loading { rx, spinner_tick: 0 };
```

The worker constructs a fresh `ClaudeSessionStore` rather than cloning the view's `Rc<dyn SessionStore>` — this avoids needing `Send` on the trait. For each view, the constructor is cheap (just resolves a path and returns a struct).

**App-level glue:**

```rust
// in App::run
while !self.should_quit {
    self.current.poll_loading();           // drain background results
    terminal.draw(|frame| self.render(frame))?;
    self.handle_events()?;                  // poll(16ms) for keys
}
```

The 16ms event poll naturally limits us to ~60fps. Spinner animation is driven by `spinner_tick`, which increments every time `poll_loading` runs (on every tick). The `Spinner::frame()` helper divides this raw tick count by `TICKS_PER_FRAME` (6) to produce a ~10fps animation — see "Spinner rendering" below. No sleeps, no separate timer thread.

**Trade-offs we're accepting:**

- **No streaming results.** A worker either returns the whole `Vec<SessionInfo>` or fails. Partial results during a slow scan would require a more complex channel protocol — out of scope.
- **No cancellation.** If the user navigates away while a worker is still running, the worker finishes and drops its message into a channel that nobody is reading. Worker is short-lived, so the wasted CPU is a few hundred ms in the worst case. Cancellation is its own future plan.
- **One worker per view at a time.** If the user mashes `r` to refresh, we don't queue extra workers — we either ignore the refresh while a load is in flight, or replace the receiver. Decision: ignore extras (return early in `on_enter` if `LoadState::Loading`).

---

## Core crate extensions (`codepeek-core`)

### Newtype IDs

Per the *Rust Design Patterns* book §3.1.3, primitive ID fields are wrapped in newtypes for compile-time type safety. Zero runtime overhead — the compiler optimizes away the wrapper.

```rust
// id.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TagId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CommitSha(pub String);

impl CommitSha {
    /// First 7 chars for display.
    pub fn short(&self) -> &str {
        let len = self.0.len().min(7);
        &self.0[..len]
    }
}
```

**Note on `serde` in core:** the core crate already takes the `serde` workspace dep transitively (the existing `apps/tui/src/config.rs` uses it via `serde::Deserialize`). Core itself currently has no `serde` dep. **In M2 we add `serde = { workspace = true, features = ["derive"] }` to `crates/core/Cargo.toml`.** This is a small concession — core no longer has zero deps — but it's the right call because every persisted/transmitted ID needs to round-trip JSON. The alternative (newtypes in a separate crate) is over-engineered.

### New domain types

```rust
// activity.rs — cross-source recent activity
pub enum ActivityKind {
    FileEdit,
    Commit,
    Session,
    Tag,
    Todo,
}

pub struct ActivityEntry {
    pub kind: ActivityKind,
    pub when: SystemTime,
    pub label: String,            // "edited file_viewer.rs", "Claude session: refactor"
    pub target: ActivityTarget,
}

pub enum ActivityTarget {
    File { path: PathBuf },
    Commit { sha: CommitSha },
    Session { id: SessionId },
    Tag { id: TagId },
    Todo { path: PathBuf, line: u32 },
    None,
}
```

```rust
// session.rs
pub struct SessionInfo {
    pub id: SessionId,
    pub started_at: SystemTime,
    pub last_active: SystemTime,
    pub message_count: usize,
    pub cwd: PathBuf,
    pub summary: Option<String>,
}
```

```rust
// tag.rs
pub struct Tag {
    pub id: TagId,
    pub created_at: SystemTime,
    pub path: PathBuf,
    pub line: u32,
    pub kind: TagKind,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TagKind { Issue, Fix }

/// Input shape for `TagStore::add_tag`. Grouped into a struct so the trait
/// signature is stable as Tag fields evolve (priority, color, etc.).
pub struct NewTag<'a> {
    pub path: &'a Path,
    pub line: u32,
    pub kind: TagKind,
    pub note: &'a str,
}
```

```rust
// todo.rs
pub struct TodoItem {
    pub path: PathBuf,
    pub line: u32,
    pub kind: TodoKind,
    pub text: String,
}

pub enum TodoKind { Todo, Fixme, Hack, Xxx }
```

```rust
// commit.rs
pub struct CommitInfo {
    pub sha: CommitSha,            // newtype
    pub author: String,
    pub when: SystemTime,
    pub summary: String,
}

pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub upstream: Option<String>,
    pub head_sha: CommitSha,
}
```

All types get `#[non_exhaustive]` (matching the existing theme structs) so adding fields later isn't a breaking change.

### New traits — no `Send + Sync`

```rust
// traits.rs (additions)

pub trait SessionStore {
    fn list_sessions(&self, repo: &Path) -> Result<Vec<SessionInfo>, SessionError>;
}

pub trait TagStore {
    fn list_tags(&self) -> Result<Vec<Tag>, StoreError>;
    fn add_tag(&self, new: NewTag<'_>) -> Result<Tag, StoreError>;
    fn remove_tag(&self, id: TagId) -> Result<(), StoreError>;
}

pub trait TodoScanner {
    fn scan(&self, root: &Path) -> Result<Vec<TodoItem>, SearchError>;
}

pub trait FileSearcher {
    /// Fuzzy-match against tracked file paths under `root`.
    fn find_files(&self, root: &Path, query: &str, limit: usize)
        -> Result<Vec<PathBuf>, SearchError>;
}

pub trait CommitLog {
    fn recent_commits(&self, limit: usize) -> Result<Vec<CommitInfo>, ChangeError>;
    fn list_branches(&self) -> Result<Vec<BranchInfo>, ChangeError>;
    fn read_at_commit(&self, sha: &CommitSha, path: &Path) -> Result<String, ChangeError>;
}
```

`add_tag` takes `NewTag<'_>` (a borrowed input struct) instead of four positional arguments. Future fields (priority, assignee, color) are added to `NewTag` without breaking the trait signature.

`list_tags` returns `Vec<Tag>` (not `impl Iterator`). The notebook flagged that iterator returns across trait boundaries entangle lifetimes with backing resources — `Vec` is the right call for a clean trait API.

### Removing `Send + Sync` from existing traits

In M2 (after adding the new types but before any view work) we update `crates/core/src/traits.rs`:

```rust
// Before
pub trait ChangeDetector: Send + Sync { … }
pub trait SyntaxHighlighter: Send + Sync { … }

// After
pub trait ChangeDetector { … }
pub trait SyntaxHighlighter { … }
```

And in `crates/git/src/detector.rs`:

```rust
// Before
pub struct GitChangeDetector { repo: Mutex<Repository> }

// After
pub struct GitChangeDetector { repo: RefCell<Repository> }
```

The `repo` field becomes `RefCell<Repository>` because `git2::Repository` is `!Sync` but we still need interior mutability for `compute_diff` (it wants `&mut diff_opts`). All existing call sites of `repo.lock()` become `repo.borrow()` / `repo.borrow_mut()`.

This is a small, mechanical change but it touches the existing tests too. **It's gated behind M2 of the new plan** rather than left as a "follow-up" so we don't ship the cleaner architecture sitting on top of legacy speculative bounds.

### New error types

```rust
// error.rs (additions, all using thiserror)
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session directory not found at {path}")]
    NotFound { path: PathBuf },
    #[error("failed to read session file {path}")]
    ReadFailed { path: PathBuf, #[source] source: std::io::Error },
    #[error("failed to parse session jsonl: {message}")]
    ParseFailed { message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("store path resolution failed")]
    NoConfigDir,
    #[error("failed to read store at {path}")]
    ReadFailed { path: PathBuf, #[source] source: std::io::Error },
    #[error("failed to write store at {path}")]
    WriteFailed { path: PathBuf, #[source] source: std::io::Error },
    #[error("store data is corrupt: {message}")]
    Corrupt { message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("search root does not exist: {path}")]
    RootMissing { path: PathBuf },
    #[error("search failed: {message}")]
    Failed { message: String },
}
```

The notebook validated this approach — per-domain `thiserror` enums with descriptive names (not `Error`), implementing `std::error::Error` + `Debug` + `Display` + `source()` + `From` conversions. Avoids the "ball of mud" anti-pattern. Clippy's `error_impl_error` lint is happy because none of them are named `Error`.

**Use `#[from]` on inner-error fields wherever the `?` operator should auto-convert.** Each `#[source]` field above represents a foreign error type bubbling up; if call sites would benefit from writing `tag_store.add_tag(new)?` instead of `.map_err(StoreError::WriteFailed)?`, replace `#[source]` with `#[from]` on that variant. Standard thiserror pattern, but worth being explicit so implementers don't forget. Two caveats:
- A variant can have at most one `#[from]` field (otherwise the auto-derive can't pick which conversion to generate).
- Variants that wrap the same source type (e.g. two `std::io::Error` variants distinguished by context) cannot both use `#[from]` — one of them must stay `#[source]` with explicit `.map_err()` at the call site.

**Existing `Box<dyn Error + Send + Sync>` patterns inside error variants are kept as-is.** The bound is free here because every wrapped type (`std::io::Error`, `git2::Error`, `serde_json::Error`) is already `Send + Sync`, the bound matches Rust ecosystem convention, and it future-proofs error propagation through `mpsc::channel` if a worker ever needs to send a typed error rather than a `String`. This is the one place the "drop `Send + Sync`" rule has an explicit exception, and it's intentional.

### Module layout (additions to `crates/core/src/`)

```
crates/core/src/
  lib.rs              (re-exports)
  id.rs               (NEW)  newtype IDs: TagId, SessionId, CommitSha
  activity.rs         (NEW)  ActivityEntry, ActivityKind, ActivityTarget
  session.rs          (NEW)  SessionInfo
  tag.rs              (NEW)  Tag, TagKind, NewTag
  todo.rs             (NEW)  TodoItem, TodoKind
  commit.rs           (NEW)  CommitInfo, BranchInfo
  traits.rs           (extended) drop Send+Sync from existing, add 4 new traits
  error.rs            (extended) add SessionError, StoreError, SearchError
```

---

## Sessions crate (`codepeek-sessions`) — NEW

### Responsibility

Implements `SessionStore`. Reads Claude Code session JSONL files from `~/.claude/projects/<encoded-path>/*.jsonl`, parses them into `SessionInfo`. Encapsulates Claude's on-disk format so the view layer never knows about JSONL.

### How Claude Code stores sessions

Confirmed by inspection of `~/.claude/projects/`:

- One directory per project, name = absolute path with `/` and `.` replaced by `-` (e.g. `-home-delucca-Workspaces-src-codepeekhq-codepeek`).
- Inside that directory: one `.jsonl` file per session, named `<uuid>.jsonl`.
- Each line is a JSON object. The first line is typically a `file-history-snapshot` entry; subsequent lines are user/assistant messages with fields like `type`, `parentUuid`, `timestamp`, `sessionId`, `cwd`, `entrypoint`, `message: { role, content }`.

### Implementation sketch

```rust
pub struct ClaudeSessionStore {
    base: PathBuf,    // ~/.claude/projects, resolved at construction
}

impl ClaudeSessionStore {
    pub fn discover() -> Result<Self, SessionError> {
        let base = dirs::home_dir()
            .map(|h| h.join(".claude").join("projects"))
            .ok_or(SessionError::NotFound { path: PathBuf::from("~/.claude/projects") })?;
        Ok(Self { base })
    }
}

impl SessionStore for ClaudeSessionStore {
    fn list_sessions(&self, repo: &Path) -> Result<Vec<SessionInfo>, SessionError> {
        // 1. Encode repo path to Claude's directory naming convention
        // 2. Read all *.jsonl files in that directory
        // 3. For each file:
        //    - Parse first + last non-snapshot lines for timestamps
        //    - Count lines for message_count
        //    - Extract `cwd` from any message line for verification
        //    - Pull first user message text as `summary` (truncated)
        // 4. Return sorted by last_active descending
    }
}
```

Note: no `Send + Sync` bound. `ClaudeSessionStore` is constructed cheaply inside the worker thread, not shared across threads.

### Path encoding

```rust
fn encode_repo_path(repo: &Path) -> String {
    repo.to_string_lossy().replace(['/', '.'], "-")
}
```

### Performance note

For each session file we only need first + last + count, NOT the full content. Use a streaming line reader (`BufRead::lines`) and skip body parsing of intermediate lines. For sessions with thousands of messages this is still O(lines) but each line is a cheap byte scan, no JSON parse.

### Module layout

```
crates/sessions/src/
  lib.rs              re-exports ClaudeSessionStore
  store.rs            SessionStore implementation
  jsonl.rs            line streaming + minimal parsing helpers
```

### Dependencies

```toml
[dependencies]
codepeek-core.workspace = true
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }     # NEW workspace dep
dirs.workspace = true
```

---

## Search crate (`codepeek-search`) — NEW

### Responsibility

Implements `FileSearcher` and `TodoScanner`. Walks a workdir respecting `.gitignore`, returns matching file paths or TODO items.

### Implementation sketch

```rust
pub struct RipgrepLikeSearcher;

impl FileSearcher for RipgrepLikeSearcher {
    fn find_files(&self, root: &Path, query: &str, limit: usize)
        -> Result<Vec<PathBuf>, SearchError>
    {
        // Use `ignore::WalkBuilder::new(root).build()` to walk respecting .gitignore.
        // For each file, fuzzy-match the relative path against the query.
        // Use a simple subsequence match (not full Levenshtein) for v1.
        // Stop after `limit` results.
    }
}

pub struct TodoCommentScanner;

impl TodoScanner for TodoCommentScanner {
    fn scan(&self, root: &Path) -> Result<Vec<TodoItem>, SearchError> {
        // Walk with `ignore::WalkBuilder`.
        // For each text file, scan line-by-line for the regex
        //   `(?i)(TODO|FIXME|HACK|XXX)[:\s]`
        // Skip binary files using the same byte heuristic as app.rs.
        // Return file:line:kind:text for each match.
    }
}
```

Both implementing structs are zero-sized — no `Send + Sync` consideration needed because they hold no state. Worker threads instantiate them as `RipgrepLikeSearcher` directly.

### Why `ignore` over shelling out to ripgrep

- `ignore` is the same crate ripgrep is built on. Same `.gitignore` semantics, no shell-out, no PATH issues.
- We get programmatic results, not text parsing.
- Trade-off: adds ~200KB to binary. Acceptable.
- Alternative considered: shell out to `rg` with `--json`. Rejected — adds an external dependency the user must install separately.

### Module layout

```
crates/search/src/
  lib.rs              re-exports RipgrepLikeSearcher, TodoCommentScanner
  files.rs            FileSearcher implementation
  todos.rs            TodoScanner implementation
  walk.rs             shared `ignore::Walk` configuration
```

### Dependencies

```toml
[dependencies]
codepeek-core.workspace = true
ignore = { workspace = true }     # NEW workspace dep
regex = { workspace = true }      # NEW workspace dep
```

---

## Store crate (`codepeek-store`) — NEW

### Responsibility

Implements `TagStore`. Reads/writes a JSON file at `~/.config/codepeek/tags.json`. Single source of truth for persistent codepeek state. (When pins land in a future plan they go here too.)

### File format

`~/.config/codepeek/tags.json`:

```json
{
  "version": 1,
  "next_id": 5,
  "tags": [
    {
      "id": 1,
      "created_at": "2026-04-07T12:34:56Z",
      "path": "src/main.rs",
      "line": 42,
      "kind": "issue",
      "note": "this allocates in a hot loop"
    }
  ]
}
```

`version` is reserved for future migrations. v1 always reads/writes version 1; an unknown version returns `StoreError::Corrupt`.

### Implementation sketch

```rust
pub struct JsonTagStore {
    path: PathBuf,
    inner: RefCell<TagFile>,    // load on construction, persist on each mutation
}

impl JsonTagStore {
    pub fn open() -> Result<Self, StoreError> { /* resolve path, load or init */ }
}

impl TagStore for JsonTagStore {
    fn list_tags(&self) -> Result<Vec<Tag>, StoreError> {
        Ok(self.inner.borrow().tags.clone())
    }

    fn add_tag(&self, new: NewTag<'_>) -> Result<Tag, StoreError> {
        let mut inner = self.inner.borrow_mut();
        let id = TagId(inner.next_id);
        inner.next_id += 1;
        let tag = Tag {
            id,
            created_at: SystemTime::now(),
            path: new.path.to_path_buf(),
            line: new.line,
            kind: new.kind,
            note: new.note.to_string(),
        };
        inner.tags.push(tag.clone());
        self.persist(&inner)?;
        Ok(tag)
    }

    fn remove_tag(&self, id: TagId) -> Result<(), StoreError> {
        let mut inner = self.inner.borrow_mut();
        inner.tags.retain(|t| t.id != id);
        self.persist(&inner)?;
        Ok(())
    }
}

impl JsonTagStore {
    fn persist(&self, inner: &TagFile) -> Result<(), StoreError> {
        // 1. Serialize `inner` to a JSON string
        // 2. Use tempfile::NamedTempFile::new_in(self.path.parent().unwrap())?
        //    to create a securely-named temp file in the same directory
        // 3. Write the JSON string to the temp file
        // 4. Call temp.persist(&self.path) to atomically rename it over the target
    }
}
```

### Atomic write — use `tempfile`, don't roll our own

The notebook flagged the v1 plan's hand-rolled `<path>.tmp` + rename helper. Replaced with `tempfile::NamedTempFile::persist()`. Reasons:

- **Garbage collection on crash:** if the process panics or is killed mid-write, `NamedTempFile` cleans up the temp file via its `Drop` impl. The hand-rolled approach leaves `.tmp` files littered across `~/.config/codepeek/`.
- **Race / symlink safety:** `tempfile` generates unique randomized names in the target directory, preventing symlink attacks and races between two codepeek instances writing simultaneously.
- **Same-filesystem guarantee:** `NamedTempFile::new_in(parent)` puts the temp on the same mount as the destination, so `persist` (which is just a rename) is atomic.
- **Less code we have to test:** `tempfile` is already a workspace dev-dep used across the workspace. We're promoting it to a real dep for `crates/store`.

### `RefCell` instead of `Mutex`

Single-threaded design. The store is held as `Rc<dyn TagStore>` and accessed only from the main thread. `RefCell::borrow_mut` is the right primitive. Clippy's `rc_mutex` lint would flag `Rc<Mutex<…>>`.

### Module layout

```
crates/store/src/
  lib.rs              re-exports JsonTagStore
  tags.rs             TagStore implementation
  file.rs             TagFile (the on-disk shape) + serde derives
```

### Dependencies

```toml
[dependencies]
codepeek-core.workspace = true
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }     # shared with sessions crate
dirs.workspace = true
tempfile = { workspace = true }       # promoted from dev-dep to real dep
```

---

## Git crate extensions (`codepeek-git`)

### CommitLog impl

`GitChangeDetector` gets a second trait implementation: `CommitLog`.

```rust
impl CommitLog for GitChangeDetector {
    fn recent_commits(&self, limit: usize) -> Result<Vec<CommitInfo>, ChangeError> {
        let repo = self.repo.borrow();
        let mut walker = repo.revwalk().map_err(/* … */)?;
        walker.push_head().map_err(/* … */)?;
        // For up to `limit` oids: lookup commit, build CommitInfo with newtype CommitSha
    }

    fn list_branches(&self) -> Result<Vec<BranchInfo>, ChangeError> {
        let repo = self.repo.borrow();
        repo.branches(Some(BranchType::Local))
            .map(|iter| {
                iter.filter_map(Result::ok)
                    .map(|(branch, _)| /* build BranchInfo */)
                    .collect()
            })
            .map_err(/* … */)
    }

    fn read_at_commit(&self, sha: &CommitSha, path: &Path) -> Result<String, ChangeError> {
        let repo = self.repo.borrow();
        let oid = git2::Oid::from_str(&sha.0).map_err(/* … */)?;
        let commit = repo.find_commit(oid).map_err(/* … */)?;
        let tree = commit.tree().map_err(/* … */)?;
        // Same blob extraction pattern as read_at_head
    }
}
```

`CommitLog` has no `Send + Sync` bound. The trait is held as `Rc<dyn CommitLog>` from views.

### What changes in the existing `GitChangeDetector`

Two changes covered in M2:

1. `Mutex<Repository>` → `RefCell<Repository>` (single-threaded)
2. All `repo.lock().expect("repo mutex poisoned")` → `repo.borrow()` / `repo.borrow_mut()`

The `ChangeDetector` trait loses its `Send + Sync` bound at the same time.

### Dependencies

No new external deps. `git2` already in.

---

## View crate refactor (`codepeek-view`) — heavy

### The View enum

```rust
// crates/view/src/views.rs (sibling file, not views/mod.rs)

use std::borrow::Cow;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use crate::action::Action;
use crate::theme::Theme;

mod home;
mod changes;
mod sessions;
mod search;
mod tags;
mod branches;
mod todos;
mod file_viewer;

pub use home::HomeView;
pub use changes::ChangesView;
pub use sessions::SessionsView;
pub use search::SearchView;
pub use tags::TagsView;
pub use branches::BranchesView;
pub use todos::TodosView;
pub use file_viewer::FileViewerView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewId {
    Home,
    Changes,
    Sessions,
    Search,
    Tags,
    Branches,
    Todos,
    FileViewer,
}

pub enum View {
    Home(HomeView),
    Changes(ChangesView),
    Sessions(SessionsView),
    Search(SearchView),
    Tags(TagsView),
    Branches(BranchesView),
    Todos(TodosView),
    FileViewer(FileViewerView),
}

impl View {
    pub fn id(&self) -> ViewId {
        match self {
            View::Home(_)       => ViewId::Home,
            View::Changes(_)    => ViewId::Changes,
            View::Sessions(_)   => ViewId::Sessions,
            View::Search(_)     => ViewId::Search,
            View::Tags(_)       => ViewId::Tags,
            View::Branches(_)   => ViewId::Branches,
            View::Todos(_)      => ViewId::Todos,
            View::FileViewer(_) => ViewId::FileViewer,
        }
    }

    pub fn title(&self) -> Cow<'_, str> {
        match self {
            View::Home(v)       => v.title(),
            View::Changes(v)    => v.title(),
            View::Sessions(v)   => v.title(),
            View::Search(v)     => v.title(),
            View::Tags(v)       => v.title(),
            View::Branches(v)   => v.title(),
            View::Todos(v)      => v.title(),
            View::FileViewer(v) => v.title(),
        }
    }

    pub fn status_hints(&self) -> Cow<'_, [(&'static str, &'static str)]> {
        match self {
            View::Home(v)       => v.status_hints(),
            View::Changes(v)    => v.status_hints(),
            View::Sessions(v)   => v.status_hints(),
            View::Search(v)     => v.status_hints(),
            View::Tags(v)       => v.status_hints(),
            View::Branches(v)   => v.status_hints(),
            View::Todos(v)      => v.status_hints(),
            View::FileViewer(v) => v.status_hints(),
        }
    }

    pub fn handle_event(&mut self, key: KeyEvent) -> Action {
        match self {
            View::Home(v)       => v.handle_event(key),
            View::Changes(v)    => v.handle_event(key),
            View::Sessions(v)   => v.handle_event(key),
            View::Search(v)     => v.handle_event(key),
            View::Tags(v)       => v.handle_event(key),
            View::Branches(v)   => v.handle_event(key),
            View::Todos(v)      => v.handle_event(key),
            View::FileViewer(v) => v.handle_event(key),
        }
    }
    // render, on_enter, poll_loading, wants_raw_keys all follow the same
    // exhaustive-match pattern. No `_ =>` wildcard arms — see below.
}
```

Each variant struct (`HomeView`, `SessionsView`, …) defines its own methods with the same shape. The `match`-and-delegate plumbing is mechanical and ~80 lines total. No `Box`, no vtable, no `dyn`.

**Delegate pattern: exhaustive match per variant, no wildcards.** Two patterns are tempting:

```rust
// Pattern A — short, but loses exhaustiveness
pub fn wants_raw_keys(&self) -> bool {
    match self {
        View::Search(v) => v.wants_raw_keys(),
        _ => false,
    }
}

// Pattern B — explicit, exhaustive
pub fn wants_raw_keys(&self) -> bool {
    match self {
        View::Home(v)       => v.wants_raw_keys(),
        View::Changes(v)    => v.wants_raw_keys(),
        View::Sessions(v)   => v.wants_raw_keys(),
        View::Search(v)     => v.wants_raw_keys(),
        View::Tags(v)       => v.wants_raw_keys(),
        View::Branches(v)   => v.wants_raw_keys(),
        View::Todos(v)      => v.wants_raw_keys(),
        View::FileViewer(v) => v.wants_raw_keys(),
    }
}
```

**This plan uses Pattern B for every delegate method.** Reasoning: the whole point of going from `Box<dyn View>` to `enum View` was to gain exhaustive-match safety from the compiler. A `_ =>` wildcard arm throws that away — adding a future view that *also* wants raw keys would silently fall through the wildcard without a compile error. Pattern B costs ~30 extra lines of boilerplate but the compiler enforces that every new view variant explicitly answers every delegate method.

To keep this from being painful, every per-variant view struct provides every delegate method even when the answer is trivial. Most views will have:

```rust
impl HomeView {
    pub fn wants_raw_keys(&self) -> bool { false }
    pub fn poll_loading(&mut self) { /* no-op or self.state.poll() */ }
    pub fn on_enter(&mut self) { /* no-op or kick off load */ }
}
```

These trivial methods are cheap to write and the compiler verifies you wrote one for every view. Aligns with the Rust style guide rule to "prefer exhaustive matches" that the rust-style-guide notebook called out explicitly.

**`status_hints()` returns `Cow<'_, [(&'static str, &'static str)]>`** — strictly more general than a bare static slice while preserving the zero-allocation common case. Views with fixed hints define a `const HINTS: &[(&str, &str)] = &[…];` and return `Cow::Borrowed(HINTS)` (zero cost). Views with state-dependent hints — e.g. FileViewer toggling between `("d","show diff")` and `("d","hide diff")`, or Tags showing `("x","remove")` only when an entry is selected — return `Cow::Owned(vec![…])` and pay one Vec allocation per frame *only when their state actually requires it*. The notebook explicitly endorsed this `Cow` shape over a bare `&'static` slice for any trait that may need occasional dynamism.

### Background loading types

```rust
// crates/view/src/loading.rs

use std::sync::mpsc;

pub enum LoadState<T> {
    Idle,
    Loading {
        rx: mpsc::Receiver<Result<T, String>>,
        spinner_tick: u8,
    },
    Ready(T),
    Failed(String),
}

impl<T> LoadState<T> {
    pub fn new() -> Self { Self::Idle }

    /// Non-blocking: drains the receiver if loading. Returns true if state changed.
    pub fn poll(&mut self) -> bool {
        let LoadState::Loading { rx, spinner_tick } = self else {
            return false;
        };
        match rx.try_recv() {
            Ok(Ok(value)) => { *self = LoadState::Ready(value); true }
            Ok(Err(msg))  => { *self = LoadState::Failed(msg); true }
            Err(mpsc::TryRecvError::Empty) => {
                *spinner_tick = spinner_tick.wrapping_add(1);
                false
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                *self = LoadState::Failed("worker dropped".into());
                true
            }
        }
    }

    pub fn start(&mut self, rx: mpsc::Receiver<Result<T, String>>) {
        *self = LoadState::Loading { rx, spinner_tick: 0 };
    }
}
```

### Spinner rendering

```rust
// crates/view/src/components/spinner.rs (NEW)

pub struct Spinner;

const FRAMES: &[char] = &['\u{280B}','\u{2819}','\u{2839}','\u{2838}','\u{283C}','\u{2834}','\u{2826}','\u{2827}','\u{2807}','\u{280F}'];

/// Render-loop ticks per spinner frame advance.
/// At ~60fps render rate, dividing by 6 gives ~10fps spinner animation,
/// which matches the conventional loading-indicator pace.
const TICKS_PER_FRAME: u8 = 6;

impl Spinner {
    pub fn frame(tick: u8) -> char {
        let index = (tick / TICKS_PER_FRAME) as usize % FRAMES.len();
        FRAMES[index]
    }

    pub fn render(/* … */) { /* centered in area, with optional label */ }
}
```

Used by every view in `LoadState::Loading`. The `TICKS_PER_FRAME` divisor matters: `LoadState::poll` runs every event-loop tick (~16ms), so without the divisor the spinner cycles through 10 frames in 167ms — visually jittery. Dividing by 6 gives a calmer ~10fps animation. The constant lives in `spinner.rs` so it's tweakable in one place.

### App refactor

```rust
// crates/view/src/app.rs (after refactor)

pub struct App {
    should_quit: bool,
    router: Router,
    current: View,                // enum, not Box<dyn>
    history: Vec<ViewId>,         // back-stack for Esc
    palette: Option<CommandPalette>,
    peek_overlay: Option<PeekOverlay>,
    error_message: Option<String>,
}

impl App {
    pub fn new(router: Router) -> Self {
        let mut current = router.build(ViewId::Home);
        current.on_enter();
        Self {
            should_quit: false,
            router,
            current,
            history: Vec::new(),
            palette: None,
            peek_overlay: None,
            error_message: None,
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        while !self.should_quit {
            self.current.poll_loading();    // drain background results
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(config::TICK_RATE)? {
            loop {
                if let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press
                {
                    self.error_message = None;

                    // Order of dispatch:
                    //   1. Palette open → route to palette
                    //   2. Peek overlay visible → route to overlay
                    //   3. Current view wants raw keys → route to view (skip global nav)
                    //   4. Global nav key → handle here
                    //   5. Otherwise → route to current view
                    let action = self.dispatch_key(key);
                    self.dispatch(action);
                }
                if !event::poll(Duration::ZERO)? { break; }
            }
        }
        Ok(())
    }

    fn navigate(&mut self, target: ViewId) {
        if self.current.id() == target { return; }
        self.history.push(self.current.id());
        self.current = self.router.build(target);
        self.current.on_enter();
    }

    fn back(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.current = self.router.build(prev);
            self.current.on_enter();
        }
    }
}
```

### The Router

```rust
// crates/view/src/router.rs

use std::cell::RefCell;
use std::rc::Rc;

pub struct Router {
    change_detector: Rc<dyn ChangeDetector>,
    highlighter: Rc<RefCell<dyn SyntaxHighlighter>>,
    session_store: Rc<dyn SessionStore>,
    tag_store: Rc<dyn TagStore>,
    todo_scanner: Rc<dyn TodoScanner>,
    file_searcher: Rc<dyn FileSearcher>,
    commit_log: Rc<dyn CommitLog>,
    repo_root: PathBuf,
}

impl Router {
    pub fn build(&self, id: ViewId) -> View {
        match id {
            ViewId::Home     => View::Home(HomeView::new(/* aggregator */)),
            ViewId::Changes  => View::Changes(ChangesView::new(self.change_detector.clone(), self.highlighter.clone())),
            ViewId::Sessions => View::Sessions(SessionsView::new(self.session_store.clone(), self.repo_root.clone())),
            ViewId::Search   => View::Search(SearchView::new(self.file_searcher.clone(), self.repo_root.clone())),
            ViewId::Tags     => View::Tags(TagsView::new(self.tag_store.clone())),
            ViewId::Branches => View::Branches(BranchesView::new(self.commit_log.clone())),
            ViewId::Todos    => View::Todos(TodosView::new(self.todo_scanner.clone(), self.repo_root.clone())),
            ViewId::FileViewer => panic!("FileViewer is built via build_file_viewer"),
        }
    }

    pub fn build_file_viewer(&self, path: PathBuf, line: Option<u32>) -> View {
        View::FileViewer(FileViewerView::new(
            path,
            line,
            self.highlighter.clone(),
            self.tag_store.clone(),     // FileViewer holds its own TagStore for `m`-to-mark
        ))
    }
}
```

### Why `Rc<dyn Trait>` for stores (and not the View enum's static dispatch)

The notebook validated this asymmetry. Two different decisions:

| | Views | Stores |
|---|---|---|
| Variant set | Closed (8 fixed types) | Open (Json + Null + future Sqlite + test stubs) |
| Need polymorphism | At compile time only | At runtime (router builds different impls based on availability) |
| Right tool | `enum View` (static dispatch) | `Rc<dyn Trait>` (dynamic dispatch) |

For stores, dynamic dispatch is the right shape because the binary builds a different graph at startup based on whether `~/.claude/projects` exists, whether `~/.config/codepeek/tags.json` is readable, etc. Each failure substitutes a `NullStore`. The view code can't know which one it got.

### Hierarchical Action enum — Component Architecture pattern

The notebook flagged the v1 plan's flat 15-variant Action enum as a "monolithic bottleneck." v2 takes the **Component Architecture** answer: strip the global enum to cross-cutting concerns only, and let each view handle its own state mutations internally.

```rust
// crates/view/src/action.rs (after revision)

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Application lifecycle.
    Quit,
    /// Cross-view navigation.
    NavigateTo(ViewId),
    /// Esc — pop the back-stack.
    Back,
    /// Refresh the current view's data source (re-trigger on_enter).
    Refresh,
    /// Open the command palette overlay.
    OpenPalette,
    /// Close the command palette overlay.
    ClosePalette,
    /// Cross-view: open a file in FileViewerView, optionally jumping to line.
    /// Used by Search, Tags, Todos when the user picks a result.
    OpenFileAt { path: PathBuf, line: Option<u32> },
    /// Cross-view: open the file list of a commit.
    /// Used by Branches when the user picks a commit.
    OpenCommit(CommitSha),
    /// Dismiss the deleted-file peek overlay.
    DismissPeek,
    /// No-op (key didn't match anything in the current context).
    Noop,
}
```

Variants the v1 plan had that are now **gone from the global enum** (they live as private state mutations inside the relevant View):

- `SelectFile(usize)` — internal to `ChangesView` (the file list lives there now)
- `ToggleDiff` — internal to `FileViewerView`
- `AddTag(…)` / `RemoveTag(…)` — internal to `FileViewerView` and `TagsView` respectively (each view holds its own `Rc<dyn TagStore>` clone and calls it directly)
- `PaletteCommand(…)` — internal to `CommandPalette`

**What "internal" means in practice:** the view's `handle_event` method does the mutation directly, then returns `Action::Noop` (or one of the cross-cutting variants if a navigation needs to happen). The App never sees the per-view mutations.

This is the Component Architecture pattern documented in Ratatui's component template. It scales: adding a new view never requires touching the global Action enum unless the new view introduces a new cross-cutting concern (which is rare).

### Cross-view navigation keys

Add to `crates/view/src/keybindings.rs`:

```rust
pub fn nav_target(key: &KeyEvent) -> Option<ViewId> {
    match key.code {
        KeyCode::Char('h') => Some(ViewId::Home),
        KeyCode::Char('c') => Some(ViewId::Changes),
        KeyCode::Char('s') => Some(ViewId::Sessions),
        KeyCode::Char('/') => Some(ViewId::Search),
        KeyCode::Char('t') => Some(ViewId::Tags),
        KeyCode::Char('b') => Some(ViewId::Branches),
        KeyCode::Char('T') => Some(ViewId::Todos),  // capital T to avoid conflict with `t`
        _ => None,
    }
}

pub fn is_palette(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char(':'))
        || (key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL))
}
```

### Conflict resolution between view-local and global keys

**Resolution chosen** (was Open Decision #1 in v1, now resolved): the `View::wants_raw_keys()` method returns `true` when a view is in an input mode (Search while typing, CommandPalette open). The App checks this **before** consulting `nav_target`:

```rust
fn dispatch_key(&mut self, key: KeyEvent) -> Action {
    if let Some(palette) = &mut self.palette {
        return palette.handle_event(key);
    }
    if let Some(overlay) = &mut self.peek_overlay {
        return overlay.handle_event(key);
    }
    if !self.current.wants_raw_keys() {
        if let Some(target) = keybindings::nav_target(&key) {
            return Action::NavigateTo(target);
        }
        if keybindings::is_palette(&key) {
            return Action::OpenPalette;
        }
    }
    self.current.handle_event(key)
}
```

`wants_raw_keys` defaults to `false`. Only `SearchView` (in input mode) and `CommandPalette` override it to `true`. This is the standard "opt-in trait capability" pattern — the notebook validated it explicitly as idiomatic.

### Status bar

The status bar already accepts `&[(&str, &str)]`. After the refactor it accepts `&[(&'static str, &'static str)]` (a normal borrow of whatever `Cow` the view returned). Each view exposes its hints via `View::status_hints() -> Cow<'_, [(&'static str, &'static str)]>` — borrowed for static-hint views, owned for dynamic-hint views. App also appends global nav hints if there's room (terminal width ≥ 120 cols).

### Layout

The existing zen-mode layouts (`zen_file_list_layout`, `zen_viewer_layout`) are reused. Home gets its own layout helper.

```rust
// crates/view/src/layout.rs (additions)

pub fn zen_home_layout(area: Rect) -> ZenLayout {
    let height = area.height.saturating_mul(config::ZEN_HOME_MAX_HEIGHT_PERCENT) / 100;
    let content_height = height + 2;

    let [centered_v] = Layout::vertical([Constraint::Length(content_height)])
        .flex(Flex::Center)
        .areas(area);

    let width = config::ZEN_HOME_MAX_WIDTH.min(area.width.saturating_sub(4));
    let [centered_h] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(centered_v);

    split_content_status(centered_h)
}
```

### Config additions

```rust
// crates/view/src/config.rs (additions)
pub const ZEN_HOME_MAX_WIDTH: u16 = 70;
pub const ZEN_HOME_MAX_HEIGHT_PERCENT: u16 = 75;

pub const HOME_ACTIVITY_LIMIT: usize = 30;
pub const RECENT_COMMITS_LIMIT: usize = 50;
pub const SEARCH_RESULT_LIMIT: usize = 200;
```

### Module layout (view crate)

```
crates/view/src/
  lib.rs                  re-exports App, ViewId
  app.rs                  App struct, dispatch, run loop
  router.rs               Router (Rc-injected stores, builds Views)
  action.rs               Action enum (small, hierarchical)
  loading.rs              LoadState<T>, mpsc channel helpers
  config.rs               (extended)
  keybindings.rs          (extended)
  layout.rs               (extended)
  render_helpers.rs       (extended — relative-time formatter added)
  theme.rs                (unchanged)
  views.rs                (NEW)  pub use everything in views/, define View enum
  views/                  (NEW)
    home.rs               HomeView (uses ActivityFeedAggregator)
    changes.rs            ChangesView (wraps FileList + FileViewer)
    sessions.rs           SessionsView (uses LoadState<Vec<SessionInfo>>)
    search.rs             SearchView (TextInput + LoadState<Vec<PathBuf>>)
    tags.rs               TagsView
    branches.rs           BranchesView (uses LoadState<(Vec<BranchInfo>, Vec<CommitInfo>)>)
    todos.rs              TodosView (uses LoadState<Vec<TodoItem>>)
    file_viewer.rs        FileViewerView (the contextual file display)
  components.rs           (unchanged structure)
  components/
    error_bar.rs          (unchanged)
    file_list.rs          (unchanged)
    file_viewer.rs        (unchanged — the LIST item component)
    peek_overlay.rs       (unchanged)
    status_bar.rs         (extended for static slice hints)
    command_palette.rs    (NEW)
    text_input.rs         (NEW)
    spinner.rs            (NEW)
```

**Note on naming:** `crates/view/src/views.rs` (sibling file) + `crates/view/src/views/home.rs` (subdirectory). This is the Rust 2018+ idiom that the rust-style-guide notebook recommends, matching the official Ratatui component template (`components.rs` + `components/home.rs`). Clippy `mod_module_files` enforces it for projects that want to.

### `FileViewerView` vs reusing existing `FileViewer` component

The existing `crates/view/src/components/file_viewer.rs` is a **reusable component** — it knows how to render highlighted lines with a gutter. It stays as-is.

`FileViewerView` (the new top-level view) **wraps** the component. It owns the file path, the highlighter `Rc`, and the tag store `Rc`. Its `handle_event` translates `m`/`M` into direct `tag_store.add_tag(NewTag { … })` calls (no global Action), and translates `Esc` into `Action::Back`. This separation keeps the rendering component pure.

### Reused components

- `FileList` (the existing `FileChange`-specialized list) stays as-is, used by `ChangesView`.
- `FileViewer` (the existing rendering component) stays as-is, used by `FileViewerView` and `ChangesView`.
- `PeekOverlay` stays as-is, used by `ChangesView` for deleted-file peeks.
- `ErrorBar` stays as-is, used by every view's `LoadState::Failed` rendering and by App for transient errors.
- `StatusBar` extended to take `&'static [(&'static str, &'static str)]`.

For Sessions/Tags/Search/Todos/Branches, each view instantiates its own ratatui `List` widget directly (~30 lines of rendering code per view). Generalizing `FileList<T>` is deliberately deferred — the duplication is small and the generic would obscure the rendering logic. If view #5+ feels painful, refactor then.

---

## TUI app updates (`apps/tui`)

```rust
// apps/tui/src/main.rs (after revision)

use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<()> {
    color_eyre::install()?;

    let (app_config, config_warning) = config::AppConfig::load();
    if let Some(warning) = config_warning { eprintln!("codepeek: {warning}"); }

    let repo_root = std::env::current_dir()?;

    // Existing
    let detector: Rc<dyn ChangeDetector> = Rc::new(GitChangeDetector::open(&repo_root)?);
    let highlighter: Rc<RefCell<dyn SyntaxHighlighter>> =
        Rc::new(RefCell::new(TreeSitter::with_languages(app_config.enabled_languages())));

    // The git detector implements both ChangeDetector and CommitLog.
    // We need a second Rc with the CommitLog vtable. Since GitChangeDetector
    // implements both, and Rc<dyn TraitA> can't be coerced to Rc<dyn TraitB>,
    // we construct the binding twice from the same underlying instance.
    // Easier: hold an Rc<GitChangeDetector> internally and coerce at injection sites.
    let git = Rc::new(GitChangeDetector::open(&repo_root)?);
    let detector: Rc<dyn ChangeDetector> = git.clone();
    let commit_log: Rc<dyn CommitLog> = git;

    // New
    let session_store: Rc<dyn SessionStore> = match ClaudeSessionStore::discover() {
        Ok(s)  => Rc::new(s),
        Err(e) => { eprintln!("codepeek: sessions disabled: {e}"); Rc::new(NullSessionStore) }
    };
    let tag_store: Rc<dyn TagStore> = match JsonTagStore::open() {
        Ok(s)  => Rc::new(s),
        Err(e) => { eprintln!("codepeek: tags disabled: {e}"); Rc::new(NullTagStore) }
    };
    let todo_scanner: Rc<dyn TodoScanner> = Rc::new(TodoCommentScanner);
    let file_searcher: Rc<dyn FileSearcher> = Rc::new(RipgrepLikeSearcher);

    let router = Router::new(
        detector,
        highlighter,
        session_store,
        tag_store,
        todo_scanner,
        file_searcher,
        commit_log,
        repo_root,
    );

    let terminal = ratatui::init();
    let result = App::new(router).run(terminal);
    ratatui::restore();
    result?;
    Ok(())
}
```

### Two `Rc`s from one `GitChangeDetector`

`Rc<dyn TraitA>` doesn't coerce to `Rc<dyn TraitB>` even when the underlying type implements both. Standard pattern: hold an `Rc<GitChangeDetector>` and coerce to each trait object separately. Both `Rc<dyn ChangeDetector>` and `Rc<dyn CommitLog>` point to the same allocation, share the same refcount, and the trait dispatch picks the right vtable at call time.

### Null stubs in the binary

`NullSessionStore` and `NullTagStore` live in `apps/tui/src/stubs.rs`. They implement the trait with empty/no-op behavior so the TUI launches even when stores fail. Library crates stay strict — only the binary defines the fallbacks.

```rust
// apps/tui/src/stubs.rs

pub struct NullSessionStore;
impl SessionStore for NullSessionStore {
    fn list_sessions(&self, _: &Path) -> Result<Vec<SessionInfo>, SessionError> {
        Ok(Vec::new())
    }
}

pub struct NullTagStore;
impl TagStore for NullTagStore {
    fn list_tags(&self) -> Result<Vec<Tag>, StoreError> { Ok(Vec::new()) }
    fn add_tag(&self, _: NewTag<'_>) -> Result<Tag, StoreError> {
        Err(StoreError::WriteFailed { /* … */ })
    }
    fn remove_tag(&self, _: TagId) -> Result<(), StoreError> { Ok(()) }
}
```

### Worker thread spawning

The binary doesn't spawn workers — each view does it inside its own `on_enter`. The binary just constructs the `Router` and runs the `App`.

### Dependency additions

`apps/tui/Cargo.toml`:

```toml
[dependencies]
codepeek-core.workspace = true
codepeek-view.workspace = true
codepeek-git.workspace = true
codepeek-syntax.workspace = true
codepeek-sessions.workspace = true     # NEW
codepeek-search.workspace = true       # NEW
codepeek-store.workspace = true        # NEW
ratatui.workspace = true
color-eyre.workspace = true
serde.workspace = true
toml.workspace = true
dirs.workspace = true
```

---

## Workspace dependency additions (root `Cargo.toml`)

```toml
[workspace.dependencies]
# existing entries unchanged

# Internal crates (new)
codepeek-sessions = { path = "crates/sessions" }
codepeek-search   = { path = "crates/search" }
codepeek-store    = { path = "crates/store" }

# JSON (new — used by sessions and store)
serde_json = "1.0.133"

# File walking and regex (new — used by search)
ignore = "0.4.23"
regex  = "1.11.1"

# Atomic writes (promoted from dev-dep to real workspace dep — used by store)
# `tempfile` was already in [workspace.dependencies] as a dev-dep; nothing to add.
```

`[workspace.members]` adds `crates/sessions`, `crates/search`, `crates/store`.

`tempfile` is already in `[workspace.dependencies]` (used as a dev-dep elsewhere). The store crate references it as a real dep via `tempfile = { workspace = true }` — no version change needed.

**Versions to verify at implementation time** via Context7: `serde_json`, `ignore`, `regex`. All others are unchanged from the existing workspace.

---

## Implementation milestones

Seventeen milestones across six phases. The phasing changed slightly from v1 — Phase A now includes a dedicated "background loading" milestone (M3) before the View enum refactor (M4), so the loading infrastructure is in place before any view that needs it lands.

Each milestone produces a working `just check` and (from M5 onward) a runnable `just run`.

---

### Phase A — Foundation

#### Milestone 1: New crate scaffolding

**You'll see:** Three new empty crates exist and compile. No new functionality yet.

**What to build:**
1. Create `crates/sessions/Cargo.toml`, `crates/search/Cargo.toml`, `crates/store/Cargo.toml`. Each has only a `lib.rs` with a comment.
2. Add the three new members to root `Cargo.toml` `[workspace.members]`.
3. Add `codepeek-sessions`, `codepeek-search`, `codepeek-store` to `[workspace.dependencies]`.
4. Add `serde_json`, `ignore`, `regex` to `[workspace.dependencies]` (versions pinned).
5. `apps/tui/Cargo.toml`: declare the three new crates as dependencies (still unused).

**Verify:** `just check` passes. All 8 workspace members compile.

**Crates touched:** root `Cargo.toml`, `crates/sessions`, `crates/search`, `crates/store`, `apps/tui`.

---

#### Milestone 2: Core types, traits, newtypes — and remove `Send + Sync`

**You'll see:** `codepeek-core` exposes new types (newtype IDs, NewTag, Tag, SessionInfo, TodoItem, CommitInfo, BranchInfo, ActivityEntry) and traits (TagStore, SessionStore, TodoScanner, FileSearcher, CommitLog). Existing `ChangeDetector`/`SyntaxHighlighter` traits no longer require `Send + Sync`. `GitChangeDetector` uses `RefCell<Repository>` instead of `Mutex<Repository>`. Tests prove everything compiles and the existing change-viewer flow still works.

**What to build:**
1. `crates/core/src/id.rs` — `TagId`, `SessionId`, `CommitSha` newtypes with `serde` derives.
2. `crates/core/src/tag.rs`, `session.rs`, `todo.rs`, `commit.rs`, `activity.rs` — domain types using the newtypes.
3. `crates/core/src/tag.rs` — define `NewTag<'a>` input struct.
4. `crates/core/Cargo.toml` — add `serde = { workspace = true, features = ["derive"] }` (small concession; first serde dep in core).
5. Extend `crates/core/src/traits.rs` with the four new traits (no `Send + Sync` bounds). **Also remove `Send + Sync` from `ChangeDetector` and `SyntaxHighlighter`.**
6. Extend `crates/core/src/error.rs` with `SessionError`, `StoreError`, `SearchError`.
7. Re-export everything from `lib.rs`.
8. **`crates/git/src/detector.rs`:** replace `Mutex<Repository>` with `RefCell<Repository>`. Replace all `repo.lock().expect(…)` with `repo.borrow()` / `repo.borrow_mut()`. Update existing tests.
9. Unit tests: TagId/SessionId/CommitSha JSON round-trip, TagKind round-trip, NewTag construction, error Display formatting, `CommitSha::short()` length-clamping.

**Verify:** `just test --workspace` passes. `just run` still launches the existing UX bit-for-bit (this milestone is invisible to the user).

**Crates touched:** `crates/core`, `crates/git`.

---

#### Milestone 3: Background loading infrastructure (`LoadState<T>` + spinner)

**You'll see:** Internal scaffolding only. No user-visible change. The view crate has a `loading.rs` module with `LoadState<T>`, a `Spinner` component, and tests showing the lifecycle works against a stub worker.

**What to build:**
1. `crates/view/src/loading.rs` — `LoadState<T>` enum with `new`, `start`, `poll` methods.
2. `crates/view/src/components/spinner.rs` — `Spinner::frame(tick)` and `Spinner::render` (centered in area, with optional label below). Includes the `TICKS_PER_FRAME = 6` constant so the spinner runs at ~10fps even though `poll_loading` runs at ~60fps.
3. `crates/view/src/components.rs` — re-export `Spinner`.
4. Unit tests using `std::sync::mpsc::channel()`:
   - `LoadState::start` transitions Idle → Loading
   - `poll` on Loading with no message returns false and ticks the spinner
   - `poll` on Loading with `Ok(value)` transitions to Ready
   - `poll` on Loading with `Err(msg)` transitions to Failed
   - `poll` on Ready/Idle/Failed returns false (no-op)
   - Disconnected sender transitions to Failed

**Verify:** `just check` passes. Tests pass. `just run` is unchanged.

**Crates touched:** `crates/view`.

---

#### Milestone 4: View enum + Router refactor (no behavior change)

**You'll see:** `just run` still launches the existing Changes flow. The internals are different but the user experience is identical. This is the riskiest refactor; doing it before any new view ensures the old one still works.

**What to build:**
1. `crates/view/src/views.rs` (sibling file) — `View` enum, `ViewId` enum, `mod` declarations and re-exports. Every delegate method (`handle_event`, `render`, `on_enter`, `poll_loading`, `wants_raw_keys`, `status_hints`, `title`) uses **Pattern B** — exhaustive `match` over every variant, no `_ =>` wildcards. This is the whole reason we picked enum dispatch; the compiler must enforce that adding a future view variant updates every delegate method. `status_hints` returns `Cow<'_, [(&'static str, &'static str)]>` so future views can return dynamic hints without a breaking trait change.
2. `crates/view/src/views/changes.rs` — `ChangesView`. Move the existing `Focus`/`FileList`/`FileViewer` orchestration out of `App` into `ChangesView`. `ChangesView::new` takes `Rc<dyn ChangeDetector>` + `Rc<RefCell<dyn SyntaxHighlighter>>`. Defines its own methods matching the View enum's delegate signatures.
3. `crates/view/src/views/file_viewer.rs` — `FileViewerView` (the contextual file-display view used by `OpenFileAt`). Takes path, optional jump-to-line, highlighter, tag store. For now it's only used internally by ChangesView (M5 will add cross-view openers).
4. `crates/view/src/router.rs` — `Router` struct holding `Rc`-cloned trait objects. `build(ViewId::Changes)` returns `View::Changes(ChangesView::new(…))`. Other variants `unimplemented!()` for now.
5. Refactor `crates/view/src/app.rs`:
   - Replace `focus`, `file_list`, `file_viewer`, `change_detector`, `highlighter` fields with `router: Router`, `current: View`, `history: Vec<ViewId>`.
   - `App::new(router: Router)` instead of `App::new(detector, highlighter)`.
   - `render` matches on `current` and delegates to the view variant.
   - `handle_events` does dispatch via the new ordered scheme (palette > overlay > raw keys > nav > view).
   - For now only Quit/Refresh/Back/Noop actions are dispatched at App level.
6. Update `apps/tui/src/main.rs` to construct a `Router` (with stub stores for slots not yet built — define a `NullSessionStore` etc. in `apps/tui/src/stubs.rs` upfront; they'll be reused in M6+). For this milestone the only stores actually used are `change_detector`, `highlighter`, `tag_store`. Use `NullTagStore` for now.
7. Move all existing app-level tests in `app.rs` into `views/changes.rs` (they're really ChangesView tests now).
8. Add new app-level tests: stub View variant via `View::Changes(stub)`, verify routing/back/quit at the App level.

**Verify:** `just check` passes. `just run` shows the file list, navigation works, file viewer works, refresh works, peek overlay works, errors render. **Bit-for-bit the same UX as before.**

**Crates touched:** `crates/view`, `apps/tui`.

---

#### Milestone 5: HomeView stub + landing redirect + cross-view nav

**You'll see:** Launching codepeek lands on a Home placeholder ("Welcome to codepeek — press `c` for Changes") instead of straight to the file list. `c` navigates to Changes; `Esc` from Changes goes back to Home. `h` in Changes goes back to Home directly.

**What to build:**
1. `crates/view/src/views/home.rs` — `HomeView`. State: nothing yet (Idle LoadState placeholder). Render: a centered title and a hint to press `c`. `handle_event` returns `Noop` for everything (the App handles `h`/`c` globally). Status hints: `&[("c","changes"), ("q","quit")]` as a const slice.
2. `Router::build(ViewId::Home)` returns `View::Home(HomeView::new())`.
3. `App::new` builds `Home` as the initial view (was `ViewId::Changes`).
4. `crates/view/src/keybindings.rs` — add `nav_target(key)`, `is_palette(key)`. For now only `h` and `c` are wired.
5. `App::dispatch_key` checks `nav_target` after the current view's `wants_raw_keys` returns false.
6. `App::dispatch` handles `Action::NavigateTo(ViewId)` and `Action::Back`.
7. Tests: two-view navigation (Home → Changes → back), nav key precedence with view-local keys, `wants_raw_keys` short-circuits global nav.

**Verify:** `just run`. You land on Home. Press `c`, you're in Changes. Press `Esc`, you're back on Home. Press `h` from Changes, also back to Home. Press `q`, it quits.

**Crates touched:** `crates/view`.

---

### Phase B — Storage primitives

#### Milestone 6: `codepeek-store` JSON tag store with `tempfile` atomic write

**You'll see:** `codepeek-store` compiles, tests pass. `~/.config/codepeek/tags.json` can be read/written atomically. No UI yet.

**What to build:**
1. `crates/store/src/file.rs` — `TagFile { version, next_id, tags }` with `serde` derives.
2. `crates/store/src/tags.rs` — `JsonTagStore` implementing `TagStore`. Holds `RefCell<TagFile>`. Load on construction. `add_tag` / `remove_tag` mutate inner and call `persist`.
3. `JsonTagStore::persist`:
   - Serialize to JSON via `serde_json::to_string_pretty`
   - Resolve parent dir of target path; create it if missing
   - `let temp = tempfile::NamedTempFile::new_in(parent)?;`
   - Write JSON bytes via `temp.as_file().write_all(json.as_bytes())?;`
   - `temp.persist(&self.path)?;` — atomic rename
4. `crates/store/Cargo.toml` — add `tempfile.workspace = true` as a real dep.
5. `crates/store/src/lib.rs` — re-export `JsonTagStore`.
6. Tests using `tempfile::TempDir` for the store path:
   - Round-trip add/list/remove
   - Version mismatch surfaces as `Corrupt`
   - Concurrent reads are safe (single-threaded, but verify `RefCell` borrow semantics)
   - Atomic write: simulated mid-write panic (drop the temp file before persist) leaves the original target file intact and the temp file gone

**Verify:** `just test --package codepeek-store` passes. `just check` clean.

**Crates touched:** `crates/store`.

---

#### Milestone 7: TagsView + tagging from FileViewerView

**You'll see:** Press `t` from anywhere → Tags view (empty initially). Open a file, press `m` to mark the current line as a tag (Issue), `M` for Fix. Press `t` again, see the new entry. Press Enter on a tag to jump back to that file:line.

**What to build:**
1. `crates/view/src/views/tags.rs` — `TagsView`. Holds `Rc<dyn TagStore>` and a `LoadState<Vec<Tag>>`. On `on_enter`, spawns a worker that calls `store.list_tags()` (yes — even though it's likely fast, use the loading pattern uniformly so the spinner code path is exercised). Renders a list (relative time, kind badge, path, line, note).
2. `Router::build(ViewId::Tags)` wired up.
3. `apps/tui/src/main.rs` — replace `NullTagStore` with real `JsonTagStore::open()`. On failure, fall back to `NullTagStore` with a stderr warning.
4. **`FileViewerView` in `views/file_viewer.rs`** holds its own `Rc<dyn TagStore>`. `handle_event`: `m` → `tag_store.add_tag(NewTag { path, line, kind: Issue, note: "" })`, `M` → `Fix`. Returns `Noop` (no global Action needed). On error, sets a local error string.
5. Add `is_mark_issue(key)` and `is_mark_fix(key)` to `keybindings.rs`.
6. Add `Action::OpenFileAt { path, line }`. App handles it by calling `router.build_file_viewer(path, line)` and pushing onto history.
7. Selecting a tag in TagsView returns `Action::OpenFileAt { path, line: Some(line) }`.
8. Tests: tag-add round-trip in TagsView, FileViewerView tag handling, jump-to-line scroll math, TagsView LoadState lifecycle.

**Verify:** `just run`. Open a file (via Changes flow). Press `m` on a line. Press `t`. See the tag (with a brief spinner if list_tags is slow). Press Enter. You're back in the file at the right line. Re-launch codepeek — the tag is still there.

**Crates touched:** `crates/view`, `apps/tui`.

---

### Phase C — Discovery views

#### Milestone 8: `codepeek-search` file walker + tests

**You'll see:** `codepeek-search` compiles. `RipgrepLikeSearcher::find_files` and `TodoCommentScanner::scan` work against a real workdir. No UI yet.

**What to build:**
1. `crates/search/src/walk.rs` — `ignore::WalkBuilder` configured with the right defaults (respect `.gitignore`, no symlinks).
2. `crates/search/src/files.rs` — `RipgrepLikeSearcher::find_files`. Subsequence-match the relative path (lowercased) against the lowercased query. Sort by match-span length ascending.
3. `crates/search/src/todos.rs` — `TodoCommentScanner::scan`. Compile a `regex::Regex` once, walk files, scan each line, push `TodoItem`s. Skip binary files via the same byte heuristic as `app.rs::open_file` (first 8KB contains 0 → binary).
4. Tests using `tempfile::TempDir`: create a fake repo with `.gitignore`, files containing TODOs, binary files, and verify expected results.

**Verify:** `just test --package codepeek-search` passes. `just check` clean.

**Crates touched:** `crates/search`.

---

#### Milestone 9: SearchView (file finder)

**You'll see:** Press `/` from any view → Search view. Type characters into the input. Spinner shows briefly while the search runs in a worker, then results appear. Enter opens the highlighted file. Esc closes Search.

**What to build:**
1. `crates/view/src/components/text_input.rs` — single-line text input. State: `value: String`, `cursor: usize`. Renders an underlined input with cursor. Handles char insertion, backspace, delete, left/right cursor.
2. `crates/view/src/views/search.rs` — `SearchView`. Holds `Rc<dyn FileSearcher>`, `repo_root: PathBuf`, `text_input: TextInput`, `state: LoadState<Vec<PathBuf>>`, `selected: usize`. **`wants_raw_keys() -> bool { true }`** so global nav doesn't fire while typing. On every character keystroke, kicks off a fresh worker (cancelling any in-flight one by replacing the receiver). Esc clears input on first press, exits view on second.
3. `Router::build(ViewId::Search)` wired up.
4. Selecting a result returns `Action::OpenFileAt { path, line: None }`.
5. Tests: render with stub searcher, typing kicks off load, selection round-trip, raw-key mode works.

**Verify:** `just run`. Press `/`. Type a partial filename. Spinner blinks, then results appear. Enter opens the file in the viewer.

**Crates touched:** `crates/view`.

---

#### Milestone 10: TodosView

**You'll see:** Press `T` (capital) → TODO/FIXME inbox view. Spinner while scanning. Then list of all TODO/FIXME/HACK/XXX comments across the workdir, with file:line and the comment text. Enter jumps to the file at that line.

**What to build:**
1. `crates/view/src/views/todos.rs` — `TodosView`. Holds `Rc<dyn TodoScanner>`, `repo_root`, `state: LoadState<Vec<TodoItem>>`. On `on_enter`, spawns a worker that runs `scan(repo_root)`. Renders a list grouped by kind (TODO/FIXME/HACK/XXX).
2. `Router::build(ViewId::Todos)` wired up.
3. Add `T` to `nav_target` in `keybindings.rs`.
4. Selecting an entry → `Action::OpenFileAt { path, line: Some(line) }`.
5. Tests: render with stub scanner, grouping correctness, empty state, LoadState transitions.

**Verify:** `just run`. Press `T`. See spinner briefly, then all TODOs in the project. Pick one. You're in the file at the right line.

**Crates touched:** `crates/view`.

---

### Phase D — Git extensions

#### Milestone 11: `CommitLog` impl on `GitChangeDetector`

**You'll see:** `codepeek-git` exports a second trait impl. Tests against a real temp repo verify recent commits and branch listing. The `GitChangeDetector` struct now provides both `ChangeDetector` and `CommitLog` traits.

**What to build:**
1. Implement `CommitLog::recent_commits` in `crates/git/src/detector.rs`. Use `repo.borrow()` (now `RefCell` after M2).
2. Implement `CommitLog::list_branches`.
3. Implement `CommitLog::read_at_commit`. Returns `String` (UTF-8 decoded blob).
4. Integration tests using `tempfile::TempDir` + `git2`: create a repo with N commits and M branches, verify trait returns them correctly. Pay attention to the newtype `CommitSha` round-trip via `git2::Oid::from_str`.

**Verify:** `just test --package codepeek-git` passes. `just check` clean.

**Crates touched:** `crates/git`.

---

#### Milestone 12: BranchesView

**You'll see:** Press `b` → Branches view. Spinner while loading. Top section: branch list with `*` next to current. Bottom section: recent commits on the current branch. Tab toggles focus between the sections. Selecting a commit dispatches `Action::OpenCommit(sha)` which opens a file list of changed files in that commit.

**What to build:**
1. `crates/view/src/views/branches.rs` — `BranchesView`. Holds `Rc<dyn CommitLog>`, `state: LoadState<(Vec<BranchInfo>, Vec<CommitInfo>)>`, two `selected: usize` indices (one per sub-list), `focus: BranchesFocus { Branches, Commits }`. Tab toggles focus.
2. On `on_enter`, spawns a worker that calls both `list_branches()` and `recent_commits(RECENT_COMMITS_LIMIT)` and bundles them.
3. `Router::build(ViewId::Branches)` wired up.
4. Selecting a commit returns `Action::OpenCommit(sha)`.
5. App dispatch for `OpenCommit`: build a `CommitDiffView` (small new view variant) that lists files changed in that commit using `CommitLog::recent_commits` filtering — actually, simpler: hand the SHA to a `FileViewerView`-like view that uses `CommitLog::read_at_commit` for each file. **Decision for v1:** keep it minimal — `OpenCommit(sha)` opens the *current* HEAD's diff for that single commit, file-by-file, by reusing the existing diff plumbing. The full "browse all files in a past commit" experience is a Phase F polish.
6. Tests: render with stub commit log, branch toggle, commit selection.

**Verify:** `just run`. Press `b`. See spinner briefly, then branches and commits. Pick a recent commit. See its files.

**Crates touched:** `crates/view`.

---

### Phase E — Sessions

#### Milestone 13: `codepeek-sessions` JSONL reader

**You'll see:** `codepeek-sessions` compiles. `ClaudeSessionStore::list_sessions(repo_root)` returns the list of sessions for the current repo with timestamps and message counts. No UI yet.

**What to build:**
1. `crates/sessions/src/jsonl.rs` — minimal streaming JSONL reader. For each session file: read first message, last message, count lines. Parse only the JSON fields we need (`type`, `timestamp`, `message.role`, `message.content`, `cwd`). Use `serde_json::Value` for tolerance — sessions evolve and we don't want to break on new fields.
2. `crates/sessions/src/store.rs` — `ClaudeSessionStore` implementing `SessionStore`. `discover()` resolves `~/.claude/projects`. `list_sessions(repo)` encodes the path, reads `.jsonl` files, returns `SessionInfo` sorted by `last_active` descending.
3. Test using fixture jsonl files in `crates/sessions/tests/fixtures/`: a small jsonl with known timestamps, verify parsing.

**Verify:** `just test --package codepeek-sessions` passes.

**Crates touched:** `crates/sessions`.

---

#### Milestone 14: SessionsView

**You'll see:** Press `s` → Sessions view. Spinner while loading. Then list of Claude Code sessions for this repo, most recent first. Each row: short id, last active (relative time), message count, summary excerpt. Enter on a session is a no-op for v1.

**What to build:**
1. `crates/view/src/views/sessions.rs` — `SessionsView`. Holds `Rc<dyn SessionStore>`, `repo_root`, `state: LoadState<Vec<SessionInfo>>`. On `on_enter`, spawns a worker that calls `list_sessions(repo_root)`.
2. `Router::build(ViewId::Sessions)` wired up.
3. `apps/tui/src/main.rs` — replace stub with real `ClaudeSessionStore::discover()`.
4. Add a relative-time formatter ("2m", "1h", "3d") in `render_helpers.rs` — used by Sessions, Branches, Tags, and Home's activity feed.
5. Tests: stub store, render, sort order, LoadState transitions.

**Verify:** `just run`. Press `s`. Spinner briefly, then your Claude Code sessions for this repo, sorted newest-first.

**Crates touched:** `crates/view`, `apps/tui`.

---

### Phase F — Polish

#### Milestone 15: Command palette overlay

**You'll see:** Press `:` → centered overlay with a text input and a fuzzy-filtered list of commands. Type to filter, Enter to invoke. Esc closes.

**What to build:**
1. `crates/view/src/components/command_palette.rs` — `CommandPalette`. Holds a `TextInput` (from M9) and a static `Vec<PaletteCommand>`. Filters by subsequence match. Selecting a command emits the corresponding `Action`. **`wants_raw_keys` analog at App level:** when palette is `Some`, App routes events to the palette before checking nav keys (already handled by the dispatch order in M4).
2. The command list (v1):
   - Go to Home / Changes / Sessions / Search / Tags / Branches / Todos
   - Refresh
   - Quit
3. The palette is part of `App` state, not a View enum variant — it's an overlay on top of the current view, not a replacement.
4. Tests: filter behavior, command-to-action mapping.

**Verify:** `just run`. Press `:`. Type "sess". Pick "Go to Sessions". You're in Sessions.

**Crates touched:** `crates/view`.

---

#### Milestone 16: Activity feed in Home

**You'll see:** Home view now shows a real activity feed: stats in the title (X changed, Y sessions, Z tags), and a vertical list of recent activity with relative timestamps. Selecting an entry navigates to the right place.

**What to build:**
1. `crates/view/src/views/home.rs` (extended) — `ActivityFeedAggregator` struct in the same file. Holds `Rc` clones of every store. Has a `collect(limit)` method that pulls from each source, merges, sorts by `when` descending, truncates.
2. `HomeView` holds the aggregator and a `LoadState<Vec<ActivityEntry>>`. `on_enter` spawns a worker that calls `aggregator.collect(HOME_ACTIVITY_LIMIT)`.
3. The stats line in the title is computed from the loaded data (counts per `ActivityKind`).
4. Selection emits the contextual action based on `entry.target`:
   - `File { path }` → `Action::OpenFileAt { path, line: None }`
   - `Commit { sha }` → `Action::OpenCommit(sha)`
   - `Session { id: _ }` → `Action::NavigateTo(ViewId::Sessions)` (selection-into-list is a future plan)
   - `Tag { id: _ }` → `Action::NavigateTo(ViewId::Tags)`
   - `Todo { path, line }` → `Action::OpenFileAt { path, line: Some(line) }`
   - `None` → `Action::Noop`
5. **Aggregator worker design:** because the stores are `Rc<dyn …>` (not `Send`), the aggregator can't be sent into a thread. Two options:
   - **Option A (chosen):** the aggregator runs synchronously in the worker by constructing fresh impls inside the worker (the binary's `main.rs` would expose factory closures, or we accept a tight coupling and re-`discover()` each store inside the worker).
   - **Option B:** the aggregator runs synchronously on the main thread because individual store calls are usually fast (list_tags is a JSON read, recent_commits is a git call). Only sessions and todos are slow, and they're already shown in their dedicated views.
   - **For v1: Option B.** The Home aggregator runs *only* the fast sources (tags + recent commits + recent edited files via filesystem mtime walk). Sessions and todos are excluded from Home's activity feed in v1 and the stats line shows them as "(not loaded)". The user gets sessions/todos by navigating to those views directly. Documenting this trade-off honestly in M16 keeps the loading model simple.
6. Tests: aggregator with stub stores, sort order, target dispatch, stats counts.

**Verify:** `just run`. Land on Home. See real activity. Pick a tag → Tags. Pick a commit → CommitDiff. Pick an edit → opens the file. Stats are accurate (excluding sessions/todos counts).

**Crates touched:** `crates/view`.

---

#### Milestone 17: Polish, edge cases, final tests, decisions log

**You'll see:** Everything works smoothly together. Empty states are handled. Errors don't crash. `just check` is clean. `docs/decisions.md` updated.

**What to build:**
1. **Empty states:** every view renders a useful message when its data is empty. Home: "No recent activity". Sessions: "No Claude Code sessions for this repo". Tags: "No tags yet — press `m` while viewing a file to add one". Etc.
2. **Error states:** every view's `LoadState::Failed` renders through `ErrorBar`.
3. **Refresh:** `r` works in every view that has a refreshable data source. Wired through `View::on_enter` after a Refresh action.
4. **Navigation conflicts:** verify that `j`/`k` still scroll inside FileViewer and don't trigger global nav. Verify `t` doesn't fire while typing in Search.
5. **Status bar:** every view's hints + global nav hints render. On narrow terminals, drop the global hints.
6. **Performance check:** `T` (Todos) on a large repo (10k files) — the spinner should be smooth, the UI shouldn't freeze. If it does, that means the worker is starving the main thread and we have a bug.
7. **Theme audit:** new components use only theme tokens, no raw palette colors.
8. **Documentation:** update `README.md` with the new commands. Update `docs/decisions.md` with the architectural decisions made in this plan:
   - 2026-04-08: View enum (closed-set static dispatch) for top-level views
   - 2026-04-08: Rc<dyn Trait> for store injection (single-threaded design)
   - 2026-04-08: LoadState<T> + std::sync::mpsc for background data loading (no tokio)
   - 2026-04-08: Hierarchical Action enum, view-local handling for non-cross-cutting actions (Component Architecture)
   - 2026-04-08: Newtype IDs (TagId, SessionId, CommitSha) for compile-time type safety
   - 2026-04-08: tempfile::NamedTempFile::persist for atomic JSON writes
   - 2026-04-08: Three new crates (sessions, search, store) for dependency isolation
   - 2026-04-08: Removed Send + Sync from ChangeDetector and SyntaxHighlighter; GitChangeDetector uses RefCell<Repository>
   - 2026-04-08: View enum delegate methods use exhaustive match per variant (no `_ =>` wildcards) to preserve compile-time enforcement
   - 2026-04-08: Spinner advances every 6th poll tick (~10fps) via `TICKS_PER_FRAME` constant
   - 2026-04-08: status_hints returns `Cow<'_, [(&'static str, &'static str)]>` for state-dependent hints without breaking the trait
   - 2026-04-08: thiserror `#[from]` used for ergonomic `?` auto-conversion; `#[source]` reserved for variants needing explicit `.map_err()`
   - 2026-04-08: `Box<dyn Error + Send + Sync>` retained inside error variants — explicit exception to the single-threaded design rule
9. `just check` is clean (fmt + lint + test).

**Verify:** Use codepeek for a day on a real project. Try every view. Try edge cases (empty repo, repo with no sessions, large repo, narrow terminal). Nothing crashes; everything degrades gracefully.

**Crates touched:** `crates/view`, `apps/tui`, root docs.

---

### Milestone summary

| # | Phase | Name | What you'll see | Key crates |
|---|-------|------|----------------|------------|
| 1 | A | Crate scaffolding | 8 crates compile | sessions, search, store, root |
| 2 | A | Core types + drop Send+Sync | New types, RefCell repo | core, git |
| 3 | A | LoadState + Spinner | Internal infrastructure | view |
| 4 | A | View enum + Router refactor | Old UX preserved, new internals | view, tui |
| 5 | A | HomeView stub + cross-view nav | Lands on Home, `c`→Changes, `h`→Home | view |
| 6 | B | JsonTagStore (atomic via tempfile) | Tags persist (no UI yet) | store |
| 7 | B | TagsView + tagging | `m` marks, `t` lists, jump works | view, tui |
| 8 | C | search crate | File walker + TODO scanner tested | search |
| 9 | C | SearchView | `/` opens fuzzy file finder with spinner | view |
| 10 | C | TodosView | `T` lists all TODOs with spinner | view |
| 11 | D | CommitLog impl | git2 commits/branches via trait | git |
| 12 | D | BranchesView | `b` shows branches + commits | view |
| 13 | E | sessions crate | JSONL reader tested | sessions |
| 14 | E | SessionsView | `s` lists Claude sessions with spinner | view, tui |
| 15 | F | Command palette | `:` opens fuzzy command picker | view |
| 16 | F | Activity feed in Home | Home shows real merged activity | view |
| 17 | F | Polish + decisions log | Edge cases, empty states, docs | view, tui, docs |

**First runnable with new architecture:** Milestone 4 (refactor preserves existing UX)
**First user-visible new feature:** Milestone 5 (Home placeholder + nav)
**First "this looks like a real multi-view tool":** Milestone 7 (Tags)
**Background loading proven end-to-end:** Milestone 9 (Search with worker)
**Foundation complete:** Milestone 12 (4 of 6 new views shipped)
**Feature-complete v1:** Milestone 16 (activity feed lit up)
**Ship-ready:** Milestone 17 (polish + docs)

---

## What we're NOT doing

- **No `tokio` / no async runtime.** Background work is `std::thread::spawn` + `std::sync::mpsc`. If a future plan needs futures composition, cancellation tokens, or massive concurrency, *that* plan introduces tokio and migrates the workers.
- **No worker cancellation.** If the user navigates away mid-load, the worker finishes and drops its result on the floor (the receiver is dropped, the send fails silently). Acceptable because workers are short-lived (≤500ms).
- **No streaming results.** A worker either returns the full `Vec<T>` or fails. Partial results during a long scan would require a more complex channel protocol — out of scope.
- **No background refresh / file watchers.** Data refreshes only on view enter or explicit `r` press. No inotify, no polling.
- **No tab strip / multi-pane chrome.** Zen mode preserved. Command palette is the only persistent visual addition.
- **No view-local config files.** All new behavior reads from the existing `~/.config/codepeek/config.toml` if it needs configuration.
- **No runtime view registration.** The `View` enum is fixed at compile time. Plugin systems are not in scope.
- **No tag editing UI.** v1 lets you add and remove tags but not edit notes — edit by remove + re-add.
- **No session launching.** SessionsView is read-only.
- **No commit graph visualization.** BranchesView is a flat list.
- **No fuzzy match library** (e.g. `nucleo`). Subsequence matching is good enough for v1.
- **No new keybindings UI.** Keys are hardcoded in `keybindings.rs`.
- **No cursor mode in FileViewer.** Tagging uses "the line at the top of visible scroll" as the current line.
- **No command palette command history.** Every `:` opens fresh.
- **No `enum_dispatch` macro.** Manual `match` boilerplate in `views.rs` is clear and adds no dependency.
- **Sessions and Todos are NOT included in Home's activity feed in v1** (the aggregator runs synchronously on the main thread and only pulls from fast sources). The user gets sessions/todos via direct navigation (`s`/`T`). Adding them to Home requires an async aggregator pattern — its own follow-up plan.

---

## Architectural decisions (resolved during v2 review against the rust-style-guide notebook)

These were Open Decisions in the v1 plan that the notebook resolved:

1. **`Box<dyn View>` vs enum-dispatch:** Resolved → `enum View`. The set is closed; exhaustive matches and zero-allocation navigation win.
2. **`Arc` vs `Rc` for store injection:** Resolved → `Rc`. Single-threaded design, Clippy `arc_with_non_send_sync` would flag the alternative.
3. **`Mutex` vs `RefCell` for `GitChangeDetector` interior mutability:** Resolved → `RefCell`. Same reasoning. Drop `Send + Sync` from `ChangeDetector` and `SyntaxHighlighter` traits.
4. **Hand-rolled `<path>.tmp` + rename vs `tempfile`:** Resolved → `tempfile::NamedTempFile::persist()`. Crash safety, race safety, less code.
5. **Synchronous I/O on view enter vs background loading:** Resolved → `LoadState<T>` + `mpsc::channel` + `std::thread::spawn` from M3 onward. Synchronous would freeze the render loop on real repos.
6. **Flat Action enum vs hierarchical / Component Architecture:** Resolved → Component Architecture. Strip the global enum to cross-cutting concerns; each view handles its own state mutations internally.
7. **`Vec<(&'static str, &'static str)>` vs `&'static [...]` vs `Cow<'_, [...]>` for status hints:** Resolved → `Cow<'_, [(&'static str, &'static str)]>`. Strictly more general than a bare static slice while preserving the zero-allocation common case (`Cow::Borrowed(STATIC_HINTS)`). Future views with state-dependent hints (FileViewer's diff toggle, Tags' selection-aware "remove" hint) return `Cow::Owned(vec![…])` without breaking the trait signature.
8. **Positional `add_tag(path, line, kind, note)` vs struct input:** Resolved → `add_tag(NewTag<'_>)`. Future-proof against new fields.
9. **Raw `u64` / `String` IDs vs newtype wrappers:** Resolved → newtypes (`TagId`, `SessionId`, `CommitSha`). Zero runtime cost, prevents parameter mix-ups.
10. **`views/mod.rs` vs `views.rs` sibling:** Resolved → sibling file (`views.rs` + `views/`). Modern Rust 2018+ idiom, matches Ratatui's component template.
11. **`wants_raw_keys() -> bool` flag:** Validated as idiomatic — standard opt-in capability pattern with default impl.
12. **Per-domain `thiserror` enums:** Validated. Avoid "ball of mud," descriptive names (not all `Error`).
13. **Three new crates (sessions, search, store):** Validated. Aligns with "Prefer Small Crates" pattern and Ratatui's own architecture.
14. **Delegate-method pattern in `View` enum:** Resolved → **Pattern B** (exhaustive match per variant, no `_ =>` wildcards). Enum dispatch is only worth the boilerplate if it preserves exhaustiveness; wildcard arms throw away the compile-time safety the enum was chosen for. ~30 extra lines of trivial method definitions per view, paid back the first time a future view needs to opt into raw-key mode.
15. **Spinner frame rate cap:** Resolved → `TICKS_PER_FRAME = 6` constant in `spinner.rs`. `LoadState::poll` runs at ~60fps, but the spinner advances every 6th tick (~10fps) to match conventional loading-indicator pacing. Tweakable in one place.
16. **`#[from]` vs `#[source]` on error variants:** Resolved → use `#[from]` wherever the `?` operator should auto-convert; reserve `#[source]` for variants that need a `.map_err()` call site (typically when two variants wrap the same source type with different context).
17. **`Box<dyn Error + Send + Sync>` inside error chains:** Resolved → kept as-is. The bound is free (every wrapped type is already `Send + Sync`), it matches Rust ecosystem convention, and it future-proofs error propagation through `mpsc::channel`. This is the one explicit exception to the "drop `Send + Sync`" rule.

---

## Open decisions (still need to resolve at implementation time)

1. **`enum_dispatch` macro adoption.** Manual `match` plumbing in `views.rs` is ~80 lines. If it becomes painful when adding views beyond v1, revisit. **Lean:** stay manual until view #10.

2. **`OpenCommit(sha)` rendering.** v1 reuses the existing diff plumbing for "current HEAD changes" view to render a past commit's diff. The user experience for this is rough — you're seeing the *commit's diff*, not the *file at that commit*. A future plan could add a proper `CommitView` that uses `read_at_commit` to show files at that commit. **Lean for v1:** ship the rough version, document the limitation.

3. **Tag store concurrency.** Two codepeek processes mutating the same JSON file would race. `tempfile::persist` is atomic per write, but the read-modify-write cycle isn't. **Lean:** v1 ignores it; document as a known limitation.

4. **Sessions: encode-path edge cases.** What if the repo path contains characters Claude encodes differently than `/` and `.`? Test against the user's actual `~/.claude/projects/` directory list during M13.

5. **Status bar global hints budget.** `h c s / t b T : q` is 9 hints, ~30 characters with separators. Threshold for showing them is "terminal width ≥ 120 cols". Tunable in `config.rs`.

6. **Empty Home behavior.** When the repo has no activity at all, show a "Welcome to codepeek" panel with one-line descriptions of each view + their nav keys. Cheap to add in M16.

7. **Should Search debounce keystrokes?** Each character press kicks off a worker. On a fast typist, that's 5+ workers in flight, all scanning the same repo. `ignore::Walk` is fast but not free. **Lean:** add a 50ms debounce timer if it shows up in performance testing in M17. v1 ships without debounce.

8. **`Rc<GitChangeDetector>` coercion ergonomics.** The "two `Rc`s, one allocation" pattern in main.rs feels awkward. Alternative: define a `pub trait GitOps: ChangeDetector + CommitLog {}` super-trait and have a single `Rc<dyn GitOps>`. Trade-off: super-trait adds an indirection for callers that only need one trait. **Lean:** ship the awkward pattern in M11; revisit if it shows up at more call sites.
