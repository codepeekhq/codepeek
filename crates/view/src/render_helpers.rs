use ratatui::text::Span;

use codepeek_core::HighlightSpan;

use crate::theme::Theme;

pub fn build_highlighted_spans<'a>(
    content: &'a str,
    spans: &[HighlightSpan],
    theme: &Theme,
) -> Vec<Span<'a>> {
    if spans.is_empty() {
        return vec![Span::raw(content)];
    }

    let mut result = Vec::new();
    let mut cursor = 0;

    for hs in spans {
        let start = hs.start.min(content.len());
        let end = hs.end.min(content.len());
        if start > cursor {
            result.push(Span::raw(&content[cursor..start]));
        }
        if start < end {
            result.push(Span::styled(
                &content[start..end],
                theme.syntax.highlight(hs.kind),
            ));
        }
        cursor = end;
    }

    if cursor < content.len() {
        result.push(Span::raw(&content[cursor..]));
    }

    result
}

pub fn build_highlighted_spans_owned(
    content: &str,
    spans: &[HighlightSpan],
    max_length: usize,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let truncated;
    let display_content = if content.len() > max_length {
        truncated = format!("{}\u{2026}", &content[..max_length]);
        &truncated
    } else {
        content
    };

    if spans.is_empty() {
        return vec![Span::raw(display_content.to_string())];
    }

    let mut result = Vec::new();
    let mut cursor = 0;

    for hs in spans {
        let start = hs.start.min(display_content.len());
        let end = hs.end.min(display_content.len());
        if start > cursor {
            result.push(Span::raw(display_content[cursor..start].to_string()));
        }
        if start < end {
            result.push(Span::styled(
                display_content[start..end].to_string(),
                theme.syntax.highlight(hs.kind),
            ));
        }
        cursor = end;
    }

    if cursor < display_content.len() {
        result.push(Span::raw(display_content[cursor..].to_string()));
    }

    result
}

pub fn line_number_width(total_lines: usize) -> usize {
    if total_lines == 0 {
        1
    } else {
        total_lines.ilog10() as usize + 1
    }
}

pub fn truncate_line(content: &str, max_length: usize) -> String {
    if content.len() > max_length {
        let mut truncated = content[..max_length].to_string();
        truncated.push('\u{2026}');
        truncated
    } else {
        content.to_string()
    }
}

pub fn dim_line_number(number: &str, theme: &Theme) -> Span<'static> {
    Span::styled(format!("{number} "), theme.text.muted)
}

#[cfg(test)]
mod tests {
    use codepeek_core::HighlightKind;

    use super::*;
    use crate::theme;

    #[test]
    fn line_number_width_for_various_sizes() {
        assert_eq!(line_number_width(0), 1);
        assert_eq!(line_number_width(1), 1);
        assert_eq!(line_number_width(9), 1);
        assert_eq!(line_number_width(10), 2);
        assert_eq!(line_number_width(99), 2);
        assert_eq!(line_number_width(100), 3);
        assert_eq!(line_number_width(999), 3);
        assert_eq!(line_number_width(1000), 4);
    }

    #[test]
    fn truncate_short_line_unchanged() {
        assert_eq!(truncate_line("short", 500), "short");
    }

    #[test]
    fn truncate_long_line_adds_ellipsis() {
        let long = "x".repeat(600);
        let result = truncate_line(&long, 500);
        assert_eq!(result.len(), 500 + "\u{2026}".len());
        assert!(result.ends_with('\u{2026}'));
    }

    #[test]
    fn highlighted_spans_empty_source() {
        let spans = build_highlighted_spans("", &[], theme::current());
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn highlighted_spans_no_highlights() {
        let spans = build_highlighted_spans("hello world", &[], theme::current());
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn highlighted_spans_with_highlight() {
        let spans = build_highlighted_spans(
            "fn main",
            &[HighlightSpan {
                start: 0,
                end: 2,
                kind: HighlightKind::Keyword,
            }],
            theme::current(),
        );
        assert_eq!(spans.len(), 2);
    }

    #[test]
    fn highlighted_spans_clamped_to_content_length() {
        let spans = build_highlighted_spans(
            "ab",
            &[HighlightSpan {
                start: 0,
                end: 100,
                kind: HighlightKind::Keyword,
            }],
            theme::current(),
        );
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn highlighted_spans_owned_truncates() {
        let long = "x".repeat(600);
        let spans = build_highlighted_spans_owned(&long, &[], 500, theme::current());
        assert_eq!(spans.len(), 1);
        let text = spans[0].content.to_string();
        assert!(text.ends_with('\u{2026}'));
    }

    #[test]
    fn highlighted_spans_gap_between_highlights() {
        let spans = build_highlighted_spans(
            "fn x = 1",
            &[
                HighlightSpan {
                    start: 0,
                    end: 2,
                    kind: HighlightKind::Keyword,
                },
                HighlightSpan {
                    start: 7,
                    end: 8,
                    kind: HighlightKind::Number,
                },
            ],
            theme::current(),
        );
        assert_eq!(spans.len(), 3);
    }
}
