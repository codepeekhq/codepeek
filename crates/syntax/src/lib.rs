mod highlighter;
mod languages;
mod mapping;

pub use highlighter::{Noop, TreeSitter};

/// All language names compiled into this build.
pub const SUPPORTED_LANGUAGES: &[&str] = &[
    "bash",
    "c",
    "cpp",
    "css",
    "go",
    "html",
    "java",
    "javascript",
    "json",
    "jsx",
    "lua",
    "markdown",
    "python",
    "ruby",
    "rust",
    "toml",
    "tsx",
    "typescript",
    "yaml",
];
