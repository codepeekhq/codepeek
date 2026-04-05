use std::path::Path;

use crate::change::FileChange;
use crate::diff::DiffHunk;
use crate::error::{ChangeError, SyntaxError};
use crate::highlight::HighlightedLine;

pub trait ChangeDetector: Send + Sync {
    fn detect_changes(&self) -> Result<Vec<FileChange>, ChangeError>;
    fn compute_diff(&self, path: &Path) -> Result<Vec<DiffHunk>, ChangeError>;
    fn read_at_head(&self, path: &Path) -> Result<String, ChangeError>;
}

pub trait SyntaxHighlighter: Send + Sync {
    // Takes `&mut self` because tree-sitter's Highlighter requires mutation.
    fn highlight(&mut self, source: &str, path: &Path)
    -> Result<Vec<HighlightedLine>, SyntaxError>;
}
