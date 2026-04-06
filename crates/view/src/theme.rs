use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

use codepeek_core::{ChangeKind, HighlightKind};

#[non_exhaustive]
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

#[non_exhaustive]
pub struct Theme {
    pub text: TextColors,
    pub border: BorderColors,
    pub change: ChangeColors,
    pub diff: DiffColors,
    pub syntax: SyntaxColors,
    pub ui: UiColors,
    pub selected: Style,
}

#[non_exhaustive]
pub struct TextColors {
    pub primary: Style,
    pub muted: Style,
    pub deleted_file: Style,
}

#[non_exhaustive]
#[allow(clippy::struct_field_names)]
pub struct BorderColors {
    pub normal_color: Color,
    pub active_color: Color,
    pub danger_color: Color,
    pub danger: Style,
}

impl BorderColors {
    pub fn block(&self) -> Block<'static> {
        rounded_block(self.normal_color)
    }

    pub fn active_block(&self) -> Block<'static> {
        rounded_block(self.active_color)
    }

    pub fn danger_block(&self) -> Block<'static> {
        rounded_block(self.danger_color)
    }
}

#[non_exhaustive]
#[allow(clippy::struct_field_names)]
pub struct ChangeColors {
    pub added_color: Color,
    pub modified_color: Color,
    pub deleted_color: Color,
    pub renamed_color: Color,
}

impl ChangeColors {
    pub fn gutter(&self, mark: &GutterMark) -> Style {
        match mark {
            GutterMark::Added => Style::new().fg(self.added_color),
            GutterMark::Modified => Style::new().fg(self.modified_color),
            GutterMark::Deleted => Style::new().fg(self.deleted_color),
            GutterMark::Unchanged => Style::new(),
        }
    }
}

#[non_exhaustive]
pub struct DiffColors {
    pub added: Style,
    pub removed: Style,
}

#[non_exhaustive]
pub struct SyntaxColors {
    pub keyword: Color,
    pub function: Color,
    pub r#type: Color,
    pub property: Color,
    pub string: Color,
    pub comment: Color,
    pub punctuation: Color,
    pub number: Color,
    pub operator: Color,
    pub variable: Color,
    pub tag: Color,
}

impl SyntaxColors {
    pub fn highlight(&self, kind: HighlightKind) -> Style {
        let color = match kind {
            HighlightKind::Keyword => self.keyword,
            HighlightKind::Function => self.function,
            HighlightKind::Type | HighlightKind::Attribute => self.r#type,
            HighlightKind::Property => self.property,
            HighlightKind::String => self.string,
            HighlightKind::Comment => self.comment,
            HighlightKind::Punctuation => self.punctuation,
            HighlightKind::Number | HighlightKind::Constant => self.number,
            HighlightKind::Operator => self.operator,
            HighlightKind::Variable => self.variable,
            HighlightKind::Tag => self.tag,
        };
        Style::new().fg(color)
    }
}

#[non_exhaustive]
pub struct UiColors {
    pub accent: Style,
    pub hint_key: Style,
    pub hint_label: Style,
    pub error_badge: Style,
    pub error_text: Style,
}

impl Theme {
    pub fn from_palette(p: &Palette) -> Self {
        let muted = Style::new().fg(p.overlay0);
        let primary = Style::new().fg(p.text);
        Self {
            text: TextColors {
                primary,
                muted,
                deleted_file: Style::new().fg(p.overlay0).add_modifier(Modifier::DIM),
            },
            border: BorderColors {
                normal_color: p.surface1,
                active_color: p.lavender,
                danger_color: p.maroon,
                danger: Style::new().fg(p.maroon),
            },
            change: ChangeColors {
                added_color: p.green,
                modified_color: p.yellow,
                deleted_color: p.red,
                renamed_color: p.sapphire,
            },
            diff: DiffColors {
                added: Style::new().fg(p.green).bg(Color::Rgb(30, 56, 36)),
                removed: Style::new().fg(p.red).bg(Color::Rgb(56, 30, 38)),
            },
            syntax: SyntaxColors {
                keyword: p.mauve,
                function: p.blue,
                r#type: p.yellow,
                property: p.lavender,
                string: p.green,
                comment: p.overlay0,
                punctuation: p.overlay2,
                number: p.peach,
                operator: p.sky,
                variable: p.text,
                tag: p.maroon,
            },
            ui: UiColors {
                accent: Style::new().fg(p.mauve),
                hint_key: Style::new().fg(p.lavender),
                hint_label: Style::new().fg(p.overlay1),
                error_badge: Style::new().fg(Color::Reset).bg(p.red),
                error_text: Style::new().fg(p.red),
            },
            selected: Style::new()
                .bg(p.surface0)
                .fg(p.text)
                .add_modifier(Modifier::BOLD),
        }
    }

    pub fn badge(&self, kind: &ChangeKind) -> Style {
        let color = match kind {
            ChangeKind::Added => self.change.added_color,
            ChangeKind::Modified => self.change.modified_color,
            ChangeKind::Deleted => self.change.deleted_color,
            ChangeKind::Renamed { .. } => self.change.renamed_color,
            ChangeKind::Unchanged => return self.text.muted.add_modifier(Modifier::BOLD),
        };
        Style::new().fg(color).add_modifier(Modifier::BOLD)
    }
}

fn rounded_block(border_color: Color) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border_color))
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_palette(&Palette::catppuccin_mocha())
    }
}

static THEME: LazyLock<Theme> = LazyLock::new(Theme::default);

/// Returns the globally-initialized theme. The app reads this once per frame
/// and passes `&Theme` down through its render tree — components themselves
/// never call this directly.
pub fn current() -> &'static Theme {
    &THEME
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

pub enum GutterMark {
    Added,
    Modified,
    #[allow(dead_code)]
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
        assert_eq!(theme.text.primary.fg, Some(palette.text));
        assert_eq!(theme.change.added_color, palette.green);
    }

    #[test]
    fn current_returns_default_theme() {
        let t = current();
        assert_eq!(t.text.primary.fg, Some(Palette::catppuccin_mocha().text));
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
    fn badge_returns_correct_colors() {
        let t = current();
        assert_eq!(t.badge(&ChangeKind::Added).fg, Some(t.change.added_color));
        assert_eq!(
            t.badge(&ChangeKind::Modified).fg,
            Some(t.change.modified_color)
        );
        assert_eq!(
            t.badge(&ChangeKind::Deleted).fg,
            Some(t.change.deleted_color)
        );
        assert_eq!(
            t.badge(&ChangeKind::Renamed {
                from: PathBuf::from("old.rs")
            })
            .fg,
            Some(t.change.renamed_color)
        );
        assert_eq!(t.badge(&ChangeKind::Unchanged).fg, t.text.muted.fg);
    }

    #[test]
    fn highlight_returns_color_for_all_kinds() {
        let t = current();
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
            assert!(t.syntax.highlight(kind).fg.is_some());
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
    fn gutter_styles_by_mark() {
        let t = current();
        assert_eq!(
            t.change.gutter(&GutterMark::Added).fg,
            Some(t.change.added_color)
        );
        assert_eq!(
            t.change.gutter(&GutterMark::Modified).fg,
            Some(t.change.modified_color)
        );
        assert_eq!(
            t.change.gutter(&GutterMark::Deleted).fg,
            Some(t.change.deleted_color)
        );
        assert_eq!(t.change.gutter(&GutterMark::Unchanged).fg, None);
    }

    #[test]
    fn selected_is_bold_with_surface_bg() {
        let t = current();
        assert!(t.selected.add_modifier.contains(Modifier::BOLD));
        assert!(t.selected.bg.is_some());
    }

    #[test]
    fn diff_added_has_green_tones() {
        let t = current();
        assert_eq!(t.diff.added.fg, Some(Palette::catppuccin_mocha().green));
        assert!(t.diff.added.bg.is_some());
    }

    #[test]
    fn diff_removed_has_red_tones() {
        let t = current();
        assert_eq!(t.diff.removed.fg, Some(Palette::catppuccin_mocha().red));
        assert!(t.diff.removed.bg.is_some());
    }

    #[test]
    fn deleted_file_is_dim() {
        let t = current();
        assert!(t.text.deleted_file.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn text_primary_and_muted_set_fg() {
        let t = current();
        assert!(t.text.primary.fg.is_some());
        assert!(t.text.muted.fg.is_some());
    }

    #[test]
    fn ui_styles_set_fg() {
        let t = current();
        assert!(t.ui.accent.fg.is_some());
        assert!(t.ui.hint_key.fg.is_some());
        assert!(t.ui.hint_label.fg.is_some());
    }

    #[test]
    fn border_danger_style() {
        let t = current();
        assert_eq!(t.border.danger.fg, Some(t.border.danger_color));
    }

    #[test]
    fn error_badge_has_error_bg_and_reset_fg() {
        let t = current();
        assert_eq!(t.ui.error_badge.fg, Some(Color::Reset));
        assert!(t.ui.error_badge.bg.is_some());
    }

    #[test]
    fn error_text_has_fg() {
        let t = current();
        assert!(t.ui.error_text.fg.is_some());
    }

    #[test]
    fn blocks_build_without_panic() {
        let t = current();
        let _ = t.border.block();
        let _ = t.border.active_block();
        let _ = t.border.danger_block();
    }

    #[test]
    fn from_palette_maps_semantic_tokens_correctly() {
        let p = Palette::catppuccin_mocha();
        let t = Theme::from_palette(&p);
        assert_eq!(t.syntax.keyword, p.mauve);
        assert_eq!(t.syntax.function, p.blue);
        assert_eq!(t.syntax.string, p.green);
        assert_eq!(t.border.danger_color, p.maroon);
    }
}
