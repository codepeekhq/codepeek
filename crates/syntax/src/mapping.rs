use codepeek_core::HighlightKind;

pub fn map_highlight(name: &str) -> Option<HighlightKind> {
    match name {
        "keyword"
        | "keyword.return"
        | "keyword.function"
        | "keyword.operator"
        | "keyword.import"
        | "keyword.export"
        | "keyword.control"
        | "keyword.conditional"
        | "keyword.repeat"
        | "keyword.exception"
        | "keyword.storage"
        | "keyword.modifier" => Some(HighlightKind::Keyword),
        "function" | "function.builtin" | "function.method" | "function.macro"
        | "function.call" => Some(HighlightKind::Function),
        "type" | "type.builtin" | "type.definition" | "type.qualifier" => Some(HighlightKind::Type),
        "string" | "string.special" | "string.escape" | "string.regex" => {
            Some(HighlightKind::String)
        }
        "comment" | "comment.line" | "comment.block" | "comment.documentation" => {
            Some(HighlightKind::Comment)
        }
        "number" | "number.float" => Some(HighlightKind::Number),
        "operator" => Some(HighlightKind::Operator),
        "variable" | "variable.builtin" | "variable.parameter" | "variable.member" => {
            Some(HighlightKind::Variable)
        }
        "punctuation.bracket" | "punctuation.delimiter" | "punctuation.special" => {
            Some(HighlightKind::Punctuation)
        }
        "constant" | "constant.builtin" | "constant.character" => Some(HighlightKind::Constant),
        "property" | "property.definition" => Some(HighlightKind::Property),
        "tag" => Some(HighlightKind::Tag),
        "attribute" => Some(HighlightKind::Attribute),
        _ => map_by_prefix(name),
    }
}

fn map_by_prefix(name: &str) -> Option<HighlightKind> {
    if name.starts_with("keyword") {
        Some(HighlightKind::Keyword)
    } else if name.starts_with("function") {
        Some(HighlightKind::Function)
    } else if name.starts_with("type") {
        Some(HighlightKind::Type)
    } else if name.starts_with("string") {
        Some(HighlightKind::String)
    } else if name.starts_with("comment") {
        Some(HighlightKind::Comment)
    } else if name.starts_with("number") {
        Some(HighlightKind::Number)
    } else if name.starts_with("variable") {
        Some(HighlightKind::Variable)
    } else if name.starts_with("punctuation") {
        Some(HighlightKind::Punctuation)
    } else if name.starts_with("constant") {
        Some(HighlightKind::Constant)
    } else if name.starts_with("property") {
        Some(HighlightKind::Property)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_variants() {
        assert_eq!(map_highlight("keyword"), Some(HighlightKind::Keyword));
        assert_eq!(
            map_highlight("keyword.return"),
            Some(HighlightKind::Keyword)
        );
        assert_eq!(
            map_highlight("keyword.control"),
            Some(HighlightKind::Keyword)
        );
    }

    #[test]
    fn function_variants() {
        assert_eq!(map_highlight("function"), Some(HighlightKind::Function));
        assert_eq!(
            map_highlight("function.builtin"),
            Some(HighlightKind::Function)
        );
        assert_eq!(
            map_highlight("function.call"),
            Some(HighlightKind::Function)
        );
    }

    #[test]
    fn type_variants() {
        assert_eq!(map_highlight("type"), Some(HighlightKind::Type));
        assert_eq!(map_highlight("type.builtin"), Some(HighlightKind::Type));
    }

    #[test]
    fn string_and_comment() {
        assert_eq!(map_highlight("string"), Some(HighlightKind::String));
        assert_eq!(map_highlight("string.escape"), Some(HighlightKind::String));
        assert_eq!(map_highlight("comment"), Some(HighlightKind::Comment));
        assert_eq!(
            map_highlight("comment.documentation"),
            Some(HighlightKind::Comment)
        );
    }

    #[test]
    fn number_operator_variable() {
        assert_eq!(map_highlight("number"), Some(HighlightKind::Number));
        assert_eq!(map_highlight("number.float"), Some(HighlightKind::Number));
        assert_eq!(map_highlight("operator"), Some(HighlightKind::Operator));
        assert_eq!(map_highlight("variable"), Some(HighlightKind::Variable));
        assert_eq!(
            map_highlight("variable.parameter"),
            Some(HighlightKind::Variable)
        );
    }

    #[test]
    fn punctuation_constant_property() {
        assert_eq!(
            map_highlight("punctuation.bracket"),
            Some(HighlightKind::Punctuation)
        );
        assert_eq!(map_highlight("constant"), Some(HighlightKind::Constant));
        assert_eq!(map_highlight("property"), Some(HighlightKind::Property));
    }

    #[test]
    fn tag_and_attribute() {
        assert_eq!(map_highlight("tag"), Some(HighlightKind::Tag));
        assert_eq!(map_highlight("attribute"), Some(HighlightKind::Attribute));
    }

    #[test]
    fn prefix_fallback_for_unknown_hierarchical_names() {
        assert_eq!(
            map_highlight("keyword.custom.thing"),
            Some(HighlightKind::Keyword)
        );
        assert_eq!(
            map_highlight("function.special"),
            Some(HighlightKind::Function)
        );
        assert_eq!(map_highlight("type.custom"), Some(HighlightKind::Type));
    }

    #[test]
    fn completely_unknown_returns_none() {
        assert_eq!(map_highlight("unknown"), None);
        assert_eq!(map_highlight("module"), None);
        assert_eq!(map_highlight("embedded"), None);
    }
}
