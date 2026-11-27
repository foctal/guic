use super::{DiffDocument, DiffLine, DiffLineKind, DisplayRow};
pub use diffy::patch_set::PatchSetParseError as DiffPatchError;
use diffy::{
    Hunk, Line,
    patch_set::{FileOperation, ParseOptions, PatchSet},
};
use gpui::SharedString;
use std::sync::Arc;

/// A file operation recorded in a unified or Git patch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffFileChange {
    /// Change content or mode.
    Modify,
    /// Add a new file.
    Create,
    /// Remove a file.
    Delete,
    /// Move a file, optionally with content edits.
    Rename,
    /// Copy a file, optionally with content edits.
    Copy,
}

/// A contiguous part of a patch with the line numbers from its hunk header.
#[derive(Clone, Debug)]
pub struct DiffPatchHunk {
    /// Optional function or section name from the hunk header.
    pub heading: Option<SharedString>,
    /// Displayable lines in this hunk, including all of its supplied context.
    pub document: DiffDocument,
}

impl DiffPatchHunk {
    fn new(hunk: &Hunk<'_, str>) -> Self {
        let mut old = hunk.old_range().start();
        let mut new = hunk.new_range().start();
        let lines: Arc<[_]> = hunk
            .lines()
            .iter()
            .map(|line| {
                let (kind, old_line, new_line, source) = match *line {
                    Line::Context(s) => (DiffLineKind::Context, Some(old), Some(new), s),
                    Line::Delete(s) => (DiffLineKind::Removal, Some(old), None, s),
                    Line::Insert(s) => (DiffLineKind::Addition, None, Some(new), s),
                };
                old = old.saturating_add(usize::from(old_line.is_some()));
                new = new.saturating_add(usize::from(new_line.is_some()));
                DiffLine::from_source(kind, old_line, new_line, source)
            })
            .collect();
        Self {
            heading: hunk
                .function_context()
                .map(|value| SharedString::from(value.trim_end_matches(['\r', '\n']))),
            document: DiffDocument {
                rows: (0..lines.len()).map(DisplayRow::Line).collect(),
                lines,
                split: Arc::default(),
            },
        }
    }
}

/// An owned file entry in a UTF-8 unified or Git patch.
#[derive(Clone, Debug)]
pub struct DiffPatchFile {
    /// Operation specified by the patch headers.
    pub change: DiffFileChange,
    /// Original file label, when the file existed.
    pub original: Option<SharedString>,
    /// Revised file label, when the file exists after the change.
    pub revised: Option<SharedString>,
    /// Whether the patch describes binary content.
    pub binary: bool,
    /// Text hunks in patch order. Metadata-only entries have no hunks.
    pub hunks: Vec<DiffPatchHunk>,
}

impl DiffPatchFile {
    /// Imports all files for display. File labels are treated as text.
    /// A malformed file returns an error for the entire import.
    pub fn parse(input: &str) -> Result<Vec<Self>, DiffPatchError> {
        let options = if input.lines().any(|line| line.starts_with("diff --git ")) {
            ParseOptions::gitdiff()
        } else {
            ParseOptions::unidiff()
        };
        PatchSet::parse(input, options)
            .map(|entry| {
                let entry = entry?;
                let (change, original, revised) = match entry.operation() {
                    FileOperation::Create(p) => (DiffFileChange::Create, None, Some(p)),
                    FileOperation::Delete(p) => (DiffFileChange::Delete, Some(p), None),
                    FileOperation::Modify { original, modified } => {
                        (DiffFileChange::Modify, Some(original), Some(modified))
                    }
                    FileOperation::Rename { from, to } => {
                        (DiffFileChange::Rename, Some(from), Some(to))
                    }
                    FileOperation::Copy { from, to } => {
                        (DiffFileChange::Copy, Some(from), Some(to))
                    }
                };
                let content = entry.patch();
                Ok(Self {
                    change,
                    original: original.map(|p| SharedString::from(p.as_ref())),
                    revised: revised.map(|p| SharedString::from(p.as_ref())),
                    binary: content.is_binary(),
                    hunks: content
                        .as_text()
                        .map(|patch| patch.hunks().iter().map(DiffPatchHunk::new).collect())
                        .unwrap_or_default(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_multiple_files_and_preserves_coordinates_and_eof() {
        let files = DiffPatchFile::parse("--- a\n+++ a\n@@ -42,2 +51,2 @@ section\n kept\n-before\n+after\n\\ No newline at end of file\n--- /dev/null\n+++ b\n@@ -0,0 +1 @@\n+created\n").expect("valid patch");
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].change, DiffFileChange::Modify);
        let hunk = &files[0].hunks[0];
        assert_eq!(hunk.heading.as_deref(), Some("section"));
        let lines = hunk.document.lines();
        assert_eq!((lines[0].old_line, lines[0].new_line), (Some(42), Some(51)));
        assert_eq!(
            (lines[2].old_line, lines[2].new_line, lines[2].ending),
            (None, Some(52), "")
        );
        assert_eq!(files[1].change, DiffFileChange::Create);
        assert_eq!(files[1].original, None);
        assert_eq!(files[1].revised.as_deref(), Some("b"));
    }

    #[test]
    fn imports_git_metadata_and_rejects_a_malformed_second_file() {
        let files = DiffPatchFile::parse("diff --git a/one b/two\nsimilarity index 100%\nrename from one\nrename to two\ndiff --git a/img b/img\nindex 0000001..0000002 100644\nBinary files a/img and b/img differ\n").expect("valid patch");
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].change, DiffFileChange::Rename);
        assert!(files[1].binary && files[1].hunks.is_empty());
        assert!(
            DiffPatchFile::parse(
                "--- a\n+++ a\n@@ -1 +1 @@\n-a\n+b\n--- b\n+++ b\n@@ invalid @@\n"
            )
            .is_err()
        );
    }
}
