use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme;

/// Stateless component that renders key binding hints in a status bar.
pub struct StatusBar;

impl StatusBar {
    /// Render key hints at the given area.
    ///
    /// Each hint is a `(key, description)` pair, e.g. `("q", "quit")`.
    pub fn render(hints: &[(&str, &str)], frame: &mut Frame, area: Rect) {
        let spans: Vec<Span> = hints
            .iter()
            .enumerate()
            .flat_map(|(i, (key, desc))| {
                let mut parts = vec![
                    Span::styled(
                        (*key).to_string(),
                        Style::default()
                            .fg(theme::TITLE_COLOR)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!(": {desc}"), Style::default().fg(theme::DIM_COLOR)),
                ];
                if i + 1 < hints.len() {
                    parts.push(Span::raw("  "));
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
}
