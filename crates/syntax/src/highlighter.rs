use std::path::Path;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};
use tree_sitter_language::LanguageFn;

use codepeek_core::{HighlightSpan, HighlightedLine, SyntaxError, SyntaxHighlighter};

use crate::languages::detect_language;
use crate::mapping::map_highlight;

// Indices here correspond to tree-sitter `Highlight` event indices.
// `HighlightConfiguration::configure` maps capture names to these positions.
const HIGHLIGHT_NAMES: &[&str] = &[
    "keyword",
    "function",
    "function.builtin",
    "function.method",
    "function.macro",
    "type",
    "type.builtin",
    "string",
    "string.special",
    "comment",
    "number",
    "operator",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "punctuation.bracket",
    "punctuation.delimiter",
    "constant",
    "constant.builtin",
    "property",
    "tag",
    "attribute",
    "keyword.return",
    "keyword.function",
    "keyword.operator",
    "keyword.control",
    "keyword.conditional",
    "keyword.repeat",
    "keyword.import",
    "keyword.exception",
    "function.call",
    "type.definition",
    "string.escape",
    "comment.documentation",
    "number.float",
    "constant.character",
    "property.definition",
    "punctuation.special",
    "string.regex",
    "keyword.export",
    "keyword.storage",
    "keyword.modifier",
    "type.qualifier",
    "variable.member",
];

pub struct TreeSitter {
    highlighter: Highlighter,
}

impl TreeSitter {
    pub fn new() -> Self {
        Self {
            highlighter: Highlighter::new(),
        }
    }
}

impl Default for TreeSitter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxHighlighter for TreeSitter {
    fn highlight(
        &mut self,
        source: &str,
        path: &Path,
    ) -> Result<Vec<HighlightedLine>, SyntaxError> {
        let lang_name = detect_language(path).ok_or_else(|| SyntaxError::UnsupportedLanguage {
            path: path.to_path_buf(),
        })?;

        let (language, highlights_query, injections_query) = get_language_config(lang_name)
            .ok_or_else(|| SyntaxError::UnsupportedLanguage {
                path: path.to_path_buf(),
            })?;

        let mut config = HighlightConfiguration::new(
            language.into(),
            lang_name,
            highlights_query,
            injections_query,
            "",
        )
        .map_err(|e| SyntaxError::ParseFailed {
            path: path.to_path_buf(),
            source: Box::new(e),
        })?;

        config.configure(HIGHLIGHT_NAMES);

        let events = self
            .highlighter
            .highlight(&config, source.as_bytes(), None, |_| None)
            .map_err(|e| SyntaxError::ParseFailed {
                path: path.to_path_buf(),
                source: Box::new(e),
            })?;

        build_highlighted_lines(source, events)
    }
}

// Each grammar crate bundles its own .scm queries as compile-time constants,
// unlike tree-sitter-language-pack which doesn't ship .scm files.
fn get_language_config(lang_name: &str) -> Option<(LanguageFn, &'static str, &'static str)> {
    match lang_name {
        "rust" => Some((
            tree_sitter_rust::LANGUAGE,
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            tree_sitter_rust::INJECTIONS_QUERY,
        )),
        "python" => Some((
            tree_sitter_python::LANGUAGE,
            tree_sitter_python::HIGHLIGHTS_QUERY,
            "",
        )),
        "javascript" => Some((
            tree_sitter_javascript::LANGUAGE,
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::INJECTIONS_QUERY,
        )),
        "jsx" => Some((
            tree_sitter_javascript::LANGUAGE,
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
            tree_sitter_javascript::INJECTIONS_QUERY,
        )),
        "typescript" => Some((
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            "",
        )),
        "tsx" => Some((
            tree_sitter_typescript::LANGUAGE_TSX,
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            "",
        )),
        "go" => Some((
            tree_sitter_go::LANGUAGE,
            tree_sitter_go::HIGHLIGHTS_QUERY,
            "",
        )),
        "c" => Some((tree_sitter_c::LANGUAGE, tree_sitter_c::HIGHLIGHT_QUERY, "")),
        "cpp" => Some((
            tree_sitter_cpp::LANGUAGE,
            tree_sitter_cpp::HIGHLIGHT_QUERY,
            "",
        )),
        "java" => Some((
            tree_sitter_java::LANGUAGE,
            tree_sitter_java::HIGHLIGHTS_QUERY,
            "",
        )),
        "ruby" => Some((
            tree_sitter_ruby::LANGUAGE,
            tree_sitter_ruby::HIGHLIGHTS_QUERY,
            "",
        )),
        "toml" => Some((
            tree_sitter_toml_ng::LANGUAGE,
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            "",
        )),
        "json" => Some((
            tree_sitter_json::LANGUAGE,
            tree_sitter_json::HIGHLIGHTS_QUERY,
            "",
        )),
        "bash" => Some((
            tree_sitter_bash::LANGUAGE,
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
        )),
        "css" => Some((
            tree_sitter_css::LANGUAGE,
            tree_sitter_css::HIGHLIGHTS_QUERY,
            "",
        )),
        "html" => Some((
            tree_sitter_html::LANGUAGE,
            tree_sitter_html::HIGHLIGHTS_QUERY,
            tree_sitter_html::INJECTIONS_QUERY,
        )),
        "yaml" => Some((
            tree_sitter_yaml::LANGUAGE,
            tree_sitter_yaml::HIGHLIGHTS_QUERY,
            "",
        )),
        "lua" => Some((
            tree_sitter_lua::LANGUAGE,
            tree_sitter_lua::HIGHLIGHTS_QUERY,
            tree_sitter_lua::INJECTIONS_QUERY,
        )),
        "markdown" => Some((
            tree_sitter_md::LANGUAGE,
            tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
            tree_sitter_md::INJECTION_QUERY_BLOCK,
        )),
        _ => None,
    }
}

pub struct Noop;

impl SyntaxHighlighter for Noop {
    fn highlight(
        &mut self,
        source: &str,
        _path: &Path,
    ) -> Result<Vec<HighlightedLine>, SyntaxError> {
        Ok(source
            .lines()
            .map(|line| HighlightedLine {
                content: line.to_string(),
                spans: Vec::new(),
            })
            .collect())
    }
}

fn build_highlighted_lines(
    source: &str,
    events: impl Iterator<Item = Result<HighlightEvent, tree_sitter_highlight::Error>>,
) -> Result<Vec<HighlightedLine>, SyntaxError> {
    // Pre-compute line boundaries from the source text.
    let line_contents: Vec<&str> = source.split('\n').collect();
    let line_count = line_contents.len();

    let mut lines: Vec<HighlightedLine> = line_contents
        .iter()
        .map(|content| HighlightedLine {
            content: (*content).to_string(),
            spans: Vec::new(),
        })
        .collect();

    // Pre-compute the byte offset where each line starts.
    let mut line_starts: Vec<usize> = Vec::with_capacity(line_count);
    let mut offset = 0;
    for (i, content) in line_contents.iter().enumerate() {
        line_starts.push(offset);
        offset += content.len();
        // Account for the '\n' separator (except after the last line).
        if i + 1 < line_count {
            offset += 1;
        }
    }

    let mut highlight_stack: Vec<&str> = Vec::new();
    let source_bytes = source.as_bytes();

    for event in events {
        let event = event.map_err(|e| SyntaxError::ParseFailed {
            path: std::path::PathBuf::from("<source>"),
            source: Box::new(e),
        })?;

        match event {
            HighlightEvent::HighlightStart(highlight) => {
                let idx = highlight.0;
                let name = HIGHLIGHT_NAMES.get(idx).copied().unwrap_or("");
                highlight_stack.push(name);
            }
            HighlightEvent::HighlightEnd => {
                highlight_stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let current_kind = highlight_stack.last().and_then(|name| map_highlight(name));

                if let Some(kind) = current_kind {
                    add_spans_for_range(source_bytes, start, end, kind, &line_starts, &mut lines);
                }
            }
        }
    }

    Ok(lines)
}

fn add_spans_for_range(
    source: &[u8],
    start: usize,
    end: usize,
    kind: codepeek_core::HighlightKind,
    line_starts: &[usize],
    lines: &mut [HighlightedLine],
) {
    // Find which line the start byte falls on.
    let start_line = match line_starts.binary_search(&start) {
        Ok(i) => i,
        Err(i) => i.saturating_sub(1),
    };

    let mut pos = start;
    let mut line_idx = start_line;

    while pos < end && line_idx < lines.len() {
        let line_start = line_starts[line_idx];
        let line_end = line_start + lines[line_idx].content.len();

        // Clamp the span to this line.
        let span_start = pos.max(line_start) - line_start;
        let span_end = end.min(line_end) - line_start;

        if span_start < span_end {
            lines[line_idx].spans.push(HighlightSpan {
                start: span_start,
                end: span_end,
                kind,
            });
        }

        // Advance past this line (including the newline byte).
        if end <= line_end {
            break;
        }
        // Move past the '\n' character to the next line.
        pos = if line_end < source.len() {
            line_end + 1
        } else {
            line_end
        };
        line_idx += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_returns_correct_line_count() {
        let source = "line one\nline two\nline three";
        let mut hl = Noop;
        let result = hl.highlight(source, Path::new("test.rs")).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].content, "line one");
        assert_eq!(result[1].content, "line two");
        assert_eq!(result[2].content, "line three");
    }

    #[test]
    fn noop_produces_no_spans() {
        let source = "fn main() {}";
        let mut hl = Noop;
        let result = hl.highlight(source, Path::new("test.rs")).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].spans.is_empty());
    }

    #[test]
    fn noop_handles_empty_source() {
        let mut hl = Noop;
        let result = hl.highlight("", Path::new("test.rs")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn rejects_unsupported_extension() {
        let mut hl = TreeSitter::new();
        let result = hl.highlight("hello", Path::new("file.xyz"));
        assert!(result.is_err());
        match result.unwrap_err() {
            SyntaxError::UnsupportedLanguage { path } => {
                assert_eq!(path, Path::new("file.xyz"));
            }
            other @ SyntaxError::ParseFailed { .. } => {
                panic!("expected UnsupportedLanguage, got {other:?}")
            }
        }
    }

    #[test]
    fn rust_snippet_has_keyword_highlight() {
        let source = "fn main() {\n    let x = 42;\n}";
        let mut hl = TreeSitter::new();
        let result = hl.highlight(source, Path::new("test.rs")).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].content, "fn main() {");
        assert_eq!(result[1].content, "    let x = 42;");
        assert_eq!(result[2].content, "}");

        assert!(
            !result[0].spans.is_empty(),
            "expected highlight spans on 'fn main() {{' — are lang-rust queries bundled?"
        );
        let has_keyword = result[0]
            .spans
            .iter()
            .any(|s| s.kind == codepeek_core::HighlightKind::Keyword);
        assert!(has_keyword, "expected 'fn' to be highlighted as Keyword");
    }

    #[test]
    fn rust_snippet_has_number_literal_highlight() {
        let source = "let x = 42;";
        let mut hl = TreeSitter::new();
        let result = hl.highlight(source, Path::new("test.rs")).unwrap();

        // tree-sitter-rust classifies integer/float literals as @constant.builtin,
        // which our mapping converts to HighlightKind::Constant.
        let has_constant = result[0]
            .spans
            .iter()
            .any(|s| s.kind == codepeek_core::HighlightKind::Constant);
        assert!(
            has_constant,
            "expected '42' to be highlighted as Constant (via @constant.builtin)"
        );
    }

    #[test]
    fn python_snippet_has_keyword_highlight() {
        let source = "def hello():\n    return 42";
        let mut hl = TreeSitter::new();
        let result = hl.highlight(source, Path::new("example.py")).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "def hello():");
        assert_eq!(result[1].content, "    return 42");

        let has_keyword = result[0]
            .spans
            .iter()
            .any(|s| s.kind == codepeek_core::HighlightKind::Keyword);
        assert!(has_keyword, "expected 'def' to be highlighted as Keyword");
    }

    #[test]
    fn highlight_queries_are_bundled_for_common_languages() {
        let languages = ["rust", "python", "javascript", "typescript", "go", "toml"];
        for lang in languages {
            assert!(
                get_language_config(lang).is_some(),
                "language config not available for '{lang}'"
            );
        }
    }

    #[test]
    fn rust_produces_keyword_spans() {
        let mut hl = TreeSitter::new();
        let result = hl.highlight("fn main() {}", Path::new("test.rs")).unwrap();
        assert!(
            !result[0].spans.is_empty(),
            "rust highlighting produced no spans"
        );
    }

    #[test]
    fn python_produces_keyword_spans() {
        let mut hl = TreeSitter::new();
        let result = hl
            .highlight("def hello(): pass", Path::new("test.py"))
            .unwrap();
        assert!(
            !result[0].spans.is_empty(),
            "python highlighting produced no spans"
        );
    }

    #[test]
    fn single_line() {
        let source = "let x = 1;";
        let mut hl = TreeSitter::new();
        let result = hl.highlight(source, Path::new("test.rs")).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "let x = 1;");
    }

    #[test]
    fn empty_source() {
        let mut hl = TreeSitter::new();
        let result = hl.highlight("", Path::new("test.rs")).unwrap();
        // Even empty source produces one "line" from split('\n').
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "");
    }

    #[test]
    fn build_highlighted_lines_with_no_events() {
        let source = "hello";
        let events: Vec<Result<HighlightEvent, tree_sitter_highlight::Error>> =
            vec![Ok(HighlightEvent::Source { start: 0, end: 5 })];
        let result = build_highlighted_lines(source, events.into_iter()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "hello");
        // No highlight start/end events, so no spans.
        assert!(result[0].spans.is_empty());
    }

    #[test]
    fn build_highlighted_lines_with_highlight_event() {
        let source = "fn x";
        let events: Vec<Result<HighlightEvent, tree_sitter_highlight::Error>> = vec![
            // "fn" is keyword (index 0 in HIGHLIGHT_NAMES)
            Ok(HighlightEvent::HighlightStart(
                tree_sitter_highlight::Highlight(0),
            )),
            Ok(HighlightEvent::Source { start: 0, end: 2 }),
            Ok(HighlightEvent::HighlightEnd),
            Ok(HighlightEvent::Source { start: 2, end: 4 }),
        ];
        let result = build_highlighted_lines(source, events.into_iter()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "fn x");
        assert_eq!(result[0].spans.len(), 1);
        assert_eq!(result[0].spans[0].start, 0);
        assert_eq!(result[0].spans[0].end, 2);
        assert_eq!(
            result[0].spans[0].kind,
            codepeek_core::HighlightKind::Keyword
        );
    }

    #[test]
    fn go_snippet_has_keyword() {
        let source = "package main\n\nfunc main() {}";
        let mut hl = TreeSitter::new();
        let result = hl.highlight(source, Path::new("main.go")).unwrap();
        let has_keyword = result[0]
            .spans
            .iter()
            .any(|s| s.kind == codepeek_core::HighlightKind::Keyword);
        assert!(
            has_keyword,
            "expected 'package' to be highlighted as Keyword"
        );
    }

    #[test]
    fn jsx_highlighting_works() {
        let source = "const App = () => <div>hello</div>;";
        let mut hl = TreeSitter::new();
        let result = hl.highlight(source, Path::new("app.jsx")).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn tsx_highlighting_works() {
        let source = "const App: React.FC = () => <div>hello</div>;";
        let mut hl = TreeSitter::new();
        let result = hl.highlight(source, Path::new("app.tsx")).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn toml_highlighting_works() {
        let source = "[package]\nname = \"test\"";
        let mut hl = TreeSitter::new();
        let result = hl.highlight(source, Path::new("Cargo.toml")).unwrap();
        let has_string = result
            .iter()
            .flat_map(|l| &l.spans)
            .any(|s| s.kind == codepeek_core::HighlightKind::String);
        assert!(has_string, "expected string highlight in TOML");
    }

    #[test]
    fn json_highlighting_works() {
        let source = r#"{"key": "value"}"#;
        let mut hl = TreeSitter::new();
        let result = hl.highlight(source, Path::new("data.json")).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn default_creates_same_as_new() {
        let a = TreeSitter::new();
        let b = TreeSitter::default();
        let source = "fn x() {}";
        let mut hl_a = a;
        let mut hl_b = b;
        let r_a = hl_a.highlight(source, Path::new("t.rs")).unwrap();
        let r_b = hl_b.highlight(source, Path::new("t.rs")).unwrap();
        assert_eq!(r_a.len(), r_b.len());
    }

    #[test]
    fn noop_handles_single_line_no_newline() {
        let mut hl = Noop;
        let result = hl.highlight("single", Path::new("test.rs")).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "single");
    }

    #[test]
    fn build_highlighted_lines_across_line_boundary() {
        let source = "ab\ncd";
        let events: Vec<Result<HighlightEvent, tree_sitter_highlight::Error>> = vec![
            // Highlight spans the entire source including newline.
            Ok(HighlightEvent::HighlightStart(
                tree_sitter_highlight::Highlight(7), // "string" in HIGHLIGHT_NAMES
            )),
            Ok(HighlightEvent::Source { start: 0, end: 5 }),
            Ok(HighlightEvent::HighlightEnd),
        ];
        let result = build_highlighted_lines(source, events.into_iter()).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "ab");
        assert_eq!(result[1].content, "cd");
        // First line: span covers bytes 0..2 ("ab")
        assert_eq!(result[0].spans.len(), 1);
        assert_eq!(result[0].spans[0].start, 0);
        assert_eq!(result[0].spans[0].end, 2);
        // Second line: span covers bytes 0..2 ("cd")
        assert_eq!(result[1].spans.len(), 1);
        assert_eq!(result[1].spans[0].start, 0);
        assert_eq!(result[1].spans[0].end, 2);
    }
}
