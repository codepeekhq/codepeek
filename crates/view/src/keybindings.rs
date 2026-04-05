use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub fn is_move_up(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Up | KeyCode::Char('k'))
}

pub fn is_move_down(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Down | KeyCode::Char('j'))
}

pub fn is_page_up(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::PageUp)
}

pub fn is_page_down(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::PageDown)
}

pub fn is_confirm(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Enter)
}

pub fn is_back(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Esc)
}

pub fn is_quit(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q'))
}

pub fn is_refresh(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('r'))
}

pub fn is_toggle_diff(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('d'))
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};

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
    fn move_up_matches_arrow_and_k() {
        assert!(is_move_up(&make_key(KeyCode::Up)));
        assert!(is_move_up(&make_key(KeyCode::Char('k'))));
        assert!(!is_move_up(&make_key(KeyCode::Down)));
        assert!(!is_move_up(&make_key(KeyCode::Char('j'))));
    }

    #[test]
    fn move_down_matches_arrow_and_j() {
        assert!(is_move_down(&make_key(KeyCode::Down)));
        assert!(is_move_down(&make_key(KeyCode::Char('j'))));
        assert!(!is_move_down(&make_key(KeyCode::Up)));
        assert!(!is_move_down(&make_key(KeyCode::Char('k'))));
    }

    #[test]
    fn page_up_and_down() {
        assert!(is_page_up(&make_key(KeyCode::PageUp)));
        assert!(!is_page_up(&make_key(KeyCode::PageDown)));
        assert!(is_page_down(&make_key(KeyCode::PageDown)));
        assert!(!is_page_down(&make_key(KeyCode::PageUp)));
    }

    #[test]
    fn confirm_matches_enter() {
        assert!(is_confirm(&make_key(KeyCode::Enter)));
        assert!(!is_confirm(&make_key(KeyCode::Char('e'))));
    }

    #[test]
    fn back_matches_esc() {
        assert!(is_back(&make_key(KeyCode::Esc)));
        assert!(!is_back(&make_key(KeyCode::Enter)));
    }

    #[test]
    fn quit_matches_q() {
        assert!(is_quit(&make_key(KeyCode::Char('q'))));
        assert!(!is_quit(&make_key(KeyCode::Char('Q'))));
    }

    #[test]
    fn refresh_matches_r() {
        assert!(is_refresh(&make_key(KeyCode::Char('r'))));
        assert!(!is_refresh(&make_key(KeyCode::Char('R'))));
    }

    #[test]
    fn toggle_diff_matches_d() {
        assert!(is_toggle_diff(&make_key(KeyCode::Char('d'))));
        assert!(!is_toggle_diff(&make_key(KeyCode::Char('D'))));
    }

    #[test]
    fn unrelated_keys_match_nothing() {
        let key = make_key(KeyCode::Char('x'));
        assert!(!is_move_up(&key));
        assert!(!is_move_down(&key));
        assert!(!is_page_up(&key));
        assert!(!is_page_down(&key));
        assert!(!is_confirm(&key));
        assert!(!is_back(&key));
        assert!(!is_quit(&key));
        assert!(!is_refresh(&key));
        assert!(!is_toggle_diff(&key));
    }
}
