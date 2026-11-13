//! Cancellable background reflow with snapshot validation and viewport anchoring.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct PendingReflow {
    source: EditorBuffer,
    font: Font,
    width: Option<Pixels>,
    pub(super) cancelled: Arc<AtomicBool>,
    _task: gpui::Task<()>,
}
impl Drop for PendingReflow {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
impl EditorSurface {
    pub(super) fn request_reflow(
        &mut self,
        font: &Font,
        width: Option<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .wrap
            .as_ref()
            .is_some_and(|map| map.matches(&self.edit.buffer, font, width))
        {
            self.reflow = None;
            return;
        }
        if self.reflow.as_ref().is_some_and(|job| {
            job.source.lines.same_snapshot(&self.edit.buffer.lines)
                && job.font == *font
                && job.width == width
        }) {
            return;
        }
        self.reflow = None;
        self.reflow_generation = self.reflow_generation.wrapping_add(1);
        let seed = self
            .wrap
            .clone()
            .unwrap_or_else(|| WrapMap::provisional(&self.edit.buffer, font.clone(), width));
        if seed.can_update_inline(&self.edit.buffer, font, width) {
            let mut map = seed;
            map.update(&self.edit.buffer, font.clone(), width, window.text_system());
            let old = self.wrap.replace(map);
            Self::discard_layout(old, cx);
            self.reveal |= std::mem::take(&mut self.reveal_after_reflow);
            return;
        }
        // Never use geometry from an older document for native input or hit testing.
        if !seed.same_source(&self.edit.buffer) || self.wrap.is_none() {
            let old =
                self.wrap
                    .replace(WrapMap::provisional(&self.edit.buffer, font.clone(), width));
            Self::discard_layout(old, cx);
        }
        let source = self.edit.buffer.clone();
        let worker_source = source.clone();
        let font = font.clone();
        let worker_font = font.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let text_system = cx.text_system().clone();
        let generation = self.reflow_generation;
        let worker = cx.background_executor().spawn(async move {
            seed.background(
                &worker_source,
                worker_font,
                width,
                text_system,
                &worker_cancelled,
            )
        });
        let task = cx.spawn(async move |entity, cx| {
            if let Some(map) = worker.await {
                let _ = entity.update(cx, |state, cx| {
                    state.finish_reflow(generation, map, cx);
                });
            }
        });
        self.reflow = Some(PendingReflow {
            source,
            font,
            width,
            cancelled,
            _task: task,
        });
    }

    fn discard_layout(map: Option<WrapMap>, cx: &Context<Self>) {
        if let Some(map) = map {
            // Releasing hundreds of thousands of old fragments can itself stall a frame.
            cx.background_executor()
                .spawn(async move {
                    drop(map);
                })
                .detach();
        }
    }

    pub(super) fn finish_reflow(&mut self, generation: u64, map: WrapMap, cx: &mut Context<Self>) {
        if generation != self.reflow_generation
            || !map.same_source(&self.edit.buffer)
            || self.reflow.as_ref().is_none_or(|job| {
                !map.matches(&job.source, &job.font, job.width)
                    || job.width.is_some() != self.props.options.soft_wrap
            })
        {
            Self::discard_layout(Some(map), cx);
            return;
        }
        // Read the anchor now: the user may have scrolled while the worker ran.
        let row = (f32::from(self.scroll.y) / LINE_HEIGHT).floor() as usize;
        let fraction = f32::from(self.scroll.y) % LINE_HEIGHT;
        let anchor = self.wrap.as_ref().map(|old| {
            let (line, fragment) = old.fragment(row);
            EditorPosition::new(line, fragment.columns.start)
        });
        if let Some(anchor) = anchor {
            self.scroll.y = px(map.row_for_position(anchor) as f32 * LINE_HEIGHT + fraction);
        }
        self.max_width = map.max_width;
        let old = self.wrap.replace(map);
        Self::discard_layout(old, cx);
        self.reflow = None;
        // Preserve a pending caret reveal after input; scrolling alone does not reveal it.
        self.reveal |= std::mem::take(&mut self.reveal_after_reflow);
        cx.notify();
    }
}
