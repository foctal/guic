//! Rectangular selection over shaped display rows, including wrapped lines.

use super::*;
use std::{cell::RefCell, collections::BTreeMap};

#[derive(Clone)]
pub(super) struct RectangleAnchor {
    byte: usize,
    x: Pixels,
    upstream: bool,
    cache: Rc<RefCell<Option<RectangleCache>>>,
}

struct RectangleCache {
    source: LineStore,
    font: Font,
    width: Pixels,
    soft_wrap: bool,
    provisional: bool,
    x: Pixels,
    rows: BTreeMap<usize, (EditorSelection, bool)>,
}

impl EditorSurface {
    pub(super) fn begin_rectangle(
        &mut self,
        position: EditorPosition,
        pointer: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(viewport) = &self.layout else {
            return;
        };
        self.rectangle_anchor = Some(RectangleAnchor {
            byte: position_byte(&self.edit.buffer, position),
            // Preserve the pointer's horizontal position even beyond a short row.
            x: (pointer.x - viewport.body.left() + self.scroll.x).max(px(0.0)),
            upstream: self.upstream,
            cache: Rc::default(),
        });
        self.selecting = true;
        self.preferred_x = None;
        self.extend_rectangle(pointer, window, cx);
    }

    pub(super) fn extend_rectangle(
        &mut self,
        pointer: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(anchor), Some(viewport)) = (self.rectangle_anchor.clone(), self.layout.clone())
        else {
            return;
        };
        self.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
        let position = byte_position(&self.edit.buffer, anchor.byte);
        let map = self.wrap.as_ref().expect("prepared wrap map");
        let mut first = map.row_for_position(position);
        if anchor.upstream
            && first > 0
            && map.fragment(first).1.columns.start == position.column
            && map.fragment(first - 1).0 == position.line
        {
            first -= 1;
        }
        let y = f32::from(pointer.y - viewport.body.top() + self.scroll.y).max(0.0);
        let last = ((y / LINE_HEIGHT).floor() as usize).min(map.row_count() - 1);
        let x = (pointer.x - viewport.body.left() + self.scroll.x).max(px(0.0));
        let mut cached = anchor.cache.borrow_mut();
        if cached.as_ref().is_none_or(|cache| {
            !cache.source.same_snapshot(&self.edit.buffer.lines)
                || cache.font != viewport.font
                || cache.width != viewport.body.size.width
                || cache.soft_wrap != self.props.options.soft_wrap
                || cache.provisional != map.is_provisional()
                || cache.x != x
        }) {
            *cached = Some(RectangleCache {
                source: self.edit.buffer.lines.clone(),
                font: viewport.font.clone(),
                width: viewport.body.size.width,
                soft_wrap: self.props.options.soft_wrap,
                provisional: map.is_provisional(),
                x,
                rows: BTreeMap::new(),
            });
        }
        let cache = cached.as_mut().expect("rectangle geometry");
        let mut selections = Vec::with_capacity(first.abs_diff(last) + 1);
        let mut heads = Vec::with_capacity(selections.capacity());
        for visual in first.min(last)..=first.max(last) {
            // A stationary horizontal span only shapes newly visited rows as
            // autoscrolling grows the block. Resize/reflow/edit invalidates it.
            let (selection, upstream) = cache.rows.entry(visual).or_insert_with(|| {
                let left = self.shape_x(visual, anchor.x, &viewport.font, window, cx);
                let right = if x == anchor.x {
                    left.clone()
                } else {
                    self.shape_x(visual, x, &viewport.font, window, cx)
                };
                let head = EditorPosition::new(right.index, right.column_for_x(x));
                let upstream = head.column == map.fragment(visual).1.columns.end;
                (
                    EditorSelection::new(
                        EditorPosition::new(left.index, left.column_for_x(anchor.x)),
                        head,
                    ),
                    upstream,
                )
            });
            selections.push(selection.clone());
            heads.push((position_byte(&self.edit.buffer, selection.head), *upstream));
        }
        drop(cached);
        self.select_pointers(selections, window, cx);
        self.rectangle_heads = heads;
        // Drag scrolling follows the pointer, not the first selected row.
        self.reveal = false;
        self.reveal_after_reflow = false;
        if let Some(pending) = &mut self.pending_edit {
            pending.reveal = false;
        }
        cx.notify();
    }

    pub(super) fn cursor_visual_row(&self, index: usize, position: EditorPosition) -> usize {
        let upstream = self
            .rectangle_heads
            .get(index)
            .filter(|(byte, _)| *byte == position_byte(&self.edit.buffer, position))
            .map_or(index == 0 && self.upstream, |(_, upstream)| *upstream);
        let map = self.wrap.as_ref().expect("prepared wrap map");
        let row = map.row_for_position(position);
        if upstream
            && row > 0
            && map.fragment(row).1.columns.start == position.column
            && map.fragment(row - 1).0 == position.line
        {
            row - 1
        } else {
            row
        }
    }

    pub(super) fn rebase_rectangle_anchor(&mut self, range: &Range<usize>, length: usize) {
        for (head, _) in &mut self.rectangle_heads {
            if *head >= range.end {
                *head = *head - range.len() + length;
            } else if *head > range.start {
                *head = range.start + length;
            }
        }
        if let Some(anchor) = &mut self.rectangle_anchor {
            if anchor.byte >= range.end {
                anchor.byte = anchor.byte - range.len() + length;
            } else if anchor.byte > range.start {
                anchor.byte = range.start + length;
            }
        }
    }
}
