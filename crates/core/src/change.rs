use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub kind: ChangeKind,
    pub mtime: SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_kind_equality() {
        assert_eq!(ChangeKind::Added, ChangeKind::Added);
        assert_ne!(ChangeKind::Added, ChangeKind::Modified);
        assert_ne!(ChangeKind::Added, ChangeKind::Deleted);
    }

    #[test]
    fn renamed_kind_includes_source_path() {
        let kind = ChangeKind::Renamed {
            from: PathBuf::from("old.rs"),
        };
        if let ChangeKind::Renamed { from } = &kind {
            assert_eq!(from, &PathBuf::from("old.rs"));
        } else {
            panic!("expected Renamed variant");
        }
    }

    #[test]
    fn renamed_variants_with_different_paths_differ() {
        let a = ChangeKind::Renamed {
            from: PathBuf::from("a.rs"),
        };
        let b = ChangeKind::Renamed {
            from: PathBuf::from("b.rs"),
        };
        assert_ne!(a, b);
    }

    #[test]
    fn file_change_debug_output() {
        let change = FileChange {
            path: PathBuf::from("src/main.rs"),
            kind: ChangeKind::Modified,
            mtime: SystemTime::UNIX_EPOCH,
        };
        let debug = format!("{change:?}");
        assert!(debug.contains("src/main.rs"));
        assert!(debug.contains("Modified"));
    }
}
