use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineChange {
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: LineChange,
    pub content: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Default)]
pub struct ChangeMap {
    pub added: HashSet<u32>,
    pub modified: HashSet<u32>,
    pub deleted: Vec<u32>,
}

impl ChangeMap {
    pub fn from_hunks(hunks: &[DiffHunk]) -> Self {
        let mut map = Self::default();
        for hunk in hunks {
            for line in &hunk.lines {
                match line.kind {
                    LineChange::Added => {
                        if let Some(n) = line.new_lineno {
                            map.added.insert(n);
                        }
                    }
                    LineChange::Removed => {
                        if let Some(n) = line.old_lineno {
                            map.deleted.push(n);
                        }
                    }
                    LineChange::Modified => {
                        if let Some(n) = line.new_lineno {
                            map.modified.insert(n);
                        }
                    }
                }
            }
        }
        map.deleted.sort_unstable();
        map.deleted.dedup();
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_hunks_maps_added_lines() {
        let hunks = vec![DiffHunk {
            old_start: 1,
            old_lines: 0,
            new_start: 1,
            new_lines: 2,
            lines: vec![
                DiffLine {
                    kind: LineChange::Added,
                    content: "line 1".into(),
                    old_lineno: None,
                    new_lineno: Some(1),
                },
                DiffLine {
                    kind: LineChange::Added,
                    content: "line 2".into(),
                    old_lineno: None,
                    new_lineno: Some(2),
                },
            ],
        }];
        let map = ChangeMap::from_hunks(&hunks);
        assert!(map.added.contains(&1));
        assert!(map.added.contains(&2));
        assert!(map.modified.is_empty());
        assert!(map.deleted.is_empty());
    }

    #[test]
    fn from_hunks_maps_removed_lines() {
        let hunks = vec![DiffHunk {
            old_start: 5,
            old_lines: 1,
            new_start: 5,
            new_lines: 0,
            lines: vec![DiffLine {
                kind: LineChange::Removed,
                content: "old line".into(),
                old_lineno: Some(5),
                new_lineno: None,
            }],
        }];
        let map = ChangeMap::from_hunks(&hunks);
        assert!(map.added.is_empty());
        assert_eq!(map.deleted, vec![5]);
    }

    #[test]
    fn from_hunks_maps_modified_lines() {
        let hunks = vec![DiffHunk {
            old_start: 10,
            old_lines: 1,
            new_start: 10,
            new_lines: 1,
            lines: vec![DiffLine {
                kind: LineChange::Modified,
                content: "changed".into(),
                old_lineno: Some(10),
                new_lineno: Some(10),
            }],
        }];
        let map = ChangeMap::from_hunks(&hunks);
        assert!(map.modified.contains(&10));
    }

    #[test]
    fn from_hunks_deduplicates_deleted() {
        let hunks = vec![
            DiffHunk {
                old_start: 3,
                old_lines: 1,
                new_start: 3,
                new_lines: 0,
                lines: vec![DiffLine {
                    kind: LineChange::Removed,
                    content: "a".into(),
                    old_lineno: Some(3),
                    new_lineno: None,
                }],
            },
            DiffHunk {
                old_start: 3,
                old_lines: 1,
                new_start: 3,
                new_lines: 0,
                lines: vec![DiffLine {
                    kind: LineChange::Removed,
                    content: "a".into(),
                    old_lineno: Some(3),
                    new_lineno: None,
                }],
            },
        ];
        let map = ChangeMap::from_hunks(&hunks);
        assert_eq!(map.deleted, vec![3]);
    }

    #[test]
    fn empty_hunks_produce_empty_map() {
        let map = ChangeMap::from_hunks(&[]);
        assert!(map.added.is_empty());
        assert!(map.modified.is_empty());
        assert!(map.deleted.is_empty());
    }
}
