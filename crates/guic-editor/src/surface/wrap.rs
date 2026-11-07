//! Incremental logical-line to visual-row mapping for the monospace editor.

use super::*;
use crate::storage::Chunk;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug)]
pub(super) struct Fragment {
    pub bytes: Range<usize>,
    pub columns: Range<usize>,
    pub tab_column: usize,
}
#[derive(Clone, Debug)]
pub(super) struct Segment {
    pub fragment: Fragment,
    pub x: Pixels,
    pub width: Pixels,
}
struct Line {
    segments: Vec<Segment>,
    fragments: Vec<Fragment>,
    width: Pixels,
}
struct LayoutChunk {
    source: Arc<Chunk>,
    lines: Vec<Line>,
    rows: Vec<usize>,
    width: Pixels,
}
#[derive(Clone)]
pub(super) struct WrapMap {
    provisional: bool,
    source: LineStore,
    chunks: Vec<Arc<LayoutChunk>>,
    rows: Vec<usize>,
    lines: Vec<usize>,
    font: Font,
    width: Option<Pixels>,
    advances: HashMap<String, Pixels>,
    pub max_width: Pixels,
    #[cfg(test)]
    pub measured_lines: usize,
}
impl WrapMap {
    pub fn provisional(buffer: &EditorBuffer, font: Font, width: Option<Pixels>) -> Self {
        Self {
            provisional: true,
            source: buffer.lines.clone(),
            chunks: Vec::new(),
            rows: Vec::new(),
            lines: Vec::new(),
            font,
            width,
            advances: HashMap::new(),
            max_width: px(0.0),
            #[cfg(test)]
            measured_lines: 0,
        }
    }
    pub fn is_provisional(&self) -> bool {
        self.provisional
    }
    pub fn same_source(&self, buffer: &EditorBuffer) -> bool {
        self.source.same_snapshot(&buffer.lines)
    }
    pub fn matches(&self, buffer: &EditorBuffer, font: &Font, width: Option<Pixels>) -> bool {
        !self.provisional && self.same_source(buffer) && self.font == *font && self.width == width
    }
    /// Bound synchronous work by both bytes and logical lines, not document size alone.
    pub fn can_update_inline(
        &self,
        buffer: &EditorBuffer,
        font: &Font,
        width: Option<Pixels>,
    ) -> bool {
        let reuse = self.font == *font && self.width == width && !self.provisional;
        let old: std::collections::HashSet<_> = if reuse {
            self.chunks.iter().map(|c| Arc::as_ptr(&c.source)).collect()
        } else {
            Default::default()
        };
        let mut bytes = 0;
        let mut lines = 0;
        for chunk in buffer.lines.chunks() {
            if old.contains(&Arc::as_ptr(chunk)) {
                continue;
            }
            bytes += chunk.byte_len();
            lines += chunk.lines.len();
            if bytes > 16_384 || lines > 256 {
                return false;
            }
        }
        true
    }
    pub fn background(
        mut self,
        buffer: &EditorBuffer,
        font: Font,
        width: Option<Pixels>,
        text_system: Arc<gpui::TextSystem>,
        cancelled: &AtomicBool,
    ) -> Option<Self> {
        if self.font != font {
            self.advances.clear();
        }
        if self.font != font || self.width != width {
            self.chunks.clear();
        }
        self.font = font;
        self.width = width;
        let text_system = gpui::WindowTextSystem::new(text_system);
        self.rebuild(buffer, &text_system, cancelled)
            .then_some(self)
    }

    pub fn new(
        buffer: &EditorBuffer,
        font: Font,
        width: Option<Pixels>,
        text_system: &gpui::WindowTextSystem,
    ) -> Self {
        let mut map = Self {
            provisional: false,
            source: buffer.lines.clone(),
            chunks: Vec::new(),
            rows: vec![0],
            lines: vec![0],
            font,
            width,
            advances: HashMap::new(),
            max_width: px(0.0),
            #[cfg(test)]
            measured_lines: 0,
        };
        map.rebuild(buffer, text_system, &AtomicBool::new(false));
        map
    }
    pub fn update(
        &mut self,
        buffer: &EditorBuffer,
        font: Font,
        width: Option<Pixels>,
        text_system: &gpui::WindowTextSystem,
    ) {
        if self.font != font || self.width != width {
            *self = Self::new(buffer, font, width, text_system);
        } else if self.provisional || !self.source.same_snapshot(&buffer.lines) {
            self.rebuild(buffer, text_system, &AtomicBool::new(false));
        }
    }
    fn rebuild(
        &mut self,
        buffer: &EditorBuffer,
        text_system: &gpui::WindowTextSystem,
        cancelled: &AtomicBool,
    ) -> bool {
        let old: HashMap<_, _> = self
            .chunks
            .drain(..)
            .map(|chunk| (Arc::as_ptr(&chunk.source), chunk))
            .collect();
        self.rows = vec![0];
        self.lines = vec![0];
        self.max_width = px(0.0);
        for source in buffer.lines.chunks() {
            if cancelled.load(Ordering::Relaxed) {
                return false;
            }
            let chunk = old.get(&Arc::as_ptr(source)).cloned().unwrap_or_else(|| {
                let mut rows = vec![0];
                let mut width = px(0.0);
                let lines = source
                    .lines
                    .iter()
                    .take_while(|_| !cancelled.load(Ordering::Relaxed))
                    .enumerate()
                    .map(|(index, text)| {
                        let _ = source.analyze(index, cancelled);
                        let line = self.measure(text, text_system, cancelled);
                        rows.push(rows.last().expect("nonempty row index") + line.fragments.len());
                        width = width.max(line.width);
                        line
                    })
                    .collect();
                Arc::new(LayoutChunk {
                    source: source.clone(),
                    lines,
                    rows,
                    width,
                })
            });
            self.rows.push(
                self.rows.last().expect("nonempty row index")
                    + chunk.rows.last().expect("nonempty row index"),
            );
            self.lines
                .push(self.lines.last().expect("nonempty row index") + chunk.lines.len());
            self.max_width = self.max_width.max(chunk.width);
            self.chunks.push(chunk);
        }
        self.source = buffer.lines.clone();
        self.provisional = false;
        !cancelled.load(Ordering::Relaxed)
    }
    fn advance(&mut self, text: &str, text_system: &gpui::WindowTextSystem) -> Pixels {
        if let Some(width) = self.advances.get(text) {
            return *width;
        }
        let width = text_system
            .shape_line(
                text.to_owned().into(),
                px(FONT_SIZE),
                &[TextRun {
                    len: text.len(),
                    font: self.font.clone(),
                    color: gpui::black(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            )
            .width();
        // Bound retained measurements for documents with many unique clusters.
        if self.advances.len() < 4096 {
            self.advances.insert(text.to_owned(), width);
        }
        width
    }
    fn measure(
        &mut self,
        text: &str,
        text_system: &gpui::WindowTextSystem,
        cancelled: &AtomicBool,
    ) -> Line {
        #[cfg(test)]
        {
            self.measured_lines += 1;
        }
        if self.width.is_none() {
            return self.measure_unwrapped(text, text_system, cancelled);
        }
        let mut fragments = Vec::new();
        let mut start_byte = 0;
        let mut start_column = 0;
        let mut start_tab = 0;
        let mut tab = 0;
        let mut width = px(0.0);
        let mut maximum = px(0.0);
        let mut columns = 0;
        // Candidate carries the width and tab column at the last word boundary.
        let mut candidate: Option<(usize, usize, usize, Pixels)> = None;
        let mut word_ends = unicode_linebreak::linebreaks(text)
            .map(|(i, _)| i)
            .peekable();
        for (byte, grapheme) in text.grapheme_indices(true) {
            if columns % 256 == 0 && cancelled.load(Ordering::Relaxed) {
                break;
            }
            let count = if grapheme == "\t" { 4 - tab % 4 } else { 1 };
            let advance = if grapheme == "\t" {
                self.advance(" ", text_system) * count as f32
            } else {
                self.advance(grapheme, text_system)
            };
            if let Some(limit) = self.width
                && width + advance > limit
                && columns > start_column
            {
                let (end_byte, end_column, end_tab, consumed) = candidate
                    .filter(|(b, _, _, _)| *b > start_byte)
                    .unwrap_or((byte, columns, tab, width));
                fragments.push(Fragment {
                    bytes: start_byte..end_byte,
                    columns: start_column..end_column,
                    tab_column: start_tab,
                });
                maximum = maximum.max(consumed);
                start_byte = end_byte;
                start_column = end_column;
                start_tab = end_tab;
                width -= consumed;
                candidate = None;
                // A word wider than a row is broken at grapheme boundaries.
                if width + advance > limit && columns > start_column {
                    fragments.push(Fragment {
                        bytes: start_byte..byte,
                        columns: start_column..columns,
                        tab_column: start_tab,
                    });
                    maximum = maximum.max(width);
                    start_byte = byte;
                    start_column = columns;
                    start_tab = tab;
                    width = px(0.0);
                }
            }
            width += advance;
            columns += 1;
            tab += count;
            let end = byte + grapheme.len();
            while word_ends.peek().is_some_and(|i| *i <= end) {
                if word_ends.next() == Some(end) {
                    candidate = Some((end, columns, tab, width));
                }
            }
        }
        fragments.push(Fragment {
            bytes: start_byte..text.len(),
            columns: start_column..columns,
            tab_column: start_tab,
        });
        Line {
            segments: Vec::new(),
            fragments,
            width: maximum.max(width),
        }
    }
    fn measure_unwrapped(
        &mut self,
        text: &str,
        system: &gpui::WindowTextSystem,
        cancelled: &AtomicBool,
    ) -> Line {
        if text
            .graphemes(true)
            .take(crate::analysis::BLOCK_COLUMNS + 1)
            .count()
            <= crate::analysis::BLOCK_COLUMNS
        {
            let mut width = px(0.0);
            let mut tab = 0;
            let mut columns = 0;
            for grapheme in text.graphemes(true) {
                let count = if grapheme == "\t" { 4 - tab % 4 } else { 1 };
                width += self.advance(if grapheme == "\t" { " " } else { grapheme }, system)
                    * count as f32;
                tab += count;
                columns += 1;
            }
            let fragment = Fragment {
                bytes: 0..text.len(),
                columns: 0..columns,
                tab_column: 0,
            };
            return Line {
                fragments: vec![fragment.clone()],
                segments: vec![Segment {
                    fragment,
                    x: px(0.0),
                    width,
                }],
                width,
            };
        }
        let mut segments = Vec::new();
        let mut display = String::new();
        let mut start_byte = 0;
        let mut start_column = 0;
        let mut start_tab = 0;
        let mut column = 0;
        let mut tab = 0;
        let mut x = px(0.0);
        let mut push = |end_byte, end_column, end_tab, display: &mut String| {
            let value: SharedString = std::mem::take(display).into();
            let width = system
                .shape_line(
                    value.clone(),
                    px(FONT_SIZE),
                    &[TextRun {
                        len: value.len(),
                        font: self.font.clone(),
                        ..Default::default()
                    }],
                    None,
                )
                .width();
            segments.push(Segment {
                fragment: Fragment {
                    bytes: start_byte..end_byte,
                    columns: start_column..end_column,
                    tab_column: start_tab,
                },
                x,
                width,
            });
            x += width;
            start_byte = end_byte;
            start_column = end_column;
            start_tab = end_tab;
        };
        for (byte, grapheme) in text.grapheme_indices(true) {
            if column % crate::analysis::BLOCK_COLUMNS == 0 && cancelled.load(Ordering::Relaxed) {
                break;
            }
            if grapheme == "\t" {
                let count = 4 - tab % 4;
                display.push_str(&" ".repeat(count));
                tab += count;
            } else {
                display.push_str(grapheme);
                tab += 1;
            }
            column += 1;
            if column % crate::analysis::BLOCK_COLUMNS == 0 {
                push(byte + grapheme.len(), column, tab, &mut display);
            }
        }
        if !display.is_empty() || column == 0 {
            push(text.len(), column, tab, &mut display);
        }
        Line {
            fragments: vec![Fragment {
                bytes: 0..text.len(),
                columns: 0..column,
                tab_column: 0,
            }],
            segments,
            width: x,
        }
    }
    fn segments(&self, row: usize) -> Option<&[Segment]> {
        if self.provisional || self.width.is_some() {
            return None;
        }
        let (chunk, line) = self.locate_line(row.min(self.source.len() - 1));
        Some(&self.chunks[chunk].lines[line].segments)
    }
    pub fn segment_for_column(&self, row: usize, column: usize) -> Segment {
        if let Some(segments) = self.segments(row) {
            let index = segments
                .partition_point(|s| s.fragment.columns.start <= column)
                .saturating_sub(1);
            return segments[index].clone();
        }
        Segment {
            fragment: self.fragment(row).1,
            x: px(0.0),
            width: px(0.0),
        }
    }
    pub fn segment_for_x(&self, row: usize, x: Pixels) -> Segment {
        if let Some(segments) = self.segments(row) {
            let index = segments.partition_point(|s| s.x <= x).saturating_sub(1);
            return segments[index].clone();
        }
        self.segment_for_column(row, 0)
    }
    pub fn visible_segments(&self, row: usize, range: Range<Pixels>) -> Vec<Segment> {
        if let Some(segments) = self.segments(row) {
            let first = segments
                .partition_point(|s| s.x + s.width < range.start)
                .min(segments.len() - 1);
            let last = segments
                .partition_point(|s| s.x <= range.end)
                .max(first + 1);
            return segments[first..last].to_vec();
        }
        vec![self.segment_for_column(row, 0)]
    }
    pub fn row_count(&self) -> usize {
        if self.provisional {
            return self.source.len();
        }
        *self.rows.last().expect("nonempty row index")
    }
    fn locate_line(&self, line: usize) -> (usize, usize) {
        let chunk = self
            .lines
            .partition_point(|i| *i <= line)
            .saturating_sub(1)
            .min(self.chunks.len() - 1);
        (chunk, line - self.lines[chunk])
    }
    pub fn line_width(&self, line: usize) -> Pixels {
        if self.provisional {
            return px(0.0);
        }
        let (chunk, local) = self.locate_line(line);
        self.chunks[chunk].lines[local].width
    }

    pub fn first_row(&self, line: usize) -> usize {
        if self.provisional {
            return line.min(self.source.len() - 1);
        }
        let (chunk, local) = self.locate_line(line);
        self.rows[chunk] + self.chunks[chunk].rows[local]
    }
    pub fn row_for_position(&self, position: EditorPosition) -> usize {
        if self.provisional {
            return position.line.min(self.source.len() - 1);
        }
        let (chunk, line) = self.locate_line(position.line);
        let fragments = &self.chunks[chunk].lines[line].fragments;
        self.rows[chunk]
            + self.chunks[chunk].rows[line]
            + fragments
                .partition_point(|f| f.columns.start <= position.column)
                .saturating_sub(1)
    }
    pub fn fragment(&self, row: usize) -> (usize, Fragment) {
        if self.provisional {
            let line = row.min(self.source.len() - 1);
            let text = &self.source[line];
            // Bounded first-frame preview, including a pathological single long line.
            let mut end = 0;
            let mut columns = 0;
            let mut limit = text.len().min(4096);
            while !text.is_char_boundary(limit) {
                limit -= 1;
            }
            for (byte, grapheme) in text[..limit].grapheme_indices(true).take(256) {
                let next = byte + grapheme.len();
                // The last cluster of a byte-limited prefix may continue beyond it.
                if next == limit && limit < text.len() {
                    break;
                }
                end = next;
                columns += 1;
            }
            return (
                line,
                Fragment {
                    bytes: 0..end,
                    columns: 0..columns,
                    tab_column: 0,
                },
            );
        }
        let row = row.min(self.row_count() - 1);
        let chunk = self.rows.partition_point(|i| *i <= row).saturating_sub(1);
        let local = row - self.rows[chunk];
        let line = self.chunks[chunk]
            .rows
            .partition_point(|i| *i <= local)
            .saturating_sub(1);
        (
            self.lines[chunk] + line,
            self.chunks[chunk].lines[line].fragments[local - self.chunks[chunk].rows[line]].clone(),
        )
    }
}
