//! Immutable per-line syntax and text-position indexes shared by snapshots.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) const BLOCK_COLUMNS: usize = 128;
const PARALLEL_BYTES: usize = 256 * 1024;
#[derive(Clone, Copy, Debug)]
pub(super) struct Checkpoint {
    pub column: usize,
    pub byte: usize,
    pub utf16: usize,
}
#[derive(Debug)]
pub(super) struct LineAnalysis {
    pub tokens: Vec<SyntaxToken>,
    pub positions: std::sync::Arc<LinePositions>,
}
impl std::ops::Deref for LineAnalysis {
    type Target = LinePositions;
    fn deref(&self) -> &Self::Target {
        &self.positions
    }
}

#[derive(Debug)]
pub(super) struct LinePositions {
    pub points: Vec<Checkpoint>,
    pub columns: usize,
}
impl LineAnalysis {
    pub fn build(text: &str, cancelled: &AtomicBool) -> Option<Self> {
        let mut graphemes = Vec::new();
        let mut points = Vec::new();
        let mut utf16 = 0;
        for (column, (byte, grapheme)) in text.grapheme_indices(true).enumerate() {
            if column % BLOCK_COLUMNS == 0 {
                if cancelled.load(Ordering::Relaxed) {
                    return None;
                }
                points.push(Checkpoint {
                    column,
                    byte,
                    utf16,
                });
            }
            graphemes.push((byte, grapheme));
            utf16 += grapheme.encode_utf16().count();
        }
        points.push(Checkpoint {
            column: graphemes.len(),
            byte: text.len(),
            utf16,
        });
        let mut tokens = Vec::new();
        let mut index = 0;
        while index < graphemes.len() {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            let start = index;
            let grapheme = graphemes[index].1;
            let kind;
            if grapheme == "/" && graphemes.get(index + 1).is_some_and(|(_, g)| *g == "/") {
                kind = SyntaxTokenKind::Comment;
                index = graphemes.len();
            } else if grapheme == "\"" {
                kind = SyntaxTokenKind::String;
                index += 1;
                while index < graphemes.len() && graphemes[index].1 != "\"" {
                    if index % BLOCK_COLUMNS == 0 && cancelled.load(Ordering::Relaxed) {
                        return None;
                    }
                    index += 1;
                }
                index = (index + 1).min(graphemes.len());
            } else if grapheme.chars().all(|c| c.is_ascii_digit()) {
                kind = SyntaxTokenKind::Number;
                while index < graphemes.len()
                    && graphemes[index].1.chars().all(|c| c.is_ascii_digit())
                {
                    if index % BLOCK_COLUMNS == 0 && cancelled.load(Ordering::Relaxed) {
                        return None;
                    }
                    index += 1;
                }
            } else if grapheme
                .chars()
                .all(|c| c.is_ascii_alphabetic() || c == '_')
            {
                while index < graphemes.len()
                    && graphemes[index]
                        .1
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    if index % BLOCK_COLUMNS == 0 && cancelled.load(Ordering::Relaxed) {
                        return None;
                    }
                    index += 1;
                }
                let end = graphemes.get(index).map_or(text.len(), |(byte, _)| *byte);
                kind = if is_keyword(&text[graphemes[start].0..end]) {
                    SyntaxTokenKind::Keyword
                } else {
                    SyntaxTokenKind::Plain
                };
            } else {
                kind = SyntaxTokenKind::Plain;
                index += 1;
            }
            tokens.push(SyntaxToken {
                kind,
                start,
                end: index,
            });
        }
        (!cancelled.load(Ordering::Relaxed)).then_some(Self {
            tokens,
            positions: std::sync::Arc::new(LinePositions {
                points,
                columns: graphemes.len(),
            }),
        })
    }
}
impl LinePositions {
    pub fn build(text: &str) -> Self {
        if text.len() >= PARALLEL_BYTES && !text.is_ascii() {
            return Self::build_partitioned(text, Self::worker_count(text.len()));
        }
        Self::build_serial(text)
    }

    fn worker_count(bytes: usize) -> usize {
        static WORKERS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
        let workers = *WORKERS
            .get_or_init(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(4)));
        workers.min(bytes / PARALLEL_BYTES).max(1)
    }

    fn partition_boundaries(text: &str, workers: usize) -> Option<Vec<usize>> {
        use unicode_segmentation::GraphemeCursor;
        let mut boundaries = vec![0];
        for part in 1..workers {
            let byte = text.ceil_char_boundary(text.len() / workers * part);
            // Limit both lookbehind and lookahead. A long regional-indicator run
            // or indivisible cluster must not trigger repeated full-line probes.
            let start = text.floor_char_boundary(byte.saturating_sub(4096));
            let end = text.ceil_char_boundary(byte.saturating_add(4096).min(text.len()));
            let chunk = &text[start..end];
            let mut cursor = GraphemeCursor::new(byte, text.len(), true);
            let boundary = match cursor.is_boundary(chunk, start).ok()? {
                true => byte,
                false => cursor.next_boundary(chunk, start).ok()??,
            };
            if boundary >= text.len() {
                return None;
            }
            if boundary > *boundaries.last().expect("initial boundary") {
                boundaries.push(boundary);
            }
        }
        boundaries.push(text.len());
        Some(boundaries)
    }

    fn build_partitioned(text: &str, workers: usize) -> Self {
        let workers = workers.min(text.len() / PARALLEL_BYTES).max(1);
        if workers == 1 {
            return Self::build_serial(text);
        }
        let Some(boundaries) = Self::partition_boundaries(text, workers) else {
            return Self::build_serial(text);
        };
        // Scoped workers borrow immutable text. Complete the index before exposing
        // the edit, preserving synchronous native input and selection semantics.
        std::thread::scope(|scope| {
            let mut jobs = Vec::new();
            for range in boundaries.windows(2).skip(1) {
                let chunk = &text[range[0]..range[1]];
                let job = std::thread::Builder::new()
                    .name("guic-text-index".into())
                    .spawn_scoped(scope, move || Self::build_serial(chunk));
                jobs.push((chunk, job));
            }
            let mut result = Self::build_serial(&text[..boundaries[1]]);
            for (chunk, job) in jobs {
                let next = match job {
                    Ok(job) => job.join().expect("text position worker panicked"),
                    Err(_) => Self::build_serial(chunk),
                };
                let end = result.points.pop().expect("terminal checkpoint");
                result
                    .points
                    .extend(next.points.into_iter().map(|p| Checkpoint {
                        byte: end.byte + p.byte,
                        column: end.column + p.column,
                        utf16: end.utf16 + p.utf16,
                    }));
                result.columns += next.columns;
            }
            result
        })
    }

    fn build_serial(text: &str) -> Self {
        // Normalized lines contain no CR/LF; every ASCII byte is a grapheme.
        if text.is_ascii() {
            let points = (0..text.len())
                .step_by(BLOCK_COLUMNS)
                .chain(std::iter::once(text.len()))
                .map(|byte| Checkpoint {
                    column: byte,
                    byte,
                    utf16: byte,
                })
                .collect();
            return Self {
                points,
                columns: text.len(),
            };
        }
        let mut points = Vec::new();
        let mut column = 0;
        let mut utf16 = 0;
        for (byte, grapheme) in text.grapheme_indices(true) {
            if column % BLOCK_COLUMNS == 0 {
                points.push(Checkpoint {
                    column,
                    byte,
                    utf16,
                });
            }
            column += 1;
            utf16 += grapheme.encode_utf16().count();
        }
        points.push(Checkpoint {
            column,
            byte: text.len(),
            utf16,
        });
        Self {
            points,
            columns: column,
        }
    }

    /// Reuses the unchanged prefix and suffix, rescanning until grapheme
    /// boundaries converge. An edit can join clusters on either side.
    pub fn spliced(
        &self,
        text: &str,
        prefix_end: usize,
        suffix: &Self,
        suffix_start: usize,
        inserted_bytes: usize,
    ) -> Self {
        if text.is_ascii()
            || (inserted_bytes >= PARALLEL_BYTES
                && text.len() / 2 <= inserted_bytes
                && Self::worker_count(text.len()) > 1)
        {
            return Self::build(text);
        }
        let first = self
            .points
            .partition_point(|p| p.byte < prefix_end)
            .saturating_sub(1);
        let origin = self.points[first];
        let mut points = self.points[..first].to_vec();
        let mut column = origin.column;
        let mut utf16 = origin.utf16;
        let mut target = suffix.points.partition_point(|p| p.byte <= suffix_start);
        let mut last_checkpoint_column = origin.column;
        for (offset, grapheme) in text[origin.byte..].grapheme_indices(true) {
            let byte = origin.byte + offset;
            while target < suffix.points.len()
                && suffix.points[target].byte - suffix_start + prefix_end + inserted_bytes < byte
            {
                target += 1;
            }
            if let Some(join) = suffix.points.get(target)
                && join.byte - suffix_start + prefix_end + inserted_bytes == byte
            {
                points.extend(suffix.points[target..].iter().map(|p| Checkpoint {
                    byte: p.byte - suffix_start + prefix_end + inserted_bytes,
                    column: p.column - join.column + column,
                    utf16: p.utf16 - join.utf16 + utf16,
                }));
                return Self {
                    points,
                    columns: suffix.columns - join.column + column,
                };
            }
            if byte == origin.byte || column - last_checkpoint_column == BLOCK_COLUMNS {
                points.push(Checkpoint {
                    column,
                    byte,
                    utf16,
                });
                last_checkpoint_column = column;
            }
            column += 1;
            utf16 += grapheme.encode_utf16().count();
        }
        points.push(Checkpoint {
            column,
            byte: text.len(),
            utf16,
        });
        Self {
            points,
            columns: column,
        }
    }

    pub fn for_column(&self, column: usize) -> Checkpoint {
        self.points[self
            .points
            .partition_point(|p| p.column <= column)
            .saturating_sub(1)]
    }
    pub fn for_byte(&self, byte: usize) -> Checkpoint {
        self.points[self
            .points
            .partition_point(|p| p.byte <= byte)
            .saturating_sub(1)]
    }
    pub fn for_utf16(&self, utf16: usize) -> Checkpoint {
        self.points[self
            .points
            .partition_point(|p| p.utf16 <= utf16)
            .saturating_sub(1)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_probes_fall_back_when_unicode_context_exceeds_the_budget() {
        let ordinary = "日本語e\u{301}👩‍💻🇺🇸 ".repeat(40_000);
        assert_eq!(
            LinePositions::partition_boundaries(&ordinary, 4)
                .expect("partitions")
                .len(),
            5
        );
        assert!(LinePositions::partition_boundaries(&"🇬".repeat(150_001), 4).is_none());
        assert!(
            LinePositions::partition_boundaries(&format!("x{}end", "\u{301}".repeat(300_000)), 4)
                .is_none()
        );
    }
    #[test]
    fn partitioned_unicode_positions_match_serial_indexes_at_all_checkpoints() {
        for text in [
            "日本語e\u{301}👩‍💻🇺🇸 ".repeat(40_000),
            "🇬".repeat(150_001),
            "\u{600}क्\u{200d}क\u{301} ".repeat(40_000),
            format!("x{}end", "\u{301}".repeat(300_000)),
        ] {
            let serial = LinePositions::build_serial(&text);
            let parallel = LinePositions::build_partitioned(&text, 4);
            assert_eq!(parallel.columns, serial.columns);
            assert_eq!(
                parallel.points.last().expect("end").utf16,
                text.encode_utf16().count()
            );
            for point in &parallel.points {
                let base = serial.for_byte(point.byte);
                assert_eq!(
                    point.column,
                    base.column + text[base.byte..point.byte].graphemes(true).count()
                );
                assert_eq!(
                    point.utf16,
                    base.utf16 + text[base.byte..point.byte].encode_utf16().count()
                );
                let origin = serial.for_column(point.column);
                assert_eq!(
                    origin.byte
                        + grapheme_to_byte(&text[origin.byte..], point.column - origin.column),
                    point.byte
                );
                assert!(text.is_char_boundary(point.byte));
            }
            for pair in parallel.points.windows(2) {
                assert!(pair[0].byte < pair[1].byte);
                assert!(pair[1].column - pair[0].column <= BLOCK_COLUMNS);
            }
        }
    }
    #[test]
    fn checkpoints_match_grapheme_and_utf16_offsets_at_every_boundary() {
        let text = "a日本🙂e\u{301}👩‍💻\t ".repeat(100);
        let buffer = EditorBuffer::from_text(&text);
        let analysis = buffer
            .lines
            .analyze(0, &AtomicBool::new(false))
            .expect("analysis");
        for (column, (byte, _)) in text.grapheme_indices(true).enumerate() {
            assert_eq!(buffer.lines.column_byte(0, column), byte);
            assert_eq!(buffer.lines.byte_column(0, byte), column);
            let units = text[..byte].encode_utf16().count();
            assert_eq!(buffer.lines.utf16_byte(units), byte);
            assert_eq!(buffer.lines.byte_utf16(byte), units);
            assert!(column - analysis.for_column(column).column < BLOCK_COLUMNS);
        }
        assert_eq!(buffer.line_len(0), text.graphemes(true).count());
        assert_eq!(buffer.lines.column_byte(0, usize::MAX), text.len());
    }
    #[test]
    fn cancelled_analysis_never_enters_the_shared_cache() {
        let buffer = EditorBuffer::from_text("word ".repeat(20_000));
        let cancelled = AtomicBool::new(true);
        assert!(buffer.lines.analyze(0, &cancelled).is_none());
        assert!(buffer.lines.cached_analysis(0).is_none());
        cancelled.store(false, Ordering::Relaxed);
        assert!(buffer.lines.analyze(0, &cancelled).is_some());
    }
}
