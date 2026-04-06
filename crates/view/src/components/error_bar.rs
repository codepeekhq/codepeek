use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::Theme;

pub struct ErrorBar;

impl ErrorBar {
    pub fn render(msg: &str, frame: &mut Frame, area: Rect, theme: &Theme) {
        let error_line = Line::from(vec![
            Span::styled(" ERROR ", theme.ui.error_badge),
            Span::styled(format!(" {msg}"), theme.ui.error_text),
        ]);
        frame.render_widget(
            Paragraph::new(error_line).alignment(Alignment::Center),
            area,
        );
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::theme;

    #[test]
    fn renders_without_panic() {
        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                ErrorBar::render("something broke", frame, frame.area(), theme::current());
            })
            .unwrap();
    }

    #[test]
    fn shows_error_label_and_message() {
        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| ErrorBar::render("test error", frame, frame.area(), theme::current()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(content.contains("ERROR"), "should show ERROR label");
        assert!(content.contains("test error"), "should show error message");
    }
}
