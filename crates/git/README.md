# codepeek-git

Git-based change detection and diff computation for codepeek.

## Scope

- `GitChangeDetector`: implements `codepeek_core::ChangeDetector` using git2
- Detects uncommitted changes (staged and unstaged) against HEAD
- Computes line-level diffs for modified files
- Reads file content at HEAD for deleted file previews
- Uses `Mutex<Repository>` for `Send + Sync` safety

## Not in scope

- Domain types or trait definitions -> `codepeek-core`
- Syntax highlighting -> `codepeek-syntax`
- UI rendering -> `codepeek-view`
