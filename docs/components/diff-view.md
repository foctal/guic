# DiffView

Enable the `diff` feature and retain a `DiffDocument` in your view. It compares
text by line, preserves original and revised line numbers, and shows three
unchanged lines on either side of a change. Compute large comparisons on a
background worker and deliver the resulting document to the host view.

```rust,ignore
DiffView::new("review", self.changes.clone())
    .label("Settings changes")
    .height(px(320.0))
    .on_expand(cx.listener(|this, range: &std::ops::Range<usize>, _, cx| {
        if this.changes.expand(range.clone()) {
            cx.notify();
        }
    }))
```

Construct the model with `DiffDocument::compare(original, revised)`. Use
`with_context(n)` to change the amount of context, or `with_context(usize::MAX)`
to show both complete revisions. Clones share the line data. Expanding a
collapsed range changes the presentation of the receiving document.

The view virtualizes fixed-height rows. Added and removed lines carry explicit
signs and separate original and revised line numbers. Long lines are truncated
in the viewport. `DiffDocument::lines()` exposes each complete line and its
original terminator. `compare_with_patch` also returns unified hunks, including
missing-newline markers, from the same comparison. Applications can prepend their
own file labels before saving or copying those hunks.

Pass `.layout(DiffLayout::Split)` to align the original and revised text in
two columns. A replacement shares one row, with an empty cell for an unmatched
insertion or deletion. Both columns scroll and expand context together.
The document prepares its split row index on first use and shares it across clones.

Use `DiffPatchFile::parse(text)?` to open an existing unified or Git patch.
It returns one entry per file, including renames, copies and binary markers.
Each text hunk supplies a `DiffDocument` for either view layout. The document
keeps the patch's original line numbers and line endings, so the viewer
can display an excerpt without loading either source file.

Use `MergeDocument::parse(working_copy)?` to review a file with Git conflict
markers. Each conflict exposes its labels, current and incoming text, and the
ancestor text when present. Render `MergeConflictView::new(id, document, index)`
and handle `.on_resolve(...)`. Apply the choice to that same snapshot with
`document.resolve(index, choice)`, then parse the returned text to refresh the
remaining conflicts. This lets the editor retain its own undo and save behavior.
