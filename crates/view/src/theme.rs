use ratatui::style::{Color, Modifier, Style};

use codepeek_core::{ChangeKind, HighlightKind};

pub const ADDED_COLOR: Color = Color::Green;
pub const MODIFIED_COLOR: Color = Color::Yellow;
pub const DELETED_COLOR: Color = Color::Red;
pub const RENAMED_COLOR: Color = Color::Cyan;

pub const SELECTED_BG: Color = Color::DarkGray;
pub const BORDER_COLOR: Color = Color::Gray;
// Reset inherits the terminal's foreground, working on dark and light backgrounds.
pub const TITLE_COLOR: Color = Color::Reset;
pub const DIM_COLOR: Color = Color::Gray;

pub fn selected_style() -> Style {
    Style::default()
        .bg(SELECTED_BG)
        .add_modifier(Modifier::BOLD)
}

pub fn badge_style(kind: &ChangeKind) -> Style {
    let color = match kind {
        ChangeKind::Added => ADDED_COLOR,
        ChangeKind::Modified => MODIFIED_COLOR,
        ChangeKind::Deleted => DELETED_COLOR,
        ChangeKind::Renamed { .. } => RENAMED_COLOR,
        ChangeKind::Unchanged => DIM_COLOR,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

pub fn change_badge(kind: &ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "A",
        ChangeKind::Modified => "M",
        ChangeKind::Deleted => "D",
        ChangeKind::Renamed { .. } => "R",
        ChangeKind::Unchanged => " ",
    }
}

pub fn highlight_style(kind: HighlightKind) -> Style {
    let color = match kind {
        HighlightKind::Keyword => Color::Magenta,
        HighlightKind::Function => Color::Blue,
        HighlightKind::Type | HighlightKind::Property => Color::Cyan,
        HighlightKind::String => Color::Green,
        HighlightKind::Comment | HighlightKind::Punctuation => Color::DarkGray,
        HighlightKind::Number | HighlightKind::Constant | HighlightKind::Attribute => Color::Yellow,
        HighlightKind::Operator | HighlightKind::Variable => Color::White,
        HighlightKind::Tag => Color::Red,
    };
    Style::default().fg(color)
}

#[allow(dead_code)]
pub enum GutterMark {
    Added,
    Modified,
    Deleted,
    Unchanged,
}

pub fn gutter_style(mark: &GutterMark) -> Style {
    match mark {
        GutterMark::Added => Style::default().fg(ADDED_COLOR),
        GutterMark::Modified => Style::default().fg(MODIFIED_COLOR),
        GutterMark::Deleted => Style::default().fg(DELETED_COLOR),
        GutterMark::Unchanged => Style::default(),
    }
}

pub fn gutter_text(mark: &GutterMark) -> &'static str {
    match mark {
        GutterMark::Added | GutterMark::Modified => "\u{258e} ",
        GutterMark::Deleted => "\u{2581} ",
        GutterMark::Unchanged => "  ",
    }
}

pub const DIFF_ADDED_BG: Color = Color::Rgb(0, 60, 0);
pub const DIFF_ADDED_FG: Color = Color::Green;
pub const DIFF_REMOVED_BG: Color = Color::Rgb(60, 0, 0);
pub const DIFF_REMOVED_FG: Color = Color::Red;

pub fn diff_added_style() -> Style {
    Style::default().fg(DIFF_ADDED_FG).bg(DIFF_ADDED_BG)
}

pub fn diff_removed_style() -> Style {
    Style::default().fg(DIFF_REMOVED_FG).bg(DIFF_REMOVED_BG)
}

pub fn deleted_file_style() -> Style {
    Style::default().fg(DIM_COLOR).add_modifier(Modifier::DIM)
}

pub fn unchanged_file_style() -> Style {
    Style::default().fg(DIM_COLOR)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn change_badge_values() {
        assert_eq!(change_badge(&ChangeKind::Added), "A");
        assert_eq!(change_badge(&ChangeKind::Modified), "M");
        assert_eq!(change_badge(&ChangeKind::Deleted), "D");
        assert_eq!(
            change_badge(&ChangeKind::Renamed {
                from: PathBuf::from("old.rs")
            }),
            "R"
        );
        assert_eq!(change_badge(&ChangeKind::Unchanged), " ");
    }

    #[test]
    fn badge_style_returns_correct_colors() {
        let added = badge_style(&ChangeKind::Added);
        assert_eq!(added.fg, Some(ADDED_COLOR));

        let modified = badge_style(&ChangeKind::Modified);
        assert_eq!(modified.fg, Some(MODIFIED_COLOR));

        let deleted = badge_style(&ChangeKind::Deleted);
        assert_eq!(deleted.fg, Some(DELETED_COLOR));

        let renamed = badge_style(&ChangeKind::Renamed {
            from: PathBuf::from("old.rs"),
        });
        assert_eq!(renamed.fg, Some(RENAMED_COLOR));

        let unchanged = badge_style(&ChangeKind::Unchanged);
        assert_eq!(unchanged.fg, Some(DIM_COLOR));
    }

    #[test]
    fn highlight_style_returns_color_for_all_kinds() {
        let kinds = [
            HighlightKind::Keyword,
            HighlightKind::Function,
            HighlightKind::Type,
            HighlightKind::String,
            HighlightKind::Comment,
            HighlightKind::Number,
            HighlightKind::Operator,
            HighlightKind::Variable,
            HighlightKind::Punctuation,
            HighlightKind::Constant,
            HighlightKind::Property,
            HighlightKind::Tag,
            HighlightKind::Attribute,
        ];
        for kind in kinds {
            let style = highlight_style(kind);
            assert!(style.fg.is_some(), "highlight_style({kind}) should set fg");
        }
    }

    #[test]
    fn gutter_text_for_all_marks() {
        assert_eq!(gutter_text(&GutterMark::Added), "\u{258e} ");
        assert_eq!(gutter_text(&GutterMark::Modified), "\u{258e} ");
        assert_eq!(gutter_text(&GutterMark::Deleted), "\u{2581} ");
        assert_eq!(gutter_text(&GutterMark::Unchanged), "  ");
    }

    #[test]
    fn gutter_style_added_is_green() {
        let style = gutter_style(&GutterMark::Added);
        assert_eq!(style.fg, Some(ADDED_COLOR));
    }

    #[test]
    fn gutter_style_modified_is_yellow() {
        let style = gutter_style(&GutterMark::Modified);
        assert_eq!(style.fg, Some(MODIFIED_COLOR));
    }

    #[test]
    fn gutter_style_deleted_is_red() {
        let style = gutter_style(&GutterMark::Deleted);
        assert_eq!(style.fg, Some(DELETED_COLOR));
    }

    #[test]
    fn gutter_style_unchanged_has_no_color() {
        let style = gutter_style(&GutterMark::Unchanged);
        assert_eq!(style.fg, None);
    }

    #[test]
    fn selected_style_is_bold_with_dark_bg() {
        let style = selected_style();
        assert_eq!(style.bg, Some(SELECTED_BG));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn diff_added_style_has_green_on_dark_green() {
        let style = diff_added_style();
        assert_eq!(style.fg, Some(DIFF_ADDED_FG));
        assert_eq!(style.bg, Some(DIFF_ADDED_BG));
    }

    #[test]
    fn diff_removed_style_has_red_on_dark_red() {
        let style = diff_removed_style();
        assert_eq!(style.fg, Some(DIFF_REMOVED_FG));
        assert_eq!(style.bg, Some(DIFF_REMOVED_BG));
    }

    #[test]
    fn deleted_file_style_is_dim() {
        let style = deleted_file_style();
        assert_eq!(style.fg, Some(DIM_COLOR));
        assert!(style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn unchanged_file_style_is_gray() {
        let style = unchanged_file_style();
        assert_eq!(style.fg, Some(DIM_COLOR));
        assert!(!style.add_modifier.contains(Modifier::DIM));
    }
}
