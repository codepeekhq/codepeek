use std::path::PathBuf;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use codepeek_core::{ChangeMap, DiffHunk, DiffLine, HighlightSpan, HighlightedLine, LineChange};

use crate::action::Action;
use crate::theme;
use crate::theme::GutterMark;

/// Lines longer than this are truncated with an ellipsis during rendering.
const MAX_LINE_LENGTH: usize = 500;

/// Pre-computed display data for a single viewer line.
struct ViewerLine {
    line_number: String,
    content: String,
    spans: Vec<HighlightSpan>,
    gutter_mark: GutterMark,
}

/// Component that displays file content with syntax highlighting,
/// gutter marks, and optional diff view.
pub struct FileViewer {
    display_lines: Vec<ViewerLine>,
    scroll_offset: usize,
    file_path: Option<PathBuf>,
    change_map: ChangeMap,
    diff_hunks: Vec<DiffHunk>,
    show_diff: bool,
}

impl FileViewer {
    pub fn new() -> Self {
        Self {
            display_lines: Vec::new(),
            scroll_offset: 0,
            file_path: None,
            change_map: ChangeMap::default(),
            diff_hunks: Vec::new(),
            show_diff: false,
        }
    }

    /// Load highlighted file content with change information.
    pub fn load_highlighted(
        &mut self,
        path: PathBuf,
        lines: Vec<HighlightedLine>,
        change_map: ChangeMap,
        diff_hunks: Vec<DiffHunk>,
    ) {
        let gutter_width = line_number_width(lines.len());

        self.display_lines = lines
            .into_iter()
            .enumerate()
            .map(|(i, hl)| {
                #[allow(clippy::cast_possible_truncation)]
                let line_num = (i + 1) as u32;
                let gutter_mark = if change_map.added.contains(&line_num) {
                    GutterMark::Added
                } else if change_map.modified.contains(&line_num) {
                    GutterMark::Modified
                } else {
                    GutterMark::Unchanged
                };
                ViewerLine {
                    line_number: format!("{:>width$}", i + 1, width = gutter_width),
                    content: hl.content,
                    spans: hl.spans,
                    gutter_mark,
                }
            })
            .collect();

        self.scroll_offset = 0;
        self.file_path = Some(path);
        self.change_map = change_map;
        self.diff_hunks = diff_hunks;
        self.show_diff = false;
    }

    /// Load plain text content (fallback when highlighting fails or for messages).
    pub fn load(&mut self, path: PathBuf, content: &str) {
        let lines: Vec<HighlightedLine> = content
            .lines()
            .map(|l| HighlightedLine {
                content: l.to_string(),
                spans: vec![],
            })
            .collect();
        self.load_highlighted(path, lines, ChangeMap::default(), Vec::new());
    }

    /// Clear the viewer content.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.display_lines.clear();
        self.scroll_offset = 0;
        self.file_path = None;
        self.change_map = ChangeMap::default();
        self.diff_hunks = Vec::new();
        self.show_diff = false;
    }

    pub fn handle_event(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
                Action::Noop
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.total_visible_lines().saturating_sub(1);
                if self.scroll_offset < max {
                    self.scroll_offset += 1;
                }
                Action::Noop
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(20);
                Action::Noop
            }
            KeyCode::PageDown => {
                let max = self.total_visible_lines().saturating_sub(1);
                self.scroll_offset = (self.scroll_offset + 20).min(max);
                Action::Noop
            }
            KeyCode::Char('d') => {
                self.show_diff = !self.show_diff;
                Action::ToggleDiff
            }
            KeyCode::Esc => Action::Back,
            KeyCode::Char('q') => Action::Quit,
            _ => Action::Noop,
        }
    }

    /// Total number of renderable lines (including interleaved diff lines).
    fn total_visible_lines(&self) -> usize {
        if self.show_diff {
            self.build_diff_lines().len()
        } else {
            self.display_lines.len()
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let title = self
            .file_path
            .as_ref()
            .map_or_else(|| " No File ".to_string(), |p| format!(" {} ", p.display()));

        let diff_indicator = if self.show_diff { " [DIFF] " } else { "" };
        let full_title = format!("{title}{diff_indicator}");

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::BORDER_COLOR))
            .title(Span::styled(
                full_title,
                Style::default().fg(theme::TITLE_COLOR),
            ));

        let inner = block.inner(area);
        let visible_height = inner.height as usize;

        let lines: Vec<Line> = if self.show_diff {
            self.build_diff_lines()
                .into_iter()
                .skip(self.scroll_offset)
                .take(visible_height)
                .collect()
        } else {
            self.display_lines
                .iter()
                .skip(self.scroll_offset)
                .take(visible_height)
                .map(|vl| render_viewer_line(vl))
                .collect()
        };

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }

    /// Build all lines for diff view, interleaving removed lines from hunks.
    fn build_diff_lines(&self) -> Vec<Line<'_>> {
        // Build a map of new_lineno -> removed lines that come before it.
        let mut removed_before: std::collections::BTreeMap<u32, Vec<&DiffLine>> =
            std::collections::BTreeMap::new();

        for hunk in &self.diff_hunks {
            let mut pending_removed: Vec<&DiffLine> = Vec::new();
            let mut last_new_lineno: Option<u32> = None;

            for dl in &hunk.lines {
                match dl.kind {
                    LineChange::Removed => {
                        pending_removed.push(dl);
                    }
                    LineChange::Added | LineChange::Modified => {
                        if let Some(n) = dl.new_lineno {
                            if !pending_removed.is_empty() {
                                removed_before
                                    .entry(n)
                                    .or_default()
                                    .append(&mut pending_removed);
                            }
                            last_new_lineno = Some(n);
                        }
                    }
                }
            }
            // Any remaining removed lines go after the last new line in the hunk.
            if !pending_removed.is_empty() {
                let key = last_new_lineno.map_or(hunk.new_start, |n| n + 1);
                removed_before
                    .entry(key)
                    .or_default()
                    .append(&mut pending_removed);
            }
        }

        let mut result: Vec<Line<'_>> = Vec::new();

        for (i, vl) in self.display_lines.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let line_num = (i + 1) as u32;

            // Insert any removed lines before this line.
            if let Some(removed) = removed_before.get(&line_num) {
                for dl in removed {
                    let mut spans = vec![Span::styled("   - ", theme::diff_removed_style())];
                    let content = truncate_line(&dl.content);
                    spans.push(Span::styled(content, theme::diff_removed_style()));
                    result.push(Line::from(spans));
                }
            }

            // Render the current line with diff styling if it's added/modified.
            if self.change_map.added.contains(&line_num)
                || self.change_map.modified.contains(&line_num)
            {
                let gutter_mark_text = theme::gutter_text(&vl.gutter_mark);
                let gutter_mark_style = theme::gutter_style(&vl.gutter_mark);
                let content = truncate_line(&vl.content);
                let spans = vec![
                    Span::styled(format!("{} ", vl.line_number), theme::diff_added_style()),
                    Span::styled(gutter_mark_text, gutter_mark_style),
                    Span::styled(content, theme::diff_added_style()),
                ];
                result.push(Line::from(spans));
            } else {
                result.push(render_viewer_line(vl));
            }
        }

        // Handle any removed lines that come after the last source line.
        #[allow(clippy::cast_possible_truncation)]
        let after_last = (self.display_lines.len() as u32) + 1;
        for (&key, removed) in &removed_before {
            if key >= after_last {
                for dl in removed {
                    let mut spans = vec![Span::styled("   - ", theme::diff_removed_style())];
                    let content = truncate_line(&dl.content);
                    spans.push(Span::styled(content, theme::diff_removed_style()));
                    result.push(Line::from(spans));
                }
            }
        }

        result
    }

    /// Returns true if the viewer has content loaded.
    #[allow(dead_code)]
    pub fn is_loaded(&self) -> bool {
        !self.display_lines.is_empty()
    }
}

/// Render a single viewer line with line number, gutter mark, and highlighted content.
fn render_viewer_line(vl: &ViewerLine) -> Line<'_> {
    let gutter_mark_text = theme::gutter_text(&vl.gutter_mark);
    let gutter_mark_style = theme::gutter_style(&vl.gutter_mark);

    let mut spans = vec![
        Span::styled(
            format!("{} ", vl.line_number),
            Style::default().fg(theme::DIM_COLOR),
        ),
        Span::styled(gutter_mark_text, gutter_mark_style),
    ];
    spans.extend(build_highlighted_spans(&vl.content, &vl.spans));
    Line::from(spans)
}

/// Build ratatui `Span`s from highlighted line content and its `HighlightSpan`s.
fn build_highlighted_spans<'a>(content: &'a str, spans: &[HighlightSpan]) -> Vec<Span<'a>> {
    let truncated;
    let display_content = if content.len() > MAX_LINE_LENGTH {
        truncated = format!("{}\u{2026}", &content[..MAX_LINE_LENGTH]);
        &truncated
    } else {
        content
    };

    if spans.is_empty() {
        return vec![Span::raw(display_content.to_string())];
    }

    let mut result = Vec::new();
    let mut cursor = 0;

    for hs in spans {
        let start = hs.start.min(display_content.len());
        let end = hs.end.min(display_content.len());
        if start > cursor {
            result.push(Span::raw(display_content[cursor..start].to_string()));
        }
        if start < end {
            result.push(Span::styled(
                display_content[start..end].to_string(),
                theme::highlight_style(hs.kind),
            ));
        }
        cursor = end;
    }

    if cursor < display_content.len() {
        result.push(Span::raw(display_content[cursor..].to_string()));
    }

    result
}

/// Truncate a line to `MAX_LINE_LENGTH`, appending ellipsis if needed.
fn truncate_line(content: &str) -> String {
    if content.len() > MAX_LINE_LENGTH {
        let mut truncated = content[..MAX_LINE_LENGTH].to_string();
        truncated.push('\u{2026}');
        truncated
    } else {
        content.to_string()
    }
}

/// Calculate the width needed for line numbers.
fn line_number_width(total_lines: usize) -> usize {
    if total_lines == 0 {
        1
    } else {
        total_lines.ilog10() as usize + 1
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    use codepeek_core::{HighlightKind, HighlightSpan, HighlightedLine};

    use super::*;

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn empty_viewer_renders_without_panic() {
        let viewer = FileViewer::new();
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| viewer.render(frame, frame.area()))
            .unwrap();
    }

    #[test]
    fn renders_line_numbers() {
        let mut viewer = FileViewer::new();
        viewer.load(PathBuf::from("test.rs"), "line one\nline two\nline three");

        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| viewer.render(frame, frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert!(content.contains("1 "), "should contain line number 1");
        assert!(content.contains("line one"), "should contain line one");
        assert!(content.contains("2 "), "should contain line number 2");
        assert!(content.contains("line two"), "should contain line two");
        assert!(content.contains("3 "), "should contain line number 3");
        assert!(content.contains("line three"), "should contain line three");
    }

    #[test]
    fn esc_returns_back() {
        let mut viewer = FileViewer::new();
        let action = viewer.handle_event(make_key(KeyCode::Esc));
        assert_eq!(action, Action::Back);
    }

    #[test]
    fn q_returns_quit() {
        let mut viewer = FileViewer::new();
        let action = viewer.handle_event(make_key(KeyCode::Char('q')));
        assert_eq!(action, Action::Quit);
    }

    #[test]
    fn scroll_clamps_at_top() {
        let mut viewer = FileViewer::new();
        viewer.load(PathBuf::from("test.rs"), "a\nb\nc");
        assert_eq!(viewer.scroll_offset, 0);
        viewer.handle_event(make_key(KeyCode::Up));
        assert_eq!(viewer.scroll_offset, 0);
    }

    #[test]
    fn scroll_down_and_up() {
        let mut viewer = FileViewer::new();
        viewer.load(PathBuf::from("test.rs"), "a\nb\nc\nd\ne");

        viewer.handle_event(make_key(KeyCode::Down));
        assert_eq!(viewer.scroll_offset, 1);

        viewer.handle_event(make_key(KeyCode::Down));
        assert_eq!(viewer.scroll_offset, 2);

        viewer.handle_event(make_key(KeyCode::Up));
        assert_eq!(viewer.scroll_offset, 1);
    }

    #[test]
    fn clear_resets_state() {
        let mut viewer = FileViewer::new();
        viewer.load(PathBuf::from("test.rs"), "content");
        assert!(viewer.is_loaded());

        viewer.clear();
        assert!(!viewer.is_loaded());
        assert!(viewer.file_path.is_none());
    }

    #[test]
    fn line_number_width_for_various_sizes() {
        assert_eq!(line_number_width(0), 1);
        assert_eq!(line_number_width(1), 1);
        assert_eq!(line_number_width(9), 1);
        assert_eq!(line_number_width(10), 2);
        assert_eq!(line_number_width(99), 2);
        assert_eq!(line_number_width(100), 3);
        assert_eq!(line_number_width(999), 3);
        assert_eq!(line_number_width(1000), 4);
    }

    #[test]
    fn empty_file_renders_gracefully() {
        let mut viewer = FileViewer::new();
        viewer.load(PathBuf::from("empty.rs"), "");

        assert!(!viewer.is_loaded());

        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| viewer.render(frame, frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            content.contains("empty.rs"),
            "should show file name for empty file"
        );
    }

    #[test]
    fn long_line_truncated_in_render() {
        let mut viewer = FileViewer::new();
        let long_line = "x".repeat(600);
        viewer.load(PathBuf::from("long.rs"), &long_line);

        let backend = TestBackend::new(800, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| viewer.render(frame, frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        // The original data should not contain 600 x's, it should be truncated.
        let x_count = content.matches('x').count();
        assert!(
            x_count <= MAX_LINE_LENGTH,
            "line should be truncated at {MAX_LINE_LENGTH} chars, found {x_count}"
        );
        assert!(
            content.contains('\u{2026}'),
            "truncated line should end with ellipsis"
        );
    }

    #[test]
    fn short_line_not_truncated() {
        let mut viewer = FileViewer::new();
        viewer.load(PathBuf::from("short.rs"), "short line");

        let backend = TestBackend::new(60, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| viewer.render(frame, frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            content.contains("short line"),
            "short lines should not be truncated"
        );
        assert!(
            !content.contains('\u{2026}'),
            "short lines should not have ellipsis"
        );
    }

    #[test]
    fn d_key_toggles_diff() {
        let mut viewer = FileViewer::new();
        viewer.load(PathBuf::from("test.rs"), "content");
        assert!(!viewer.show_diff);

        let action = viewer.handle_event(make_key(KeyCode::Char('d')));
        assert_eq!(action, Action::ToggleDiff);
        assert!(viewer.show_diff);

        let action = viewer.handle_event(make_key(KeyCode::Char('d')));
        assert_eq!(action, Action::ToggleDiff);
        assert!(!viewer.show_diff);
    }

    #[test]
    fn highlighted_lines_render_with_syntax_colors() {
        let lines = vec![
            HighlightedLine {
                content: "fn main() {}".to_string(),
                spans: vec![HighlightSpan {
                    start: 0,
                    end: 2,
                    kind: HighlightKind::Keyword,
                }],
            },
            HighlightedLine {
                content: "    let x = 42;".to_string(),
                spans: vec![
                    HighlightSpan {
                        start: 4,
                        end: 7,
                        kind: HighlightKind::Keyword,
                    },
                    HighlightSpan {
                        start: 12,
                        end: 14,
                        kind: HighlightKind::Number,
                    },
                ],
            },
        ];

        let mut viewer = FileViewer::new();
        viewer.load_highlighted(
            PathBuf::from("test.rs"),
            lines,
            ChangeMap::default(),
            Vec::new(),
        );

        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| viewer.render(frame, frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert!(
            content.contains("fn main()"),
            "should render highlighted content"
        );
        assert!(content.contains("let x = 42"), "should render second line");
    }

    #[test]
    fn gutter_marks_show_for_changed_lines() {
        let lines = vec![
            HighlightedLine {
                content: "unchanged line".to_string(),
                spans: vec![],
            },
            HighlightedLine {
                content: "added line".to_string(),
                spans: vec![],
            },
            HighlightedLine {
                content: "modified line".to_string(),
                spans: vec![],
            },
        ];

        let mut change_map = ChangeMap::default();
        change_map.added.insert(2);
        change_map.modified.insert(3);

        let mut viewer = FileViewer::new();
        viewer.load_highlighted(PathBuf::from("test.rs"), lines, change_map, Vec::new());

        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| viewer.render(frame, frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        // Gutter marks use thin vertical bars for added/modified.
        assert!(
            content.contains('\u{258e}'),
            "should show gutter mark for changed lines"
        );
    }

    #[test]
    fn diff_toggle_shows_removed_lines() {
        use codepeek_core::{DiffHunk, DiffLine, LineChange};

        let lines = vec![
            HighlightedLine {
                content: "line one".to_string(),
                spans: vec![],
            },
            HighlightedLine {
                content: "new line two".to_string(),
                spans: vec![],
            },
        ];

        let mut change_map = ChangeMap::default();
        change_map.added.insert(2);

        let hunks = vec![DiffHunk {
            old_start: 2,
            old_lines: 1,
            new_start: 2,
            new_lines: 1,
            lines: vec![
                DiffLine {
                    kind: LineChange::Removed,
                    content: "old line two".to_string(),
                    old_lineno: Some(2),
                    new_lineno: None,
                },
                DiffLine {
                    kind: LineChange::Added,
                    content: "new line two".to_string(),
                    old_lineno: None,
                    new_lineno: Some(2),
                },
            ],
        }];

        let mut viewer = FileViewer::new();
        viewer.load_highlighted(PathBuf::from("test.rs"), lines, change_map, hunks);

        // Toggle diff on.
        viewer.handle_event(make_key(KeyCode::Char('d')));
        assert!(viewer.show_diff);

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| viewer.render(frame, frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert!(
            content.contains("old line two"),
            "diff view should show removed lines"
        );
        assert!(
            content.contains("new line two"),
            "diff view should show added lines"
        );
    }
}
