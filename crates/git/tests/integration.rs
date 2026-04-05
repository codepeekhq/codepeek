use std::fs;
use std::path::Path;

use git2::{Repository, Signature};
use tempfile::tempdir;

use codepeek_core::ChangeKind;
use codepeek_core::traits::ChangeDetector;
use codepeek_git::GitChangeDetector;

fn init_repo_with_commit(dir: &Path) -> Repository {
    let repo = Repository::init(dir).expect("failed to init repo");

    // Configure a test identity so commits work in CI
    let mut config = repo.config().expect("failed to get config");
    config
        .set_str("user.name", "Test")
        .expect("failed to set user.name");
    config
        .set_str("user.email", "test@test.com")
        .expect("failed to set user.email");

    // Create an initial file and commit it
    let file_path = dir.join("hello.txt");
    fs::write(&file_path, "hello\n").expect("failed to write file");

    let mut index = repo.index().expect("failed to get index");
    index
        .add_path(Path::new("hello.txt"))
        .expect("failed to add path");
    index.write().expect("failed to write index");

    let tree_oid = index.write_tree().expect("failed to write tree");
    {
        let tree = repo.find_tree(tree_oid).expect("failed to find tree");
        let sig = Signature::now("Test", "test@test.com").expect("failed to create signature");
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
            .expect("failed to commit");
    }

    repo
}

#[test]
fn modified_file_detected() {
    let dir = tempdir().expect("failed to create tempdir");
    init_repo_with_commit(dir.path());

    fs::write(dir.path().join("hello.txt"), "hello world\n").expect("failed to modify file");

    let detector = GitChangeDetector::open(dir.path()).expect("failed to open detector");
    let changes = detector.detect_changes().expect("failed to detect changes");

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, Path::new("hello.txt"));
    assert_eq!(changes[0].kind, ChangeKind::Modified);
}

#[test]
fn added_file_detected() {
    let dir = tempdir().expect("failed to create tempdir");
    init_repo_with_commit(dir.path());

    fs::write(dir.path().join("new_file.txt"), "new content\n").expect("failed to write new file");

    let detector = GitChangeDetector::open(dir.path()).expect("failed to open detector");
    let changes = detector.detect_changes().expect("failed to detect changes");

    let added: Vec<_> = changes
        .iter()
        .filter(|c| c.kind == ChangeKind::Added)
        .collect();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].path, Path::new("new_file.txt"));

    // The committed hello.txt should appear as Unchanged
    let unchanged: Vec<_> = changes
        .iter()
        .filter(|c| c.kind == ChangeKind::Unchanged)
        .collect();
    assert_eq!(unchanged.len(), 1);
    assert_eq!(unchanged[0].path, Path::new("hello.txt"));
}

#[test]
fn deleted_file_detected() {
    let dir = tempdir().expect("failed to create tempdir");
    let repo = init_repo_with_commit(dir.path());

    fs::remove_file(dir.path().join("hello.txt")).expect("failed to delete file");

    // Stage the deletion so git sees it as INDEX_DELETED
    let mut index = repo.index().expect("failed to get index");
    index
        .remove_path(Path::new("hello.txt"))
        .expect("failed to remove from index");
    index.write().expect("failed to write index");

    let detector = GitChangeDetector::open(dir.path()).expect("failed to open detector");
    let changes = detector.detect_changes().expect("failed to detect changes");

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, Path::new("hello.txt"));
    assert_eq!(changes[0].kind, ChangeKind::Deleted);
}

#[test]
fn diff_for_modified_file() {
    let dir = tempdir().expect("failed to create tempdir");
    init_repo_with_commit(dir.path());

    fs::write(dir.path().join("hello.txt"), "hello world\n").expect("failed to modify file");

    let detector = GitChangeDetector::open(dir.path()).expect("failed to open detector");
    let hunks = detector
        .compute_diff(Path::new("hello.txt"))
        .expect("failed to compute diff");

    assert!(!hunks.is_empty(), "expected at least one hunk");

    let all_lines: Vec<_> = hunks.iter().flat_map(|h| &h.lines).collect();
    assert!(
        all_lines
            .iter()
            .any(|l| l.kind == codepeek_core::LineChange::Removed),
        "expected a removed line"
    );
    assert!(
        all_lines
            .iter()
            .any(|l| l.kind == codepeek_core::LineChange::Added),
        "expected an added line"
    );
}

#[test]
fn read_original_content_at_head() {
    let dir = tempdir().expect("failed to create tempdir");
    init_repo_with_commit(dir.path());

    fs::write(dir.path().join("hello.txt"), "changed content\n").expect("failed to modify file");

    let detector = GitChangeDetector::open(dir.path()).expect("failed to open detector");
    let content = detector
        .read_at_head(Path::new("hello.txt"))
        .expect("failed to read at HEAD");

    assert_eq!(content, "hello\n");
}

#[test]
fn empty_repo_returns_no_changes() {
    let dir = tempdir().expect("failed to create tempdir");
    Repository::init(dir.path()).expect("failed to init repo");

    // No commits yet -- should return empty, not error
    let detector = GitChangeDetector::open(dir.path()).expect("failed to open detector");
    let changes = detector.detect_changes().expect("failed to detect changes");

    assert!(changes.is_empty());
}

#[test]
fn untracked_files_in_subdirectory_listed_individually() {
    let dir = tempdir().expect("failed to create tempdir");
    init_repo_with_commit(dir.path());

    // Create untracked files inside a new directory
    let sub = dir.path().join("src");
    fs::create_dir_all(&sub).expect("failed to create dir");
    fs::write(sub.join("foo.rs"), "fn foo() {}").expect("failed to write");
    fs::write(sub.join("bar.rs"), "fn bar() {}").expect("failed to write");

    let detector = GitChangeDetector::open(dir.path()).expect("failed to open detector");
    let changes = detector.detect_changes().expect("failed to detect changes");

    let paths: Vec<_> = changes
        .iter()
        .map(|c| c.path.to_string_lossy().to_string())
        .collect();

    // Must list individual files, NOT the directory "src/"
    assert!(
        paths.contains(&"src/foo.rs".to_string()),
        "expected src/foo.rs in {paths:?}"
    );
    assert!(
        paths.contains(&"src/bar.rs".to_string()),
        "expected src/bar.rs in {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p == "src/" || p == "src"),
        "should not contain directory entry, got {paths:?}"
    );
}

#[test]
fn untracked_files_in_deeply_nested_directory_listed_individually() {
    let dir = tempdir().expect("failed to create tempdir");
    init_repo_with_commit(dir.path());

    // Create a file 3 levels deep
    let deep = dir.path().join("a").join("b").join("c");
    fs::create_dir_all(&deep).expect("failed to create dirs");
    fs::write(deep.join("deep.txt"), "deep content").expect("failed to write");

    let detector = GitChangeDetector::open(dir.path()).expect("failed to open detector");
    let changes = detector.detect_changes().expect("failed to detect changes");

    let paths: Vec<_> = changes
        .iter()
        .map(|c| c.path.to_string_lossy().to_string())
        .collect();

    assert!(
        paths.contains(&"a/b/c/deep.txt".to_string()),
        "expected a/b/c/deep.txt in {paths:?}"
    );
    // Must not contain any directory entries
    for p in &paths {
        assert!(
            !p.ends_with('/') && p.contains('.'),
            "entry '{p}' looks like a directory, not a file"
        );
    }
}

#[test]
fn read_at_head_missing_file_errors() {
    let dir = tempdir().expect("failed to create tempdir");
    init_repo_with_commit(dir.path());

    let detector = GitChangeDetector::open(dir.path()).expect("failed to open detector");
    let result = detector.read_at_head(Path::new("nonexistent.txt"));

    assert!(result.is_err());
}

// git2 status doesn't detect renames without find_similar() on the diff.
// Staging a remove+add shows as Deleted+Added, not Renamed. This test
// verifies the current behavior: rename appears as a delete + add pair.
#[test]
fn rename_via_stage_shows_as_delete_and_add() {
    let dir = tempdir().expect("failed to create tempdir");
    let repo = init_repo_with_commit(dir.path());

    let old_path = dir.path().join("hello.txt");
    let new_path = dir.path().join("greeting.txt");
    fs::rename(&old_path, &new_path).expect("failed to rename file");

    let mut index = repo.index().expect("failed to get index");
    index
        .remove_path(Path::new("hello.txt"))
        .expect("failed to remove old path");
    index
        .add_path(Path::new("greeting.txt"))
        .expect("failed to add new path");
    index.write().expect("failed to write index");

    let detector = GitChangeDetector::open(dir.path()).expect("failed to open detector");
    let changes = detector.detect_changes().expect("failed to detect changes");

    assert_eq!(changes.len(), 2, "expected delete + add, got: {changes:?}");
    let has_deleted = changes.iter().any(|c| c.kind == ChangeKind::Deleted);
    let has_added = changes.iter().any(|c| c.kind == ChangeKind::Added);
    assert!(has_deleted, "expected a deleted entry for the old file");
    assert!(has_added, "expected an added entry for the new file");
}

#[test]
fn diff_for_added_file() {
    let dir = tempdir().expect("failed to create tempdir");
    let repo = init_repo_with_commit(dir.path());

    fs::write(dir.path().join("brand_new.txt"), "new content\n").expect("failed to write");

    let mut index = repo.index().expect("failed to get index");
    index
        .add_path(Path::new("brand_new.txt"))
        .expect("failed to add");
    index.write().expect("failed to write index");

    let detector = GitChangeDetector::open(dir.path()).expect("failed to open detector");
    let hunks = detector
        .compute_diff(Path::new("brand_new.txt"))
        .expect("failed to compute diff");

    assert!(
        !hunks.is_empty(),
        "expected at least one hunk for added file"
    );
    let has_added = hunks
        .iter()
        .flat_map(|h| &h.lines)
        .any(|l| l.kind == codepeek_core::LineChange::Added);
    assert!(has_added, "expected added lines in diff");
}

#[test]
fn multiple_changes_sorted_by_mtime() {
    let dir = tempdir().expect("failed to create tempdir");
    init_repo_with_commit(dir.path());

    fs::write(dir.path().join("first.txt"), "first").expect("failed to write");
    std::thread::sleep(std::time::Duration::from_millis(50));
    fs::write(dir.path().join("second.txt"), "second").expect("failed to write");

    let detector = GitChangeDetector::open(dir.path()).expect("failed to open detector");
    let changes = detector.detect_changes().expect("failed to detect changes");

    assert!(changes.len() >= 2);
    assert!(
        changes[0].mtime >= changes[1].mtime,
        "changes should be sorted by mtime descending"
    );
}

#[test]
fn open_nonexistent_repo_errors() {
    let result = GitChangeDetector::open(Path::new("/nonexistent/path"));
    assert!(result.is_err());
}
