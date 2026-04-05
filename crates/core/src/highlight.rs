use std::fmt;

/// Categories of syntax elements for highlighting.
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

/// A span within a single line that has a specific highlight kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub kind: HighlightKind,
}

/// A single line of source code with its highlight spans.
#[derive(Debug, Clone)]
pub struct HighlightedLine {
    pub content: String,
    pub spans: Vec<HighlightSpan>,
}
