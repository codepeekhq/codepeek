use ratatui::style::{Color, Modifier, Style};

use codepeek_core::{ChangeKind, HighlightKind};

// Change kind colors
pub const ADDED_COLOR: Color = Color::Green;
pub const MODIFIED_COLOR: Color = Color::Yellow;
pub const DELETED_COLOR: Color = Color::Red;
pub const RENAMED_COLOR: Color = Color::Cyan;

// UI colors
pub const SELECTED_BG: Color = Color::DarkGray;
pub const BORDER_COLOR: Color = Color::Gray;
// Use `Reset` so the title inherits the terminal's foreground color,
// working on both dark and light backgrounds.
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
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

/// Return a short badge string for the change kind.
pub fn change_badge(kind: &ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "A",
        ChangeKind::Modified => "M",
        ChangeKind::Deleted => "D",
        ChangeKind::Renamed { .. } => "R",
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
