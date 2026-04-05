use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

use codepeek_core::{ChangeKind, HighlightKind};

#[allow(dead_code)]
pub struct Palette {
    pub base: Color,
    pub mantle: Color,
    pub crust: Color,
    pub surface0: Color,
    pub surface1: Color,
    pub surface2: Color,
    pub overlay0: Color,
    pub overlay1: Color,
    pub overlay2: Color,
    pub subtext0: Color,
    pub subtext1: Color,
    pub text: Color,
    pub lavender: Color,
    pub blue: Color,
    pub sapphire: Color,
    pub sky: Color,
    pub teal: Color,
    pub green: Color,
    pub yellow: Color,
    pub peach: Color,
    pub maroon: Color,
    pub red: Color,
    pub mauve: Color,
    pub pink: Color,
    pub flamingo: Color,
    pub rosewater: Color,
}

impl Palette {
    pub fn catppuccin_mocha() -> Self {
        Self {
            base: Color::Rgb(30, 30, 46),
            mantle: Color::Rgb(24, 24, 37),
            crust: Color::Rgb(17, 17, 27),
            surface0: Color::Rgb(49, 50, 68),
            surface1: Color::Rgb(69, 71, 90),
            surface2: Color::Rgb(88, 91, 112),
            overlay0: Color::Rgb(108, 112, 134),
            overlay1: Color::Rgb(127, 132, 156),
            overlay2: Color::Rgb(147, 153, 178),
            subtext0: Color::Rgb(166, 173, 200),
            subtext1: Color::Rgb(186, 194, 222),
            text: Color::Rgb(205, 214, 244),
            lavender: Color::Rgb(180, 190, 254),
            blue: Color::Rgb(137, 180, 250),
            sapphire: Color::Rgb(116, 199, 236),
            sky: Color::Rgb(137, 220, 235),
            teal: Color::Rgb(148, 226, 213),
            green: Color::Rgb(166, 227, 161),
            yellow: Color::Rgb(249, 226, 175),
            peach: Color::Rgb(250, 179, 135),
            maroon: Color::Rgb(235, 160, 172),
            red: Color::Rgb(243, 139, 168),
            mauve: Color::Rgb(203, 166, 247),
            pink: Color::Rgb(245, 194, 231),
            flamingo: Color::Rgb(242, 205, 205),
            rosewater: Color::Rgb(245, 224, 220),
        }
    }
}

pub struct Theme {
    pub base_bg: Color,
    pub selected_bg: Color,
    pub diff_added_bg: Color,
    pub diff_removed_bg: Color,

    pub text: Color,
    pub text_dim: Color,

    pub border: Color,
    pub border_focused: Color,

    pub added: Color,
    pub modified: Color,
    pub deleted: Color,
    pub renamed: Color,

    pub syntax_keyword: Color,
    pub syntax_function: Color,
    pub syntax_type: Color,
    pub syntax_property: Color,
    pub syntax_string: Color,
    pub syntax_comment: Color,
    pub syntax_punctuation: Color,
    pub syntax_number: Color,
    pub syntax_operator: Color,
    pub syntax_variable: Color,
    pub syntax_tag: Color,

    pub accent: Color,
    pub destructive: Color,
    pub error_bg: Color,
    pub error_fg: Color,
    pub status_key: Color,
    pub status_desc: Color,
    pub status_separator: Color,
    pub diff_added_fg: Color,
    pub diff_removed_fg: Color,
}

impl Theme {
    pub fn from_palette(p: &Palette) -> Self {
        Self {
            base_bg: p.base,
            selected_bg: p.surface0,
            diff_added_bg: Color::Rgb(30, 56, 36),
            diff_removed_bg: Color::Rgb(56, 30, 38),

            text: p.text,
            text_dim: p.overlay0,

            border: p.surface1,
            border_focused: p.lavender,

            added: p.green,
            modified: p.yellow,
            deleted: p.red,
            renamed: p.sapphire,

            syntax_keyword: p.mauve,
            syntax_function: p.blue,
            syntax_type: p.yellow,
            syntax_property: p.lavender,
            syntax_string: p.green,
            syntax_comment: p.overlay0,
            syntax_punctuation: p.overlay2,
            syntax_number: p.peach,
            syntax_operator: p.sky,
            syntax_variable: p.text,
            syntax_tag: p.maroon,

            accent: p.mauve,
            destructive: p.maroon,
            error_bg: p.red,
            error_fg: p.red,
            status_key: p.lavender,
            status_desc: p.overlay1,
            status_separator: p.surface2,
            diff_added_fg: p.green,
            diff_removed_fg: p.red,
        }
    }

    pub fn selected_style(&self) -> Style {
        Style::new()
            .bg(self.selected_bg)
            .fg(self.text)
            .add_modifier(Modifier::BOLD)
    }

    pub fn badge_style(&self, kind: &ChangeKind) -> Style {
        let color = self.change_color(kind);
        Style::new().fg(color).add_modifier(Modifier::BOLD)
    }

    pub fn highlight_style(&self, kind: HighlightKind) -> Style {
        let color = match kind {
            HighlightKind::Keyword => self.syntax_keyword,
            HighlightKind::Function => self.syntax_function,
            HighlightKind::Type | HighlightKind::Attribute => self.syntax_type,
            HighlightKind::Property => self.syntax_property,
            HighlightKind::String => self.syntax_string,
            HighlightKind::Comment => self.syntax_comment,
            HighlightKind::Punctuation => self.syntax_punctuation,
            HighlightKind::Number | HighlightKind::Constant => self.syntax_number,
            HighlightKind::Operator => self.syntax_operator,
            HighlightKind::Variable => self.syntax_variable,
            HighlightKind::Tag => self.syntax_tag,
        };
        Style::new().fg(color)
    }

    pub fn gutter_style(&self, mark: &GutterMark) -> Style {
        match mark {
            GutterMark::Added => Style::new().fg(self.added),
            GutterMark::Modified => Style::new().fg(self.modified),
            GutterMark::Deleted => Style::new().fg(self.deleted),
            GutterMark::Unchanged => Style::new(),
        }
    }

    pub fn diff_added_style(&self) -> Style {
        Style::new().fg(self.diff_added_fg).bg(self.diff_added_bg)
    }

    pub fn diff_removed_style(&self) -> Style {
        Style::new()
            .fg(self.diff_removed_fg)
            .bg(self.diff_removed_bg)
    }

    pub fn deleted_file_style(&self) -> Style {
        Style::new().fg(self.text_dim).add_modifier(Modifier::DIM)
    }

    pub fn unchanged_file_style(&self) -> Style {
        Style::new().fg(self.text_dim)
    }

    pub fn rounded_block(&self) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(self.border))
    }

    pub fn focused_block(&self) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(self.border_focused))
    }

    pub fn destructive_block(&self) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(self.destructive))
    }

    fn change_color(&self, kind: &ChangeKind) -> Color {
        match kind {
            ChangeKind::Added => self.added,
            ChangeKind::Modified => self.modified,
            ChangeKind::Deleted => self.deleted,
            ChangeKind::Renamed { .. } => self.renamed,
            ChangeKind::Unchanged => self.text_dim,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_palette(&Palette::catppuccin_mocha())
    }
}

static THEME: LazyLock<Theme> = LazyLock::new(Theme::default);

pub fn current() -> &'static Theme {
    &THEME
}

pub fn selected_style() -> Style {
    current().selected_style()
}

pub fn badge_style(kind: &ChangeKind) -> Style {
    current().badge_style(kind)
}

pub fn highlight_style(kind: HighlightKind) -> Style {
    current().highlight_style(kind)
}

pub fn gutter_style(mark: &GutterMark) -> Style {
    current().gutter_style(mark)
}

pub fn diff_added_style() -> Style {
    current().diff_added_style()
}

pub fn diff_removed_style() -> Style {
    current().diff_removed_style()
}

pub fn deleted_file_style() -> Style {
    current().deleted_file_style()
}

pub fn unchanged_file_style() -> Style {
    current().unchanged_file_style()
}

pub fn rounded_block() -> Block<'static> {
    current().rounded_block()
}

pub fn focused_block() -> Block<'static> {
    current().focused_block()
}

pub fn destructive_block() -> Block<'static> {
    current().destructive_block()
}

pub fn change_badge(kind: &ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "+",
        ChangeKind::Modified => "\u{25cf}",
        ChangeKind::Deleted => "\u{2715}",
        ChangeKind::Renamed { .. } => "\u{2023}",
        ChangeKind::Unchanged => "\u{00b7}",
    }
}

pub fn change_label(kind: &ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "added",
        ChangeKind::Modified => "modified",
        ChangeKind::Deleted => "deleted",
        ChangeKind::Renamed { .. } => "renamed",
        ChangeKind::Unchanged => "",
    }
}

#[allow(dead_code)]
pub enum GutterMark {
    Added,
    Modified,
    Deleted,
    Unchanged,
}

pub fn gutter_text(mark: &GutterMark) -> &'static str {
    match mark {
        GutterMark::Added | GutterMark::Modified => "\u{2502} ",
        GutterMark::Deleted => "\u{2574} ",
        GutterMark::Unchanged => "  ",
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn default_theme_builds_from_catppuccin_mocha() {
        let theme = Theme::default();
        let palette = Palette::catppuccin_mocha();
        assert_eq!(theme.base_bg, palette.base);
        assert_eq!(theme.text, palette.text);
        assert_eq!(theme.added, palette.green);
    }

    #[test]
    fn current_returns_default_theme() {
        let t = current();
        assert_eq!(t.base_bg, Palette::catppuccin_mocha().base);
    }

    #[test]
    fn change_badge_values() {
        assert_eq!(change_badge(&ChangeKind::Added), "+");
        assert_eq!(change_badge(&ChangeKind::Modified), "\u{25cf}");
        assert_eq!(change_badge(&ChangeKind::Deleted), "\u{2715}");
        assert_eq!(
            change_badge(&ChangeKind::Renamed {
                from: PathBuf::from("old.rs")
            }),
            "\u{2023}"
        );
        assert_eq!(change_badge(&ChangeKind::Unchanged), "\u{00b7}");
    }

    #[test]
    fn change_label_values() {
        assert_eq!(change_label(&ChangeKind::Added), "added");
        assert_eq!(change_label(&ChangeKind::Modified), "modified");
        assert_eq!(change_label(&ChangeKind::Deleted), "deleted");
        assert_eq!(
            change_label(&ChangeKind::Renamed {
                from: PathBuf::from("old.rs")
            }),
            "renamed"
        );
        assert_eq!(change_label(&ChangeKind::Unchanged), "");
    }

    #[test]
    fn badge_style_returns_correct_colors() {
        let t = current();
        let added = badge_style(&ChangeKind::Added);
        assert_eq!(added.fg, Some(t.added));

        let modified = badge_style(&ChangeKind::Modified);
        assert_eq!(modified.fg, Some(t.modified));

        let deleted = badge_style(&ChangeKind::Deleted);
        assert_eq!(deleted.fg, Some(t.deleted));

        let renamed = badge_style(&ChangeKind::Renamed {
            from: PathBuf::from("old.rs"),
        });
        assert_eq!(renamed.fg, Some(t.renamed));

        let unchanged = badge_style(&ChangeKind::Unchanged);
        assert_eq!(unchanged.fg, Some(t.text_dim));
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
        assert_eq!(gutter_text(&GutterMark::Added), "\u{2502} ");
        assert_eq!(gutter_text(&GutterMark::Modified), "\u{2502} ");
        assert_eq!(gutter_text(&GutterMark::Deleted), "\u{2574} ");
        assert_eq!(gutter_text(&GutterMark::Unchanged), "  ");
    }

    #[test]
    fn gutter_style_added_is_green() {
        let t = current();
        let style = gutter_style(&GutterMark::Added);
        assert_eq!(style.fg, Some(t.added));
    }

    #[test]
    fn gutter_style_modified_is_yellow() {
        let t = current();
        let style = gutter_style(&GutterMark::Modified);
        assert_eq!(style.fg, Some(t.modified));
    }

    #[test]
    fn gutter_style_deleted_is_red() {
        let t = current();
        let style = gutter_style(&GutterMark::Deleted);
        assert_eq!(style.fg, Some(t.deleted));
    }

    #[test]
    fn gutter_style_unchanged_has_no_color() {
        let style = gutter_style(&GutterMark::Unchanged);
        assert_eq!(style.fg, None);
    }

    #[test]
    fn selected_style_is_bold_with_surface_bg() {
        let t = current();
        let style = selected_style();
        assert_eq!(style.bg, Some(t.selected_bg));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn diff_added_style_has_green_tones() {
        let t = current();
        let style = diff_added_style();
        assert_eq!(style.fg, Some(t.diff_added_fg));
        assert_eq!(style.bg, Some(t.diff_added_bg));
    }

    #[test]
    fn diff_removed_style_has_red_tones() {
        let t = current();
        let style = diff_removed_style();
        assert_eq!(style.fg, Some(t.diff_removed_fg));
        assert_eq!(style.bg, Some(t.diff_removed_bg));
    }

    #[test]
    fn deleted_file_style_is_dim() {
        let t = current();
        let style = deleted_file_style();
        assert_eq!(style.fg, Some(t.text_dim));
        assert!(style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn unchanged_file_style_is_overlay() {
        let t = current();
        let style = unchanged_file_style();
        assert_eq!(style.fg, Some(t.text_dim));
        assert!(!style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn rounded_block_builds_without_panic() {
        let _ = rounded_block();
    }

    #[test]
    fn focused_block_builds_without_panic() {
        let _ = focused_block();
    }

    #[test]
    fn destructive_block_builds_without_panic() {
        let _ = destructive_block();
    }

    #[test]
    fn from_palette_maps_semantic_tokens_correctly() {
        let p = Palette::catppuccin_mocha();
        let t = Theme::from_palette(&p);
        assert_eq!(t.syntax_keyword, p.mauve);
        assert_eq!(t.syntax_function, p.blue);
        assert_eq!(t.syntax_string, p.green);
        assert_eq!(t.destructive, p.maroon);
        assert_eq!(t.status_key, p.lavender);
    }
}
