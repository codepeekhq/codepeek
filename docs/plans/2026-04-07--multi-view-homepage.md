# Multi-View Homepage

> **Revision history**
> - **v1** drafted 2026-04-07 — initial multi-view design.
> - **v2** revised 2026-04-08 after a deep review against the **rust-style-guide** notebook (Rc/RefCell, exhaustive match, newtype IDs, tempfile atomic writes, mpsc loading).
> - **v3** revised 2026-04-11 after a deep review against the **software-architecture-guide** notebook (Screaming Architecture, Bounded Contexts, Dependency Inversion, composition root placement, application layer) **plus** a second pass of the rust-style-guide notebook (CommitSha memory footprint, `#[non_exhaustive]` scoping, explicit Component Architecture naming).
>
> **v3 is a structural revision.** The user experience of every milestone is unchanged from v2, but the code is reorganized around bounded contexts with an explicit application layer. See **"What v3 changes vs v2"** near the top of the Architecture section for the full diff.

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
- **A Command/Query Bus.** The application layer in v3 exposes use cases directly. A command bus (with handler registration) is a future refactor if cross-cutting concerns like audit logging or command history appear.

---

## Architecture

### Driving principles

Three principles frame every architectural decision below:

1. **Screaming Architecture** — at the module level, the code organization reveals *what codepeek does*, not *which tools it uses*. Opening `crates/core/src/` shows `changes/`, `tags/`, `sessions/`, `search/`, `todos/`, `branches/` — the bounded contexts of a source code explorer — not `controllers/`, `repositories/`, `services/`.
2. **Dependency Inversion** — dependencies point strictly inward toward the domain core. Outer layers (infrastructure adapters, UI chrome) know about inner layers (ports, domain entities). Inner layers know *nothing* about outer layers. A crate-level graph enforces this with no cycles.
3. **Component Architecture (explicitly, not TEA)** — presentation follows ratatui's Component Architecture template, not The Elm Architecture. Each view encapsulates its own state and handlers; the central `Action` enum only carries cross-cutting concerns (navigation, quit, palette open, cross-view open). Per-view state mutations happen inline inside each view's `handle_event`. v3 names this pattern explicitly — v2 adopted it quietly.

### Layered view (concentric rings)

```
            ┌──────────────────────────────────────────────┐
            │               apps/tui (binary)              │
            │           COMPOSITION ROOT ONLY               │
            │   builds adapters → wraps in use cases →      │
            │   injects use cases into Router → runs App    │
            └───────────────────┬──────────────────────────┘
                                │
         ┌──────────────────────┴──────────────────────┐
         │                                             │
         ▼                                             ▼
   ┌──────────┐                            ┌─────────────────────┐
   │   view   │                            │  Infrastructure     │
   │  crate   │                            │  (5 adapter crates) │
   │          │                            │                     │
   │ Every    │                            │  git, syntax,       │
   │ ViewX    │                            │  store, sessions,   │
   │ holds    │                            │  search             │
   │ Rc<UC>   │                            │                     │
   │ only     │                            │  Each impls one or  │
   │          │                            │  more domain ports  │
   └────┬─────┘                            └──────────┬──────────┘
        │                                             │
        │     BOTH sides depend on the inner core     │
        ▼                                             ▼
   ┌─────────────────────────────────────────────────────────┐
   │                      core crate                         │
   │                                                         │
   │  ┌──────────────┐   APPLICATION LAYER                   │
   │  │ Use cases    │   (orchestrate the domain)            │
   │  │ per context  │                                       │
   │  └──────┬───────┘                                       │
   │         │ depends on                                    │
   │         ▼                                               │
   │  ┌──────────────┐   DOMAIN LAYER                        │
   │  │ Entities +   │   (pure business types)               │
   │  │ Value Obj +  │                                       │
   │  │ Ports        │   (trait definitions)                 │
   │  │ per context  │                                       │
   │  └──────────────┘                                       │
   │                                                         │
   │  ┌──────────────┐   SHARED KERNEL                       │
   │  │ HighlightXxx │   (minimal cross-context types)       │
   │  │ ActivityXxx  │                                       │
   │  └──────────────┘                                       │
   │                                                         │
   │   core has zero Rust dependencies beyond thiserror,     │
   │   serde (for ID round-trip), and std. No ratatui,       │
   │   no git2, no tree-sitter, no ignore, no regex, no dirs.│
   └─────────────────────────────────────────────────────────┘
                  ↑
         All arrows point inward.
      Domain knows nothing about Application.
      Application knows nothing about Infra.
     Infra and UI know about Application + Domain.
      Binary knows about everything and wires it together.
```

### Bounded contexts

Each bounded context is a self-contained slice of the domain. Adding a new context is additive — it does not force changes to the existing ones. The contexts for codepeek:

```
┌───────────────┐ ┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│   changes     │ │    tags       │ │   sessions    │ │   branches    │
├───────────────┤ ├───────────────┤ ├───────────────┤ ├───────────────┤
│ entities:     │ │ entities:     │ │ entities:     │ │ entities:     │
│  FileChange   │ │  Tag          │ │  SessionInfo  │ │  BranchInfo   │
│  ChangeKind   │ │  NewTag<'a>   │ │               │ │  CommitInfo   │
│  DiffHunk     │ │               │ │ value objs:   │ │               │
│  DiffLine     │ │ value objs:   │ │  SessionId    │ │ value objs:   │
│  LineChange   │ │  TagId        │ │               │ │  CommitSha    │
│  ChangeMap    │ │  TagKind      │ │ port:         │ │               │
│               │ │               │ │  SessionStore │ │ port:         │
│ port:         │ │ port:         │ │ use cases:    │ │  CommitLog    │
│  ChangeDetec. │ │  TagStore     │ │  ListRepoSes. │ │ use cases:    │
│ use cases:    │ │ use cases:    │ │               │ │  ListBranches │
│  RefreshCh.   │ │  AddTag       │ │               │ │  RecentComm.  │
│  OpenChgFile  │ │  ListTags     │ │               │ │  ReadAtCommit │
│  PeekDeleted  │ │  RemoveTag    │ │               │ │               │
└───────────────┘ └───────────────┘ └───────────────┘ └───────────────┘

┌───────────────┐ ┌───────────────┐
│    search     │ │    todos      │
├───────────────┤ ├───────────────┤
│ port:         │ │ entities:     │
│  FileSearcher │ │  TodoItem     │
│ use cases:    │ │  TodoKind     │
│  FindFilesBy. │ │ port:         │
│               │ │  TodoScanner  │
│               │ │ use cases:    │
│               │ │  ScanRepoTod. │
└───────────────┘ └───────────────┘

          ┌────────────────────────────────┐
          │        SHARED KERNEL           │
          ├────────────────────────────────┤
          │  HighlightedLine               │
          │  HighlightSpan                 │
          │  HighlightKind                 │
          │  SyntaxHighlighter (trait)     │   — cross-cutting render port
          │  SyntaxError                   │
          │  ActivityEntry                 │   — cross-context read aggregator type
          │  ActivityKind                  │
          │  ActivityTarget                │
          └────────────────────────────────┘
```

Rules for what belongs in the Shared Kernel:
- **Only what must be shared** across multiple bounded contexts
- **Only minimal types** — no entities with their own business rules
- **The `SyntaxHighlighter` port is exceptionally kernel-level** because every view that displays source code uses it and it's not owned by any single context (if we put it in `changes`, `tags`/`search`/`todos` would have to import from `changes` for a completely unrelated concern).
- **`ActivityEntry` and friends live in the kernel** because `home` reads them from every context and needs a shared shape; putting them in `home` would force every context to depend on `home` (violating the acyclic rule).

### Crate structure (v3 — same crate count as v2, internally reorganized)

The Rust notebook's "prefer small crates" and the architecture notebook's "package by component" pull in two directions. Small crates help compile time and API stability. Package-by-component helps cognitive load and Screaming Architecture. For codepeek's current scale, **v3 keeps the v2 crate count roughly the same but reorganizes modules inside each crate by bounded context**. The new external adapters (store, sessions, search) are still their own crates because their external dependencies are distinct.

```
apps/
  tui/                    COMPOSITION ROOT (thin binary)
    src/
      main.rs             build adapters → build use cases → build Router → run App
      app.rs              App struct + run loop + event dispatch
      router.rs           Router::build(ViewId) → View
      views_enum.rs       pub enum View { Home, Changes, … } + exhaustive delegates
      stubs.rs             NullTagStore, NullSessionStore fallbacks for graceful degradation
      config.rs           AppConfig TOML loader (existing)

crates/
  core/                   DOMAIN + APPLICATION + SHARED KERNEL
    src/
      lib.rs              top-level re-exports
      kernel/             Shared Kernel — cross-context primitives
        mod.rs
        highlight.rs      HighlightedLine, HighlightSpan, HighlightKind
        activity.rs       ActivityEntry, ActivityKind, ActivityTarget
      syntax/             Cross-cutting rendering port
        mod.rs
        port.rs           trait SyntaxHighlighter { highlight(&mut self, …) }
        error.rs          SyntaxError
      changes/            Bounded context: Changes
        mod.rs
        domain.rs         FileChange, ChangeKind, DiffHunk, DiffLine, LineChange, ChangeMap
        port.rs           trait ChangeDetector
        error.rs          ChangeError
        app.rs            RefreshChangesUseCase, OpenChangedFileUseCase, PeekDeletedFileUseCase
      tags/               Bounded context: Tags
        mod.rs
        domain.rs         Tag, TagId, TagKind, NewTag<'a>
        port.rs           trait TagStore
        error.rs          TagError
        app.rs            AddTagUseCase, ListTagsUseCase, RemoveTagUseCase
      sessions/           Bounded context: Sessions
        mod.rs
        domain.rs         SessionInfo, SessionId
        port.rs           trait SessionStore
        error.rs          SessionError
        app.rs            ListRepoSessionsUseCase
      search/             Bounded context: Search
        mod.rs
        domain.rs         FileMatch (newtype: path + score)
        port.rs           trait FileSearcher
        error.rs          SearchError
        app.rs            FindFilesByQueryUseCase
      todos/              Bounded context: Todos
        mod.rs
        domain.rs         TodoItem, TodoKind
        port.rs           trait TodoScanner
        error.rs          TodoError
        app.rs            ScanRepoTodosUseCase
      branches/           Bounded context: Branches
        mod.rs
        domain.rs         CommitInfo, CommitSha, BranchInfo
        port.rs           trait CommitLog
        error.rs          BranchError
        app.rs            ListBranchesUseCase, RecentCommitsUseCase, ReadAtCommitUseCase

  git/                    INFRASTRUCTURE ADAPTER (git2 → changes + branches ports)
    src/
      lib.rs
      detector.rs         GitChangeDetector — impls both ChangeDetector AND CommitLog
                          (one struct, two trait impls; the crate's only export)

  syntax/                 INFRASTRUCTURE ADAPTER (tree-sitter → SyntaxHighlighter port)
    src/
      (existing structure unchanged)

  store/                  (NEW) INFRASTRUCTURE ADAPTER (JSON file → TagStore port)
    src/
      lib.rs
      json_tag_store.rs   JsonTagStore — impls TagStore
      file.rs             TagFile (the on-disk shape)

  sessions/               (NEW) INFRASTRUCTURE ADAPTER (Claude JSONL → SessionStore port)
    src/
      lib.rs
      claude_store.rs     ClaudeSessionStore — impls SessionStore
      jsonl.rs            streaming line reader + minimal parse helpers

  search/                 (NEW) INFRASTRUCTURE ADAPTER (ignore crate → FileSearcher + TodoScanner)
    src/
      lib.rs
      ripgrep_like.rs     RipgrepLikeSearcher — impls FileSearcher
      todo_scanner.rs     TodoCommentScanner — impls TodoScanner
      walk.rs             shared WalkBuilder configuration

  view/                   PRESENTATION — chrome + per-context views
    src/
      lib.rs              re-exports only what apps/tui needs
      chrome/             Shared UI framework, no context-specific logic
        mod.rs
        theme.rs          Palette, Theme, semantic tokens (unchanged from v2)
        layout.rs         ZenLayout helpers (extended for Home)
        loading.rs        LoadState<T>, mpsc channel helpers
        render_helpers.rs (unchanged)
        keybindings.rs    (extended with nav_target, is_palette, is_mark_issue, is_mark_fix)
        action.rs         Action enum — cross-cutting concerns only
        components.rs     re-exports
        components/
          status_bar.rs
          error_bar.rs
          peek_overlay.rs
          spinner.rs         (NEW)
          text_input.rs      (NEW)
          command_palette.rs (NEW)
          file_list.rs       (the FileList widget — used by changes view)
          file_viewer.rs     (the FileViewer widget — used by changes + file_viewer views)
      views.rs            sibling file for views/
      views/
        mod.rs            (implicit via sibling file)
        home.rs           HomeView + ActivityFeedAggregator (reads every context)
        changes.rs        ChangesView (orchestrates FileList + FileViewer + PeekOverlay)
        tags.rs           TagsView
        sessions.rs       SessionsView
        search.rs         SearchView
        todos.rs          TodosView
        branches.rs       BranchesView
        file_viewer.rs    FileViewerView (the top-level view, distinct from the component)
```

Crate count: **1 binary + 8 libraries = 9**, same as v2 (`core`, `git`, `syntax`, `view` today + `store`, `sessions`, `search` new). v3 doesn't add any crates; it reshapes what's *inside* them.

### Dependency graph (crate-level, v3)

```
                       ┌────────┐
                       │apps/tui│
                       └────┬───┘
                            │
         ┌──────┬──────┬────┼────┬──────┬──────┐
         │      │      │    │    │      │      │
         ▼      ▼      ▼    ▼    ▼      ▼      ▼
      ┌────┐ ┌────┐ ┌────┐┌────┐┌────┐┌────┐┌────┐
      │view│ │git │ │synt││stor││sess││sear│ (infra adapters)
      └──┬─┘ └──┬─┘ └──┬─┘└──┬─┘└──┬─┘└──┬─┘
         │      │      │    │    │    │
         └──────┴──────┴────┼────┴────┴────┐
                            │              │
                            ▼              ▼
                         ┌────┐          ┌────┐
                         │core│          │core│  (same crate)
                         └────┘          └────┘
```

Enforced rules:
- **`core` depends on nothing codepeek-specific.** Only `thiserror`, `serde`, `std`.
- **Each infra adapter crate (`git`, `syntax`, `store`, `sessions`, `search`) depends on `core` only.** It does NOT depend on `view`. It does NOT depend on other infra crates.
- **`view` depends on `core` only.** It does NOT depend on any infra crate. (The view layer talks to adapters exclusively through the ports defined in `core`, and receives concrete adapter instances only via the Router wiring in `apps/tui`.)
- **`apps/tui` depends on everything.** It is the only place that knows which concrete adapter implements which port, and the only place that constructs use cases.

No cycles are possible. Every crate has a single stable direction of import. **v3 adds a workspace check script** (`scripts/check_dep_graph.sh`) to M1 that parses each `Cargo.toml` and asserts these rules so they cannot silently drift.

### Dependency graph (module-level, inside `core`)

```
                  apps/tui / view
                        │ (imports use cases)
                        ▼
    core::{changes,tags,sessions,search,todos,branches}::app
                        │ (uses)
                        ▼
    core::{changes,tags,sessions,search,todos,branches}::{port,domain,error}
                        │ (uses)
                        ▼
                 core::kernel::{highlight,activity}
                 core::syntax::{port,error}
```

Inside each bounded context module, the layering is strict:
- `app` may import `port`, `domain`, `error` from its own context AND anything from `kernel`/`syntax`
- `port` may import `domain`, `error` from its own context AND anything from `kernel`
- `domain` may import only from `kernel`
- **No bounded context module imports from another bounded context module** — e.g. `core::tags::app` cannot import `core::changes::domain`. Cross-context coordination happens in the composition root (apps/tui) or in `home` which uses cross-context reads by design.

### What v3 changes vs v2

| Concern | v2 | v3 | Why |
|---|---|---|---|
| Crate count | 8 (core/git/syntax/view + 3 new) | 8 (same) | Rust notebook's "prefer small crates" still applies |
| `core` internal shape | Flat: `change.rs`, `diff.rs`, `traits.rs`, `error.rs`, … | Bounded-context modules: `changes/`, `tags/`, `sessions/`, … each with `domain.rs`/`port.rs`/`error.rs`/`app.rs` | Architecture guide: Screaming Architecture + Package by Component |
| Application layer | None — views call store traits directly | **NEW: use cases per context** (e.g. `AddTagUseCase`, `RefreshChangesUseCase`); views hold `Rc<UseCase>` not `Rc<dyn Store>` | Architecture guide: Application layer between view and domain is mandatory for mutations |
| Router location | `crates/view/src/router.rs` | `apps/tui/src/router.rs` | Architecture guide: composition root is in the binary, not in a library crate |
| App + View enum location | `crates/view/src/app.rs`, `views.rs` | `apps/tui/src/app.rs`, `views_enum.rs` | Same reason — the set of views is a composition decision |
| `view` crate responsibilities | App, Router, Views, chrome | Chrome + per-context View structs (no App, no Router, no View enum) | Clean separation of "what views exist" (binary) from "how views are built" (library) |
| Milestones | 17 technical phases (Foundation → Storage → Discovery → Git → Sessions → Polish) | **9 vertical slices** aligned to bounded contexts (Shell refactor → Tags → Search → Todos → Branches → Sessions → Home → Palette → Polish) | Architecture guide: prefer vertical slices; milestones deliver complete contexts, not technical layers |
| `#[non_exhaustive]` | "All types get it" | **Narrowed**: only on open enums (`ChangeKind`, `TagKind`, `TodoKind`, `ActivityKind`) and on `Theme` sub-structs. NOT on Data Objects (`FileChange`, `DiffHunk`, `Tag`, `TodoItem`, `CommitInfo`). | Rust notebook: `#[non_exhaustive]` on Data Objects harms readability by forcing `..` wildcards in callers |
| `CommitSha` type | `CommitSha(String)` | `CommitSha([u8; 20])` with hex encoding for display | Rust notebook: a Git SHA is exactly 20 bytes; String clones allocate |
| `SessionId` type | `SessionId(String)` | `SessionId(Rc<str>)` | Rust notebook: variable-length IDs clone frequently in cross-view dispatch; `Rc<str>` makes clone free |
| Presentation pattern naming | "Component Architecture pattern" (parenthetical) | **Explicitly "Component Architecture, not TEA"** — documented in `docs/decisions.md` and `chrome/action.rs` module doc | Rust notebook: per-view state mutation is a formal departure from TEA and deserves an explicit name |
| Use-case-vs-store in views | Views hold `Rc<dyn TagStore>` and call methods directly | Views hold `Rc<ListTagsUseCase>`, `Rc<AddTagUseCase>`, etc. Stores are invisible to the view. | Architecture guide: view never touches infra ports for mutations |
| Pure-read access from views | Same as mutation | **CQRS read-side:** Home's activity aggregator may read port methods directly (via a tiny `read` submodule). Mutations always go through a use case. | Architecture guide: CQRS allows the read side to skip the application layer for performance and simplicity |

Everything else from v2 is preserved: enum View with Pattern B exhaustive delegates, LoadState<T> + mpsc background loading, Rc over Arc, RefCell over Mutex, drop Send+Sync from traits, `Box<dyn Error + Send + Sync>` retained inside error sources, `tempfile::NamedTempFile::persist` for atomic JSON writes, `Cow<'_, [(&'static str, &'static str)]>` for status hints, `TICKS_PER_FRAME = 6` for spinner pacing, newtype IDs for compile-time type safety, NewTag<'a> input struct, `#[from]` preferred over `#[source]` for ergonomic `?`.

### Single-threaded design — `Rc` over `Arc`, no `Send + Sync` bounds (preserved from v2)

Codepeek runs a single render+event loop on the main thread. The only multi-threaded code introduced by this plan is **short-lived background worker threads** (see "Background data loading" below). Those workers don't share trait objects with the main thread — they hand back owned `T` values via `mpsc::channel`.

Therefore:

- **Shared use-case ownership uses `Rc<UseCase>`, not `Arc<UseCase>`.** Multiple views can hold a clone of the same `Rc<AddTagUseCase>` without atomic refcount overhead. (`Box<UseCase>` is wrong here because exclusive ownership prevents sharing.)
- **Use cases internally hold `Rc<dyn Trait>` for their ports.** E.g. `AddTagUseCase { store: Rc<dyn TagStore> }`. Again, not `Arc` — single-threaded.
- **Interior mutability uses `Rc<RefCell<T>>`, not `Arc<Mutex<T>>`.** Specifically: the `SyntaxHighlighter` adapter takes `&mut self` (tree-sitter requires it) and is held as `Rc<RefCell<dyn SyntaxHighlighter>>`. The `GitChangeDetector` becomes `RefCell<Repository>` internally.
- **None of the new traits have `: Send + Sync` bounds.** `TagStore`, `SessionStore`, `TodoScanner`, `FileSearcher`, `CommitLog` are all `pub trait Foo { … }` — no thread-safety constraint.
- **Worker threads consume owned values, not trait objects.** When SessionsView spawns a background scan, the worker is a `move` closure that owns a fresh `ClaudeSessionStore` value (cheap to construct), not a clone of the view's `Rc`. This sidesteps `Send` requirements entirely.

The existing `ChangeDetector` and `SyntaxHighlighter` traits in `core` are currently declared `Send + Sync`. Those bounds were added speculatively in the original change-viewer plan. They're harmless today (the `git2::Repository` inside `GitChangeDetector` is wrapped in a `Mutex` to satisfy `Sync`) but they don't pull their weight. **This plan removes those bounds in M1** and replaces the `Mutex` inside `GitChangeDetector` with a `RefCell`. If a future plan needs to ship work to a worker thread, it can construct a fresh detector inside the worker the same way SessionsView does.

**Independent validation:** the rust-style-guide notebook explicitly endorsed this pattern in the v3 review — Clippy's `arc_with_non_send_sync`, `rc_mutex`, and `mutex_atomic` would all flag the multi-threaded-primitive-in-single-threaded-code anti-pattern. The `move`-owned-data worker pattern "cleanly isolates your single-threaded UI from your asynchronous I/O."

### Static dispatch for views — `enum View`, not `Box<dyn View>` (preserved from v2)

The set of top-level views is **closed and known at compile time**: Home, Changes, Sessions, Search, Tags, Branches, Todos, FileViewer. New views require a code change. There's no plugin system, no runtime registration.

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

- **Exhaustive matches.** The compiler forces every match on `View` to handle every variant. (Aligns with the Rust style guide's "Prefer exhaustive matches" rule.)
- **No heap allocation on navigation.** Each variant carries its concrete type inline; navigation is `self.current = View::Sessions(SessionsView::new(…))`, no `Box::new`.
- **Compiler inlining.** `match` over an enum lets the compiler inline render/update logic per-variant.
- **No object-safety constraints.** Associated constants, generic methods, etc. are available on individual variant types.

The cost is the `views_enum.rs` file in `apps/tui` with one line per variant per delegate method. See "Delegate pattern: exhaustive match per variant, no wildcards" below for why v3 still uses Pattern B.

**v3 placement note:** the `View` enum lives in `apps/tui/src/views_enum.rs`, not in `crates/view`. Rationale: the set of views is a composition decision (which views exist, which use cases each view needs) that belongs in the composition root. The per-context view *structs* (`HomeView`, `ChangesView`, …) still live in `crates/view/src/views/*.rs`; the enum that aggregates them lives in the binary that decides which set of views to assemble.

### Background data loading — `mpsc` channels and worker threads (preserved from v2)

Walking 100+ session JSONL files, scanning a workdir for TODO comments, or asking `git2` for the recent commit log can each take 100–500ms on a real repo. The render loop has a 16ms budget per frame (for 60fps); blocking on these calls inside `View::on_enter` would freeze the UI.

**The pattern:** every view that does I/O has a `LoadState<T>` field rather than holding `T` directly:

```rust
// crates/view/src/chrome/loading.rs
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

1. `View::on_enter` is called when the user navigates to the view. It transitions `Idle → Loading { rx }` and `std::thread::spawn`s a worker that owns whatever it needs.
2. The App run loop calls `View::poll_loading(&mut self)` once per tick (between `draw` and `handle_events`). `poll_loading` does a non-blocking `rx.try_recv()`. If a result has arrived, transition to `Ready(t)` or `Failed(msg)`. If still pending, increment `spinner_tick` so the spinner animates.
3. `View::render` checks `LoadState` and renders the appropriate state.

**Why `std::thread::spawn` and not `tokio::spawn`?**
- No new dependency, no runtime to set up
- The work is one-shot per view enter, not a long-lived async task
- The worker only needs to send one message back; we don't need futures composition
- `std::sync::mpsc::Receiver::try_recv` is exactly the non-blocking poll we need
- Adding `tokio` would force every other crate's traits into `async fn` and balloon the plan

**v3 note on the application layer:** the use cases sit between views and workers. A view's `on_enter` calls a use case synchronously on the worker thread:

```rust
// Inside SessionsView::on_enter (simplified)
let (tx, rx) = mpsc::channel();
let repo = self.repo_root.clone();
let use_case = self.list_use_case.clone();  // Rc<ListRepoSessionsUseCase> — NOT Send
std::thread::spawn(move || {
    // Can't send the Rc<UseCase> into the worker because Rc is !Send.
    // Instead, the worker constructs a fresh ClaudeSessionStore and wraps it
    // in a fresh use case that lives entirely on the worker thread.
    let store = ClaudeSessionStore::discover();
    let result = store
        .and_then(|s| ListRepoSessionsUseCase::new(Rc::new(s)).execute(&repo))
        .map_err(|e| e.to_string());
    let _ = tx.send(result);
});
self.state = LoadState::Loading { rx, spinner_tick: 0 };
```

This works because:
- The worker constructs owned, thread-local `Rc`s that never cross the thread boundary
- The worker computes a `Result<Vec<SessionInfo>, String>` and sends only owned data back
- Neither `Rc<UseCase>` nor `Rc<dyn SessionStore>` is required to be `Send`
- The same pattern applies to every slow view (Sessions, Todos, Search, Branches)

**Trade-offs we're accepting:**
- **No streaming results.** A worker either returns the whole `Vec` or fails.
- **No cancellation.** If the user navigates away, the worker finishes and drops its message into a disconnected channel.
- **One worker per view at a time.** If the user mashes `r`, we don't queue extra workers — we return early if `LoadState::Loading`.

---

## Shared Kernel (`crates/core/src/kernel/`)

The minimal set of types that cross context boundaries. The rule: if you find yourself wanting to add something to `kernel`, first ask whether it belongs in exactly ONE context instead.

```rust
// crates/core/src/kernel/highlight.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    Keyword, Function, Type, String, Comment, Number,
    Operator, Variable, Punctuation, Constant, Property,
    Tag, Attribute,
}

// Data Object — NO #[non_exhaustive]. Every view pattern-matches on this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan { pub start: usize, pub end: usize, pub kind: HighlightKind }

#[derive(Debug, Clone)]
pub struct HighlightedLine { pub content: String, pub spans: Vec<HighlightSpan> }
```

```rust
// crates/core/src/kernel/activity.rs
use std::path::PathBuf;
use std::time::SystemTime;
use crate::branches::domain::CommitSha;   // wait — this would be a cross-context import!
use crate::tags::domain::TagId;
use crate::sessions::domain::SessionId;

// ActivityEntry must reference IDs from multiple bounded contexts. 
// Two design choices:
//
// (a) Use the concrete ID newtypes from each context as fields.
//     Pros: type-safe, zero-cost, matches exactly what Home needs.
//     Cons: kernel imports from context modules — inverts the layering.
//     Workaround: declare ActivityEntry INSIDE the context where it's 
//     "owned" (home). But home isn't its own crate in v3 — it's a view 
//     in view/views/home.rs, which can import every context freely.
//
// (b) Use opaque ID strings (ActivityTarget::Commit(String)).
//     Pros: kernel depends on nothing.
//     Cons: loses type safety at the exact boundary where Home dispatches
//     to another view; the dispatch code has to parse the opaque string.
//
// v3 chooses (a) but moves ActivityEntry OUT of the kernel and into 
// crates/view/src/views/home.rs. The kernel is not the right place for a 
// type that imports from every bounded context.
```

**Revision:** `ActivityEntry`, `ActivityKind`, `ActivityTarget` do **NOT** live in the Shared Kernel. They live in `crates/view/src/views/home.rs` because `home` is the one view that's allowed to know about every context. Moving them to kernel would invert the layering (kernel importing from contexts).

So the Shared Kernel shrinks to just:

```rust
// crates/core/src/kernel/mod.rs
pub mod highlight;
pub use highlight::{HighlightKind, HighlightSpan, HighlightedLine};
```

And the syntax port lives one level up, in its own module:

```rust
// crates/core/src/syntax/mod.rs
pub mod port;
pub mod error;
pub use port::SyntaxHighlighter;
pub use error::SyntaxError;
```

```rust
// crates/core/src/syntax/port.rs
use std::path::Path;
use crate::kernel::HighlightedLine;
use crate::syntax::error::SyntaxError;

pub trait SyntaxHighlighter {
    fn highlight(&mut self, source: &str, path: &Path)
        -> Result<Vec<HighlightedLine>, SyntaxError>;
}
```

Note no `Send + Sync` bound (v2 decision preserved, v3 confirmed).

---

## Bounded context: `changes`

### Domain (`crates/core/src/changes/domain.rs`)

```rust
use std::path::PathBuf;
use std::time::SystemTime;

// Data Object — NO #[non_exhaustive]. Views match all fields.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub kind: ChangeKind,
    pub mtime: SystemTime,
}

// Open enum — YES #[non_exhaustive]. Adding Untracked later should not break callers.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
    Unchanged,
}

// Data Objects — NO #[non_exhaustive]. 
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: LineChange,
    pub content: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineChange { Added, Removed, Modified }

#[derive(Debug, Clone, Default)]
pub struct ChangeMap {
    pub added: std::collections::HashSet<u32>,
    pub modified: std::collections::HashSet<u32>,
    pub deleted: Vec<u32>,
}
```

### Port (`crates/core/src/changes/port.rs`)

```rust
use std::path::Path;
use crate::changes::domain::{FileChange, DiffHunk};
use crate::changes::error::ChangeError;

pub trait ChangeDetector {
    fn detect_changes(&self) -> Result<Vec<FileChange>, ChangeError>;
    fn compute_diff(&self, path: &Path) -> Result<Vec<DiffHunk>, ChangeError>;
    fn read_at_head(&self, path: &Path) -> Result<String, ChangeError>;
}
```

No `Send + Sync` bound.

### Error (`crates/core/src/changes/error.rs`)

```rust
use std::path::PathBuf;

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
```

`Box<dyn Error + Send + Sync>` bounds are retained per v2 (confirmed by v3 rust review). The bound is free (every wrapped type is `Send + Sync` already) and future-proofs error propagation through `mpsc::channel`.

### Use cases (`crates/core/src/changes/app.rs`)

```rust
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::cell::RefCell;

use crate::changes::domain::{FileChange, ChangeKind, DiffHunk, ChangeMap};
use crate::changes::port::ChangeDetector;
use crate::changes::error::ChangeError;
use crate::kernel::HighlightedLine;
use crate::syntax::{SyntaxHighlighter, SyntaxError};

/// DTO returned by OpenChangedFileUseCase — the exact shape the view needs.
#[derive(Debug)]
pub struct OpenedFile {
    pub path: PathBuf,
    pub lines: Vec<HighlightedLine>,
    pub change_map: ChangeMap,
    pub diff_hunks: Vec<DiffHunk>,
}

/// Refresh the list of file changes. Pure read — no side effects.
pub struct RefreshChangesUseCase {
    detector: Rc<dyn ChangeDetector>,
}

impl RefreshChangesUseCase {
    pub fn new(detector: Rc<dyn ChangeDetector>) -> Self { Self { detector } }

    pub fn execute(&self) -> Result<Vec<FileChange>, ChangeError> {
        self.detector.detect_changes()
    }
}

/// Open a changed file: read bytes, highlight, compute diff, assemble DTO.
///
/// This orchestrates three operations that the view would otherwise have
/// to coordinate. Moving it here means the view just renders the DTO.
pub struct OpenChangedFileUseCase {
    detector: Rc<dyn ChangeDetector>,
    highlighter: Rc<RefCell<dyn SyntaxHighlighter>>,
}

impl OpenChangedFileUseCase {
    pub fn new(
        detector: Rc<dyn ChangeDetector>,
        highlighter: Rc<RefCell<dyn SyntaxHighlighter>>,
    ) -> Self {
        Self { detector, highlighter }
    }

    pub fn execute(&self, path: &Path, kind: &ChangeKind) -> Result<OpenedFile, ChangeError> {
        // 1. Read bytes (this is where binary detection would live; we let the
        //    view still do that because it decides how to render binary content)
        // 2. Highlight via self.highlighter.borrow_mut().highlight(...)
        // 3. Compute diff hunks
        // 4. Build ChangeMap from hunks; if Added, mark every line as added
        // 5. Return OpenedFile DTO
        //
        // The binary-content check stays in the view layer in v3 because it's 
        // about *how to render*, not *what the content means*. A future refactor 
        // could move it here as an OpenedFile::Binary variant.
        todo!()  // filled in during M2
    }
}

/// Read a deleted file's content at HEAD, highlighted, for the peek overlay.
pub struct PeekDeletedFileUseCase {
    detector: Rc<dyn ChangeDetector>,
    highlighter: Rc<RefCell<dyn SyntaxHighlighter>>,
}

impl PeekDeletedFileUseCase {
    pub fn new(
        detector: Rc<dyn ChangeDetector>,
        highlighter: Rc<RefCell<dyn SyntaxHighlighter>>,
    ) -> Self {
        Self { detector, highlighter }
    }

    pub fn execute(&self, path: &Path) -> Result<Vec<HighlightedLine>, ChangeError> {
        let content = self.detector.read_at_head(path)?;
        let highlighted = self
            .highlighter
            .borrow_mut()
            .highlight(&content, path)
            .unwrap_or_else(|_| {
                // Fallback: render raw lines with no syntax highlighting
                content
                    .lines()
                    .map(|l| HighlightedLine { content: l.to_string(), spans: vec![] })
                    .collect()
            });
        Ok(highlighted)
    }
}
```

Note the use case's `new` functions take `Rc<dyn Trait>` — they're dependency-injected by the composition root. The view takes `Rc<OpenChangedFileUseCase>`, which is strictly more specific than `Rc<dyn ChangeDetector>`.

### View struct (`crates/view/src/views/changes.rs`)

```rust
use std::rc::Rc;
use codepeek_core::changes::app::{
    RefreshChangesUseCase, OpenChangedFileUseCase, PeekDeletedFileUseCase
};

pub struct ChangesView {
    refresh_use_case: Rc<RefreshChangesUseCase>,
    open_use_case: Rc<OpenChangedFileUseCase>,
    peek_use_case: Rc<PeekDeletedFileUseCase>,
    file_list: crate::chrome::components::FileList,
    file_viewer: crate::chrome::components::FileViewer,
    peek_overlay: Option<crate::chrome::components::PeekOverlay>,
    focus: Focus,       // FileList | FileViewer — local private state
    error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus { FileList, FileViewer }
```

The view holds three `Rc<UseCase>` clones, not raw store traits. The view is completely ignorant of git2.

### Adapter (`crates/git/src/detector.rs`)

`GitChangeDetector` implements both `ChangeDetector` and `CommitLog` (the branches-context port). This is the one adapter that's allowed to know two bounded contexts — it's specifically the git2 binding that exposes git-backed behaviors to both.

```rust
use std::cell::RefCell;
use git2::Repository;

use codepeek_core::changes::port::ChangeDetector;
use codepeek_core::changes::domain::{FileChange, DiffHunk, /* … */};
use codepeek_core::changes::error::ChangeError;
use codepeek_core::branches::port::CommitLog;
use codepeek_core::branches::domain::{CommitInfo, BranchInfo, CommitSha};
use codepeek_core::branches::error::BranchError;

pub struct GitChangeDetector { repo: RefCell<Repository> }

impl GitChangeDetector {
    pub fn open(path: &Path) -> Result<Self, ChangeError> {
        let repo = Repository::discover(path)
            .map_err(|_| ChangeError::RepoNotFound { path: path.to_path_buf() })?;
        Ok(Self { repo: RefCell::new(repo) })
    }
}

impl ChangeDetector for GitChangeDetector { /* … */ }
impl CommitLog for GitChangeDetector { /* … */ }
```

One struct, two traits, one allocation. The binary coerces the same `Rc<GitChangeDetector>` to both trait objects.

---

## Bounded context: `tags`

### Domain (`crates/core/src/tags/domain.rs`)

```rust
use std::path::{Path, PathBuf};
use std::time::SystemTime;

// Opaque ID newtype — serialized as u64 in JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TagId(pub u64);

// Open enum — adding TagKind::Note later should not be breaking.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TagKind { Issue, Fix }

// Data Object — NO #[non_exhaustive].
#[derive(Debug, Clone)]
pub struct Tag {
    pub id: TagId,
    pub created_at: SystemTime,
    pub path: PathBuf,
    pub line: u32,
    pub kind: TagKind,
    pub note: String,
}

// Input struct — stable signature for TagStore::add_tag.
pub struct NewTag<'a> {
    pub path: &'a Path,
    pub line: u32,
    pub kind: TagKind,
    pub note: &'a str,
}
```

### Port (`crates/core/src/tags/port.rs`)

```rust
use crate::tags::domain::{Tag, TagId, NewTag};
use crate::tags::error::TagError;

pub trait TagStore {
    fn list_tags(&self) -> Result<Vec<Tag>, TagError>;
    fn add_tag(&self, new: NewTag<'_>) -> Result<Tag, TagError>;
    fn remove_tag(&self, id: TagId) -> Result<(), TagError>;
}
```

### Error (`crates/core/src/tags/error.rs`)

```rust
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum TagError {
    #[error("tag store path resolution failed")]
    NoConfigDir,

    #[error("failed to read tag store at {path}")]
    ReadFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write tag store at {path}")]
    WriteFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("tag store data is corrupt: {message}")]
    Corrupt { message: String },

    #[error("tag {id} not found")]
    NotFound { id: u64 },
}
```

(The old `StoreError` is renamed `TagError` because it's scoped to the tags context. If a future context needs its own store, it gets its own error type — no shared StoreError.)

### Use cases (`crates/core/src/tags/app.rs`)

```rust
use std::rc::Rc;

use crate::tags::domain::{Tag, TagId, NewTag};
use crate::tags::port::TagStore;
use crate::tags::error::TagError;

pub struct AddTagUseCase { store: Rc<dyn TagStore> }

impl AddTagUseCase {
    pub fn new(store: Rc<dyn TagStore>) -> Self { Self { store } }

    pub fn execute(&self, new: NewTag<'_>) -> Result<Tag, TagError> {
        // v1: forward to the store directly.
        // Future: emit a domain event, update an index, validate note length,
        // enforce per-file tag limits, etc.
        self.store.add_tag(new)
    }
}

pub struct ListTagsUseCase { store: Rc<dyn TagStore> }

impl ListTagsUseCase {
    pub fn new(store: Rc<dyn TagStore>) -> Self { Self { store } }

    pub fn execute(&self) -> Result<Vec<Tag>, TagError> {
        self.store.list_tags()
    }
}

pub struct RemoveTagUseCase { store: Rc<dyn TagStore> }

impl RemoveTagUseCase {
    pub fn new(store: Rc<dyn TagStore>) -> Self { Self { store } }

    pub fn execute(&self, id: TagId) -> Result<(), TagError> {
        self.store.remove_tag(id)
    }
}
```

Three tiny use cases per mutation/query. Each is trivial in v1 — but the indirection exists specifically so that future changes (validation, events, indexing, rate limiting) don't require touching any view. This is the architecture guide's point about the application layer being the orchestration seam.

### View struct (`crates/view/src/views/tags.rs`)

```rust
use std::rc::Rc;
use codepeek_core::tags::app::{ListTagsUseCase, RemoveTagUseCase};
use codepeek_core::tags::domain::Tag;

use crate::chrome::loading::LoadState;

pub struct TagsView {
    list_use_case: Rc<ListTagsUseCase>,
    remove_use_case: Rc<RemoveTagUseCase>,
    state: LoadState<Vec<Tag>>,
    selected: usize,
    error_message: Option<String>,
}
```

`FileViewerView` also holds `Rc<AddTagUseCase>` so pressing `m` while viewing a file adds a tag without going through a global Action. That's the Component Architecture in action.

### Adapter (`crates/store/src/json_tag_store.rs`)

```rust
use std::cell::RefCell;
use std::path::PathBuf;

use codepeek_core::tags::port::TagStore;
use codepeek_core::tags::domain::{Tag, TagId, NewTag};
use codepeek_core::tags::error::TagError;

pub struct JsonTagStore {
    path: PathBuf,
    inner: RefCell<TagFile>,
}

struct TagFile {
    version: u32,
    next_id: u64,
    tags: Vec<Tag>,
}

impl JsonTagStore {
    pub fn open() -> Result<Self, TagError> { /* resolve, load, or init */ }
}

impl TagStore for JsonTagStore {
    fn list_tags(&self) -> Result<Vec<Tag>, TagError> {
        Ok(self.inner.borrow().tags.clone())
    }

    fn add_tag(&self, new: NewTag<'_>) -> Result<Tag, TagError> { /* … */ }
    fn remove_tag(&self, id: TagId) -> Result<(), TagError> { /* … */ }
}

impl JsonTagStore {
    fn persist(&self, inner: &TagFile) -> Result<(), TagError> {
        // 1. Serialize inner to JSON string
        // 2. Resolve parent dir of self.path; create if missing
        // 3. let temp = tempfile::NamedTempFile::new_in(parent)?;
        // 4. Write JSON bytes via temp.as_file().write_all(...)?;
        // 5. temp.persist(&self.path)?;   // atomic rename
        todo!()
    }
}
```

**Atomic write via `tempfile`** is preserved from v2. Crash safety, race safety, less code to test.

---

## Bounded context: `branches`

### Domain (`crates/core/src/branches/domain.rs`)

```rust
use std::fmt;
use std::time::SystemTime;

/// A Git SHA is exactly 20 bytes (SHA-1). Storing it as a fixed byte array
/// eliminates heap allocation on every clone.
///
/// Serialization goes through hex encoding so the on-disk/wire format is
/// still a 40-character ASCII string, matching what users see in `git log`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommitSha([u8; 20]);

impl CommitSha {
    pub fn from_bytes(bytes: [u8; 20]) -> Self { Self(bytes) }

    pub fn from_hex(hex: &str) -> Result<Self, &'static str> {
        if hex.len() != 40 {
            return Err("git sha must be 40 hex chars");
        }
        let mut bytes = [0u8; 20];
        for i in 0..20 {
            bytes[i] = u8::from_str_radix(&hex[i*2..i*2+2], 16)
                .map_err(|_| "invalid hex char in git sha")?;
        }
        Ok(Self(bytes))
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// First 7 chars of hex for display.
    pub fn short(&self) -> String {
        self.0[..4].iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
            .chars().take(7).collect()
    }
}

impl fmt::Display for CommitSha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl serde::Serialize for CommitSha {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> serde::Deserialize<'de> for CommitSha {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        CommitSha::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

// Data Object.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub sha: CommitSha,
    pub author: String,
    pub when: SystemTime,
    pub summary: String,
}

// Data Object.
#[derive(Debug, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub upstream: Option<String>,
    pub head_sha: CommitSha,
}
```

The `CommitSha([u8; 20])` design is **a v3 correction from v2's `String`**. Every `Action::OpenCommit(sha)` dispatch now copies 20 bytes on the stack instead of allocating on the heap.

### Port, error, use cases

(Same shape as the tags context, omitted for brevity. See Milestone G below.)

---

## Bounded context: `sessions`

### Domain (`crates/core/src/sessions/domain.rs`)

```rust
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;

/// Claude Code session IDs are UUID-formatted strings of variable length.
/// Stored as Rc<str> so cloning into Actions and LoadState is free.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SessionId(pub Rc<str>);

impl SessionId {
    pub fn new(s: impl Into<Rc<str>>) -> Self { Self(s.into()) }
    pub fn as_str(&self) -> &str { &self.0 }
}

// Data Object.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: SessionId,
    pub started_at: SystemTime,
    pub last_active: SystemTime,
    pub message_count: usize,
    pub cwd: PathBuf,
    pub summary: Option<String>,
}
```

`SessionId` uses `Rc<str>` — a **v3 refinement from v2's `String`**. Per the Rust notebook: variable-length IDs that cross many boundaries should be reference-counted, not cloned string-by-string. `Rc<str>` clone is essentially a refcount bump.

**Concurrency caveat:** `Rc` is `!Send`. Since we pass `SessionId` through `Action::OpenSession(id)` on the main thread and through the `mpsc::channel` from a worker back to the main thread, a worker that needs to *return* a `SessionId` cannot use `Rc<str>` directly — it would need to send a `String` back and have the main thread wrap it in `Rc<str>` on receipt. v3 pattern: `SessionInfo` in the worker uses `SessionId(Rc<str>)` but the worker's `Result<Vec<SessionInfo>, …>` is received on the main thread after the worker has returned, at which point the `Rc` is valid because the worker's thread has ended and the ownership has moved. **This works because `mpsc::channel` transfers ownership of the `Vec<SessionInfo>` across the thread boundary via `send(T)` where `T: Send`.** But `Vec<SessionInfo>` is only `Send` if `SessionInfo` is `Send`, which it isn't if it contains `Rc<str>`. So we need to construct sessions with `Arc<str>` in the worker and convert to `Rc<str>` on receipt, or use `String` throughout.

**v3 decision:** use `String` for `SessionId` inside the worker and wrap into `Rc<str>` only when the main thread receives the `Vec<SessionInfo>` and is ready to dispatch through `Action::OpenSession(SessionId)`. But that's ugly — the worker has to know about two types. Simpler: **`SessionId` is `Rc<str>` and we never cross-thread `SessionInfo` directly**. Instead, the worker sends `Vec<RawSessionInfo>` where `RawSessionInfo` uses `String`, and the main thread's `View::poll_loading` converts `Vec<RawSessionInfo> → Vec<SessionInfo>`. One mapping pass on receive, O(n) with n tiny, acceptable.

Alternative considered and rejected: `SessionId(Arc<str>)`. Works across threads but requires atomic refcount ops. In a single-threaded UI this is wasted work. The `Rc` + receive-side conversion wins on hot-path cost.

**Final v3 answer:** inside `crates/core/src/sessions/domain.rs` we expose both:
- `RawSessionInfo` — cross-thread shape (`String` for id, `String` for cwd), `Send`
- `SessionInfo` — main-thread shape (`Rc<str>` for id), `!Send`
- `impl From<RawSessionInfo> for SessionInfo`

The worker produces `Vec<RawSessionInfo>`, sends via mpsc, main thread converts. One O(n) mapping pass on receive. Clean.

*(Honest caveat: this is more machinery than `SessionId(String)` everywhere. If the Rc optimization turns out to not matter in practice — profile in M15 — revert to `SessionId(String)` and delete `RawSessionInfo`. The v3 plan includes a task to measure this in the polish milestone.)*

---

## Bounded contexts: `search`, `todos`

(Domain/port/error/app structure parallels the above; adapters in `crates/search`. Details compressed in the milestone descriptions below.)

---

## Presentation layer

### Component Architecture, explicitly

The rust-style-guide notebook's v3 review was clear: by handling per-view state mutations inline inside `handle_event` rather than producing a central `Action` that a central `update` function consumes, codepeek is no longer pure TEA. It's the **Component Architecture** pattern from ratatui's template. v3 names it explicitly:

- `docs/decisions.md` gets an entry: *"2026-04-11: Presentation follows Component Architecture (ratatui template pattern), not pure TEA. Per-view state is private to each View; the central `Action` enum carries only cross-cutting concerns (navigation, quit, palette open, cross-view open). This is a deliberate departure from The Elm Architecture's central update function, made because cross-view mutations are rare and per-view state is high-cardinality (hundreds of small mutations per view)."*
- `crates/view/src/chrome/action.rs` gets a module doc comment referencing this decision.
- The view-development guide in `README.md` (added in M9) documents the pattern for future contributors.

The important distinction per the Rust notebook: **the Component Architecture is a valid scalable pattern** ("finer-grained approach to event handling, with each component only dealing with the events it's interested in") — the issue was that v2 adopted it implicitly without naming it. v3 fixes the documentation gap.

### The central `Action` enum (cross-cutting only)

```rust
// crates/view/src/chrome/action.rs

use std::path::PathBuf;
use codepeek_core::branches::domain::CommitSha;

//! Cross-cutting actions produced by views that require App-level handling.
//!
//! Per-view state mutations (selecting a file, scrolling a viewer, toggling
//! diff view, adding a tag) happen inline inside the view's handle_event.
//! They do not produce an Action variant.
//!
//! This is the Component Architecture pattern documented in the ratatui
//! template. See docs/decisions.md 2026-04-11.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Application lifecycle.
    Quit,
    /// Cross-view navigation.
    NavigateTo(crate::views::ViewId),
    /// Esc — pop the back-stack.
    Back,
    /// Refresh the current view's data source (re-trigger on_enter).
    Refresh,
    /// Open the command palette overlay.
    OpenPalette,
    /// Close the command palette overlay.
    ClosePalette,
    /// Cross-view: open a file in FileViewerView, optionally jumping to line.
    /// Used by Search, Tags, Todos, Home when the user picks a result.
    OpenFileAt { path: PathBuf, line: Option<u32> },
    /// Cross-view: open the file list of a commit.
    /// Used by Branches + Home when the user picks a commit.
    OpenCommit(CommitSha),
    /// Dismiss the deleted-file peek overlay.
    DismissPeek,
    /// No-op (key didn't match anything in the current context).
    Noop,
}
```

Gone from the global enum vs v1 plan:
- `SelectFile(usize)` — internal to `ChangesView`
- `ToggleDiff` — internal to `FileViewerView`
- `AddTag`/`RemoveTag` — internal to `FileViewerView` / `TagsView` (each holds the relevant use case)
- `PaletteCommand(…)` — internal to `CommandPalette`

### The View enum with Pattern B exhaustive delegates

Per v2, re-confirmed in v3: the `View` enum lives in `apps/tui/src/views_enum.rs`. Every delegate method is an exhaustive match — no `_ =>` wildcards. For 8 variants and 7 methods that's ~70 lines of trivial boilerplate. The compiler enforces every new view variant updates every delegate method, which is the whole reason we chose an enum over a trait object.

```rust
// apps/tui/src/views_enum.rs

use std::borrow::Cow;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use codepeek_view::chrome::action::Action;
use codepeek_view::chrome::theme::Theme;
use codepeek_view::views::{
    HomeView, ChangesView, SessionsView, SearchView, TagsView,
    BranchesView, TodosView, FileViewerView,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewId {
    Home, Changes, Sessions, Search, Tags, Branches, Todos, FileViewer,
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

    pub fn title(&self) -> Cow<'_, str> { /* exhaustive match */ }
    pub fn status_hints(&self) -> Cow<'_, [(&'static str, &'static str)]> { /* exhaustive */ }
    pub fn handle_event(&mut self, key: KeyEvent) -> Action { /* exhaustive */ }
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) { /* exhaustive */ }
    pub fn on_enter(&mut self) { /* exhaustive */ }
    pub fn poll_loading(&mut self) { /* exhaustive */ }
    pub fn wants_raw_keys(&self) -> bool { /* exhaustive */ }
}
```

### The Router in `apps/tui`

```rust
// apps/tui/src/router.rs

use std::path::PathBuf;
use std::rc::Rc;

use codepeek_core::branches::app::{ListBranchesUseCase, RecentCommitsUseCase, ReadAtCommitUseCase};
use codepeek_core::changes::app::{RefreshChangesUseCase, OpenChangedFileUseCase, PeekDeletedFileUseCase};
use codepeek_core::search::app::FindFilesByQueryUseCase;
use codepeek_core::sessions::app::ListRepoSessionsUseCase;
use codepeek_core::tags::app::{AddTagUseCase, ListTagsUseCase, RemoveTagUseCase};
use codepeek_core::todos::app::ScanRepoTodosUseCase;

use codepeek_view::views::{
    ChangesView, FileViewerView, HomeView, SearchView,
    SessionsView, TagsView, TodosView, BranchesView,
};

use crate::views_enum::{View, ViewId};

/// Every use case the app depends on. Built in main.rs from concrete adapters.
pub struct AppDeps {
    pub refresh_changes: Rc<RefreshChangesUseCase>,
    pub open_changed_file: Rc<OpenChangedFileUseCase>,
    pub peek_deleted_file: Rc<PeekDeletedFileUseCase>,
    pub add_tag: Rc<AddTagUseCase>,
    pub list_tags: Rc<ListTagsUseCase>,
    pub remove_tag: Rc<RemoveTagUseCase>,
    pub list_repo_sessions: Rc<ListRepoSessionsUseCase>,
    pub find_files: Rc<FindFilesByQueryUseCase>,
    pub scan_todos: Rc<ScanRepoTodosUseCase>,
    pub list_branches: Rc<ListBranchesUseCase>,
    pub recent_commits: Rc<RecentCommitsUseCase>,
    pub read_at_commit: Rc<ReadAtCommitUseCase>,
}

pub struct Router {
    deps: AppDeps,
    repo_root: PathBuf,
}

impl Router {
    pub fn new(deps: AppDeps, repo_root: PathBuf) -> Self {
        Self { deps, repo_root }
    }

    pub fn build(&self, id: ViewId) -> View {
        match id {
            ViewId::Home => View::Home(HomeView::new(/* aggregator: reads multiple use cases */)),
            ViewId::Changes => View::Changes(ChangesView::new(
                self.deps.refresh_changes.clone(),
                self.deps.open_changed_file.clone(),
                self.deps.peek_deleted_file.clone(),
            )),
            ViewId::Sessions => View::Sessions(SessionsView::new(
                self.deps.list_repo_sessions.clone(),
                self.repo_root.clone(),
            )),
            ViewId::Search => View::Search(SearchView::new(
                self.deps.find_files.clone(),
                self.repo_root.clone(),
            )),
            ViewId::Tags => View::Tags(TagsView::new(
                self.deps.list_tags.clone(),
                self.deps.remove_tag.clone(),
            )),
            ViewId::Branches => View::Branches(BranchesView::new(
                self.deps.list_branches.clone(),
                self.deps.recent_commits.clone(),
            )),
            ViewId::Todos => View::Todos(TodosView::new(
                self.deps.scan_todos.clone(),
                self.repo_root.clone(),
            )),
            ViewId::FileViewer => {
                // FileViewer is always built via build_file_viewer because it
                // needs a path + optional line, not just an id.
                panic!("FileViewer is built via build_file_viewer(), not build()")
            }
        }
    }

    pub fn build_file_viewer(&self, path: PathBuf, line: Option<u32>) -> View {
        View::FileViewer(FileViewerView::new(
            path,
            line,
            self.deps.open_changed_file.clone(),
            self.deps.add_tag.clone(),   // `m` adds a tag inline via the use case
        ))
    }
}
```

The Router is now:
- **In `apps/tui`** (composition root placement)
- Holding `Rc<UseCase>` only — never a raw port trait object
- Tiny: about 70 lines including the exhaustive match in `build`

### The App in `apps/tui`

```rust
// apps/tui/src/app.rs (skeleton)

use ratatui::DefaultTerminal;
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::{Margin, Rect};

use codepeek_view::chrome::action::Action;
use codepeek_view::chrome::components::{CommandPalette, ErrorBar, StatusBar};
use codepeek_view::chrome::{config, layout, theme};

use crate::router::Router;
use crate::views_enum::{View, ViewId};

pub struct App {
    should_quit: bool,
    router: Router,
    current: View,
    history: Vec<ViewId>,
    palette: Option<CommandPalette>,
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
            error_message: None,
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> std::io::Result<()> {
        while !self.should_quit {
            self.current.poll_loading();
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let theme = theme::current();
        let full = frame.area();
        let area = full.inner(Margin::new(config::OUTER_MARGIN, config::OUTER_MARGIN));

        // Delegate render to current view
        self.current.render(frame, area, theme);

        // Overlay palette if open
        if let Some(palette) = &self.palette {
            palette.render(frame, full, theme);
        }
    }

    fn handle_events(&mut self) -> std::io::Result<()> {
        // Dispatch order (unchanged from v2):
        // 1. Palette → palette handler
        // 2. Current view wants raw keys → view handler (skip global nav)
        // 3. Global nav key → App
        // 4. Otherwise → view handler
        // ...
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

---

## Composition root (`apps/tui/src/main.rs`)

```rust
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use codepeek_core::changes::app::{
    RefreshChangesUseCase, OpenChangedFileUseCase, PeekDeletedFileUseCase,
};
use codepeek_core::changes::port::ChangeDetector;
use codepeek_core::branches::app::{ListBranchesUseCase, RecentCommitsUseCase, ReadAtCommitUseCase};
use codepeek_core::branches::port::CommitLog;
use codepeek_core::search::app::FindFilesByQueryUseCase;
use codepeek_core::search::port::FileSearcher;
use codepeek_core::sessions::app::ListRepoSessionsUseCase;
use codepeek_core::sessions::port::SessionStore;
use codepeek_core::syntax::SyntaxHighlighter;
use codepeek_core::tags::app::{AddTagUseCase, ListTagsUseCase, RemoveTagUseCase};
use codepeek_core::tags::port::TagStore;
use codepeek_core::todos::app::ScanRepoTodosUseCase;
use codepeek_core::todos::port::TodoScanner;

use codepeek_git::GitChangeDetector;
use codepeek_search::{RipgrepLikeSearcher, TodoCommentScanner};
use codepeek_sessions::ClaudeSessionStore;
use codepeek_store::JsonTagStore;
use codepeek_syntax::TreeSitter;

use color_eyre::Result;

mod app;
mod config;
mod router;
mod stubs;
mod views_enum;

fn main() -> Result<()> {
    color_eyre::install()?;

    let (app_config, config_warning) = config::AppConfig::load();
    if let Some(warning) = config_warning {
        eprintln!("codepeek: {warning}");
    }

    let repo_root = std::env::current_dir()?;

    // ─── 1. Build infrastructure adapters ───────────────────────────────────
    let git = Rc::new(GitChangeDetector::open(&repo_root)?);
    let detector: Rc<dyn ChangeDetector> = git.clone();
    let commit_log: Rc<dyn CommitLog> = git;

    let highlighter: Rc<RefCell<dyn SyntaxHighlighter>> = Rc::new(RefCell::new(
        TreeSitter::with_languages(app_config.enabled_languages()),
    ));

    let session_store: Rc<dyn SessionStore> = match ClaudeSessionStore::discover() {
        Ok(s) => Rc::new(s),
        Err(e) => {
            eprintln!("codepeek: sessions disabled: {e}");
            Rc::new(stubs::NullSessionStore)
        }
    };

    let tag_store: Rc<dyn TagStore> = match JsonTagStore::open() {
        Ok(s) => Rc::new(s),
        Err(e) => {
            eprintln!("codepeek: tags disabled: {e}");
            Rc::new(stubs::NullTagStore)
        }
    };

    let file_searcher: Rc<dyn FileSearcher> = Rc::new(RipgrepLikeSearcher);
    let todo_scanner: Rc<dyn TodoScanner> = Rc::new(TodoCommentScanner);

    // ─── 2. Wrap adapters in application-layer use cases ────────────────────
    let deps = router::AppDeps {
        refresh_changes: Rc::new(RefreshChangesUseCase::new(detector.clone())),
        open_changed_file: Rc::new(OpenChangedFileUseCase::new(
            detector.clone(),
            highlighter.clone(),
        )),
        peek_deleted_file: Rc::new(PeekDeletedFileUseCase::new(
            detector.clone(),
            highlighter.clone(),
        )),
        add_tag: Rc::new(AddTagUseCase::new(tag_store.clone())),
        list_tags: Rc::new(ListTagsUseCase::new(tag_store.clone())),
        remove_tag: Rc::new(RemoveTagUseCase::new(tag_store.clone())),
        list_repo_sessions: Rc::new(ListRepoSessionsUseCase::new(session_store.clone())),
        find_files: Rc::new(FindFilesByQueryUseCase::new(file_searcher.clone())),
        scan_todos: Rc::new(ScanRepoTodosUseCase::new(todo_scanner.clone())),
        list_branches: Rc::new(ListBranchesUseCase::new(commit_log.clone())),
        recent_commits: Rc::new(RecentCommitsUseCase::new(commit_log.clone())),
        read_at_commit: Rc::new(ReadAtCommitUseCase::new(commit_log.clone())),
    };

    // ─── 3. Wire Router ─────────────────────────────────────────────────────
    let router = router::Router::new(deps, repo_root);

    // ─── 4. Run the App ─────────────────────────────────────────────────────
    let terminal = ratatui::init();
    let result = app::App::new(router).run(terminal);
    ratatui::restore();

    result?;
    Ok(())
}
```

The composition root has three clearly-separated phases:
1. **Build adapters** — instantiate concrete infrastructure
2. **Wrap in use cases** — lift adapters through the application layer
3. **Inject into the Router** — the Router holds use cases, never adapters

This is the pattern the architecture guide explicitly called out: *"The `tui` crate should instantiate the infrastructure adapters, inject them into the Application Services, and then inject those Application Services into the view's Router."*

### Null stubs (`apps/tui/src/stubs.rs`)

```rust
use std::path::Path;
use codepeek_core::sessions::port::SessionStore;
use codepeek_core::sessions::domain::SessionInfo;
use codepeek_core::sessions::error::SessionError;
use codepeek_core::tags::port::TagStore;
use codepeek_core::tags::domain::{Tag, TagId, NewTag};
use codepeek_core::tags::error::TagError;

pub struct NullSessionStore;

impl SessionStore for NullSessionStore {
    fn list_sessions(&self, _: &Path) -> Result<Vec<SessionInfo>, SessionError> {
        Ok(Vec::new())
    }
}

pub struct NullTagStore;

impl TagStore for NullTagStore {
    fn list_tags(&self) -> Result<Vec<Tag>, TagError> { Ok(Vec::new()) }

    fn add_tag(&self, _: NewTag<'_>) -> Result<Tag, TagError> {
        Err(TagError::WriteFailed {
            path: std::path::PathBuf::from("/dev/null"),
            source: std::io::Error::other("tag store unavailable"),
        })
    }

    fn remove_tag(&self, _: TagId) -> Result<(), TagError> { Ok(()) }
}
```

Stubs live in the binary, not in library crates. Library crates stay strict — only the binary provides graceful-degradation fallbacks.

---

## Rust-level refinements (v3 delta)

### `#[non_exhaustive]` — applied selectively

**Rust notebook v3 verdict:** applying `#[non_exhaustive]` to Data Objects actively harms readability by forcing `..` wildcards in every pattern match. Only use `#[non_exhaustive]` when adding a field/variant should be non-breaking AND callers don't routinely pattern-match on the type.

**v3 rules for codepeek:**
- ✅ Apply to open enums where a future variant is plausible:
  - `ChangeKind`, `TagKind`, `TodoKind`, `LineChange` (in domain.rs files)
  - `ActivityKind` (in home.rs — not kernel)
  - `Theme` sub-structs (`TextColors`, `BorderColors`, `ChangeColors`, `DiffColors`, `SyntaxColors`, `UiColors`) — already done per 2026-04-06 decision
- ❌ Do NOT apply to Data Objects where callers construct or pattern-match frequently:
  - `FileChange`, `DiffHunk`, `DiffLine`, `ChangeMap`
  - `Tag`, `SessionInfo`, `TodoItem`, `CommitInfo`, `BranchInfo`
  - `HighlightSpan`, `HighlightedLine`
  - `ActivityEntry`, `ActivityTarget` (home view matches all fields when dispatching)

**Revert v2:** the v2 plan said "All types get `#[non_exhaustive]` matching the existing theme structs." That's wrong — walk it back per the Rust notebook's v3 verdict.

### `CommitSha` is `[u8; 20]`, not `String`

Covered in the branches context above. Summary: Git SHA-1 is a 20-byte fixed-width hash; wrap it in `CommitSha([u8; 20])` to eliminate heap allocation on every `Action::OpenCommit(sha)` dispatch. Display via hex encoding; serde via hex string for on-disk/wire compatibility.

Performance difference: `Clone` goes from "allocate 40 chars + memcpy" to "memcpy 20 bytes". At a few dispatches per frame this is measurable.

### `SessionId` is `Rc<str>`, not `String`

Covered in the sessions context above. Summary: variable-length UUID-like IDs that cross many view boundaries benefit from refcounted sharing. Caveat: `Rc<str>` is `!Send`, so the worker thread produces `RawSessionInfo { id: String, … }` and the main thread converts to `SessionInfo { id: SessionId(Rc::from(s)), … }` on receive.

**Honest trade-off:** this adds a `RawSessionInfo` → `SessionInfo` conversion step. Measure first (M17), revert if the `String` baseline is fine for 100-session scale.

### Component Architecture — explicit documentation

Already covered. Summary: `docs/decisions.md` gets a formal entry, `chrome/action.rs` gets a module doc comment, `README.md` gets a view-dev guide in M9.

### Everything else from v2's Rust decisions is preserved

- Drop `Send + Sync` from `ChangeDetector` and `SyntaxHighlighter`
- `Box<dyn Error + Send + Sync>` retained inside error source fields
- `Mutex<Repository>` → `RefCell<Repository>`
- `Rc<UseCase>` for shared use-case ownership; `Rc<dyn Trait>` inside use cases for port injection
- `Rc<RefCell<dyn SyntaxHighlighter>>` for the one shared-mutable port (acceptable per v3 rust review; revisit with `RenderContext<'a>` pattern if runtime borrow-check panics become a real problem — see Open Decisions)
- `tempfile::NamedTempFile::persist()` for atomic JSON writes
- `thiserror` `#[from]` preferred over `#[source]` for ergonomic `?`; `#[source]` when two variants wrap the same type
- `Cow<'_, [(&'static str, &'static str)]>` for status_hints
- `enum View` with Pattern B exhaustive delegates (no `_ =>` wildcards)
- `TICKS_PER_FRAME = 6` for the spinner
- Newtype IDs (revised to type-appropriate backing per above)
- `NewTag<'a>` input struct for stable trait signature

---

## Sequence diagrams for key flows

### Flow 1: Add a tag while viewing a file

```
User          App      FileViewerView    AddTagUseCase    TagStore (adapter)    File system
 │             │            │                  │                │                    │
 │ press 'm'   │            │                  │                │                    │
 ├─────────────▶            │                  │                │                    │
 │             │ handle_event(m)                │                │                    │
 │             ├────────────▶                  │                │                    │
 │             │            │ execute(NewTag)  │                │                    │
 │             │            ├──────────────────▶                │                    │
 │             │            │                  │ add_tag(new)   │                    │
 │             │            │                  ├────────────────▶                    │
 │             │            │                  │                │ write temp         │
 │             │            │                  │                ├────────────────────▶
 │             │            │                  │                │                    │
 │             │            │                  │                │ persist (rename)   │
 │             │            │                  │                ├────────────────────▶
 │             │            │                  │                │                    │
 │             │            │                  │                │◀── Ok ─────────────┤
 │             │            │                  │◀── Ok(tag) ────┤                    │
 │             │            │◀── Ok(tag) ──────┤                                    │
 │             │            │ update local tag-count state      │                    │
 │             │◀── Action::Noop (no global nav required)       │                    │
 │             │            │                                                        │
 │             │ draw(next frame — tag count in title updates)                       │
 │◀────────────                                                                       
```

Notes:
- The view never touches the `TagStore` trait. It holds `Rc<AddTagUseCase>`.
- The use case's `execute` is synchronous (the persist is ~1ms for a small tag file) and blocks the main thread.
- If the persist fails, `execute` returns `Err(TagError::…)`; the view sets `self.error_message = Some("…")`.

### Flow 2: Navigate Home → Sessions with background load

```
User         App       Router     SessionsView    (worker thread)    ClaudeSessionStore
 │            │           │             │                 │                    │
 │ press 's'  │           │             │                 │                    │
 ├────────────▶           │             │                 │                    │
 │            │ nav_target(s) → Some(ViewId::Sessions)    │                    │
 │            │           │             │                 │                    │
 │            │ Action::NavigateTo(Sessions)              │                    │
 │            ├──build(Sessions)───────▶                  │                    │
 │            │           │             │                 │                    │
 │            │ View::Sessions(SessionsView::new(...))    │                    │
 │            │           │             │                 │                    │
 │            ├──current.on_enter()────▶                  │                    │
 │            │           │             ├ spawn worker ──▶                    │
 │            │           │             │                 │ discover()         │
 │            │           │             │                 ├────────────────────▶
 │            │           │             │                 │◀── Ok(store) ──────┤
 │            │           │             │                 │ list_sessions(...)  │
 │            │           │             │                 ├────────────────────▶
 │            │           │             │                 │  read 47 JSONL     │
 │            │           │             │                 │   files (~200ms)   │
 │            │           │             │                 │◀── Vec<Raw> ───────┤
 │            │           │             │ ◀── tx.send ────┤
 │            │ (meanwhile: draw/draw/draw — spinner ticks, ~12 frames pass)   │
 │            │           │             │                                      │
 │            │ next tick: poll_loading                                         │
 │            ├────────────────────────▶ try_recv → Ok(Raw vec)                │
 │            │                         │ map Raw → SessionInfo (Rc<str>)      │
 │            │                         │ state = LoadState::Ready(vec)        │
 │            │                         │                                      │
 │            │ draw → render Ready state: show session list                  │
 │◀───────────                                                                   
```

Notes:
- The View enum's `poll_loading` is called once per tick. SessionsView's `poll_loading` calls `state.poll()` on its `LoadState<Vec<SessionInfo>>`.
- The worker owns a `Sender<Result<Vec<RawSessionInfo>, String>>`; the view owns the matching `Receiver`.
- Because `Rc<str>` is `!Send`, the worker produces `Vec<RawSessionInfo>` (with `String` ids) and the main thread converts on receive.
- The `SessionInfo` DTO that the view actually works with has `SessionId(Rc<str>)` so repeated clones are free.

### Flow 3: Refresh Changes after a file edit

```
User      App      ChangesView   RefreshChangesUseCase   ChangeDetector (git2)
 │         │            │                  │                      │
 │ press 'r' in Changes view                                       │
 ├─────────▶            │                  │                      │
 │         │ handle_event(r)                │                      │
 │         ├────────────▶                  │                      │
 │         │            │ (returns Action::Refresh)               │
 │         ◀────────────┤                  │                      │
 │         │ dispatch(Action::Refresh)      │                      │
 │         │ → current.on_enter()           │                      │
 │         ├────────────▶                  │                      │
 │         │            │ refresh_use_case.execute()               │
 │         │            ├──────────────────▶                      │
 │         │            │                  │ detect_changes()      │
 │         │            │                  ├──────────────────────▶
 │         │            │                  │◀── Vec<FileChange> ──┤
 │         │            │◀── Vec<FileChange> ┤                    │
 │         │            │ file_list.update_files(vec)             │
 │         │            │ (preserves selection by path if possible)│
 │         │◀───────────                                           │
 │         │ draw                                                  │
 │◀────────                                                         
```

Notes:
- This flow is synchronous because `detect_changes()` on a local repo is typically <10ms.
- A future refactor could wrap it in the LoadState pattern for consistency; v3 keeps it synchronous because the UX trade-off (flash of spinner on fast refresh) isn't worth it.

---

## Implementation milestones — v3 vertical slices

**v2 had 17 technical phases (Foundation, Storage, Discovery, Git, Sessions, Polish).** The architecture guide's review pointed out that this is "package by tool" phasing — it organizes the plan around layers rather than features, which makes it hard to deliver a single coherent slice of the domain end-to-end.

**v3 has 9 vertical slices.** Each milestone M2–M8 delivers a complete bounded context (domain + port + use cases + adapter + view) in one go. This is Option 3 of the phase ordering — it leans on M1 (the Shell refactor) to lay all the groundwork so every subsequent milestone can focus on shipping ONE context end-to-end.

| # | Milestone | What ships | Context(s) touched | Risk |
|---|-----------|-----------|--------------------|------|
| 0 | Pre-refactor hygiene | Drop `Send + Sync` from traits; `Mutex<Repository>` → `RefCell<Repository>` | core, git | **Low** — 2 files, no semantic change |
| 1 | Shell refactor | Core reorganized, use cases introduced, Router in apps/tui, Changes vertical slice verified | core/*, view, apps/tui, git | **High** — touches every file |
| 2 | Tags slice | `t` marks, `T`ags view, JSON persistence, FileViewer integration | `tags`, `store`, view, apps/tui | Medium |
| 3 | Search slice | `/` fuzzy file finder with background load | `search`, `search` (crate), view | Medium |
| 4 | Todos slice | `T` (capital) TODO/FIXME inbox | `todos`, `search` (crate reuse), view | Low |
| 5 | Branches slice | `b` branch list + recent commits | `branches`, `git` (extended), view | Medium |
| 6 | Sessions slice | `s` Claude session list with JSONL reader | `sessions`, `sessions` (crate), view | Medium |
| 7 | Home + activity feed | Home view with cross-context aggregator | `home.rs`, depends on 1–6 | Medium |
| 8 | Command palette | `:` overlay with fuzzy command filter | view chrome | Low |
| 9 | Polish + docs + decisions | Empty states, errors, perf check, decisions.md, README | everything | Low |

**Each milestone delivers a bit of user-visible value.** No milestone is "internal scaffolding only." No milestone takes more than ~1 week at a steady pace.

---

### Milestone 0: Pre-refactor hygiene pass

**You'll see:** no user-visible change. Two small single-threaded cleanups that the Rust notebook flagged in the v3 review, done against the *current* file layout before the M1 reorganization begins. Doing them here shrinks M1's blast radius and ships as a clean standalone commit.

**Risk: low.** Two files touched, existing tests cover every code path, zero semantic change.

**What to build:**

1. **Drop `Send + Sync` from the current traits.**
   - `crates/core/src/traits.rs`: change `pub trait ChangeDetector: Send + Sync { … }` to `pub trait ChangeDetector { … }`.
   - Same file: `pub trait SyntaxHighlighter: Send + Sync { … }` → `pub trait SyntaxHighlighter { … }`.
   - Nothing in the workspace requires the bound (the only consumer is `App`, which holds `Box<dyn ChangeDetector>` + `Box<dyn SyntaxHighlighter>` — `Box<dyn T>` doesn't need `T: Send + Sync` unless the `Box` itself crosses thread boundaries, which it doesn't).

2. **`Mutex<Repository>` → `RefCell<Repository>` in the git adapter.**
   - `crates/git/src/detector.rs`: replace `use std::sync::Mutex;` with `use std::cell::RefCell;`.
   - Replace `repo: Mutex<Repository>` (the struct field) with `repo: RefCell<Repository>`.
   - `GitChangeDetector::open`: replace `Mutex::new(repo)` with `RefCell::new(repo)`.
   - `detect_changes`: replace `let repo = self.repo.lock().expect("repo mutex poisoned");` with `let repo = self.repo.borrow();`. The function uses `repo` read-only (calls `.head()`, `.workdir()`, `.statuses()`, `.index()`), so `borrow()` is sufficient.
   - `compute_diff`: same substitution — `borrow()` is fine because `DiffOptions` is a local `let mut diff_opts` that the `diff_tree_to_workdir_with_index` call consumes separately; the `repo` handle itself is only read.
   - `read_at_head`: same substitution — fully read-only.
   - Remove the `.expect("repo mutex poisoned")` expectation string entirely. If `RefCell::borrow` ever panics, it's because the code has a genuine re-entrancy bug (the panic message is `RefCell<T> already mutably borrowed`), which is the exact bug-catching guarantee the `Mutex` was providing.

**Verify:** `just check` passes. `just run` launches the existing UX bit-for-bit (no observable difference — the whole point is that the cleanup is invisible).

**Why it's safe:**
- The codebase is single-threaded today. The `Mutex` was never lock-contended because there's only one thread. Clippy's `mutex_atomic` / `rc_mutex` lints would flag this pattern in a single-threaded app per the Rust notebook's v3 review.
- The `RefCell` swap is a zero-semantic-change refactor. Every existing test continues to pass unchanged.
- Dropping `Send + Sync` from the traits is a pure relaxation of bounds — no existing call site depends on them.

**Why pre-M1 rather than inside M1:**
- M0 is a small, fast, low-risk commit that can ship on its own and be validated end-to-end before touching the bigger architectural moves.
- M1's shell refactor then has a smaller blast radius — it focuses entirely on the reorganization and use-case introduction, not on incidental Rust cleanups.
- A future contributor reading the git log sees a clean progression: "tidy the trait bounds and interior mutability" → "reorganize the crate layout" → "introduce the application layer" — rather than a single giant commit mixing all three.

**Not in M0:**
- The `Box<dyn ChangeDetector>` → `Rc<dyn ChangeDetector>` change in `App`. That's bundled with the Router refactor in M1 because sharing the detector across views is what makes the `Rc` necessary.
- Any module reorganization (that's M1 sub-step 1).
- Any new types or traits.

**Crates touched:** `crates/core`, `crates/git`.

**Decisions log:** append `2026-04-11: Dropped Send + Sync speculative bounds from ChangeDetector and SyntaxHighlighter; migrated GitChangeDetector's interior mutability from Mutex<Repository> to RefCell<Repository>. Single-threaded design per the rust-style-guide notebook's review.`

---

### Milestone 1: Shell refactor — Shared Kernel + application layer + composition root move

**You'll see:** Launching codepeek shows the same file list and file viewer as today. No new features. But `ls crates/core/src/` shows bounded-context directories, `ls apps/tui/src/` shows `router.rs` + `app.rs` + `views_enum.rs`, and `cargo tree` shows the v3 dependency graph. **This milestone is the gate for everything else.**

**Risk: high.** This touches nearly every source file. Run `just check` after every sub-step. Keep the work on a branch. Budget 2–3 sittings.

**Sub-steps (each one independently compiles):**

1. **Reorganize `core` internally.**
   - Create `crates/core/src/kernel/` with `highlight.rs` (move `HighlightKind`, `HighlightSpan`, `HighlightedLine` here)
   - Create `crates/core/src/syntax/` with `port.rs` (`SyntaxHighlighter` trait, no `Send + Sync`) and `error.rs` (`SyntaxError`)
   - Create `crates/core/src/changes/` with `domain.rs` (move `FileChange`, `ChangeKind`, `DiffHunk`, `DiffLine`, `LineChange`, `ChangeMap`), `port.rs` (`ChangeDetector` trait, no `Send + Sync`), `error.rs` (`ChangeError`)
   - Update `crates/core/src/lib.rs` re-exports
   - Delete the old flat files (`change.rs`, `diff.rs`, `highlight.rs`, `error.rs`, `traits.rs`)
   - Fix every `use codepeek_core::XX` site in `crates/git`, `crates/syntax`, `crates/view`, `apps/tui` (they still work because of re-exports, but idiomatic imports should be updated)
   - Run `just test --workspace` — all existing tests still pass.

2. **Verify M0's trait-bound removal and `RefCell` migration are preserved at the new paths.**
   - M0 already dropped `Send + Sync` from `ChangeDetector` and `SyntaxHighlighter` in `crates/core/src/traits.rs`, and migrated `GitChangeDetector` to `RefCell<Repository>`.
   - Sub-step 1 above moves the trait definitions from `crates/core/src/traits.rs` to `crates/core/src/changes/port.rs` and `crates/core/src/syntax/port.rs`. After the move, confirm the new files have no `: Send + Sync` bounds. No new code change — this sub-step is a sanity check that M0's cleanup survived the reorganization intact.

3. **Introduce use cases in `crates/core/src/changes/app.rs`.**
   - `RefreshChangesUseCase { detector: Rc<dyn ChangeDetector> }` with `new` + `execute(&self) -> Result<Vec<FileChange>, ChangeError>`
   - `OpenChangedFileUseCase { detector: Rc<dyn ChangeDetector>, highlighter: Rc<RefCell<dyn SyntaxHighlighter>> }` with `execute(&self, path, kind) -> Result<OpenedFile, ChangeError>`
   - `PeekDeletedFileUseCase` with `execute(&self, path) -> Result<Vec<HighlightedLine>, ChangeError>`
   - Define `OpenedFile` DTO: `{ path, lines, change_map, diff_hunks }`
   - Unit tests with a stub `ChangeDetector` implementing the trait in-file

4. **Reorganize `view` crate into `chrome/` and `views/`.**
   - Create `crates/view/src/chrome/` subdirectory
   - Move `theme.rs`, `layout.rs`, `render_helpers.rs`, `keybindings.rs`, `action.rs`, `components.rs`, `components/` into `chrome/`
   - Update re-exports so existing call sites still work via `use codepeek_view::chrome::*`
   - Create `crates/view/src/views/` and `crates/view/src/views.rs` (sibling)
   - Move the existing App/dispatch/focus logic into a new `views/changes.rs` as `ChangesView`
   - `ChangesView::new` takes `Rc<RefreshChangesUseCase>`, `Rc<OpenChangedFileUseCase>`, `Rc<PeekDeletedFileUseCase>` — no more `Box<dyn ChangeDetector>`
   - `FileList`, `FileViewer`, `PeekOverlay` stay in `chrome/components/` (they're reusable widgets, not views)
   - Build + test; the existing tests move from `app.rs` to `views/changes.rs`

5. **Move App + Router + View enum to `apps/tui`.**
   - Create `apps/tui/src/app.rs` with the `App` struct (move from `crates/view/src/app.rs`)
   - Create `apps/tui/src/router.rs` with `AppDeps` and `Router::build`
   - Create `apps/tui/src/views_enum.rs` with `enum View { Changes(ChangesView) }` (just one variant for now) and the exhaustive-match delegate methods
   - Create `apps/tui/src/stubs.rs` (empty for now; will add Null stubs in later milestones)
   - Delete `crates/view/src/app.rs`
   - Update `crates/view/src/lib.rs`: `pub use chrome::*; pub use views::*;` — no more `pub use app::App`
   - Update `apps/tui/src/main.rs`: build the `GitChangeDetector`, wrap in `RefreshChangesUseCase` etc., build `AppDeps`, build `Router`, construct `App::new(router)`, run

6. **Dependency graph check script.**
   - Create `scripts/check_dep_graph.sh` that parses each `crates/*/Cargo.toml` and asserts: (a) `core` has no `codepeek-*` deps, (b) `git`/`syntax` depend only on `core`, (c) `view` depends only on `core`, (d) `apps/tui` may depend on anything
   - Add to `just check` so CI fails if the graph drifts

7. **Decisions log update.**
   - Append to `docs/decisions.md`:
     - `2026-04-11: Reorganized core into bounded contexts (kernel, syntax, changes, …) with domain/port/error/app sub-modules per context.`
     - `2026-04-11: Introduced application layer (use cases per context). Views hold Rc<UseCase>, not Rc<dyn Port>.`
     - `2026-04-11: Moved Router + App + View enum from crates/view to apps/tui. Composition root is now in the binary.`
     - `2026-04-11: Dropped Send + Sync from ChangeDetector and SyntaxHighlighter. Replaced Mutex<Repository> with RefCell<Repository>.`
     - `2026-04-11: Presentation follows Component Architecture (ratatui template pattern), not pure TEA. Per-view state is private; the central Action enum carries only cross-cutting concerns.`
     - `2026-04-11: Dependency-graph enforcement via scripts/check_dep_graph.sh in just check.`

**Verify:** `just check` passes (fmt + lint + test). `just run` shows the existing Changes view UX bit-for-bit. **User pressing `q/j/k/Enter/r/Esc/d` all work identically.**

**Crates touched:** all of them.

**Git strategy:** single long-lived branch named `m1-shell-refactor`. Each sub-step is its own commit. Squash on merge if desired.

---

### Milestone 2: Tags vertical slice

**You'll see:** Press `t` from anywhere → TagsView (empty initially, with a helpful empty-state hint). Open a file via Changes. Press `m` to mark the current line as an Issue tag, `M` for a Fix tag. Press `t` again — see the new entry. Press Enter on a tag to jump back to that file:line. Relaunch codepeek — tags persist.

**What to build:**

1. **`tags` bounded context in core:**
   - `crates/core/src/tags/domain.rs`: `TagId(u64)`, `TagKind` (with `#[non_exhaustive]`), `Tag` (without), `NewTag<'a>` input struct
   - `crates/core/src/tags/port.rs`: `TagStore` trait (no `Send + Sync`)
   - `crates/core/src/tags/error.rs`: `TagError`
   - `crates/core/src/tags/app.rs`: `AddTagUseCase`, `ListTagsUseCase`, `RemoveTagUseCase`
   - Unit tests with stub TagStore

2. **`store` crate (new) — JSON adapter:**
   - `crates/store/Cargo.toml` with `serde`, `serde_json`, `dirs`, `tempfile` workspace deps
   - `crates/store/src/lib.rs` re-exports `JsonTagStore`
   - `crates/store/src/json_tag_store.rs`: `JsonTagStore` implementing `TagStore`
   - `crates/store/src/file.rs`: `TagFile { version, next_id, tags }` (the on-disk shape)
   - `persist()` uses `tempfile::NamedTempFile::new_in(parent)` + `write_all` + `persist(&self.path)`
   - Version mismatch returns `TagError::Corrupt`
   - Tests using `tempfile::TempDir`: add/list/remove round-trip, version mismatch handling, atomic write via crash simulation (drop the NamedTempFile before persist, verify target untouched)

3. **`TagsView` in `crates/view/src/views/tags.rs`:**
   - Holds `Rc<ListTagsUseCase>`, `Rc<RemoveTagUseCase>`, `LoadState<Vec<Tag>>`, `selected: usize`, `error_message`
   - `on_enter` spawns a worker that calls `list_use_case.execute()` (even though it's fast, use the loading pattern uniformly)
   - Renders a list: relative time, kind badge, path, line, note
   - `j`/`k` navigate, `Enter` emits `Action::OpenFileAt { path, line: Some(tag.line) }`, `x` calls `remove_use_case.execute(tag.id)`

4. **`FileViewerView` enhancements:**
   - Holds `Rc<AddTagUseCase>`
   - `handle_event`: `m` → `add_tag_use_case.execute(NewTag { path, line: cursor_line, kind: Issue, note: "" })`, `M` → `kind: Fix`
   - On error, sets `self.error_message`
   - Returns `Action::Noop` (the mutation is local, no cross-cutting dispatch needed)

5. **`keybindings.rs` additions:** `is_mark_issue(key)`, `is_mark_fix(key)`, `is_open_tags(key)` (for `t`)

6. **Update `apps/tui/src/main.rs`:** build `JsonTagStore::open()` (falling back to `NullTagStore` on failure with stderr warning); construct `AddTagUseCase`, `ListTagsUseCase`, `RemoveTagUseCase`; add to `AppDeps`; add `ViewId::Tags` to the `View` enum and `Router::build`

7. **Cross-view navigation:** `keybindings::nav_target` now maps `t` → `Some(ViewId::Tags)`. `App::dispatch_key` checks `nav_target` after `wants_raw_keys`.

**Verify:** Open a file, press `m`. Press `t`. See the tag. Press Enter — you're back in the file. Relaunch — tag persists. `just check` clean.

**Crates touched:** `core`, `store` (new), `view`, `apps/tui`.

---

### Milestone 3: Search vertical slice

**You'll see:** Press `/` from any view → SearchView. Type characters into the input. Spinner blinks, then results appear. Enter opens the highlighted file.

**What to build:**

1. **`search` context in core:** domain (`FileMatch`), port (`FileSearcher` trait), error (`SearchError`), use case (`FindFilesByQueryUseCase`)

2. **`search` crate (new) — ignore/regex adapter:**
   - `crates/search/Cargo.toml` with `ignore` and `regex` workspace deps
   - `RipgrepLikeSearcher` impl of `FileSearcher`
   - Subsequence match for v1 (not full fuzzy)
   - Tests using `tempfile::TempDir` with fake repo including `.gitignore`

3. **`text_input` chrome component:** single-line input with cursor, char insertion, backspace, left/right cursor

4. **`SearchView`:** holds `Rc<FindFilesByQueryUseCase>`, `repo_root`, `TextInput`, `LoadState<Vec<FileMatch>>`, `selected`; **`wants_raw_keys() -> true`** during input; each keystroke kicks off a fresh worker (replacing any in-flight receiver)

5. **Router + apps/tui wiring:** `Router::build(ViewId::Search)`, dep on `find_files` use case, `NullFileSearcher` stub for fallback

6. **Cross-view navigation:** `/` → `ViewId::Search`

**Verify:** Press `/`, type, spinner, results, Enter opens the file.

**Crates touched:** `core`, `search` (new), `view`, `apps/tui`.

---

### Milestone 4: Todos vertical slice

**You'll see:** Press `T` (capital) → TodosView. Spinner while scanning. Then grouped TODO/FIXME/HACK/XXX list across the repo. Enter jumps to file:line.

**What to build:**

1. **`todos` context in core:** domain (`TodoItem`, `TodoKind`), port (`TodoScanner`), error (`TodoError`), use case (`ScanRepoTodosUseCase`)

2. **Reuse `search` crate** for the `TodoCommentScanner` impl (it was planned for the same crate in v2; in v3 both `FileSearcher` and `TodoScanner` adapters live in the `search` crate because they share the `ignore::Walk` configuration)

3. **`TodosView`:** holds `Rc<ScanRepoTodosUseCase>`, `repo_root`, `LoadState<Vec<TodoItem>>`; groups by kind; Enter → `Action::OpenFileAt { path, line: Some(line) }`

4. **Router + wiring:** `Router::build(ViewId::Todos)`, `scan_todos` use case in `AppDeps`

5. **Cross-view navigation:** `T` (capital) → `ViewId::Todos`

**Verify:** Press `T`, see spinner, then scanned todos, pick one, jump.

**Crates touched:** `core`, `search` (extend), `view`, `apps/tui`.

---

### Milestone 5: Branches vertical slice

**You'll see:** Press `b` → BranchesView. Top section: branches with `*` on current. Bottom section: recent commits. Tab toggles focus. Selecting a commit opens its file list.

**What to build:**

1. **`branches` context in core:** domain (`CommitSha([u8; 20])`, `CommitInfo`, `BranchInfo`), port (`CommitLog`), error (`BranchError`), use cases (`ListBranchesUseCase`, `RecentCommitsUseCase`, `ReadAtCommitUseCase`)

2. **Extend `GitChangeDetector`** in `crates/git/src/detector.rs` to implement `CommitLog` (in addition to `ChangeDetector`). One struct, two trait impls. Uses `self.repo.borrow()` (now `RefCell` after M1).

3. **`BranchesView`:** holds `Rc<ListBranchesUseCase>`, `Rc<RecentCommitsUseCase>`, `LoadState<(Vec<BranchInfo>, Vec<CommitInfo>)>`, two selected indices, Tab-toggle focus

4. **Router + wiring:** both use cases in `AppDeps`; `Rc<GitChangeDetector>` coerced to both `Rc<dyn ChangeDetector>` and `Rc<dyn CommitLog>`

5. **Cross-view navigation:** `b` → `ViewId::Branches`

6. **Commit opening (v1 minimal):** `Action::OpenCommit(sha)` uses the existing file viewer machinery to show the commit's diff. Full "browse files at commit" is a future plan.

**Verify:** Press `b`, see branches + commits, Tab toggles, Enter opens a commit.

**Crates touched:** `core`, `git`, `view`, `apps/tui`.

---

### Milestone 6: Sessions vertical slice

**You'll see:** Press `s` → SessionsView. Spinner briefly while scanning `~/.claude/projects/<repo>`. Then the list of Claude Code sessions for this repo, newest first. Each row: short id, last-active relative time, message count, summary excerpt. Enter is a no-op in v1.

**What to build:**

1. **`sessions` context in core:** domain (`SessionId(Rc<str>)`, `SessionInfo`, `RawSessionInfo` cross-thread shape), port (`SessionStore`), error (`SessionError`), use case (`ListRepoSessionsUseCase`)

2. **`sessions` crate (new) — Claude JSONL adapter:**
   - `crates/sessions/Cargo.toml` with `serde`, `serde_json`, `dirs` workspace deps
   - `ClaudeSessionStore::discover()` + `list_sessions(&repo_root)`
   - Path encoding: `repo.to_string_lossy().replace(['/', '.'], "-")`
   - Streaming line reader for first+last+count (no full parse of body messages)
   - The worker constructs `RawSessionInfo` (String ids); main thread converts to `SessionInfo` (Rc<str> ids)
   - Tests against fixture JSONL files in `crates/sessions/tests/fixtures/`

3. **`SessionsView`:** `LoadState<Vec<SessionInfo>>`; renders relative time via shared helper

4. **`render_helpers` addition:** `relative_time(t: SystemTime) -> String` — returns "2m", "1h", "3d". Used by Sessions, Branches, Tags, Home.

5. **Router + wiring:** `list_repo_sessions` use case in `AppDeps`; `ClaudeSessionStore::discover()` with `NullSessionStore` fallback on failure

6. **Cross-view navigation:** `s` → `ViewId::Sessions`

**Verify:** Press `s`, see spinner, then your Claude sessions for this repo.

**Crates touched:** `core`, `sessions` (new), `view`, `apps/tui`.

---

### Milestone 7: Home view with cross-context activity feed

**You'll see:** Launching codepeek lands on Home (not Changes). Home shows a stats line ("3 changes · 5 tags · 12 recent commits") and a vertical list of recent activity (edits, tags, commits) with relative timestamps. Picking an entry navigates to the right place.

**What to build:**

1. **`ActivityEntry`, `ActivityKind`, `ActivityTarget`** in `crates/view/src/views/home.rs` (NOT in kernel, as discussed above — kernel would need to import from every bounded context)

2. **`ActivityFeedAggregator`** in the same file. Holds `Rc<ListTagsUseCase>`, `Rc<RefreshChangesUseCase>`, `Rc<RecentCommitsUseCase>` (the fast-read sources). Method `collect(limit) -> Vec<ActivityEntry>` merges and sorts by timestamp.

3. **`HomeView`** holds `ActivityFeedAggregator`, `LoadState<Vec<ActivityEntry>>`. `on_enter` spawns a worker — but the aggregator's use cases are `!Send`. Two options:
   - **Option A (preferred):** the aggregator is constructed synchronously on the main thread, and `execute(limit)` is called on the main thread (no worker). The use cases hit are all fast (`list_tags`, `detect_changes`, `recent_commits`) — each <50ms, so total is <200ms. Acceptable block.
   - **Option B:** factory closures passed into the worker to construct fresh store instances. Complex; not worth it for v1.
   - **v3 decision: Option A.** Home's feed aggregates only from fast sources; slow sources (sessions, todos, search) are not included in the feed. The stats line shows "sessions: see s" and "todos: see T" instead of counts.

4. **Selection dispatch** — each activity kind maps to an Action:
   - `FileEdit { path }` → `Action::OpenFileAt { path, line: None }`
   - `Commit { sha }` → `Action::OpenCommit(sha)`
   - `Tag { id: _ }` → `Action::NavigateTo(ViewId::Tags)`
   - `Session { id: _ }` → `Action::NavigateTo(ViewId::Sessions)`
   - `Todo { path, line }` → `Action::OpenFileAt { path, line: Some(line) }`

5. **`App::new` builds `Home` as the initial view** (was `Changes` in M1)

6. **Cross-view navigation:** `h` → `ViewId::Home`

7. **Empty Home handling:** if activity is empty (new repo, no tags, no commits), render a "Welcome to codepeek" panel with a one-line hint for each top-level view + its nav key

**Verify:** Relaunch codepeek. You land on Home. See a stats line + activity. Pick a tag → Tags view. Pick an edit → file opens. Pick a commit → commit diff opens.

**Crates touched:** `view`, `apps/tui`.

---

### Milestone 8: Command palette

**You'll see:** Press `:` → centered overlay with a text input and a filtered command list. Type to filter, Enter to invoke. Esc closes.

**What to build:**

1. **`command_palette.rs` in `chrome/components/`** — holds a `TextInput`, a static `Vec<PaletteCommand>`, subsequence filter. Emits the corresponding `Action` on Enter.

2. **Command list v1:** Go to Home/Changes/Sessions/Search/Tags/Branches/Todos, Refresh, Quit. Each is a nav or direct Action.

3. **`App` state:** `palette: Option<CommandPalette>`. When `Some`, App routes events to the palette before global nav.

4. **`keybindings::is_palette`** → `:` or `Ctrl+P`

**Verify:** Press `:`, type "sess", pick "Go to Sessions", you're in Sessions.

**Crates touched:** `view`, `apps/tui`.

---

### Milestone 9: Polish, final tests, decisions log

**You'll see:** Everything works smoothly. Empty states are handled. Errors don't crash. Narrow terminals still work. `just check` is clean. Docs updated.

**What to build:**

1. **Empty states** in every view: Home ("No recent activity"), Sessions ("No Claude sessions for this repo"), Tags ("No tags yet — press `m` while viewing a file to add one"), etc.

2. **Error states:** every view's `LoadState::Failed` renders through `ErrorBar`.

3. **Refresh consistency:** `r` works in every view that has a refreshable data source. Wired through `View::on_enter` after `Action::Refresh`.

4. **Navigation-conflict tests:** `j/k` scroll inside FileViewer without triggering global nav. `t` doesn't fire while typing in Search. 

5. **Status bar budget:** `h c s / t b T : q` is 9 hints, ~30 characters. Show only when terminal width ≥ 120 cols. Threshold in `config.rs`.

6. **Performance check:** press `T` on a large repo (10k files). Spinner should animate smoothly, UI shouldn't freeze. If it does, the worker is starving the main thread — bug.

7. **Theme audit:** new components use only theme tokens, no raw palette colors.

8. **v3 naming audit:** every `docs/decisions.md` entry from v3 is present; `crates/view/src/chrome/action.rs` has a module doc comment referencing the Component Architecture decision.

9. **README update:** new commands, view map, the "view dev guide" (how to add a new view: add a bounded context in `core/`, add a new adapter crate, add a view struct, add an enum variant + exhaustive matches, add to Router, add to AppDeps, add to main.rs).

10. **Profile the `Rc<str>` vs `String` choice for `SessionId`.** If `String` baseline is fine for the 100-session scale, revert the `RawSessionInfo`/`SessionInfo` split and document the simpler choice in decisions.md.

11. **Run the dep-graph check script** in CI to confirm no accidental cycles introduced.

**Verify:** Use codepeek for a real work day. Everything feels solid.

**Crates touched:** `view`, `apps/tui`, docs.

---

### Milestone summary

| # | Name | Delivers | Key crates | Risk |
|---|------|----------|------------|------|
| 0 | Pre-refactor hygiene | Drop Send+Sync; Mutex→RefCell | core, git | **Low** |
| 1 | Shell refactor | v3 architecture in place, existing UX unchanged | all | **High** |
| 2 | Tags slice | `m` mark, `t` list, persistence | core/tags, store, view, tui | Medium |
| 3 | Search slice | `/` fuzzy file finder | core/search, search, view, tui | Medium |
| 4 | Todos slice | `T` todo inbox | core/todos, search, view, tui | Low |
| 5 | Branches slice | `b` branches + commits | core/branches, git, view, tui | Medium |
| 6 | Sessions slice | `s` Claude sessions | core/sessions, sessions, view, tui | Medium |
| 7 | Home + activity | Home landing with cross-context feed | view, tui | Medium |
| 8 | Command palette | `:` overlay | view, tui | Low |
| 9 | Polish + docs | empty states, perf, decisions log | all | Low |

**First runnable with v3 architecture:** M1 (refactor is transparent).
**First user-visible new feature:** M2 (Tags).
**Background loading proven end-to-end:** M3 (Search with worker).
**Full view set minus Home:** M6.
**Feature-complete:** M7 (Home lights up the activity feed).
**Ship-ready:** M9 (polish).

---

## What we're NOT doing

- **No `tokio` / no async runtime.** Background work is `std::thread::spawn` + `std::sync::mpsc`. Future plan if needed.
- **No worker cancellation.** Workers are short-lived.
- **No streaming results.** `Vec<T>` or fail.
- **No background refresh / file watchers.** Data refreshes only on view enter or explicit `r`.
- **No tab strip / multi-pane chrome.** Zen mode preserved. Command palette is the only persistent visual addition.
- **No view-local config files.** Existing `~/.config/codepeek/config.toml` handles everything.
- **No runtime view registration.** The `View` enum is fixed at compile time.
- **No tag editing UI.** Add + remove; edit = remove + re-add in v1.
- **No session launching.** SessionsView is read-only in v1.
- **No commit graph visualization.** BranchesView is a flat list.
- **No fuzzy match library.** Subsequence matching is good enough for v1.
- **No new keybindings UI.** Hardcoded in `keybindings.rs`.
- **No cursor mode in FileViewer.** Tagging uses "line at top of visible scroll" as the current line.
- **No command palette history.** Every `:` opens fresh.
- **No `enum_dispatch` macro.** Manual exhaustive match in `views_enum.rs` per Rust notebook's verdict.
- **Sessions and Todos NOT in Home's activity feed in v1.** Home's aggregator runs synchronously; only fast sources (Tags, Changes, Recent commits) contribute. Adding Sessions/Todos requires an async aggregator — its own future plan.
- **No Command/Query Bus.** Use cases are injected directly. A bus is a future refactor if cross-cutting concerns (audit logging, command history) appear.
- **No `RenderContext<'a>` refactor of the highlighter.** The Rust notebook suggested this as an alternative to `Rc<RefCell<dyn SyntaxHighlighter>>`. v3 ships with the Rc version and revisits if the runtime borrow check ever panics in practice.
- **No Shared Kernel crate split.** The kernel is a module inside `core`, not a separate crate. One crate per bounded context would be more "Screaming" but costs compile time and crate-boilerplate more than it's worth at codepeek's scale.

---

## Architectural decisions resolved

### Resolved in v2 (rust-style-guide review) — preserved in v3

1. **`Box<dyn View>` vs enum-dispatch:** `enum View`. Closed set, exhaustive matches, no allocation.
2. **`Arc` vs `Rc` for shared ownership:** `Rc`. Single-threaded design.
3. **`Mutex` vs `RefCell` for interior mutability:** `RefCell`. Same reasoning.
4. **Hand-rolled `.tmp` + rename vs `tempfile`:** `tempfile::NamedTempFile::persist()`. Crash + race safety.
5. **Sync I/O on view enter vs background loading:** `LoadState<T>` + mpsc + `std::thread::spawn`.
6. **Flat vs hierarchical Action enum:** hierarchical (cross-cutting only); per-view state mutations inline.
7. **Status hints shape:** `Cow<'_, [(&'static str, &'static str)]>`.
8. **`add_tag(positional args)` vs `add_tag(NewTag)`:** input struct for stable signatures.
9. **Raw IDs vs newtypes:** newtypes for type safety.
10. **`views/mod.rs` vs sibling file:** sibling file (Rust 2018+ idiom).
11. **`wants_raw_keys` flag:** idiomatic opt-in capability pattern.
12. **Per-domain `thiserror` enums:** validated. Avoid ball-of-mud.
13. **Small adapter crates per external dep:** validated. Matches Ratatui's architecture.
14. **Delegate pattern in View enum:** Pattern B exhaustive match, no `_ =>` wildcards.
15. **Spinner frame-rate cap:** `TICKS_PER_FRAME = 6`.
16. **`#[from]` vs `#[source]`:** prefer `#[from]`; `#[source]` only for context-dependent wrapping.
17. **`Box<dyn Error + Send + Sync>` inside error variants:** kept as-is. Explicit exception.

### Resolved in v3 (software-architecture-guide review + second rust-style-guide pass)

18. **Package by tool vs package by component (bounded contexts):** **bounded contexts** at the module level, inside stable crate boundaries. `core` has `changes/`, `tags/`, `sessions/`, `search/`, `todos/`, `branches/`, `kernel/`, `syntax/`.
19. **Application layer (use cases):** **introduced.** Views hold `Rc<UseCase>`, not raw port traits. Every mutation goes through a use case. CQRS read-side lets `home` aggregator read directly.
20. **Composition root placement:** **`apps/tui`.** Router, App, View enum all move from `crates/view` to `apps/tui`. The view crate is chrome + per-context views only.
21. **Milestone phasing:** **vertical slices.** Nine context-focused milestones instead of seventeen technical phases.
22. **`#[non_exhaustive]` scope:** **open enums only,** not Data Objects. Walks back v2's "all types get it" instruction.
23. **`CommitSha` type:** **`[u8; 20]`** with hex encoding for display/serde. Avoids heap allocation on every dispatch.
24. **`SessionId` type:** **`Rc<str>`** on the main thread, with `RawSessionInfo { id: String }` cross-thread shape. Profile in M9 and revert to `String` if not worth the machinery.
25. **Component Architecture explicit naming:** **documented in `docs/decisions.md`, `chrome/action.rs` module docs, and `README.md` view-dev guide.** Acknowledges the departure from pure TEA.
26. **Shared Kernel scope:** **minimal** — just `highlight` types and the `syntax` port. `ActivityEntry` lives in `home.rs`, not kernel, to preserve acyclic layering.
27. **Dependency-graph enforcement:** **`scripts/check_dep_graph.sh` in `just check`** to prevent drift.

---

## Open decisions (still to resolve at implementation time)

1. **`enum_dispatch` macro adoption.** Manual `match` plumbing in `views_enum.rs` is ~70 lines. Stay manual until view #10.

2. **`OpenCommit(sha)` rendering quality.** v1 reuses the existing diff plumbing for "current HEAD changes" to render a past commit's diff. The UX is rough (you see the commit's diff, not the file *at* that commit). Future plan could add a proper commit browser. **Lean:** ship the rough version, document the limitation.

3. **Tag store concurrency across codepeek instances.** Two processes mutating the same JSON file race on read-modify-write. Atomic writes protect single operations, not sequences. **Lean:** v1 ignores; document as known limitation.

4. **Sessions path-encoding edge cases.** What if repo path contains characters Claude encodes differently than `/` and `.`? Test against actual `~/.claude/projects/` directory list in M6.

5. **Status bar global-hints threshold.** 120 cols is the current guess; tunable.

6. **Search debounce.** Each keystroke kicks off a worker. Fast typist → 5 workers in flight. `ignore::Walk` is fast but not free. **Lean:** add 50ms debounce if it shows in M9 perf testing.

7. **`Rc<GitChangeDetector>` coercion ergonomics.** The "two `Rc`s, one allocation" pattern in `main.rs` feels awkward. Alternative: `pub trait GitOps: ChangeDetector + CommitLog {}` super-trait and a single `Rc<dyn GitOps>`. Trade-off: super-trait adds an indirection. **Lean:** ship the awkward pattern; revisit if it shows up at more adapter call sites.

8. **`RenderContext<'a>` for the syntax highlighter.** The rust notebook suggested threading `&mut SyntaxHighlighter` through the render path instead of `Rc<RefCell<dyn>>`. Trade-off: eliminates runtime borrow-check panic risk, but requires every `render` method to take a new parameter. **Lean:** ship with `Rc<RefCell<dyn>>`. Revisit if a runtime borrow panic ever shows up in practice.

9. **`SessionId(Rc<str>)` vs `SessionId(String)`.** Profile in M9. The `RawSessionInfo`/`SessionInfo` split adds machinery; simpler `String` may be fine at the 100-session scale. **Decision gated on M9 measurement.**

10. **Vertical-slice crate split (future).** If the `core` crate's bounded-context modules start stepping on each other at compile time, split into per-context crates (`codepeek-changes`, `codepeek-tags`, etc.). **Threshold:** when `cargo check` in the `core` crate takes >10s on a clean build, extract the most-churned context. Until then, modules inside one crate are simpler.

---

## Appendix: how to add a new bounded context (view-dev guide)

A future contributor should be able to add a new bounded context (e.g. "diagnostics") by following these steps:

1. **Domain + application layer:**
   - Create `crates/core/src/diagnostics/{mod.rs,domain.rs,port.rs,error.rs,app.rs}`
   - Define entities/value objects in `domain.rs`
   - Define a `DiagnosticsStore` trait in `port.rs` (no `Send + Sync`)
   - Define `DiagnosticsError` in `error.rs` (own variants, use `thiserror`)
   - Define use cases in `app.rs` (`ListDiagnosticsUseCase`, etc.)
   - Re-export from `crates/core/src/lib.rs`

2. **Infrastructure adapter:**
   - If the adapter has distinct external deps, create `crates/diagnostics_lsp/` or similar
   - Otherwise, add to an existing adapter crate (e.g. `crates/search` extended)
   - Implement the port trait

3. **Presentation:**
   - Create `crates/view/src/views/diagnostics.rs` with a `DiagnosticsView` struct
   - Hold the relevant `Rc<UseCase>` instances
   - Implement the methods matching the `View` enum delegates: `title`, `status_hints`, `handle_event`, `render`, `on_enter`, `poll_loading`, `wants_raw_keys`

4. **Composition root:**
   - Add `ViewId::Diagnostics` to `apps/tui/src/views_enum.rs`
   - Add a `View::Diagnostics(DiagnosticsView)` variant
   - Add the variant to **every** delegate method's exhaustive match — the compiler forces this
   - Add the use cases to `apps/tui/src/router.rs::AppDeps`
   - Add `Router::build(ViewId::Diagnostics)` arm
   - Build the adapter + use cases in `apps/tui/src/main.rs` and pass to the `AppDeps`

5. **Navigation:**
   - Add a single-letter key to `keybindings::nav_target` if top-level navigable
   - Add to the command palette's command list (M8)

6. **Tests:**
   - Unit tests for the use cases in `core/diagnostics/app.rs` using a stub `DiagnosticsStore`
   - Integration tests for the view in `view/views/diagnostics.rs` using a stub view + `TestBackend`

7. **Decisions log:**
   - Add an entry to `docs/decisions.md` describing why the context was added and any non-obvious trade-offs

The compiler enforces that every new view updates every delegate method (that's the whole point of Pattern B), so forgetting a step is impossible.

---

## Appendix: reading this plan

- **Architecture section** defines the rules (principles, layering, bounded contexts, crate/module layout, dependency graph).
- **Bounded context sections** show the domain/port/error/app shape for each context, with enough code to verify types compile.
- **Presentation layer section** covers the cross-cutting UI machinery (View enum, App, Router, chrome components).
- **Composition root section** shows how everything wires together in `apps/tui/src/main.rs`.
- **Rust-level refinements** lists the v3 deltas from v2 with the reasoning.
- **Sequence diagrams** illustrate the hot flows (add tag, navigate with load, refresh).
- **Milestones** break the work into nine slices, each a complete vertical through a bounded context.
- **What we're NOT doing** lists the explicit non-goals so future PR reviews can flag scope creep.
- **Architectural decisions resolved** is the authoritative decisions log for this plan (ported to `docs/decisions.md` in M9).
- **Open decisions** are things we'll resolve at implementation time, with leans.
- **Appendices** are the view-dev guide and reading guide.
