use codepeek_core::{ChangeKind, FileChange};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::action::Action;
use crate::theme;

pub struct FileList {
    files: Vec<FileChange>,
    display_items: Vec<(String, ChangeKind)>,
    selected: usize,
}

impl FileList {
    pub fn new(files: Vec<FileChange>) -> Self {
        let display_items = files
            .iter()
            .map(|f| {
                let badge = theme::change_badge(&f.kind);
                let label = match &f.kind {
                    ChangeKind::Renamed { from } => {
                        format!("{} \u{2192} {}", from.display(), f.path.display())
                    }
                    _ => f.path.display().to_string(),
                };
                (format!("{badge} {label}"), f.kind.clone())
            })
            .collect();
        Self {
            files,
            display_items,
            selected: 0,
        }
    }

    pub fn handle_event(&mut self, key: KeyEvent) -> Action {
        if self.files.is_empty() {
            return match key.code {
                KeyCode::Char('q') => Action::Quit,
                _ => Action::Noop,
            };
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                Action::Noop
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.files.len() {
                    self.selected += 1;
                }
                Action::Noop
            }
            KeyCode::Enter => Action::SelectFile(self.selected),
            KeyCode::Char('r') => Action::Refresh,
            KeyCode::Char('q') => Action::Quit,
            _ => Action::Noop,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::BORDER_COLOR))
            .title(Span::styled(
                " Files ",
                Style::default().fg(theme::TITLE_COLOR),
            ));

        let items: Vec<ListItem> = self
            .display_items
            .iter()
            .map(|(text, kind)| {
                let badge_len = theme::change_badge(kind).len();
                let badge_text = &text[..badge_len];
                let rest = &text[badge_len..];
                let rest_style = if *kind == ChangeKind::Deleted {
                    theme::deleted_file_style()
                } else if *kind == ChangeKind::Unchanged {
                    theme::unchanged_file_style()
                } else {
                    Style::default()
                };
                let line = Line::from(vec![
                    Span::styled(badge_text.to_string(), theme::badge_style(kind)),
                    Span::styled(rest.to_string(), rest_style),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(theme::selected_style());

        let mut state = ListState::default();
        state.select(Some(self.selected));

        frame.render_stateful_widget(list, area, &mut state);
    }

    #[cfg(test)]
    pub fn selected_file(&self) -> Option<&FileChange> {
        self.files.get(self.selected)
    }

    pub fn update_files(&mut self, files: Vec<FileChange>) {
        let display_items = files
            .iter()
            .map(|f| {
                let badge = theme::change_badge(&f.kind);
                let label = match &f.kind {
                    ChangeKind::Renamed { from } => {
                        format!("{} \u{2192} {}", from.display(), f.path.display())
                    }
                    _ => f.path.display().to_string(),
                };
                (format!("{badge} {label}"), f.kind.clone())
            })
            .collect();
        self.files = files;
        self.display_items = display_items;
        self.selected = 0;
    }

    pub fn files(&self) -> &[FileChange] {
        &self.files
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::SystemTime;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    use super::*;

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
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
                path: PathBuf::from("src/lib.rs"),
                kind: ChangeKind::Added,
                mtime: SystemTime::now(),
            },
            FileChange {
                path: PathBuf::from("old.rs"),
                kind: ChangeKind::Deleted,
                mtime: SystemTime::now(),
            },
        ]
    }

    #[test]
    fn empty_list_renders_without_panic() {
        let list = FileList::new(vec![]);
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| list.render(frame, frame.area()))
            .unwrap();
    }

    #[test]
    fn selection_clamps_at_bounds() {
        let mut list = FileList::new(sample_files());
        assert_eq!(list.selected, 0);

        // Move up from 0 stays at 0.
        list.handle_event(make_key(KeyCode::Up));
        assert_eq!(list.selected, 0);

        // Move down to last.
        list.handle_event(make_key(KeyCode::Down));
        list.handle_event(make_key(KeyCode::Down));
        assert_eq!(list.selected, 2);

        // Move down past last stays at last.
        list.handle_event(make_key(KeyCode::Down));
        assert_eq!(list.selected, 2);
    }

    #[test]
    fn enter_returns_select_file() {
        let mut list = FileList::new(sample_files());
        list.handle_event(make_key(KeyCode::Down));
        let action = list.handle_event(make_key(KeyCode::Enter));
        assert_eq!(action, Action::SelectFile(1));
    }

    #[test]
    fn q_returns_quit() {
        let mut list = FileList::new(sample_files());
        let action = list.handle_event(make_key(KeyCode::Char('q')));
        assert_eq!(action, Action::Quit);
    }

    #[test]
    fn selected_file_returns_correct_entry() {
        let files = sample_files();
        let mut list = FileList::new(files);
        list.handle_event(make_key(KeyCode::Down));
        let selected = list.selected_file().unwrap();
        assert_eq!(selected.path, PathBuf::from("src/lib.rs"));
    }

    #[test]
    fn is_empty_when_no_files() {
        let list = FileList::new(vec![]);
        assert!(list.is_empty());
    }

    #[test]
    fn is_not_empty_when_has_files() {
        let list = FileList::new(sample_files());
        assert!(!list.is_empty());
    }

    #[test]
    fn renamed_file_shows_old_and_new_paths() {
        let files = vec![FileChange {
            path: PathBuf::from("new.rs"),
            kind: ChangeKind::Renamed {
                from: PathBuf::from("old.rs"),
            },
            mtime: SystemTime::now(),
        }];
        let list = FileList::new(files);

        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| list.render(frame, frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert!(
            content.contains("old.rs"),
            "should show old path for renamed file"
        );
        assert!(
            content.contains("new.rs"),
            "should show new path for renamed file"
        );
        assert!(
            content.contains("\u{2192}"),
            "should show arrow between old and new paths"
        );
    }

    #[test]
    fn r_returns_refresh_action() {
        let mut list = FileList::new(sample_files());
        let action = list.handle_event(make_key(KeyCode::Char('r')));
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn update_files_replaces_list_and_resets_selection() {
        let mut list = FileList::new(sample_files());
        list.handle_event(make_key(KeyCode::Down));
        assert_eq!(list.selected, 1);

        let new_files = vec![FileChange {
            path: PathBuf::from("fresh.rs"),
            kind: ChangeKind::Added,
            mtime: SystemTime::now(),
        }];
        list.update_files(new_files);

        assert_eq!(list.selected, 0);
        assert_eq!(list.files().len(), 1);
        assert_eq!(list.files()[0].path, PathBuf::from("fresh.rs"));
    }

    #[test]
    fn empty_list_q_still_quits() {
        let mut list = FileList::new(vec![]);
        let action = list.handle_event(make_key(KeyCode::Char('q')));
        assert_eq!(action, Action::Quit);
    }

    #[test]
    fn empty_list_other_keys_return_noop() {
        let mut list = FileList::new(vec![]);
        let action = list.handle_event(make_key(KeyCode::Enter));
        assert_eq!(action, Action::Noop);
    }

    #[test]
    fn k_moves_up() {
        let mut list = FileList::new(sample_files());
        list.handle_event(make_key(KeyCode::Down));
        assert_eq!(list.selected, 1);
        list.handle_event(make_key(KeyCode::Char('k')));
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn j_moves_down() {
        let mut list = FileList::new(sample_files());
        list.handle_event(make_key(KeyCode::Char('j')));
        assert_eq!(list.selected, 1);
    }

    #[test]
    fn unchanged_file_renders_with_dim_style() {
        let files = vec![FileChange {
            path: PathBuf::from("stable.rs"),
            kind: ChangeKind::Unchanged,
            mtime: SystemTime::now(),
        }];
        let list = FileList::new(files);

        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| list.render(frame, frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            content.contains("stable.rs"),
            "should show unchanged file path"
        );
    }

    #[test]
    fn deleted_file_shows_with_badge() {
        let files = vec![FileChange {
            path: PathBuf::from("gone.rs"),
            kind: ChangeKind::Deleted,
            mtime: SystemTime::now(),
        }];
        let list = FileList::new(files);

        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| list.render(frame, frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(content.contains('D'), "should show D badge for deleted");
        assert!(content.contains("gone.rs"), "should show file path");
    }
}
