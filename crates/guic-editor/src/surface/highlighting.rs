//! Retained syntax work for the current language and buffer.
use super::*;
use crate::grammar::HighlightSpans;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

pub(super) struct HighlightJob {
    pub(super) source: EditorBuffer,
    grammar: EditorGrammar,
    cancel: Arc<AtomicUsize>,
    pub(super) spans: Option<HighlightSpans>,
    _task: gpui::Task<()>,
}

impl Drop for HighlightJob {
    fn drop(&mut self) {
        self.cancel.store(1, Ordering::Relaxed);
    }
}

impl EditorSurface {
    pub(super) fn update_highlighting(&mut self, cx: &mut Context<Self>) {
        let Some(grammar) = self
            .props
            .grammar
            .as_ref()
            .filter(|_| self.props.options.syntax_highlighting)
        else {
            self.highlighting = None;
            return;
        };
        if self.highlighting.as_ref().is_some_and(|job| {
            job.source.lines.same_snapshot(&self.edit.buffer.lines)
                && Arc::ptr_eq(&grammar.config, &job.grammar.config)
        }) {
            return;
        }
        self.highlighting = None;
        let source = self.edit.buffer.clone();
        let cancel = Arc::new(AtomicUsize::new(0));
        let worker = cx.background_executor().spawn({
            let source = source.clone();
            let grammar = grammar.clone();
            let cancel = cancel.clone();
            async move { grammar.highlight(&source, &cancel) }
        });
        let identity = cancel.clone();
        let task = cx.spawn(async move |entity, cx| {
            if let Some(spans) = worker.await {
                let _ = entity.update(cx, |state, cx| {
                    if let Some(job) = &mut state.highlighting
                        && Arc::ptr_eq(&job.cancel, &identity)
                        && job.source.lines.same_snapshot(&state.edit.buffer.lines)
                    {
                        job.spans = Some(spans);
                        cx.notify();
                    }
                });
            }
        });
        self.highlighting = Some(HighlightJob {
            source,
            grammar: grammar.clone(),
            cancel,
            spans: None,
            _task: task,
        });
    }
}
