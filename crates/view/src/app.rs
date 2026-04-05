use std::io;
use std::time::Duration;

use ratatui::DefaultTerminal;
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use codepeek_core::{ChangeDetector, ChangeKind, ChangeMap, SyntaxHighlighter};

use crate::action::Action;
use crate::components::{FileList, FileViewer, PeekOverlay, StatusBar};
use crate::theme;

const TICK_RATE: Duration = Duration::from_millis(16);

/// Which component currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    FileList,
    FileViewer,
}

pub struct App {
    should_quit: bool,
    focus: Focus,
    file_list: FileList,
    file_viewer: FileViewer,
    change_detector: Box<dyn ChangeDetector>,
    highlighter: Box<dyn SyntaxHighlighter>,
    /// Floating overlay showing HEAD content of a deleted file.
    peek_overlay: Option<PeekOverlay>,
    /// Transient error message shown in the status bar area.
    error_message: Option<String>,
}

impl App {
    pub fn new(
        change_detector: Box<dyn ChangeDetector>,
        highlighter: Box<dyn SyntaxHighlighter>,
    ) -> Result<Self, codepeek_core::ChangeError> {
        let files = change_detector.detect_changes()?;
        Ok(Self {
            should_quit: false,
            focus: Focus::FileList,
            file_list: FileList::new(files),
            file_viewer: FileViewer::new(),
            change_detector,
            highlighter,
            peek_overlay: None,
            error_message: None,
        })
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        // Reserve the bottom row for the status bar.
        let [main_area, status_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);

        match self.focus {
            Focus::FileList => {
                // File list fills the entire main area.
                self.file_list.render(frame, main_area);
                StatusBar::render(
                    &[
                        ("q", "quit"),
                        ("\u{2191}\u{2193}", "navigate"),
                        ("Enter", "open"),
                        ("r", "refresh"),
                    ],
                    frame,
                    status_area,
                );
            }
            Focus::FileViewer => {
                // Two-panel layout: file list (30%) | file viewer (70%).
                let [left, right] =
                    Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
                        .areas(main_area);

                self.file_list.render(frame, left);
                self.file_viewer.render(frame, right);
                StatusBar::render(
                    &[
                        ("Esc", "back"),
                        ("\u{2191}\u{2193}", "scroll"),
                        ("d", "diff"),
                        ("q", "quit"),
                    ],
                    frame,
                    status_area,
                );
            }
        }

        // Overlay error message on the status bar if present.
        if let Some(msg) = &self.error_message {
            let error_line = Line::from(vec![
                Span::styled(" ERROR ", Style::default().fg(Color::White).bg(Color::Red)),
                Span::styled(format!(" {msg}"), Style::default().fg(theme::DELETED_COLOR)),
            ]);
            frame.render_widget(Paragraph::new(error_line), status_area);
        }

        // Render peek overlay on top of everything when visible.
        if let Some(overlay) = &self.peek_overlay {
            overlay.render(frame, area);
        }
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(TICK_RATE)? {
            loop {
                if let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press
                {
                    // Clear any transient error message on the next keypress.
                    self.error_message = None;

                    let action = if let Some(overlay) = &mut self.peek_overlay {
                        overlay.handle_event(key)
                    } else {
                        match self.focus {
                            Focus::FileList => self.file_list.handle_event(key),
                            Focus::FileViewer => self.file_viewer.handle_event(key),
                        }
                    };
                    self.dispatch(&action);
                }

                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
        Ok(())
    }

    fn dispatch(&mut self, action: &Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::SelectFile(idx) => self.open_file(*idx),
            Action::Back => {
                self.focus = Focus::FileList;
            }
            Action::Refresh => self.refresh_files(),
            Action::DismissPeek => {
                self.peek_overlay = None;
            }
            Action::ToggleDiff
            | Action::Noop
            | Action::ScrollUp(_)
            | Action::ScrollDown(_)
            | Action::PeekDeleted(_) => {}
        }
    }

    fn open_file(&mut self, idx: usize) {
        let files = self.file_list.files();
        let Some(file) = files.get(idx) else {
            return;
        };

        let path = file.path.clone();
        let kind = file.kind.clone();

        // For deleted files, show a peek overlay with HEAD content.
        if kind == ChangeKind::Deleted {
            self.open_deleted_peek(&path);
            return;
        }

        // Read raw bytes first for binary detection.
        match std::fs::read(&path) {
            Ok(bytes) => {
                // Binary heuristic: null byte in the first 8KB.
                let check_len = bytes.len().min(8192);
                if bytes[..check_len].contains(&0) {
                    self.file_viewer
                        .load(path, &format!("[ Binary file ({} bytes) ]", bytes.len()));
                } else {
                    let content = String::from_utf8_lossy(&bytes);
                    self.open_with_highlighting(&path, &content, &kind);
                }
            }
            Err(e) => {
                self.file_viewer
                    .load(path.clone(), &format!("[ Error reading file: {e} ]"));
                self.error_message = Some(format!("Failed to read {}: {e}", path.display()));
            }
        }
        self.focus = Focus::FileViewer;
    }

    /// Open a file with syntax highlighting and diff information.
    fn open_with_highlighting(&mut self, path: &std::path::Path, content: &str, kind: &ChangeKind) {
        // Attempt syntax highlighting.
        let highlighted = match self.highlighter.highlight(content, path) {
            Ok(lines) => lines,
            Err(_) => {
                // Fall back to plain text.
                content
                    .lines()
                    .map(|l| codepeek_core::HighlightedLine {
                        content: l.to_string(),
                        spans: vec![],
                    })
                    .collect()
            }
        };

        // Compute diff hunks and build change map.
        let hunks = self.change_detector.compute_diff(path).unwrap_or_default();
        let mut change_map = ChangeMap::from_hunks(&hunks);

        // For newly added files, mark all lines as added.
        if *kind == ChangeKind::Added {
            #[allow(clippy::cast_possible_truncation)]
            let line_count = highlighted.len() as u32;
            for i in 1..=line_count {
                change_map.added.insert(i);
            }
        }

        self.file_viewer
            .load_highlighted(path.to_path_buf(), highlighted, change_map, hunks);
    }

    /// Open a peek overlay for a deleted file, reading content from HEAD.
    fn open_deleted_peek(&mut self, path: &std::path::Path) {
        match self.change_detector.read_at_head(path) {
            Ok(content) => {
                let highlighted = match self.highlighter.highlight(&content, path) {
                    Ok(lines) => lines,
                    Err(_) => content
                        .lines()
                        .map(|l| codepeek_core::HighlightedLine {
                            content: l.to_string(),
                            spans: vec![],
                        })
                        .collect(),
                };
                self.peek_overlay = Some(PeekOverlay::new(path.to_path_buf(), highlighted));
            }
            Err(e) => {
                self.error_message = Some(format!("Cannot read deleted file at HEAD: {e}"));
            }
        }
    }

    fn refresh_files(&mut self) {
        match self.change_detector.detect_changes() {
            Ok(files) => {
                self.file_list.update_files(files);
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Refresh failed: {e}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use codepeek_core::{
        ChangeError, ChangeKind, DiffHunk, FileChange, HighlightedLine, SyntaxError,
        SyntaxHighlighter,
    };

    use super::*;

    /// Stub change detector that returns a fixed list of files.
    struct StubDetector {
        files: Vec<FileChange>,
    }

    impl StubDetector {
        fn new(files: Vec<FileChange>) -> Self {
            Self { files }
        }

        fn empty() -> Self {
            Self { files: vec![] }
        }
    }

    impl ChangeDetector for StubDetector {
        fn detect_changes(&self) -> Result<Vec<FileChange>, ChangeError> {
            Ok(self.files.clone())
        }

        fn compute_diff(&self, _path: &Path) -> Result<Vec<DiffHunk>, ChangeError> {
            Ok(Vec::new())
        }

        fn read_at_head(&self, _path: &Path) -> Result<String, ChangeError> {
            Ok(String::new())
        }
    }

    /// Stub highlighter.
    struct StubHighlighter;

    impl SyntaxHighlighter for StubHighlighter {
        fn highlight(
            &mut self,
            source: &str,
            _path: &Path,
        ) -> Result<Vec<HighlightedLine>, SyntaxError> {
            Ok(source
                .lines()
                .map(|l| HighlightedLine {
                    content: l.to_string(),
                    spans: vec![],
                })
                .collect())
        }
    }

    fn sample_files() -> Vec<FileChange> {
        vec![
            FileChange {
                path: PathBuf::from("src/main.rs"),
                kind: ChangeKind::Modified,
                mtime: SystemTime::now(),
            },
            FileChange {
                path: PathBuf::from("src/new.rs"),
                kind: ChangeKind::Added,
                mtime: SystemTime::now(),
            },
        ]
    }

    #[test]
    fn app_with_empty_files_renders() {
        let app = App::new(Box::new(StubDetector::empty()), Box::new(StubHighlighter)).unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(content.contains("Changed Files"));
    }

    #[test]
    fn app_with_files_shows_file_list() {
        let app = App::new(
            Box::new(StubDetector::new(sample_files())),
            Box::new(StubHighlighter),
        )
        .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(content.contains("src/main.rs"), "should show file path");
        assert!(content.contains("src/new.rs"), "should show added file");
    }

    #[test]
    fn dispatch_quit_sets_should_quit() {
        let mut app = App::new(Box::new(StubDetector::empty()), Box::new(StubHighlighter)).unwrap();

        assert!(!app.should_quit);
        app.dispatch(&Action::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn dispatch_back_switches_to_file_list() {
        let mut app = App::new(
            Box::new(StubDetector::new(sample_files())),
            Box::new(StubHighlighter),
        )
        .unwrap();

        app.focus = Focus::FileViewer;
        app.dispatch(&Action::Back);
        assert_eq!(app.focus, Focus::FileList);
    }

    #[test]
    fn viewer_layout_renders_both_panels() {
        let mut app = App::new(
            Box::new(StubDetector::new(sample_files())),
            Box::new(StubHighlighter),
        )
        .unwrap();

        // Simulate opening a file by loading content directly.
        app.file_viewer
            .load(PathBuf::from("test.rs"), "fn main() {}\n");
        app.focus = Focus::FileViewer;

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(content.contains("Changed Files"), "should show file list");
        assert!(content.contains("test.rs"), "should show file viewer");
    }

    #[test]
    fn dispatch_refresh_calls_detect_changes() {
        let mut app = App::new(
            Box::new(StubDetector::new(sample_files())),
            Box::new(StubHighlighter),
        )
        .unwrap();

        assert_eq!(app.file_list.files().len(), 2);
        app.dispatch(&Action::Refresh);
        // StubDetector always returns the same files, so list is refreshed with same data.
        assert_eq!(app.file_list.files().len(), 2);
        assert!(app.error_message.is_none());
    }

    /// Stub detector that always fails.
    struct FailingDetector;

    impl ChangeDetector for FailingDetector {
        fn detect_changes(&self) -> Result<Vec<FileChange>, ChangeError> {
            Err(ChangeError::RepoNotFound {
                path: PathBuf::from("."),
            })
        }

        fn compute_diff(&self, _path: &Path) -> Result<Vec<DiffHunk>, ChangeError> {
            Ok(Vec::new())
        }

        fn read_at_head(&self, _path: &Path) -> Result<String, ChangeError> {
            Ok(String::new())
        }
    }

    #[test]
    fn refresh_failure_sets_error_message() {
        // Build app with a working detector first, then swap.
        let mut app = App::new(
            Box::new(StubDetector::new(sample_files())),
            Box::new(StubHighlighter),
        )
        .unwrap();

        // Replace the change_detector with a failing one.
        app.change_detector = Box::new(FailingDetector);
        app.dispatch(&Action::Refresh);

        assert!(
            app.error_message.is_some(),
            "should set error message on refresh failure"
        );
        assert!(
            app.error_message
                .as_ref()
                .unwrap()
                .contains("Refresh failed"),
            "error should mention refresh failure"
        );
    }

    #[test]
    fn error_message_renders_in_status_area() {
        let mut app = App::new(
            Box::new(StubDetector::new(sample_files())),
            Box::new(StubHighlighter),
        )
        .unwrap();

        app.error_message = Some("test error".to_string());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            content.contains("ERROR"),
            "should show ERROR label in status area"
        );
        assert!(
            content.contains("test error"),
            "should show error message text"
        );
    }

    #[test]
    fn status_bar_shows_refresh_hint() {
        let app = App::new(
            Box::new(StubDetector::new(sample_files())),
            Box::new(StubHighlighter),
        )
        .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            content.contains("r: refresh"),
            "status bar should show refresh hint in file list mode"
        );
    }

    #[test]
    fn status_bar_shows_diff_hint_in_viewer() {
        let mut app = App::new(
            Box::new(StubDetector::new(sample_files())),
            Box::new(StubHighlighter),
        )
        .unwrap();

        app.file_viewer.load(PathBuf::from("test.rs"), "content");
        app.focus = Focus::FileViewer;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            content.contains("d: diff"),
            "status bar should show diff hint in file viewer mode"
        );
    }

    #[test]
    fn dismiss_peek_clears_overlay() {
        let mut app = App::new(
            Box::new(StubDetector::new(sample_files())),
            Box::new(StubHighlighter),
        )
        .unwrap();

        // Manually set a peek overlay.
        app.peek_overlay = Some(PeekOverlay::new(
            PathBuf::from("deleted.rs"),
            vec![HighlightedLine {
                content: "old content".to_string(),
                spans: vec![],
            }],
        ));
        assert!(app.peek_overlay.is_some());

        app.dispatch(&Action::DismissPeek);
        assert!(app.peek_overlay.is_none());
    }

    #[test]
    fn peek_overlay_renders_on_top() {
        let mut app = App::new(
            Box::new(StubDetector::new(sample_files())),
            Box::new(StubHighlighter),
        )
        .unwrap();

        app.peek_overlay = Some(PeekOverlay::new(
            PathBuf::from("deleted.rs"),
            vec![HighlightedLine {
                content: "deleted content".to_string(),
                spans: vec![],
            }],
        ));

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            content.contains("Deleted: deleted.rs"),
            "peek overlay should show deleted file title"
        );
    }

    /// Stub detector that provides HEAD content for deleted files.
    struct StubDetectorWithHead {
        files: Vec<FileChange>,
        head_content: String,
    }

    impl ChangeDetector for StubDetectorWithHead {
        fn detect_changes(&self) -> Result<Vec<FileChange>, ChangeError> {
            Ok(self.files.clone())
        }

        fn compute_diff(&self, _path: &Path) -> Result<Vec<DiffHunk>, ChangeError> {
            Ok(Vec::new())
        }

        fn read_at_head(&self, _path: &Path) -> Result<String, ChangeError> {
            Ok(self.head_content.clone())
        }
    }

    #[test]
    fn open_deleted_file_shows_peek_overlay() {
        let files = vec![FileChange {
            path: PathBuf::from("removed.rs"),
            kind: ChangeKind::Deleted,
            mtime: SystemTime::now(),
        }];

        let detector = StubDetectorWithHead {
            files: files.clone(),
            head_content: "fn old_code() {}".to_string(),
        };

        let mut app = App::new(Box::new(detector), Box::new(StubHighlighter)).unwrap();

        app.dispatch(&Action::SelectFile(0));

        assert!(
            app.peek_overlay.is_some(),
            "opening a deleted file should show a peek overlay"
        );
    }
}
