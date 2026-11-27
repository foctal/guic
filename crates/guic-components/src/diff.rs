//! A virtualized, line-based comparison of two text revisions.

mod merge;
mod patch;
pub use merge::{MergeChoice, MergeConflict, MergeConflictView, MergeDocument, MergeParseError};
pub use patch::{DiffFileChange, DiffPatchError, DiffPatchFile, DiffPatchHunk};

use crate::VirtualList;
use gpui::{
    App, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled as _, UniformListScrollHandle, Window,
    div, px,
};
use guic_tokens::Theme;
use similar::{Algorithm, ChangeTag, TextDiff};
use std::{
    ops::Range,
    rc::Rc,
    sync::{Arc, OnceLock},
};

/// The relationship of a line to the two revisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineKind {
    /// Present in both texts.
    Context,
    /// Present only in the original text.
    Removal,
    /// Present only in the revised text.
    Addition,
}

/// A source line with one-based line numbers and its original terminator.
#[derive(Clone, Debug)]
pub struct DiffLine {
    /// The change represented by this line.
    pub kind: DiffLineKind,
    /// Original line number, when present.
    pub old_line: Option<usize>,
    /// Revised line number, when present.
    pub new_line: Option<usize>,
    /// Source text without its line ending.
    pub text: SharedString,
    /// The original line terminator: LF, CRLF, CR, or the empty string at EOF.
    pub ending: &'static str,
}

impl DiffLine {
    fn from_source(
        kind: DiffLineKind,
        old_line: Option<usize>,
        new_line: Option<usize>,
        source: &str,
    ) -> Self {
        let ending = ["\r\n", "\n", "\r"]
            .into_iter()
            .find(|ending| source.ends_with(ending))
            .unwrap_or("");
        Self {
            kind,
            old_line,
            new_line,
            text: SharedString::from(&source[..source.len() - ending.len()]),
            ending,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum DisplayRow {
    Line(usize),
    Hidden(Range<usize>),
}

/// Arrangement of the original and revised lines.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DiffLayout {
    /// Show removals and additions in a single column.
    #[default]
    Unified,
    /// Align original and revised lines in two equal-width columns.
    Split,
}

#[derive(Clone, Debug, PartialEq)]
enum SplitRow {
    Pair {
        old: Option<usize>,
        new: Option<usize>,
    },
    Hidden(Range<usize>),
}

/// A shareable comparison model. Build it once per revision pair. For large
/// inputs call `compare` on a background worker before updating the view.
#[derive(Clone, Debug)]
pub struct DiffDocument {
    lines: Arc<[DiffLine]>,
    rows: Arc<[DisplayRow]>,
    split: Arc<OnceLock<Vec<SplitRow>>>,
}

impl DiffDocument {
    /// Compares two texts with the patience algorithm and three context lines per change.
    #[must_use]
    pub fn compare(original: &str, revised: &str) -> Self {
        Self::from_diff(&compare_lines(original, revised))
    }

    /// Returns a comparison and its unified hunks with three context lines.
    /// File labels can be prepended by the caller. Both results use the same edit script.
    #[must_use]
    pub fn compare_with_patch(original: &str, revised: &str) -> (Self, String) {
        let diff = compare_lines(original, revised);
        let patch = diff.unified_diff().to_string();
        (Self::from_diff(&diff), patch)
    }

    fn from_diff<'a>(diff: &TextDiff<'a, 'a, 'a, str>) -> Self {
        let lines = diff
            .iter_all_changes()
            .map(|change| {
                DiffLine::from_source(
                    match change.tag() {
                        ChangeTag::Equal => DiffLineKind::Context,
                        ChangeTag::Delete => DiffLineKind::Removal,
                        ChangeTag::Insert => DiffLineKind::Addition,
                    },
                    change.old_index().map(|index| index + 1),
                    change.new_index().map(|index| index + 1),
                    change.value(),
                )
            })
            .collect();
        Self {
            lines,
            rows: Arc::default(),
            split: Arc::default(),
        }
        .with_context(3)
    }

    /// Sets the number of unchanged lines shown beside a change.
    /// `usize::MAX` displays complete texts. This resets expanded ranges.
    #[must_use]
    pub fn with_context(mut self, context: usize) -> Self {
        let mut rows = Vec::new();
        let mut start = 0;
        while start < self.lines.len() {
            if self.lines[start].kind != DiffLineKind::Context {
                rows.push(DisplayRow::Line(start));
                start += 1;
                continue;
            }
            let end = start
                + self.lines[start..]
                    .iter()
                    .take_while(|line| line.kind == DiffLineKind::Context)
                    .count();
            let leading = if start > 0 || context == usize::MAX {
                context.min(end - start)
            } else {
                0
            };
            let trailing = if end < self.lines.len() {
                context.min(end - start - leading)
            } else {
                0
            };
            rows.extend((start..start + leading).map(DisplayRow::Line));
            if start + leading < end - trailing {
                rows.push(DisplayRow::Hidden(start + leading..end - trailing));
            }
            rows.extend((end - trailing..end).map(DisplayRow::Line));
            start = end;
        }
        self.rows = rows.into();
        self.split = Arc::default();
        self
    }

    fn split_rows(&self) -> &[SplitRow] {
        self.split.get_or_init(|| {
            let mut result = Vec::new();
            let mut rows = self.rows.as_ref();
            while let Some((row, rest)) = rows.split_first() {
                match row {
                    DisplayRow::Hidden(range) => result.push(SplitRow::Hidden(range.clone())),
                    DisplayRow::Line(i) if self.lines[*i].kind == DiffLineKind::Context => result
                        .push(SplitRow::Pair {
                            old: Some(*i),
                            new: Some(*i),
                        }),
                    DisplayRow::Line(_) => {
                        let count = rows.iter().take_while(|r| matches!(r,
                            DisplayRow::Line(i) if self.lines[*i].kind != DiffLineKind::Context
                        )).count();
                        let changes = &rows[..count];
                        let side = |kind| {
                            changes.iter().filter_map(move |r| match r {
                                DisplayRow::Line(i) if self.lines[*i].kind == kind => Some(*i),
                                _ => None,
                            })
                        };
                        let mut removals = side(DiffLineKind::Removal);
                        let mut additions = side(DiffLineKind::Addition);
                        loop {
                            let old = removals.next();
                            let new = additions.next();
                            if old.is_none() && new.is_none() {
                                break;
                            }
                            result.push(SplitRow::Pair { old, new });
                        }
                        rows = &rows[count..];
                        continue;
                    }
                }
                rows = rest;
            }
            result
        })
    }

    /// All comparison lines in display order, including collapsed context.
    #[must_use]
    pub fn lines(&self) -> &[DiffLine] {
        &self.lines
    }

    /// Expands the exact collapsed range emitted by this revision's view.
    /// Returns whether the presentation changed.
    pub fn expand(&mut self, range: Range<usize>) -> bool {
        let Some(index) = self
            .rows
            .iter()
            .position(|row| matches!(row, DisplayRow::Hidden(hidden) if *hidden == range))
        else {
            return false;
        };
        let mut rows = Vec::with_capacity(self.rows.len() - 1 + range.len());
        rows.extend_from_slice(&self.rows[..index]);
        rows.extend(range.map(DisplayRow::Line));
        rows.extend_from_slice(&self.rows[index + 1..]);
        self.rows = rows.into();
        self.split = Arc::default();
        true
    }
}

fn compare_lines<'a>(original: &'a str, revised: &'a str) -> TextDiff<'a, 'a, 'a, str> {
    TextDiff::configure()
        .algorithm(Algorithm::Patience)
        .diff_lines(original, revised)
}

type ExpandHandler = Rc<dyn Fn(&Range<usize>, &mut Window, &mut App)>;

/// A read-only unified diff with old and new line numbers and collapsible context.
#[derive(gpui::IntoElement)]
pub struct DiffView {
    id: SharedString,
    label: SharedString,
    document: DiffDocument,
    layout: DiffLayout,
    height: Pixels,
    scroll: Option<UniformListScrollHandle>,
    on_expand: Option<ExpandHandler>,
}

impl DiffView {
    /// Creates a comparison view backed by a retained document.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, document: DiffDocument) -> Self {
        Self {
            id: id.into(),
            label: "Changes".into(),
            document,
            layout: DiffLayout::Unified,
            height: px(320.),
            scroll: None,
            on_expand: None,
        }
    }

    /// Chooses a unified or side-by-side presentation of the retained comparison.
    #[must_use]
    pub fn layout(mut self, layout: DiffLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Sets the accessible name.
    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Sets the height of the scrollable body.
    #[must_use]
    pub fn height(mut self, height: Pixels) -> Self {
        if height > px(0.) && height.to_f64().is_finite() {
            self.height = height;
        }
        self
    }

    /// Attaches a host-owned uniform-list handle for programmatic scrolling.
    #[must_use]
    pub fn track_scroll(mut self, scroll: UniformListScrollHandle) -> Self {
        self.scroll = Some(scroll);
        self
    }

    /// Makes collapsed context interactive. Call `DiffDocument::expand` in the handler
    /// and notify the host view to show the newly revealed lines.
    #[must_use]
    pub fn on_expand(
        mut self,
        handler: impl Fn(&Range<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_expand = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for DiffView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx).clone();
        let mut root = div()
            .id(self.id)
            .w_full()
            .role(gpui::Role::List)
            .aria_label(self.label);
        if self.document.rows.is_empty() {
            return root.child(guic_core::translate(cx, "diff.empty", "No changes"));
        }
        let hidden_label = guic_core::translate(cx, "diff.unchanged_lines", "Unchanged lines");
        let no_newline = guic_core::translate(cx, "diff.no_newline", "No newline");
        let count = match self.layout {
            DiffLayout::Unified => self.document.rows.len(),
            DiffLayout::Split => self.document.split_rows().len(),
        };
        let list = VirtualList::new("diff-rows", count, move |range, _, _| {
            range
                .map(|index| {
                    let base = div()
                        .w_full()
                        .h(px(26.))
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .overflow_hidden();
                    let hidden = match self.layout {
                        DiffLayout::Split => match &self.document.split_rows()[index] {
                            SplitRow::Hidden(range) => range,
                            SplitRow::Pair { old, new } => {
                                let mut row = base
                                    .id(("pair", index))
                                    .role(gpui::Role::ListItem)
                                    .aria_position_in_set(index + 1)
                                    .aria_size_of_set(count)
                                    .font_family("monospace");
                                for (side, source) in [("old", old), ("new", new)] {
                                    let mut cell = div()
                                        .flex_1()
                                        .min_w_0()
                                        .h_full()
                                        .overflow_hidden()
                                        .debug_selector(move || format!("guic-diff-{side}-{index}"))
                                        .flex()
                                        .items_center()
                                        .gap_1();
                                    if let Some(i) = source {
                                        let line = &self.document.lines[*i];
                                        let number = if side == "old" {
                                            line.old_line
                                        } else {
                                            line.new_line
                                        };
                                        cell = cell.child(line_number(number, &theme));
                                        cell = line_content(cell, line, &theme, &no_newline);
                                    }
                                    row = row.child(cell);
                                }
                                return row.into_any_element();
                            }
                        },
                        DiffLayout::Unified => match &self.document.rows[index] {
                            DisplayRow::Hidden(range) => range,
                            DisplayRow::Line(i) => {
                                let line = &self.document.lines[*i];
                                let row = base
                                    .id(("line", *i))
                                    .role(gpui::Role::ListItem)
                                    .aria_position_in_set(index + 1)
                                    .aria_size_of_set(count)
                                    .font_family("monospace")
                                    .child(line_number(line.old_line, &theme))
                                    .child(line_number(line.new_line, &theme));
                                return line_content(row, line, &theme, &no_newline)
                                    .into_any_element();
                            }
                        },
                    };
                    {
                        let mut row = base
                            .id(("gap", hidden.start))
                            .role(gpui::Role::ListItem)
                            .debug_selector(|| format!("guic-diff-gap-{}", hidden.start))
                            .bg(theme.secondary())
                            .text_color(theme.muted_foreground())
                            .child(format!("{hidden_label} ({})", hidden.len()));
                        if let Some(handler) = self.on_expand.clone() {
                            let hidden = hidden.clone();
                            row = row
                                .role(gpui::Role::Button)
                                .tab_index(0)
                                .cursor_pointer()
                                .focus(|s| s.border_1().border_color(theme.primary()))
                                .on_click(move |_, window, cx| handler(&hidden, window, cx));
                        }
                        row.into_any_element()
                    }
                })
                .collect()
        })
        .height(self.height)
        .item_height(px(26.));
        root = root.child(if let Some(handle) = self.scroll {
            list.track_scroll(handle)
        } else {
            list
        });
        root
    }
}

fn line_number(number: Option<usize>, theme: &Theme) -> gpui::Div {
    div()
        .w(px(54.))
        .flex_shrink_0()
        .text_right()
        .text_color(theme.muted_foreground())
        .child(number.map(|n| n.to_string()).unwrap_or_default())
}

fn line_content<E: gpui::Styled + gpui::ParentElement>(
    mut row: E,
    line: &DiffLine,
    theme: &Theme,
    no_newline: &SharedString,
) -> E {
    let (marker, color) = match line.kind {
        DiffLineKind::Context => (" ", theme.muted_foreground()),
        DiffLineKind::Removal => ("−", theme.danger()),
        DiffLineKind::Addition => ("+", theme.success()),
    };
    if line.kind != DiffLineKind::Context {
        row = row.bg(color.opacity(0.10));
    }
    row = row
        .child(
            div()
                .w(px(14.))
                .flex_shrink_0()
                .text_color(color)
                .child(marker),
        )
        .child(div().min_w_0().truncate().child(line.text.clone()));
    if line.kind != DiffLineKind::Context && line.ending != "\n" {
        let ending = match line.ending {
            "\r\n" => "CRLF".into(),
            "\r" => "CR".into(),
            _ => no_newline.clone(),
        };
        row = row.child(
            div()
                .flex_shrink_0()
                .text_color(theme.muted_foreground())
                .child(ending),
        );
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn additions_deletions_and_terminators_round_trip() {
        for (before, after) in [
            ("", ""),
            ("", "new"),
            ("old", ""),
            ("a\r\nb\r\n", "a\r\nc\n"),
            ("old\rline", "new\rline"),
            ("x\n", "x"),
            ("🙂\n", "e\u{301}\n"),
        ] {
            let (doc, patch) = DiffDocument::compare_with_patch(before, after);
            for (old, source) in [(true, before), (false, after)] {
                let mut rebuilt = String::new();
                for (i, line) in doc
                    .lines()
                    .iter()
                    .filter(|line| {
                        if old {
                            line.old_line.is_some()
                        } else {
                            line.new_line.is_some()
                        }
                    })
                    .enumerate()
                {
                    assert_eq!(if old { line.old_line } else { line.new_line }, Some(i + 1));
                    rebuilt.push_str(&line.text);
                    rebuilt.push_str(line.ending);
                }
                assert_eq!(rebuilt, source);
            }
            assert_eq!(patch.is_empty(), before == after);
            if before == "x\n" {
                assert!(patch.contains("\\ No newline at end of file"));
            }
        }
    }

    #[test]
    fn split_aligns_each_change_block_and_keeps_both_revisions() {
        let mut doc = DiffDocument::compare("a\nb\nc\nd\nold\ndeleted\nz", "a\nb\nc\nd\nnew\nz")
            .with_context(1);
        let folded = doc.clone();
        assert!(matches!(folded.split_rows()[0], SplitRow::Hidden(_)));
        assert!(doc.expand(0..3));
        let rows = doc.split_rows();
        assert_eq!(rows.len(), 7);
        assert!(matches!(folded.split_rows()[0], SplitRow::Hidden(_)));
        for (index, (a, b)) in [("old", Some("new")), ("deleted", None)]
            .into_iter()
            .enumerate()
        {
            let SplitRow::Pair {
                old: Some(old),
                new,
            } = rows[4 + index]
            else {
                panic!("missing aligned edit")
            };
            assert_eq!(doc.lines[old].text.as_ref(), a);
            assert_eq!(new.map(|i| doc.lines[i].text.as_ref()), b);
        }
    }

    #[gpui::test]
    fn split_places_both_sides_of_a_replacement_on_one_row(cx: &mut gpui::TestAppContext) {
        struct Review;
        impl gpui::Render for Review {
            fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
                DiffView::new("review", DiffDocument::compare("old", "new"))
                    .layout(DiffLayout::Split)
            }
        }
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
        });
        let (_, cx) = cx.add_window_view(|_, _| Review);
        let old = cx.debug_bounds("guic-diff-old-0").expect("original column");
        let new = cx.debug_bounds("guic-diff-new-0").expect("revised column");
        assert!(old.right() <= new.left());
        assert_eq!(old.top(), new.top());
        assert!((old.size.width - new.size.width).abs() <= px(1.));
    }

    #[gpui::test]
    fn activating_a_fold_retains_the_original_snapshot(cx: &mut gpui::TestAppContext) {
        use gpui::{Context, Render};
        struct Review(DiffDocument);
        impl Render for Review {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                DiffView::new("review", self.0.clone()).on_expand(cx.listener(
                    |view, range: &Range<usize>, _, cx| {
                        assert!(view.0.expand(range.clone()));
                        cx.notify();
                    },
                ))
            }
        }
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
        });
        let original = DiffDocument::compare("a\nb\nc\nold\nz", "a\nb\nc\nnew\nz").with_context(1);
        let (view, cx) = cx.add_window_view(|_, _| Review(original.clone()));
        let gap = cx.debug_bounds("guic-diff-gap-0").expect("fold");
        cx.simulate_click(gap.center(), gpui::Modifiers::none());
        view.read_with(cx, |review, _| {
            assert!(matches!(review.0.rows[0], DisplayRow::Line(0)));
            assert!(matches!(original.rows[0], DisplayRow::Hidden(_)));
        });
        assert!(cx.debug_bounds("guic-diff-gap-0").is_none());
        assert!(
            DiffDocument::compare("a\nb", "a\nb")
                .with_context(usize::MAX)
                .rows
                .iter()
                .all(|r| matches!(r, DisplayRow::Line(_)))
        );
    }
}
