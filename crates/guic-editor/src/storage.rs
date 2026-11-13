//! Persistent line chunks and indexed native text offsets.

use super::*;
use crate::analysis::{LineAnalysis, LinePositions};
use std::sync::atomic::AtomicBool;
use std::{
    ops::Index,
    sync::{Arc, OnceLock, Weak},
};

const CHUNK_LINES: usize = 128;

#[derive(Clone, Copy, Debug, Default)]
struct Summary {
    lines: usize,
    bytes: usize,
    utf16: usize,
}
impl Summary {
    fn add(self, other: Self) -> Self {
        Self {
            lines: self.lines + other.lines,
            bytes: self.bytes + other.bytes,
            utf16: self.utf16 + other.utf16,
        }
    }
}

#[derive(Debug)]
struct LineMetadata {
    summary: Summary,
    positions: OnceLock<Arc<LinePositions>>,
    analysis: OnceLock<Arc<LineAnalysis>>,
}
impl LineMetadata {
    fn new(line: &str) -> Self {
        Self::with_positions(line, None)
    }
    fn with_positions(line: &str, positions: Option<Arc<LinePositions>>) -> Self {
        let utf16 = positions.as_ref().map_or_else(
            || line.encode_utf16().count(),
            |p| p.points.last().expect("terminal checkpoint").utf16,
        );
        Self {
            summary: Summary {
                lines: 1,
                bytes: line.len() + 1,
                utf16: utf16 + 1,
            },
            positions: positions.map(OnceLock::from).unwrap_or_default(),
            analysis: OnceLock::new(),
        }
    }
}

#[derive(Debug)]
pub(super) struct Chunk {
    pub(super) lines: Vec<SharedString>,
    offsets: Vec<Summary>,
    metadata: Vec<Arc<LineMetadata>>,
}
impl Chunk {
    pub(super) fn byte_len(&self) -> usize {
        self.summary().bytes
    }
    fn new(lines: Vec<SharedString>) -> Self {
        let metadata = lines
            .iter()
            .map(|line| Arc::new(LineMetadata::new(line)))
            .collect();
        Self::with_metadata(lines, metadata)
    }
    fn with_metadata(lines: Vec<SharedString>, metadata: Vec<Arc<LineMetadata>>) -> Self {
        let mut offsets = vec![Summary::default()];
        for line in &metadata {
            offsets.push(
                offsets
                    .last()
                    .copied()
                    .expect("nonempty offset index")
                    .add(line.summary),
            );
        }
        Self {
            lines,
            offsets,
            metadata,
        }
    }
    pub(super) fn analyze(&self, line: usize, cancelled: &AtomicBool) -> Option<Arc<LineAnalysis>> {
        if let Some(ready) = self.metadata[line].analysis.get() {
            return Some(ready.clone());
        }
        let mut result = LineAnalysis::build(&self.lines[line], cancelled)?;
        if let Some(positions) = self.metadata[line].positions.get() {
            result.positions = positions.clone();
        }
        let result = Arc::new(result);
        let _ = self.metadata[line].positions.set(result.positions.clone());
        let _ = self.metadata[line].analysis.set(result);
        self.metadata[line].analysis.get().cloned()
    }
    fn summary(&self) -> Summary {
        *self.offsets.last().expect("nonempty offset index")
    }
}
#[derive(Debug)]
struct Data {
    chunks: Vec<Arc<Chunk>>,
    offsets: Vec<Summary>,
    flattened: OnceLock<Vec<SharedString>>,
    // Verified negative comparisons only. Inequality is never inferred through
    // ancestry: a later edit may restore the exact text of an older snapshot.
    unequal: [OnceLock<Weak<Data>>; 3],
}
#[derive(Clone, Debug)]
pub(super) struct LineStore(Arc<Data>);
impl PartialEq for LineStore {
    fn eq(&self, other: &Self) -> bool {
        if Arc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        if self.knows_difference(other) || other.knows_difference(self) {
            return false;
        }
        if self.len() != other.len() || self.byte_len() != other.byte_len() {
            return false;
        }
        if self.0.chunks.len() == other.0.chunks.len()
            && self
                .0
                .chunks
                .iter()
                .zip(&other.0.chunks)
                .all(|(a, b)| a.lines.len() == b.lines.len())
        {
            return self
                .0
                .chunks
                .iter()
                .zip(&other.0.chunks)
                .all(|(a, b)| Arc::ptr_eq(a, b) || a.lines == b.lines);
        }
        self.iter().eq(other.iter())
    }
}
impl Eq for LineStore {}
impl From<Vec<SharedString>> for LineStore {
    fn from(lines: Vec<SharedString>) -> Self {
        Self::from_chunks(
            lines
                .chunks(CHUNK_LINES)
                .map(|lines| Arc::new(Chunk::new(lines.to_vec())))
                .collect(),
        )
    }
}
impl Index<usize> for LineStore {
    type Output = SharedString;
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("valid buffer line")
    }
}
impl LineStore {
    fn knows_difference(&self, other: &Self) -> bool {
        self.0.unequal.iter().any(|entry| {
            entry
                .get()
                .is_some_and(|snapshot| std::ptr::eq(snapshot.as_ptr(), Arc::as_ptr(&other.0)))
        })
    }

    /// Runs a comparison on the preparation worker and remembers negative results
    /// so controlled host callbacks do not rescan equally sized pasted documents.
    pub(super) fn compare_and_remember_difference(&self, other: &Self) -> bool {
        if self == other {
            return false;
        }
        if !self.knows_difference(other)
            && let Some(entry) = self.0.unequal.iter().find(|entry| entry.get().is_none())
        {
            let _ = entry.set(Arc::downgrade(&other.0));
        }
        true
    }

    fn from_chunks(chunks: Vec<Arc<Chunk>>) -> Self {
        let mut offsets = vec![Summary::default()];
        for chunk in &chunks {
            offsets.push(
                offsets
                    .last()
                    .copied()
                    .expect("nonempty offset index")
                    .add(chunk.summary()),
            );
        }
        Self(Arc::new(Data {
            chunks,
            offsets,
            flattened: OnceLock::new(),
            unequal: std::array::from_fn(|_| OnceLock::new()),
        }))
    }
    pub(super) fn chunks(&self) -> &[Arc<Chunk>] {
        &self.0.chunks
    }
    pub(super) fn same_snapshot(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    fn summary(&self) -> Summary {
        *self.0.offsets.last().expect("nonempty offset index")
    }
    pub(super) fn len(&self) -> usize {
        self.summary().lines
    }
    pub(super) fn byte_len(&self) -> usize {
        self.summary().bytes.saturating_sub(1)
    }
    pub(super) fn utf16_len(&self) -> usize {
        self.summary().utf16.saturating_sub(1)
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = &SharedString> {
        self.0.chunks.iter().flat_map(|c| c.lines.iter())
    }
    pub(super) fn flattened(&self) -> &[SharedString] {
        self.0
            .flattened
            .get_or_init(|| self.iter().cloned().collect())
    }
    fn locate_line(&self, line: usize) -> (usize, usize) {
        let chunk = self
            .0
            .offsets
            .partition_point(|s| s.lines <= line)
            .saturating_sub(1)
            .min(self.0.chunks.len() - 1);
        (chunk, line - self.0.offsets[chunk].lines)
    }
    pub(super) fn analyze(
        &self,
        index: usize,
        cancelled: &AtomicBool,
    ) -> Option<Arc<LineAnalysis>> {
        if index >= self.len() {
            return None;
        }
        let (chunk, local) = self.locate_line(index);
        self.0.chunks[chunk].analyze(local, cancelled)
    }
    pub(super) fn cached_analysis(&self, index: usize) -> Option<&Arc<LineAnalysis>> {
        if index >= self.len() {
            return None;
        }
        let (chunk, local) = self.locate_line(index);
        self.0.chunks[chunk].metadata[local].analysis.get()
    }
    fn metadata(&self, index: usize) -> &Arc<LineMetadata> {
        let (chunk, local) = self.locate_line(index);
        &self.0.chunks[chunk].metadata[local]
    }
    fn positions(&self, index: usize) -> &Arc<LinePositions> {
        let positions = &self.metadata(index).positions;
        if positions.get().is_none() {
            // Do not hold OnceLock's initialization lock during a long scan:
            // a foreground reader must never wait for a background index build.
            let built = Arc::new(LinePositions::build(&self[index]));
            let _ = positions.set(built);
        }
        positions.get().expect("published position index")
    }
    pub(super) fn column_count(&self, index: usize) -> usize {
        if index >= self.len() {
            return 0;
        }
        self.positions(index).columns
    }
    pub(super) fn column_byte(&self, index: usize, column: usize) -> usize {
        if column == 0 {
            return 0;
        }
        let p = self.positions(index).for_column(column);
        p.byte + grapheme_to_byte(&self[index][p.byte..], column.saturating_sub(p.column))
    }
    pub(super) fn byte_column(&self, index: usize, byte: usize) -> usize {
        if byte == 0 {
            return 0;
        }
        let p = self.positions(index).for_byte(byte);
        p.column + byte_to_grapheme(&self[index][p.byte..], byte.saturating_sub(p.byte))
    }
    pub(super) fn get(&self, index: usize) -> Option<&SharedString> {
        if index >= self.len() {
            return None;
        }
        let (chunk, line) = self.locate_line(index);
        self.0.chunks[chunk].lines.get(line)
    }
    fn line_offset(&self, line: usize) -> Summary {
        let (chunk, local) = self.locate_line(line);
        self.0.offsets[chunk].add(self.0.chunks[chunk].offsets[local])
    }
    pub(super) fn line_byte(&self, line: usize) -> usize {
        self.line_offset(line).bytes
    }
    pub(super) fn byte_line(&self, byte: usize) -> (usize, usize) {
        let byte = byte.min(self.byte_len());
        let chunk = self
            .0
            .offsets
            .partition_point(|s| s.bytes <= byte)
            .saturating_sub(1);
        let local = byte - self.0.offsets[chunk].bytes;
        let line = self.0.chunks[chunk]
            .offsets
            .partition_point(|s| s.bytes <= local)
            .saturating_sub(1);
        (
            self.0.offsets[chunk].lines + line,
            local - self.0.chunks[chunk].offsets[line].bytes,
        )
    }
    pub(super) fn utf16_byte(&self, offset: usize) -> usize {
        let offset = offset.min(self.utf16_len());
        let chunk = self
            .0
            .offsets
            .partition_point(|s| s.utf16 <= offset)
            .saturating_sub(1);
        let local = offset - self.0.offsets[chunk].utf16;
        let line = self.0.chunks[chunk]
            .offsets
            .partition_point(|s| s.utf16 <= local)
            .saturating_sub(1);
        let units = local - self.0.chunks[chunk].offsets[line].utf16;
        let point = self.0.chunks[chunk].metadata[line]
            .positions
            .get()
            .map(|a| a.for_utf16(units));
        let byte = point.map_or(0, |p| p.byte);
        self.0.offsets[chunk].bytes
            + self.0.chunks[chunk].offsets[line].bytes
            + byte
            + super::surface::utf16_byte(
                &self.0.chunks[chunk].lines[line][byte..],
                units - point.map_or(0, |p| p.utf16),
            )
    }

    pub(super) fn byte_utf16(&self, byte: usize) -> usize {
        let (line, mut column) = self.byte_line(byte);
        let text = &self[line];
        while !text.is_char_boundary(column) {
            column += 1;
        }
        let point = self
            .metadata(line)
            .positions
            .get()
            .map(|a| a.for_byte(column));
        self.line_offset(line).utf16
            + point.map_or(0, |p| p.utf16)
            + text[point.map_or(0, |p| p.byte)..column]
                .encode_utf16()
                .count()
    }
    pub(super) fn splice(
        &mut self,
        range: std::ops::RangeInclusive<usize>,
        inserted: Vec<SharedString>,
        positions: Vec<Option<Arc<LinePositions>>>,
    ) {
        let (first, first_line) = self.locate_line(*range.start());
        let (last, last_line) = self.locate_line(*range.end());
        let mut metadata = self.0.chunks[first].metadata[..first_line].to_vec();
        metadata.extend(
            inserted
                .iter()
                .zip(positions)
                .map(|(line, positions)| Arc::new(LineMetadata::with_positions(line, positions))),
        );
        metadata.extend_from_slice(&self.0.chunks[last].metadata[last_line + 1..]);
        let mut replacement = self.0.chunks[first].lines[..first_line].to_vec();
        replacement.extend(inserted);
        replacement.extend_from_slice(&self.0.chunks[last].lines[last_line + 1..]);
        let mut chunks = self.0.chunks[..first].to_vec();
        chunks.extend(
            replacement
                .chunks(CHUNK_LINES)
                .zip(metadata.chunks(CHUNK_LINES))
                .map(|(lines, metadata)| {
                    Arc::new(Chunk::with_metadata(lines.to_vec(), metadata.to_vec()))
                }),
        );
        chunks.extend_from_slice(&self.0.chunks[last + 1..]);
        *self = Self::from_chunks(chunks);
        // Amortize fragmentation after repeated line insertions/deletions.
        if self.0.chunks.len() > self.len().div_ceil(CHUNK_LINES) * 2 + 2 {
            let lines = self.iter().cloned().collect::<Vec<_>>();
            let metadata = self
                .0
                .chunks
                .iter()
                .flat_map(|c| c.metadata.iter().cloned())
                .collect::<Vec<_>>();
            *self = Self::from_chunks(
                lines
                    .chunks(CHUNK_LINES)
                    .zip(metadata.chunks(CHUNK_LINES))
                    .map(|(lines, metadata)| {
                        Arc::new(Chunk::with_metadata(lines.to_vec(), metadata.to_vec()))
                    })
                    .collect(),
            );
        }
    }
}

impl EditorBuffer {
    pub(super) fn native_range(&self, range: Range<usize>) -> Range<usize> {
        let start = self.lines.utf16_byte(range.start);
        start..self.lines.utf16_byte(range.end).max(start)
    }
    pub(super) fn utf16_range(&self, range: Range<usize>) -> Range<usize> {
        self.lines.byte_utf16(range.start)..self.lines.byte_utf16(range.end)
    }
    pub(super) fn byte_text(&self, range: Range<usize>) -> String {
        let (start, first) = self.lines.byte_line(range.start);
        let (end, last) = self.lines.byte_line(range.end);
        if start == end {
            return self.lines[start][first..last].to_owned();
        }
        let mut text = self.lines[start][first..].to_owned();
        for line in start + 1..end {
            text.push('\n');
            text.push_str(&self.lines[line]);
        }
        text.push('\n');
        text.push_str(&self.lines[end][..last]);
        text
    }
    pub(super) fn replace_bytes(&mut self, range: Range<usize>, replacement: &str) {
        let (start, first) = self.lines.byte_line(range.start);
        let (end, last) = self.lines.byte_line(range.end);
        if start == end && self.lines[start][first..last] == *replacement {
            return;
        }
        if let Some(replacements) = &mut self.replacements {
            replacements.push((range.clone(), replacement.len()));
        }
        let first_positions = self.lines.metadata(start).positions.get();
        let last_positions = self.lines.metadata(end).positions.get();
        let prefix = &self.lines[start][..first];
        let suffix = &self.lines[end][last..];
        let mut parts = replacement.split('\n').peekable();
        let mut inserted = Vec::new();
        let mut text = String::new();
        text.push_str(prefix);
        while let Some(part) = parts.next() {
            text.push_str(part);
            if parts.peek().is_none() {
                text.push_str(suffix);
            }
            inserted.push(SharedString::from(std::mem::take(&mut text)));
        }
        let mut positions = vec![None; inserted.len()];
        if inserted.len() == 1 {
            if let (Some(prefix), Some(suffix)) = (first_positions, last_positions) {
                positions[0] = Some(Arc::new(prefix.spliced(
                    &inserted[0],
                    first,
                    suffix,
                    last,
                    replacement.len(),
                )));
            }
        } else {
            if let Some(prefix) = first_positions {
                positions[0] = Some(Arc::new(prefix.spliced(
                    &inserted[0],
                    first,
                    prefix,
                    self.lines[start].len(),
                    inserted[0].len() - first,
                )));
            }
            let tail = inserted.len() - 1;
            if let Some(suffix) = last_positions {
                positions[tail] = Some(Arc::new(suffix.spliced(
                    &inserted[tail],
                    0,
                    suffix,
                    last,
                    inserted[tail].len() - (self.lines[end].len() - last),
                )));
            }
        }
        self.lines.splice(start..=end, inserted, positions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_positions(buffer: &EditorBuffer) {
        for (line, text) in buffer.iter_lines().enumerate() {
            let expected = text
                .grapheme_indices(true)
                .map(|(byte, _)| byte)
                .chain(std::iter::once(text.len()))
                .collect::<Vec<_>>();
            assert_eq!(buffer.line_len(line), expected.len() - 1);
            for (column, byte) in expected.into_iter().enumerate() {
                assert_eq!(buffer.lines.column_byte(line, column), byte);
                assert_eq!(buffer.lines.byte_column(line, byte), column);
            }
            let positions = buffer.lines.positions(line);
            for pair in positions.points.windows(2) {
                assert!(pair[0].byte < pair[1].byte);
                assert!(pair[1].column - pair[0].column <= crate::analysis::BLOCK_COLUMNS);
            }
        }
        let text = buffer.text();
        let mut units = 0;
        for (byte, ch) in text.char_indices() {
            assert_eq!(buffer.lines.byte_utf16(byte), units);
            assert_eq!(buffer.lines.utf16_byte(units), byte);
            units += ch.len_utf16();
        }
        assert_eq!(buffer.lines.utf16_len(), units);
    }

    #[test]
    fn prepared_difference_cache_preserves_equality_after_text_is_restored() {
        let a = EditorBuffer::from_text("日本🙂a".repeat(20_000));
        let mut b = a.clone();
        let end = b.line_len(0);
        b.replace_range(
            EditorPosition::new(0, end - 1)..EditorPosition::new(0, end),
            "b",
        );
        assert!(b.lines.compare_and_remember_difference(&a.lines));
        assert!(b.lines.knows_difference(&a.lines));
        assert_ne!(a, b);
        let mut c = b.clone();
        c.replace_range(
            EditorPosition::new(0, end - 1)..EditorPosition::new(0, end),
            "a",
        );
        assert!(c.lines.compare_and_remember_difference(&b.lines));
        assert!(!c.lines.compare_and_remember_difference(&a.lines));
        assert_eq!(a, c);
        drop(b);
        assert_eq!(a, c);
    }

    #[test]
    fn incremental_positions_match_unicode_oracle_across_scalar_edits() {
        let mut buffer =
            EditorBuffer::from_text("a日本🙂e\u{301}👩‍💻🇺🇸\u{600}क्\u{200d}क ".repeat(80));
        let mut expected = buffer.text();
        let mut seed = 31usize;
        let inserts = [
            "\u{301}",
            "🇬",
            "\u{200d}",
            "",
            "日本\n🇺",
            "\n",
            "abc",
            "\u{600}",
            "क्",
        ];
        for iteration in 0..180 {
            check_positions(&buffer);
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let boundaries = expected
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(expected.len()))
                .collect::<Vec<_>>();
            let index = seed % boundaries.len();
            let start = boundaries[index];
            let end = boundaries[(index + iteration % 5).min(boundaries.len() - 1)];
            let replacement = inserts[iteration % inserts.len()];
            buffer.replace_bytes(start..end, replacement);
            expected.replace_range(start..end, replacement);
            assert_eq!(buffer.text(), expected);
        }
        check_positions(&buffer);
    }

    #[test]
    fn split_join_and_regional_indicator_changes_reuse_correct_suffix_positions() {
        for text in ["a🙂".repeat(500), "🇺".repeat(1000), "a\u{301}".repeat(1000)] {
            let mut buffer = EditorBuffer::from_text(&text);
            check_positions(&buffer);
            let split = text.char_indices().nth(127).expect("split").0;
            buffer.replace_bytes(split..split, "X\n\u{301}");
            assert!(buffer.lines.metadata(0).positions.get().is_some());
            assert!(buffer.lines.metadata(1).positions.get().is_some());
            check_positions(&buffer);
            buffer.replace_bytes(split..split + "X\n\u{301}".len(), "🇬");
            assert!(buffer.lines.metadata(0).positions.get().is_some());
            check_positions(&buffer);
        }
    }

    #[test]
    fn changed_chunks_preserve_neighbor_analysis_and_history_indexes() {
        let buffer =
            EditorBuffer::from_text(format!("{}\nsmall\n{}", "🙂word ".repeat(2000), "tail"));
        let analysis = buffer
            .lines
            .analyze(0, &AtomicBool::new(false))
            .expect("analysis");
        let mut session = EditorSession::new(buffer.clone());
        session.set_selections(vec![EditorSelection::cursor(EditorPosition::new(1, 2))]);
        session.insert("text\nmore");
        assert!(Arc::ptr_eq(
            session
                .buffer()
                .lines
                .cached_analysis(0)
                .expect("preserved"),
            &analysis
        ));
        assert!(session.undo());
        assert!(session.buffer().lines.same_snapshot(&buffer.lines));
        assert!(session.redo());
        assert!(Arc::ptr_eq(
            session.buffer().lines.cached_analysis(0).expect("restored"),
            &analysis
        ));
        let mut edited = buffer.clone();
        edited.insert(EditorPosition::new(0, 130), "x");
        assert!(edited.lines.metadata(0).positions.get().is_some());
        assert!(edited.lines.cached_analysis(0).is_none());
        assert!(buffer.lines.cached_analysis(0).is_some());
        check_positions(&edited);
    }
    #[test]
    fn snapshots_share_unchanged_chunks_and_keep_contiguous_api_compatible() {
        let mut buffer = EditorBuffer::from_text("hello\n".repeat(100_000));
        let snapshot = buffer.clone();
        assert!(buffer.lines.same_snapshot(&snapshot.lines));
        buffer.insert(EditorPosition::new(90_000, 2), "🙂");
        assert_eq!(
            snapshot.line(90_000).expect("existing line").as_ref(),
            "hello"
        );
        assert_eq!(
            buffer.line(90_000).expect("existing line").as_ref(),
            "he🙂llo"
        );
        let shared = buffer
            .lines
            .chunks()
            .iter()
            .zip(snapshot.lines.chunks())
            .filter(|(a, b)| Arc::ptr_eq(a, b))
            .count();
        assert_eq!(shared + 1, buffer.lines.chunks().len());
        assert!(buffer.lines.0.flattened.get().is_none());
        assert_eq!(
            buffer.lines(),
            buffer.iter_lines().cloned().collect::<Vec<_>>()
        );
    }
    #[test]
    fn indexed_native_offsets_match_flat_text_after_splices() {
        let mut buffer = EditorBuffer::from_text("a🙂e\u{301}\n日本\n".repeat(140));
        let mut expected = buffer.text();
        let mut seed = 7usize;
        for iteration in 0..180 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let boundaries = expected
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(expected.len()))
                .collect::<Vec<_>>();
            let start = boundaries[seed % boundaries.len()];
            let end = boundaries[(seed / 17) % boundaries.len()].max(start);
            let replacement = ["x", "🙂\n日本", "", "e\u{301}", "\n\n"][iteration % 5];
            buffer.replace_bytes(start..end, replacement);
            expected.replace_range(start..end, replacement);
            assert_eq!(buffer.text(), expected);
            assert_eq!(buffer.lines.utf16_len(), expected.encode_utf16().count());
            for unit in 0..=expected.encode_utf16().count() + 1 {
                let byte = super::super::surface::utf16_byte(&expected, unit);
                assert_eq!(buffer.lines.utf16_byte(unit), byte);
                assert_eq!(
                    buffer.lines.byte_utf16(byte),
                    expected[..byte].encode_utf16().count()
                );
            }
            assert_eq!(buffer.byte_text(0..expected.len()), expected);
        }
    }
    #[test]
    fn fragmentation_is_bounded_and_equality_is_independent_of_chunks() {
        let mut buffer = EditorBuffer::from_text("line\n".repeat(500));
        for _ in 0..600 {
            buffer.insert(EditorPosition::new(1, 0), "\n");
        }
        assert!(buffer.lines.chunks().len() <= buffer.line_count().div_ceil(CHUNK_LINES) * 2 + 2);
        assert_eq!(buffer, EditorBuffer::from_text(buffer.text()));
        buffer.replace_range(EditorPosition::new(0, 0)..buffer_end(&buffer), "");
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.text(), "");
    }
}
