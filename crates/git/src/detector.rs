use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

use git2::{DiffFormat, DiffOptions, Repository, Status, StatusOptions};

use codepeek_core::traits::ChangeDetector;
use codepeek_core::{ChangeError, ChangeKind, DiffHunk, DiffLine, FileChange, LineChange};

pub struct GitChangeDetector {
    repo: Mutex<Repository>,
}

impl GitChangeDetector {
    pub fn open(path: &Path) -> Result<Self, ChangeError> {
        let repo = Repository::discover(path).map_err(|_| ChangeError::RepoNotFound {
            path: path.to_path_buf(),
        })?;
        Ok(Self {
            repo: Mutex::new(repo),
        })
    }
}

impl ChangeDetector for GitChangeDetector {
    fn detect_changes(&self) -> Result<Vec<FileChange>, ChangeError> {
        let repo = self.repo.lock().expect("repo mutex poisoned");

        if repo.head().is_err() {
            return Ok(Vec::new());
        }

        let statuses = repo
            .statuses(Some(
                StatusOptions::new()
                    .include_untracked(true)
                    .recurse_untracked_dirs(true),
            ))
            .map_err(|e| ChangeError::StatusFailed(Box::new(e)))?;

        let workdir = repo.workdir().ok_or_else(|| {
            ChangeError::StatusFailed("bare repository has no working directory".into())
        })?;

        let mut changes = Vec::new();

        for entry in statuses.iter() {
            let Some(rel_path) = entry.path() else {
                continue;
            };

            let status = entry.status();
            let kind = if status.intersects(Status::INDEX_DELETED | Status::WT_DELETED) {
                ChangeKind::Deleted
            } else if status.intersects(Status::INDEX_NEW | Status::WT_NEW) {
                ChangeKind::Added
            } else if status.intersects(Status::INDEX_RENAMED) {
                let from = entry
                    .head_to_index()
                    .and_then(|delta| delta.old_file().path().map(Path::to_path_buf))
                    .unwrap_or_default();
                ChangeKind::Renamed { from }
            } else if status.intersects(Status::INDEX_MODIFIED | Status::WT_MODIFIED) {
                ChangeKind::Modified
            } else {
                continue;
            };

            let mtime = if matches!(kind, ChangeKind::Deleted) {
                SystemTime::UNIX_EPOCH
            } else {
                let abs = workdir.join(rel_path);
                std::fs::metadata(&abs)
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH)
            };

            changes.push(FileChange {
                path: rel_path.into(),
                kind,
                mtime,
            });
        }

        changes.sort_by(|a, b| b.mtime.cmp(&a.mtime));
        Ok(changes)
    }

    fn compute_diff(&self, path: &Path) -> Result<Vec<DiffHunk>, ChangeError> {
        let repo = self.repo.lock().expect("repo mutex poisoned");

        let head_tree =
            repo.head()
                .and_then(|h| h.peel_to_tree())
                .map_err(|e| ChangeError::DiffFailed {
                    path: path.to_path_buf(),
                    source: Box::new(e),
                })?;

        let mut diff_opts = DiffOptions::new();
        diff_opts.pathspec(path);

        let diff = repo
            .diff_tree_to_workdir_with_index(Some(&head_tree), Some(&mut diff_opts))
            .map_err(|e| ChangeError::DiffFailed {
                path: path.to_path_buf(),
                source: Box::new(e),
            })?;

        let mut hunks: Vec<DiffHunk> = Vec::new();

        diff.print(DiffFormat::Patch, |_delta, hunk, line| {
            if let Some(h) = hunk {
                // Push a new hunk if we haven't seen this one yet
                let needs_new = hunks.last().is_none_or(|last: &DiffHunk| {
                    last.old_start != h.old_start() || last.new_start != h.new_start()
                });
                if needs_new {
                    hunks.push(DiffHunk {
                        old_start: h.old_start(),
                        old_lines: h.old_lines(),
                        new_start: h.new_start(),
                        new_lines: h.new_lines(),
                        lines: Vec::new(),
                    });
                }
            }

            let kind = match line.origin() {
                '+' => LineChange::Added,
                '-' => LineChange::Removed,
                _ => return true, // context / header lines, skip
            };

            let content = String::from_utf8_lossy(line.content()).to_string();

            let diff_line = DiffLine {
                kind,
                content,
                old_lineno: line.old_lineno(),
                new_lineno: line.new_lineno(),
            };

            if let Some(current_hunk) = hunks.last_mut() {
                current_hunk.lines.push(diff_line);
            }
            true
        })
        .map_err(|e| ChangeError::DiffFailed {
            path: path.to_path_buf(),
            source: Box::new(e),
        })?;

        Ok(hunks)
    }

    fn read_at_head(&self, path: &Path) -> Result<String, ChangeError> {
        let repo = self.repo.lock().expect("repo mutex poisoned");

        let head_tree =
            repo.head()
                .and_then(|h| h.peel_to_tree())
                .map_err(|_| ChangeError::FileNotInHead {
                    path: path.to_path_buf(),
                })?;

        let entry = head_tree
            .get_path(path)
            .map_err(|_| ChangeError::FileNotInHead {
                path: path.to_path_buf(),
            })?;

        let object = entry
            .to_object(&repo)
            .map_err(|_| ChangeError::FileNotInHead {
                path: path.to_path_buf(),
            })?;

        let blob = object.as_blob().ok_or_else(|| ChangeError::FileNotInHead {
            path: path.to_path_buf(),
        })?;

        String::from_utf8(blob.content().to_vec()).map_err(|_| ChangeError::FileNotInHead {
            path: path.to_path_buf(),
        })
    }
}
