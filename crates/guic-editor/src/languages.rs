//! File-extension lookup for the optional bundled grammars.

use crate::EditorGrammar;
use std::{path::Path, sync::OnceLock};
use tree_sitter_javascript as js;
use tree_sitter_typescript as ts;

macro_rules! cached {
    ($query:expr) => {{
        static CONFIG: OnceLock<EditorGrammar> = OnceLock::new();
        CONFIG
            .get_or_init(|| $query.expect("bundled grammar and queries match"))
            .clone()
    }};
}

impl EditorGrammar {
    /// Looks up a shared grammar by file extension, including JavaScript and CSS
    /// embedded in HTML. Returns `None` for an unrecognized extension.
    #[must_use]
    pub fn for_path(path: impl AsRef<Path>) -> Option<Self> {
        let ext = path.as_ref().extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "rs" => Self::rust(),
            "json" | "geojson" => cached!(Self::new(
                "json",
                tree_sitter_json::LANGUAGE,
                tree_sitter_json::HIGHLIGHTS_QUERY,
                ""
            )),
            "py" | "pyw" | "pyi" => cached!(Self::new(
                "python",
                tree_sitter_python::LANGUAGE,
                tree_sitter_python::HIGHLIGHTS_QUERY,
                ""
            )),
            "js" | "mjs" | "cjs" | "jsx" => Self::javascript(),
            "css" => Self::css(),
            "ts" | "mts" | "cts" => cached!(Self::typescript(false)),
            "tsx" => cached!(Self::typescript(true)),
            "htm" | "html" => cached!(Self::with_embedded_languages(
                "html",
                tree_sitter_html::LANGUAGE,
                tree_sitter_html::HIGHLIGHTS_QUERY,
                "",
                tree_sitter_html::INJECTIONS_QUERY,
                vec![Self::javascript(), Self::css()]
            )),
            _ => return None,
        })
    }

    fn css() -> Self {
        cached!(Self::new(
            "css",
            tree_sitter_css::LANGUAGE,
            tree_sitter_css::HIGHLIGHTS_QUERY,
            ""
        ))
    }

    fn javascript() -> Self {
        cached!(Self::new(
            "javascript",
            js::LANGUAGE,
            &[js::HIGHLIGHT_QUERY, js::JSX_HIGHLIGHT_QUERY].join("\n"),
            js::LOCALS_QUERY
        ))
    }

    fn typescript(jsx: bool) -> Result<Self, tree_sitter::QueryError> {
        let (name, language, jsx_query) = if jsx {
            ("tsx", ts::LANGUAGE_TSX, js::JSX_HIGHLIGHT_QUERY)
        } else {
            ("typescript", ts::LANGUAGE_TYPESCRIPT, "")
        };
        Self::new(
            name,
            language,
            &[js::HIGHLIGHT_QUERY, jsx_query, ts::HIGHLIGHTS_QUERY].join("\n"),
            &[js::LOCALS_QUERY, ts::LOCALS_QUERY].join("\n"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EditorBuffer, SyntaxTokenKind};
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn bundled_queries_compile_and_recognize_tokens() {
        for (file, text, needle, kind) in [
            ("code.rs", "fn main() {}", "fn", SyntaxTokenKind::Keyword),
            (
                "data.JSON",
                "{\"count\": 123}",
                "123",
                SyntaxTokenKind::Number,
            ),
            ("main.py", "# note\npass", "#", SyntaxTokenKind::Comment),
            (
                "app.jsx",
                "const a = <div />",
                "const",
                SyntaxTokenKind::Keyword,
            ),
            (
                "main.ts",
                "interface T { count: number }",
                "interface",
                SyntaxTokenKind::Keyword,
            ),
            (
                "main.tsx",
                "const a: T = <div />",
                "const",
                SyntaxTokenKind::Keyword,
            ),
            (
                "site.css",
                "/* note */ body {}",
                "/*",
                SyntaxTokenKind::Comment,
            ),
            (
                "site.htm",
                "<script>const x = 5;</script>",
                "const",
                SyntaxTokenKind::Keyword,
            ),
        ] {
            let grammar = EditorGrammar::for_path(file).expect(file);
            let spans = grammar
                .highlight(&EditorBuffer::from_text(text), &AtomicUsize::new(0))
                .expect(file);
            let byte = text.find(needle).expect("token in source");
            let tokens = spans.fragment(byte, 0, &[(0, 0), (1, 1)]);
            assert_eq!(tokens[0].kind, kind, "{file}");
        }
        assert!(EditorGrammar::for_path("image.png").is_none());
    }
}
