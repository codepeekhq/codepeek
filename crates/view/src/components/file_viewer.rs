use std::path::PathBuf;

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use codepeek_core::{ChangeMap, DiffHunk, DiffLine, HighlightSpan, HighlightedLine, LineChange};

use crate::action::Action;
use crate::config;
use crate::keybindings;
use crate::render_helpers::{
    build_highlighted_spans_owned, dim_line_number, line_number_width, truncate_line,
};
use crate::theme;
use crate::theme::GutterMark;

struct ViewerLine {
    line_number: String,
    content: String,
    spans: Vec<HighlightSpan>,
    gutter_mark: GutterMark,
}

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

    pub fn handle_event(&mut self, key: KeyEvent) -> Action {
        if keybindings::is_move_up(&key) {
            if self.scroll_offset > 0 {
                self.scroll_offset -= 1;
            }
            Action::Noop
        } else if keybindings::is_move_down(&key) {
            let max = self.total_visible_lines().saturating_sub(1);
            if self.scroll_offset < max {
                self.scroll_offset += 1;
            }
            Action::Noop
        } else if keybindings::is_page_up(&key) {
            self.scroll_offset = self.scroll_offset.saturating_sub(config::PAGE_SCROLL_LINES);
            Action::Noop
        } else if keybindings::is_page_down(&key) {
            let max = self.total_visible_lines().saturating_sub(1);
            self.scroll_offset = (self.scroll_offset + config::PAGE_SCROLL_LINES).min(max);
            Action::Noop
        } else if keybindings::is_toggle_diff(&key) {
            self.show_diff = !self.show_diff;
            Action::ToggleDiff
        } else if keybindings::is_back(&key) {
            Action::Back
        } else if keybindings::is_quit(&key) {
            Action::Quit
        } else {
            Action::Noop
        }
    }

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

    fn build_diff_lines(&self) -> Vec<Line<'_>> {
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

            if let Some(removed) = removed_before.get(&line_num) {
                for dl in removed {
                    let content = truncate_line(&dl.content, config::MAX_LINE_LENGTH);
                    let spans = vec![
                        Span::styled("   - ", theme::diff_removed_style()),
                        Span::styled(content, theme::diff_removed_style()),
                    ];
                    result.push(Line::from(spans));
                }
            }

            if self.change_map.added.contains(&line_num)
                || self.change_map.modified.contains(&line_num)
            {
                let gutter_mark_text = theme::gutter_text(&vl.gutter_mark);
                let gutter_mark_style = theme::gutter_style(&vl.gutter_mark);
                let content = truncate_line(&vl.content, config::MAX_LINE_LENGTH);
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

        #[allow(clippy::cast_possible_truncation)]
        let after_last = (self.display_lines.len() as u32) + 1;
        for (&key, removed) in &removed_before {
            if key >= after_last {
                for dl in removed {
                    let content = truncate_line(&dl.content, config::MAX_LINE_LENGTH);
                    let spans = vec![
                        Span::styled("   - ", theme::diff_removed_style()),
                        Span::styled(content, theme::diff_removed_style()),
                    ];
                    result.push(Line::from(spans));
                }
            }
        }

        result
    }

    #[cfg(test)]
    pub fn is_loaded(&self) -> bool {
        !self.display_lines.is_empty()
    }
}

fn render_viewer_line(vl: &ViewerLine) -> Line<'_> {
    let gutter_mark_text = theme::gutter_text(&vl.gutter_mark);
    let gutter_mark_style = theme::gutter_style(&vl.gutter_mark);

    let mut spans = vec![
        dim_line_number(&vl.line_number),
        Span::styled(gutter_mark_text, gutter_mark_style),
    ];
    spans.extend(build_highlighted_spans_owned(
        &vl.content,
        &vl.spans,
        config::MAX_LINE_LENGTH,
    ));
    Line::from(spans)
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
    fn is_loaded_false_when_empty() {
        let viewer = FileViewer::new();
        assert!(!viewer.is_loaded());
    }

    #[test]
    fn is_loaded_true_when_content() {
        let mut viewer = FileViewer::new();
        viewer.load(PathBuf::from("test.rs"), "content");
        assert!(viewer.is_loaded());
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

        let x_count = content.matches('x').count();
        let max = config::MAX_LINE_LENGTH;
        assert!(
            x_count <= max,
            "line should be truncated at {max} chars, found {x_count}"
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

    #[test]
    fn page_up_and_down() {
        let mut viewer = FileViewer::new();
        let content: String = (0..50)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        viewer.load(PathBuf::from("test.rs"), &content);

        viewer.handle_event(make_key(KeyCode::PageDown));
        assert_eq!(viewer.scroll_offset, 20);

        viewer.handle_event(make_key(KeyCode::PageUp));
        assert_eq!(viewer.scroll_offset, 0);
    }

    #[test]
    fn k_and_j_keys_scroll() {
        let mut viewer = FileViewer::new();
        viewer.load(PathBuf::from("test.rs"), "a\nb\nc\nd\ne");

        viewer.handle_event(make_key(KeyCode::Char('j')));
        assert_eq!(viewer.scroll_offset, 1);

        viewer.handle_event(make_key(KeyCode::Char('k')));
        assert_eq!(viewer.scroll_offset, 0);
    }

    #[test]
    fn unknown_key_returns_noop() {
        let mut viewer = FileViewer::new();
        let action = viewer.handle_event(make_key(KeyCode::Char('x')));
        assert_eq!(action, Action::Noop);
    }

    #[test]
    fn diff_indicator_in_title() {
        let mut viewer = FileViewer::new();
        viewer.load(PathBuf::from("test.rs"), "content");
        viewer.show_diff = true;

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
        assert!(content.contains("[DIFF]"), "should show DIFF indicator");
    }
}
