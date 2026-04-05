use std::path::PathBuf;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use codepeek_core::{HighlightSpan, HighlightedLine};

use crate::action::Action;
use crate::theme;

/// Pre-computed display data for a peek overlay line.
struct PeekLine {
    line_number: String,
    content: String,
    spans: Vec<HighlightSpan>,
}

/// Floating overlay that shows the HEAD content of a deleted file.
pub struct PeekOverlay {
    display_lines: Vec<PeekLine>,
    scroll_offset: usize,
    file_path: PathBuf,
}

impl PeekOverlay {
    pub fn new(path: PathBuf, lines: Vec<HighlightedLine>) -> Self {
        let gutter_width = line_number_width(lines.len());
        let display_lines = lines
            .into_iter()
            .enumerate()
            .map(|(i, hl)| PeekLine {
                line_number: format!("{:>width$}", i + 1, width = gutter_width),
                content: hl.content,
                spans: hl.spans,
            })
            .collect();

        Self {
            display_lines,
            scroll_offset: 0,
            file_path: path,
        }
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
                if self.scroll_offset + 1 < self.display_lines.len() {
                    self.scroll_offset += 1;
                }
                Action::Noop
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(20);
                Action::Noop
            }
            KeyCode::PageDown => {
                let max = self.display_lines.len().saturating_sub(1);
                self.scroll_offset = (self.scroll_offset + 20).min(max);
                Action::Noop
            }
            KeyCode::Esc => Action::DismissPeek,
            KeyCode::Char('q') => Action::Quit,
            _ => Action::Noop,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(area, 70, 80);

        // Clear the area behind the popup.
        frame.render_widget(Clear, popup);

        let title = format!(" Deleted: {} ", self.file_path.display());
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::DELETED_COLOR))
            .title(Span::styled(
                title,
                Style::default().fg(theme::DELETED_COLOR),
            ));

        let inner = block.inner(popup);
        let visible_height = inner.height as usize;

        let lines: Vec<Line> = self
            .display_lines
            .iter()
            .skip(self.scroll_offset)
            .take(visible_height)
            .map(|pl| {
                let mut spans = vec![Span::styled(
                    format!("{} ", pl.line_number),
                    Style::default().fg(theme::DIM_COLOR),
                )];
                spans.extend(build_highlighted_spans(&pl.content, &pl.spans));
                Line::from(spans)
            })
            .collect();

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, popup);
    }
}

/// Build ratatui `Span`s from highlighted line content and its `HighlightSpan`s.
fn build_highlighted_spans<'a>(content: &'a str, spans: &[HighlightSpan]) -> Vec<Span<'a>> {
    if spans.is_empty() {
        return vec![Span::raw(content)];
    }

    let mut result = Vec::new();
    let mut cursor = 0;

    for hs in spans {
        let start = hs.start.min(content.len());
        let end = hs.end.min(content.len());
        if start > cursor {
            result.push(Span::raw(&content[cursor..start]));
        }
        if start < end {
            result.push(Span::styled(
                &content[start..end],
                theme::highlight_style(hs.kind),
            ));
        }
        cursor = end;
    }

    if cursor < content.len() {
        result.push(Span::raw(&content[cursor..]));
    }

    result
}

/// Calculate a centered rectangle within `area` using percentage width/height.
fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let width = area.width * percent_x / 100;
    let height = area.height * percent_y / 100;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
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

    fn sample_lines() -> Vec<HighlightedLine> {
        vec![
            HighlightedLine {
                content: "fn main() {}".to_string(),
                spans: vec![HighlightSpan {
                    start: 0,
                    end: 2,
                    kind: HighlightKind::Keyword,
                }],
            },
            HighlightedLine {
                content: "    println!(\"hello\");".to_string(),
                spans: vec![],
            },
        ]
    }

    #[test]
    fn renders_with_title_and_content() {
        let overlay = PeekOverlay::new(PathBuf::from("deleted.rs"), sample_lines());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| overlay.render(frame, frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert!(
            content.contains("Deleted: deleted.rs"),
            "should show deleted file path in title"
        );
        assert!(content.contains("fn"), "should show file content");
    }

    #[test]
    fn esc_returns_dismiss_peek() {
        let mut overlay = PeekOverlay::new(PathBuf::from("test.rs"), sample_lines());
        let action = overlay.handle_event(make_key(KeyCode::Esc));
        assert_eq!(action, Action::DismissPeek);
    }

    #[test]
    fn scroll_works() {
        let lines: Vec<HighlightedLine> = (0..50)
            .map(|i| HighlightedLine {
                content: format!("line {i}"),
                spans: vec![],
            })
            .collect();
        let mut overlay = PeekOverlay::new(PathBuf::from("big.rs"), lines);

        assert_eq!(overlay.scroll_offset, 0);
        overlay.handle_event(make_key(KeyCode::Down));
        assert_eq!(overlay.scroll_offset, 1);
        overlay.handle_event(make_key(KeyCode::Up));
        assert_eq!(overlay.scroll_offset, 0);
    }

    #[test]
    fn centered_rect_computes_correct_dimensions() {
        let area = Rect::new(0, 0, 100, 50);
        let popup = centered_rect(area, 70, 80);
        assert_eq!(popup.width, 70);
        assert_eq!(popup.height, 40);
        assert_eq!(popup.x, 15);
        assert_eq!(popup.y, 5);
    }
}
