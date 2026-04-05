use std::path::PathBuf;
use std::time::SystemTime;

/// The kind of change detected for a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
}

/// A file with uncommitted changes.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub kind: ChangeKind,
    pub mtime: SystemTime,
}
