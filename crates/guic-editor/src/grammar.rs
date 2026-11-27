//! Background grammar highlighting for retained editor snapshots.

use crate::{EditorBuffer, SyntaxToken, SyntaxTokenKind};
use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// A compiled Tree-sitter grammar and its highlight query. Clones share the query.
#[derive(Clone)]
pub struct EditorGrammar {
    pub(crate) config: Arc<HighlightConfiguration>,
    embedded: Arc<[EditorGrammar]>,
}

const CAPTURES: &[(&str, SyntaxTokenKind)] = &[
    ("keyword", SyntaxTokenKind::Keyword),
    ("type", SyntaxTokenKind::Keyword),
    ("function", SyntaxTokenKind::Keyword),
    ("tag", SyntaxTokenKind::Keyword),
    ("operator", SyntaxTokenKind::Keyword),
    ("string", SyntaxTokenKind::String),
    ("comment", SyntaxTokenKind::Comment),
    ("number", SyntaxTokenKind::Number),
    ("constant", SyntaxTokenKind::Number),
];

impl EditorGrammar {
    /// Compiles a grammar's highlight and local-variable queries. Pass an empty
    /// locals query for grammars that do not supply one. Retain the result across frames.
    pub fn new(
        name: impl Into<String>,
        language: impl Into<tree_sitter::Language>,
        highlights: &str,
        locals: &str,
    ) -> Result<Self, tree_sitter::QueryError> {
        Self::with_embedded_languages(name, language, highlights, locals, "", Vec::new())
    }

    /// Compiles a grammar with an injection query and named embedded languages.
    /// Injection names must match the embedded grammar names.
    pub fn with_embedded_languages(
        name: impl Into<String>,
        language: impl Into<tree_sitter::Language>,
        highlights: &str,
        locals: &str,
        injections: &str,
        embedded: Vec<EditorGrammar>,
    ) -> Result<Self, tree_sitter::QueryError> {
        let mut config =
            HighlightConfiguration::new(language.into(), name, highlights, injections, locals)?;
        config.configure(
            &CAPTURES
                .iter()
                .map(|(capture, _)| *capture)
                .collect::<Vec<_>>(),
        );
        Ok(Self {
            config: Arc::new(config),
            embedded: embedded.into(),
        })
    }

    fn embedded_config(&self, name: &str) -> Option<&HighlightConfiguration> {
        self.embedded.iter().find_map(|grammar| {
            if grammar.config.language_name == name {
                Some(grammar.config.as_ref())
            } else {
                grammar.embedded_config(name)
            }
        })
    }

    /// Returns the shared built-in Rust grammar.
    #[cfg(feature = "tree-sitter-rust")]
    #[must_use]
    pub fn rust() -> Self {
        static RUST: std::sync::OnceLock<EditorGrammar> = std::sync::OnceLock::new();
        RUST.get_or_init(|| {
            Self::new(
                "rust",
                tree_sitter_rust::LANGUAGE,
                tree_sitter_rust::HIGHLIGHTS_QUERY,
                "",
            )
            .expect("bundled Rust grammar and query match")
        })
        .clone()
    }

    pub(crate) fn highlight(
        &self,
        buffer: &EditorBuffer,
        cancel: &AtomicUsize,
    ) -> Option<HighlightSpans> {
        let mut source = String::with_capacity(buffer.lines.byte_len());
        for (index, line) in buffer.iter_lines().enumerate() {
            if cancel.load(Ordering::Relaxed) != 0 {
                return None;
            }
            if index > 0 {
                source.push('\n');
            }
            source.push_str(line);
        }
        let mut parser = Highlighter::new();
        let events = parser
            .highlight(&self.config, source.as_bytes(), Some(cancel), |name| {
                self.embedded_config(name)
            })
            .ok()?;
        let mut scopes = Vec::new();
        let mut spans = Vec::new();
        for event in events {
            match event.ok()? {
                HighlightEvent::HighlightStart(highlight) => scopes.push(highlight.0),
                HighlightEvent::HighlightEnd => {
                    scopes.pop();
                }
                HighlightEvent::Source { start, end } => {
                    let Some(scope) = scopes.last() else { continue };
                    let kind = CAPTURES[*scope].1;
                    spans.push((start..end, kind));
                }
            }
        }
        Some(HighlightSpans(spans))
    }
}

/// Non-overlapping ranges in the normalized UTF-8 snapshot.
pub(crate) struct HighlightSpans(Vec<(Range<usize>, SyntaxTokenKind)>);

impl HighlightSpans {
    /// Project only the visible fragment onto the editor's grapheme boundaries.
    pub(crate) fn fragment(
        &self,
        byte_start: usize,
        column_start: usize,
        boundaries: &[(usize, usize)],
    ) -> Vec<SyntaxToken> {
        let mut cursor = self.0.partition_point(|(span, _)| span.end <= byte_start);
        let mut tokens = Vec::<SyntaxToken>::new();
        for (offset, boundary) in boundaries.windows(2).enumerate() {
            let byte = byte_start + boundary[0].0;
            while self.0.get(cursor).is_some_and(|(r, _)| r.end <= byte) {
                cursor += 1;
            }
            let kind = self
                .0
                .get(cursor)
                .filter(|(r, _)| r.contains(&byte))
                .map_or(SyntaxTokenKind::Plain, |(_, kind)| *kind);
            let start = column_start + offset;
            if let Some(token) = tokens.last_mut()
                && token.kind == kind
            {
                token.end = start + 1;
            } else {
                tokens.push(SyntaxToken {
                    start,
                    end: start + 1,
                    kind,
                });
            }
        }
        tokens
    }
}

#[cfg(all(test, feature = "tree-sitter-rust"))]
mod tests {
    use super::*;
    use unicode_segmentation::UnicodeSegmentation;

    #[test]
    fn rust_multiline_scopes_survive_grapheme_clipping() {
        let buffer = EditorBuffer::from_text("/* first\n🙂e\u{301} */ let n = 12;");
        let cancel = AtomicUsize::new(0);
        let grammar = EditorGrammar::rust();
        let spans = grammar
            .highlight(&buffer, &cancel)
            .expect("completed parse");
        let source = buffer.line(1).expect("second line");
        let mut boundaries: Vec<_> = source
            .grapheme_indices(true)
            .map(|(byte, _)| (byte, byte))
            .collect();
        boundaries.push((source.len(), source.len()));
        let tokens = spans.fragment(buffer.lines.line_byte(1), 0, &boundaries);
        assert_eq!(
            tokens[0],
            SyntaxToken {
                start: 0,
                end: 5,
                kind: SyntaxTokenKind::Comment
            }
        );
        assert!(
            tokens
                .iter()
                .any(|t| t.kind == SyntaxTokenKind::Keyword && t.start == 6)
        );
        assert!(
            tokens
                .iter()
                .any(|t| t.kind == SyntaxTokenKind::Number && t.end - t.start == 2)
        );
        cancel.store(1, Ordering::Relaxed);
        assert!(grammar.highlight(&buffer, &cancel).is_none());
    }
}
