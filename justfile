default:
    @just --list

# Run the TUI application
run *args:
    cargo run --package codepeek {{ args }}

# Build all crates
build *args:
    cargo build --workspace {{ args }}

# Build release
release:
    cargo build --workspace --release

# Run all tests
test *args:
    cargo test --workspace {{ args }}

# Run clippy lints
lint:
    cargo clippy --workspace --all-targets

# Format code
fmt:
    cargo fmt --all

# Check formatting without modifying
fmt-check:
    cargo fmt --all -- --check

# Run all checks (fmt, lint, test)
check: fmt-check lint test

# Clean build artifacts
clean:
    cargo clean
