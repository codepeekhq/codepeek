# codepeek

A terminal-native development tool for AI-assisted coding workflows. Codepeek is designed to replace a traditional IDE when your primary coding interface is Claude Code (or similar AI agents) running in the terminal.

Open your project, inspect changes, tag issues and fixes, launch new Claude Code sessions, or jump into your preferred editor (helix, neovim, etc.) - all without leaving the terminal.

## Workspace layout

This is a Cargo workspace monorepo organized into apps and crates:

```
apps/
  tui/          -> Terminal user interface (the main binary)
crates/
  core/         -> Domain logic, shared types, and trait definitions
  git/          -> Git-based change detection and diff computation (git2)
  syntax/       -> Tree-sitter syntax highlighting (17 language grammars)
  view/         -> Presentation layer and UI components (ratatui)
```

Dependency direction: `apps/tui` -> `codepeek-view` -> `codepeek-core`, `codepeek-git` -> `codepeek-core`, `codepeek-syntax` -> `codepeek-core`.

## Usage

This project uses [just](https://github.com/casey/just) as a command runner. Run `just` to see all available commands.

```sh
just run        # run the TUI
just build      # build all crates
just release    # build release
just test       # run all tests
just lint       # run clippy
just fmt        # format code
just check      # run all checks (fmt, lint, test)
just clean      # clean build artifacts
```

## License

MIT
