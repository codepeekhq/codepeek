use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme;

pub struct StatusBar;

impl StatusBar {
    pub fn render(hints: &[(&str, &str)], frame: &mut Frame, area: Rect) {
        let t = theme::current();
        let spans: Vec<Span> = hints
            .iter()
            .enumerate()
            .flat_map(|(i, (key, desc))| {
                let mut parts = vec![
                    Span::styled(
                        (*key).to_string(),
                        ratatui::style::Style::new().fg(t.status_key),
                    ),
                    Span::styled(
                        format!(" {desc}"),
                        ratatui::style::Style::new().fg(t.status_desc),
                    ),
                ];
                if i + 1 < hints.len() {
                    parts.push(Span::styled(
                        "   ",
                        ratatui::style::Style::new().fg(t.status_separator),
                    ));
                }
                parts
            })
            .collect();

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, area);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    #[test]
    fn renders_without_panic() {
        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                StatusBar::render(
                    &[("q", "quit"), ("\u{2191}\u{2193}", "navigate")],
                    frame,
                    frame.area(),
                );
            })
            .unwrap();
    }

    #[test]
    fn empty_hints_renders_without_panic() {
        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                StatusBar::render(&[], frame, frame.area());
            })
            .unwrap();
    }

    #[test]
    fn renders_hint_text() {
        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                StatusBar::render(&[("q", "quit"), ("r", "refresh")], frame, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(content.contains('q'), "should show key");
        assert!(content.contains("quit"), "should show description");
        assert!(content.contains('r'), "should show second key");
        assert!(
            content.contains("refresh"),
            "should show second description"
        );
    }
}
