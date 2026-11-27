//! Review and resolve marked text without modifying the backing file.

use super::{DiffDocument, DiffLayout, DiffView};
use crate::Button;
use gpui::{
    App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div, px,
};
use std::{
    fmt,
    ops::Range,
    rc::Rc,
    sync::{Arc, OnceLock},
};

/// A replacement for one conflict in a working copy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeChoice {
    /// Use the current branch's text.
    Current,
    /// Use the other branch's text.
    Incoming,
    /// Keep the current text followed by the incoming text.
    Both,
    /// Restore the ancestor text in a three-way conflict.
    Base,
}

/// Location of a malformed or incomplete conflict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeParseError {
    /// One-based source line at which the expected marker was missing.
    pub line: usize,
}

impl fmt::Display for MergeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Unexpected or missing conflict marker at line {}",
            self.line
        )
    }
}
impl std::error::Error for MergeParseError {}

/// One conflict in a working copy, including optional diff3 ancestor text.
#[derive(Debug)]
pub struct MergeConflict {
    source: Arc<str>,
    whole: Range<usize>,
    current: Range<usize>,
    incoming: Range<usize>,
    base: Option<Range<usize>>,
    current_label: SharedString,
    incoming_label: SharedString,
    comparison: OnceLock<DiffDocument>,
}

impl MergeConflict {
    /// Current branch's source text, including its original line endings.
    pub fn current(&self) -> &str {
        &self.source[self.current.clone()]
    }
    /// Incoming branch's source text.
    pub fn incoming(&self) -> &str {
        &self.source[self.incoming.clone()]
    }
    /// Ancestor text supplied by diff3 or zdiff3 markers.
    pub fn base(&self) -> Option<&str> {
        self.base.as_ref().map(|r| &self.source[r.clone()])
    }
    /// Source label on the opening marker.
    pub fn current_label(&self) -> &str {
        &self.current_label
    }
    /// Source label on the closing marker.
    pub fn incoming_label(&self) -> &str {
        &self.incoming_label
    }
    /// Lazily prepares a full-context comparison of the current and incoming sides.
    pub fn comparison(&self) -> &DiffDocument {
        self.comparison.get_or_init(|| {
            DiffDocument::compare(self.current(), self.incoming()).with_context(usize::MAX)
        })
    }
}

/// An immutable working-copy snapshot that can contain several conflicts.
#[derive(Clone, Debug)]
pub struct MergeDocument {
    source: Arc<str>,
    conflicts: Arc<[MergeConflict]>,
}

fn marker(text: &str) -> Option<(u8, usize, &str)> {
    let line = text.trim_end_matches(['\r', '\n']);
    let byte = *line.as_bytes().first()?;
    if !matches!(byte, b'<' | b'|' | b'=' | b'>') {
        return None;
    }
    let width = line.bytes().take_while(|c| *c == byte).count();
    let tail = &line[width..];
    if width < 7
        || (!tail.is_empty() && !tail.starts_with([' ', '\t']))
        || (byte == b'=' && !tail.trim().is_empty())
    {
        return None;
    }
    Some((byte, width, tail.trim()))
}

impl MergeDocument {
    /// Parses conflict markers at the beginning of source lines. Marker widths
    /// of seven or more are supported. An ordinary file yields zero conflicts.
    pub fn parse(text: impl Into<Arc<str>>) -> Result<Self, MergeParseError> {
        let source = text.into();
        let mut offset = 0;
        let mut line_count = 0;
        let mut conflicts = Vec::new();
        let mut lines = source.split_inclusive('\n').map(|line| {
            let start = offset;
            offset += line.len();
            line_count += 1;
            (line_count, start..offset, line)
        });
        while let Some((_, opening, line)) = lines.next() {
            let Some((b'<', width, current_label)) = marker(line) else {
                continue;
            };
            let mut base_marker: Option<Range<usize>> = None;
            let mut separator: Option<Range<usize>> = None;
            let mut closed = false;
            for (number, range, line) in lines.by_ref() {
                let Some((kind, other_width, label)) = marker(line) else {
                    continue;
                };
                if other_width != width {
                    continue;
                }
                match kind {
                    b'|' if base_marker.is_none() && separator.is_none() => {
                        base_marker = Some(range)
                    }
                    b'=' if separator.is_none() => separator = Some(range),
                    b'>' if separator.is_some() => {
                        let split = separator.take().expect("matched separator");
                        conflicts.push(MergeConflict {
                            source: source.clone(),
                            current: opening.end
                                ..base_marker.as_ref().map_or(split.start, |r| r.start),
                            base: base_marker.map(|r| r.end..split.start),
                            incoming: split.end..range.start,
                            whole: opening.start..range.end,
                            current_label: current_label.into(),
                            incoming_label: label.into(),
                            comparison: OnceLock::new(),
                        });
                        closed = true;
                        break;
                    }
                    _ => return Err(MergeParseError { line: number }),
                }
            }
            if !closed {
                return Err(MergeParseError {
                    line: line_count + 1,
                });
            }
        }
        Ok(Self {
            source,
            conflicts: conflicts.into(),
        })
    }

    /// Returns conflicts in source order.
    pub fn conflicts(&self) -> &[MergeConflict] {
        &self.conflicts
    }

    /// Produces new text for one resolution, preserving all other source bytes.
    /// An unavailable choice or index returns `None`.
    #[must_use]
    pub fn resolve(&self, index: usize, choice: MergeChoice) -> Option<String> {
        let conflict = self.conflicts.get(index)?;
        let (first, second) = match choice {
            MergeChoice::Current => (conflict.current(), ""),
            MergeChoice::Incoming => (conflict.incoming(), ""),
            MergeChoice::Both => (conflict.current(), conflict.incoming()),
            MergeChoice::Base => (conflict.base()?, ""),
        };
        let mut text = String::with_capacity(
            self.source.len() - conflict.whole.len() + first.len() + second.len(),
        );
        text.push_str(&self.source[..conflict.whole.start]);
        text.push_str(first);
        text.push_str(second);
        text.push_str(&self.source[conflict.whole.end..]);
        Some(text)
    }
}

type ResolveHandler = Rc<dyn Fn(&MergeChoice, &mut Window, &mut App)>;

/// Side-by-side review and keyboard-accessible resolution controls for a conflict.
#[derive(gpui::IntoElement)]
pub struct MergeConflictView {
    id: SharedString,
    document: MergeDocument,
    index: usize,
    height: gpui::Pixels,
    on_resolve: Option<ResolveHandler>,
}

impl MergeConflictView {
    /// Selects one conflict from a retained working-copy snapshot.
    pub fn new(id: impl Into<SharedString>, document: MergeDocument, index: usize) -> Self {
        Self {
            id: id.into(),
            document,
            index,
            height: px(240.),
            on_resolve: None,
        }
    }

    /// Sets the height of the virtualized comparison.
    #[must_use]
    pub fn height(mut self, height: gpui::Pixels) -> Self {
        self.height = height;
        self
    }

    /// Registers a choice callback. The host owns the working copy and undo history.
    #[must_use]
    pub fn on_resolve(
        mut self,
        handler: impl Fn(&MergeChoice, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resolve = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for MergeConflictView {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut root = div().w_full().flex().flex_col().gap_2();
        let Some(conflict) = self.document.conflicts().get(self.index) else {
            return root;
        };
        let current = if conflict.current_label().is_empty() {
            "Current"
        } else {
            conflict.current_label()
        };
        let incoming = if conflict.incoming_label().is_empty() {
            "Incoming"
        } else {
            conflict.incoming_label()
        };
        root = root.child(
            div()
                .flex()
                .child(div().flex_1().child(SharedString::from(current)))
                .child(div().flex_1().child(SharedString::from(incoming))),
        );
        let mut controls = div().flex().flex_wrap().gap_2();
        for (label, choice) in [
            ("Keep current", MergeChoice::Current),
            ("Keep incoming", MergeChoice::Incoming),
            ("Keep both", MergeChoice::Both),
            ("Restore base", MergeChoice::Base),
        ] {
            if choice == MergeChoice::Base && conflict.base().is_none() {
                continue;
            }
            let mut button = Button::new(label)
                .id(format!("{}-{choice:?}", self.id))
                .disabled(self.on_resolve.is_none());
            if let Some(handler) = self.on_resolve.clone() {
                button = button.on_click(move |_, window, cx| handler(&choice, window, cx));
            }
            controls = controls.child(button);
        }
        root.child(
            DiffView::new(format!("{}-sides", self.id), conflict.comparison().clone())
                .height(self.height)
                .layout(DiffLayout::Split),
        )
        .child(controls)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diff3_resolution_preserves_crlf_and_other_conflicts() {
        let tail = "middle\r\n<<<<<<<< other\r\n========\r\nmore\r\n>>>>>>>> next\r\ntail";
        let source = format!(
            "head\r\n<<<<<<<< HEAD\r\nours\r\n|||||||| base\r\nancestor\r\n========\r\ntheirs\r\n>>>>>>>> other\r\n{tail}"
        );
        let document = MergeDocument::parse(source).expect("diff3 and normal conflicts");
        assert_eq!(document.conflicts().len(), 2);
        assert_eq!(document.conflicts()[0].base(), Some("ancestor\r\n"));
        assert_eq!(
            document.resolve(0, MergeChoice::Both),
            Some(format!("head\r\nours\r\ntheirs\r\n{tail}"))
        );
        assert_eq!(
            document.resolve(0, MergeChoice::Base),
            Some(format!("head\r\nancestor\r\n{tail}"))
        );
        assert!(document.resolve(1, MergeChoice::Base).is_none());
        assert_eq!(document.conflicts()[1].current(), "");
    }

    #[gpui::test]
    fn choosing_incoming_replaces_only_the_displayed_conflict(cx: &mut gpui::TestAppContext) {
        use gpui::{Context, Modifiers, Render};
        struct Review(MergeDocument, String);
        impl Render for Review {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                MergeConflictView::new("review", self.0.clone(), 0).on_resolve(cx.listener(
                    |review, choice, _, cx| {
                        review.1 = review.0.resolve(0, *choice).expect("available choice");
                        review.0 =
                            MergeDocument::parse(review.1.as_str()).expect("resolved document");
                        cx.notify();
                    },
                ))
            }
        }
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
        });
        let doc =
            MergeDocument::parse("prefix\n<<<<<<< ours\nold\n=======\nnew\n>>>>>>> theirs\nsuffix")
                .expect("valid conflict");
        let (view, cx) = cx.add_window_view(|_, _| Review(doc, String::new()));
        let button = cx
            .debug_bounds("guic-button-review-Incoming")
            .expect("resolution button");
        cx.simulate_click(button.center(), Modifiers::none());
        view.read_with(cx, |view, _| assert_eq!(view.1, "prefix\nnew\nsuffix"));
        assert!(cx.debug_bounds("guic-button-review-Incoming").is_none());
    }

    #[test]
    fn incomplete_and_nested_markers_fail_without_partial_resolution() {
        assert_eq!(
            MergeDocument::parse("<<<<<<< HEAD\nours\n")
                .expect_err("unclosed")
                .line,
            3
        );
        assert_eq!(
            MergeDocument::parse("<<<<<<< HEAD\n<<<<<<< nested\n")
                .expect_err("nested")
                .line,
            2
        );
        let document =
            MergeDocument::parse("=======\nlet s = \"<<<<<<< HEAD\";").expect("ordinary text");
        assert!(document.conflicts().is_empty());
    }
}
