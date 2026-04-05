pub mod change;
pub mod diff;
pub mod error;
pub mod highlight;
pub mod traits;

pub use change::{ChangeKind, FileChange};
pub use diff::{ChangeMap, DiffHunk, DiffLine, LineChange};
pub use error::{ChangeError, SyntaxError};
pub use highlight::{HighlightKind, HighlightSpan, HighlightedLine};
pub use traits::{ChangeDetector, SyntaxHighlighter};

pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_set() {
        assert!(!version().is_empty());
    }
}
