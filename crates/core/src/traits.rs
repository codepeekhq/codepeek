use std::path::Path;

use crate::change::FileChange;
use crate::diff::DiffHunk;
use crate::error::{ChangeError, SyntaxError};
use crate::highlight::HighlightedLine;

/// Detects uncommitted changes in a repository.
pub trait ChangeDetector: Send + Sync {
    /// List all uncommitted changes, both staged and unstaged.
    fn detect_changes(&self) -> Result<Vec<FileChange>, ChangeError>;

    /// Compute line-level diff for a specific file against HEAD.
    fn compute_diff(&self, path: &Path) -> Result<Vec<DiffHunk>, ChangeError>;

    /// Read file content at HEAD (for deleted files / old version).
    fn read_at_head(&self, path: &Path) -> Result<String, ChangeError>;
}

/// Highlights source code using syntax-aware parsing.
pub trait SyntaxHighlighter: Send + Sync {
    /// Highlight source code, returning one `HighlightedLine` per line.
    /// Takes `&mut self` because tree-sitter's Highlighter requires mutation.
    fn highlight(&mut self, source: &str, path: &Path)
    -> Result<Vec<HighlightedLine>, SyntaxError>;
}
