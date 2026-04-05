use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    Keyword,
    Function,
    Type,
    String,
    Comment,
    Number,
    Operator,
    Variable,
    Punctuation,
    Constant,
    Property,
    Tag,
    Attribute,
}

impl fmt::Display for HighlightKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Keyword => "keyword",
            Self::Function => "function",
            Self::Type => "type",
            Self::String => "string",
            Self::Comment => "comment",
            Self::Number => "number",
            Self::Operator => "operator",
            Self::Variable => "variable",
            Self::Punctuation => "punctuation",
            Self::Constant => "constant",
            Self::Property => "property",
            Self::Tag => "tag",
            Self::Attribute => "attribute",
        };
        f.write_str(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub kind: HighlightKind,
}

#[derive(Debug, Clone)]
pub struct HighlightedLine {
    pub content: String,
    pub spans: Vec<HighlightSpan>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_all_highlight_kinds() {
        assert_eq!(HighlightKind::Keyword.to_string(), "keyword");
        assert_eq!(HighlightKind::Function.to_string(), "function");
        assert_eq!(HighlightKind::Type.to_string(), "type");
        assert_eq!(HighlightKind::String.to_string(), "string");
        assert_eq!(HighlightKind::Comment.to_string(), "comment");
        assert_eq!(HighlightKind::Number.to_string(), "number");
        assert_eq!(HighlightKind::Operator.to_string(), "operator");
        assert_eq!(HighlightKind::Variable.to_string(), "variable");
        assert_eq!(HighlightKind::Punctuation.to_string(), "punctuation");
        assert_eq!(HighlightKind::Constant.to_string(), "constant");
        assert_eq!(HighlightKind::Property.to_string(), "property");
        assert_eq!(HighlightKind::Tag.to_string(), "tag");
        assert_eq!(HighlightKind::Attribute.to_string(), "attribute");
    }

    #[test]
    fn highlight_kind_equality_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(HighlightKind::Keyword);
        set.insert(HighlightKind::Keyword);
        assert_eq!(set.len(), 1);
        set.insert(HighlightKind::Function);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn highlight_span_equality() {
        let a = HighlightSpan {
            start: 0,
            end: 5,
            kind: HighlightKind::Keyword,
        };
        let b = HighlightSpan {
            start: 0,
            end: 5,
            kind: HighlightKind::Keyword,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn highlight_span_inequality_on_kind() {
        let a = HighlightSpan {
            start: 0,
            end: 5,
            kind: HighlightKind::Keyword,
        };
        let b = HighlightSpan {
            start: 0,
            end: 5,
            kind: HighlightKind::Function,
        };
        assert_ne!(a, b);
    }

    #[test]
    fn highlighted_line_with_no_spans() {
        let line = HighlightedLine {
            content: "plain text".to_string(),
            spans: vec![],
        };
        assert_eq!(line.content, "plain text");
        assert!(line.spans.is_empty());
    }

    #[test]
    fn highlighted_line_with_spans() {
        let line = HighlightedLine {
            content: "fn main()".to_string(),
            spans: vec![HighlightSpan {
                start: 0,
                end: 2,
                kind: HighlightKind::Keyword,
            }],
        };
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].kind, HighlightKind::Keyword);
    }
}
