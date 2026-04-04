use std::io;
use std::time::Duration;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, layout::Alignment};

const TICK_RATE: Duration = Duration::from_millis(16);

pub struct App {
    should_quit: bool,
    greeting: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            greeting: format!("codepeek v{}", codepeek_core::version()),
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let paragraph = Paragraph::new(self.greeting.as_str()).alignment(Alignment::Center);
        frame.render_widget(paragraph, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(TICK_RATE)? {
            loop {
                if let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Char('q')
                {
                    self.should_quit = true;
                }

                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn new_app_is_not_quitting() {
        let app = App::new();
        assert!(!app.should_quit);
    }

    #[test]
    fn new_app_has_greeting() {
        let app = App::new();
        assert!(app.greeting.starts_with("codepeek v"));
    }

    #[test]
    fn default_delegates_to_new() {
        let from_new = App::new();
        let from_default = App::default();
        assert_eq!(from_new.should_quit, from_default.should_quit);
        assert_eq!(from_new.greeting, from_default.greeting);
    }

    #[test]
    fn renders_version_banner() {
        let app = App::new();
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
            content.contains(&app.greeting),
            "Buffer should contain '{}'",
            app.greeting
        );
    }
}
