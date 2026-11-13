//! Ordered clipboard reads and an independent native composition context.
use super::*;
use std::collections::BTreeMap;

pub(super) enum PendingAction {
    Left,
    Right,
    Up,
    Down,
    SelectUp,
    SelectDown,
    SelectLeft,
    SelectRight,
    SelectAll,
    Home,
    End,
    Newline,
    Backspace,
    Delete,
    Copy,
    Cut,
    Undo,
    Redo,
}

enum Work {
    Paste {
        read: gpui::Task<Option<String>>,
        copied: usize,
    },
    Insert(String),
    Action(PendingAction, usize),
    Select(Range<usize>, bool),
}

#[derive(Default)]
pub(super) struct PendingInput {
    queue: VecDeque<Work>,
    pub text: String,
    pub selected: Range<usize>,
    pub marked: Option<Range<usize>>,
    pub pointer: Option<(Range<usize>, bool)>,
    pub replaying: bool,
    waiting: bool,
    sequence: usize,
    clipboards: BTreeMap<usize, String>,
    task: Option<gpui::Task<()>>,
}

impl PendingInput {
    pub fn units(&self, range: &Range<usize>) -> Range<usize> {
        self.text[..range.start].encode_utf16().count()
            ..self.text[..range.end].encode_utf16().count()
    }
    pub fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        marked: bool,
    ) {
        let range = range
            .map(|range| utf16_range(&self.text, &range))
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone());
        self.text.replace_range(range.clone(), text);
        self.selected = selected
            .map(|selected_range| {
                let range_in_text = utf16_range(text, &selected_range);
                range.start + range_in_text.start..range.start + range_in_text.end
            })
            .unwrap_or(range.start + text.len()..range.start + text.len());
        self.marked = (marked && !text.is_empty()).then_some(range.start..range.start + text.len());
    }
    pub fn rebase(&mut self, replaced: &Range<usize>, inserted: usize) {
        let rebase = |offset: &mut usize| {
            if *offset >= replaced.end {
                *offset = *offset - replaced.len() + inserted;
            } else if *offset > replaced.start {
                *offset = replaced.start + inserted;
            }
        };
        if let Some((range, _)) = &mut self.pointer {
            rebase(&mut range.start);
            rebase(&mut range.end);
        }
        for work in &mut self.queue {
            if let Work::Select(range, _) = work {
                rebase(&mut range.start);
                rebase(&mut range.end);
            }
        }
    }
}

impl TextInput {
    pub(super) fn cancel_pending_input(&mut self) {
        self.input_generation = self.input_generation.wrapping_add(1);
        self.pending = None;
    }
    pub(super) fn commit_pending_text(&mut self) {
        if let Some(pending) = &mut self.pending {
            let text = std::mem::take(&mut pending.text);
            pending.selected = 0..0;
            pending.marked = None;
            if !text.is_empty() {
                pending.queue.push_back(Work::Insert(text));
            }
        }
    }
    pub(super) fn queue_paste(
        &mut self,
        read: gpui::Task<Option<String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_composition();
        self.pending.get_or_insert_with(PendingInput::default);
        self.commit_pending_text();
        let pending = self.pending.as_mut().expect("pending clipboard read");
        pending.queue.push_back(Work::Paste {
            read,
            copied: pending.sequence,
        });
        self.drain_pending_input(window, cx);
        cx.notify();
    }
    pub(super) fn defer_input_action(
        &mut self,
        action: PendingAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self
            .pending
            .as_ref()
            .is_none_or(|pending| pending.replaying)
        {
            return false;
        }
        self.commit_pending_text();
        let pending = self.pending.as_mut().expect("pending input");
        if matches!(action, PendingAction::Copy | PendingAction::Cut) {
            pending.sequence += 1;
        }
        pending
            .queue
            .push_back(Work::Action(action, pending.sequence));
        self.drain_pending_input(window, cx);
        true
    }
    pub(super) fn queue_pointer(
        &mut self,
        range: Range<usize>,
        reversed: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self
            .pending
            .as_ref()
            .is_none_or(|pending| pending.replaying)
        {
            return false;
        }
        self.commit_pending_text();
        let pending = self.pending.as_mut().expect("pending input");
        pending.pointer = Some((range.clone(), reversed));
        if matches!(pending.queue.back(), Some(Work::Select(..))) {
            pending.queue.pop_back();
        }
        pending.queue.push_back(Work::Select(range, reversed));
        cx.notify();
        true
    }
    fn next_pending_frame(&self, window: &mut Window, cx: &mut Context<Self>) {
        let entity = cx.entity().downgrade();
        let generation = self.input_generation;
        window.on_next_frame(move |window, cx| {
            let _ = entity.update(cx, |input, cx| {
                if input.input_generation == generation {
                    input.drain_pending_input(window, cx);
                }
            });
        });
        window.refresh();
    }
    pub(super) fn drain_pending_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pending) = &mut self.pending else {
            return;
        };
        if pending.waiting || pending.replaying {
            return;
        }
        let Some(work) = pending.queue.pop_front() else {
            if pending.marked.is_none() && pending.text.is_empty() {
                self.pending = None;
            }
            cx.notify();
            return;
        };
        if let Work::Paste { read, copied } = work {
            let copied_text = pending
                .clipboards
                .range(..=copied)
                .next_back()
                .map(|(_, text)| text.clone());
            pending.waiting = true;
            let generation = self.input_generation;
            pending.task = Some(cx.spawn_in(window, async move |entity, cx| {
                let text = if let Some(text) = copied_text {
                    Some(text)
                } else {
                    read.await
                };
                let _ = entity.update_in(cx, |input, window, cx| {
                    if input.input_generation != generation || input.disabled {
                        return;
                    }
                    let Some(pending) = &mut input.pending else {
                        return;
                    };
                    pending.waiting = false;
                    pending.replaying = true;
                    if let Some(text) = text.filter(|text| !text.is_empty()) {
                        input.replace_text_in_range(None, &text, window, cx);
                    }
                    if let Some(pending) = &mut input.pending {
                        pending.replaying = false;
                    }
                    input.next_pending_frame(window, cx);
                    cx.notify();
                });
            }));
            return;
        }
        pending.replaying = true;
        match work {
            Work::Insert(text) => {
                self.replace_text_in_range(None, &text, window, cx);
            }
            Work::Select(range, reversed) => {
                self.selected_range = self
                    .value
                    .floor_char_boundary(range.start.min(self.value.len()))
                    ..self
                        .value
                        .floor_char_boundary(range.end.min(self.value.len()));
                self.selection_reversed = reversed;
                if let Some(pending) = &mut self.pending
                    && !pending
                        .queue
                        .iter()
                        .any(|work| matches!(work, Work::Select(..)))
                {
                    pending.pointer = None;
                }
            }
            Work::Action(action, sequence) => {
                if matches!(action, PendingAction::Copy | PendingAction::Cut)
                    && self.kind != TextInputKind::Password
                    && !self.selected_range.is_empty()
                {
                    let text = self.value[self.selected_range.clone()].to_owned();
                    if let Some(pending) = &mut self.pending {
                        pending.clipboards.insert(sequence, text);
                    }
                }
                match action {
                    PendingAction::Left => self.left(&Left, window, cx),
                    PendingAction::Right => self.right(&Right, window, cx),
                    PendingAction::Up => self.up(&Up, window, cx),
                    PendingAction::Down => self.down(&Down, window, cx),
                    PendingAction::SelectUp => self.select_up(&SelectUp, window, cx),
                    PendingAction::SelectDown => self.select_down(&SelectDown, window, cx),
                    PendingAction::SelectLeft => self.select_left(&SelectLeft, window, cx),
                    PendingAction::SelectRight => self.select_right(&SelectRight, window, cx),
                    PendingAction::SelectAll => self.select_all(&SelectAll, window, cx),
                    PendingAction::Home => self.home(&Home, window, cx),
                    PendingAction::End => self.end(&End, window, cx),
                    PendingAction::Newline => self.newline(&Newline, window, cx),
                    PendingAction::Backspace => self.backspace(&Backspace, window, cx),
                    PendingAction::Delete => self.delete(&Delete, window, cx),
                    PendingAction::Copy => self.copy(&Copy, window, cx),
                    PendingAction::Cut => self.cut(&Cut, window, cx),
                    PendingAction::Undo => self.undo(&Undo, window, cx),
                    PendingAction::Redo => self.redo(&Redo, window, cx),
                }
            }
            Work::Paste { .. } => unreachable!("clipboard reads return before replay"),
        }
        if let Some(pending) = &mut self.pending {
            pending.replaying = false;
        }
        self.next_pending_frame(window, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext};
    use std::time::Duration;

    fn input<'a>(
        cx: &'a mut TestAppContext,
        value: &str,
    ) -> (Entity<TextInput>, &'a mut VisualTestContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        cx.add_window_view(|_, cx| TextInput::new("queued", cx).value(value))
    }
    fn slow_paste(
        input: &mut TextInput,
        text: &str,
        window: &mut Window,
        cx: &mut Context<TextInput>,
    ) {
        let timer = cx.background_executor().timer(Duration::from_millis(100));
        let text = text.to_owned();
        let read = cx.background_executor().spawn(async move {
            timer.await;
            Some(text)
        });
        input.queue_paste(read, window, cx);
    }
    fn finish(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(100));
        for _ in 0..16 {
            cx.run_until_parked();
            cx.update(|window, cx| {
                window.simulate_next_frame(cx);
                window.draw(cx).clear(cx);
            });
        }
        cx.run_until_parked();
    }
    #[gpui::test]
    fn delayed_clipboard_preserves_input_and_history_order(cx: &mut TestAppContext) {
        let (entity, cx) = input(cx, "A");
        cx.update(|window, cx| {
            entity.update(cx, |input, cx| {
                slow_paste(input, "\u{1f642}", window, cx);
                input.replace_text_in_range(None, "x", window, cx);
                input.undo(&Undo, window, cx);
                input.redo(&Redo, window, cx);
                assert_eq!(input.value, "A");
            })
        });
        finish(cx);
        entity.read_with(cx, |input, _| {
            assert_eq!(input.value, "A\u{1f642}x");
            assert!(input.pending.is_none());
            assert_eq!(input.undo.len(), 2);
        });
    }
    #[gpui::test]
    fn pending_ime_queries_and_commit_use_the_draft(cx: &mut TestAppContext) {
        let (entity, cx) = input(cx, "A");
        cx.update(|window, cx| {
            entity.update(cx, |input, cx| {
                slow_paste(input, "P", window, cx);
                input.replace_and_mark_text_in_range(None, "a", None, window, cx);
                input.replace_and_mark_text_in_range(
                    None,
                    "\u{65e5}\u{672c}\u{1f642}",
                    Some(4..4),
                    window,
                    cx,
                );
                assert_eq!(input.marked_text_range(window, cx), Some(0..4));
                assert_eq!(
                    input
                        .selected_text_range(false, window, cx)
                        .expect("selection")
                        .range,
                    4..4
                );
                let mut actual = None;
                assert_eq!(
                    input
                        .text_for_range(0..4, &mut actual, window, cx)
                        .as_deref(),
                    Some("\u{65e5}\u{672c}\u{1f642}")
                );
                input.unmark_text(window, cx);
            })
        });
        finish(cx);
        cx.update(|window, cx| {
            entity.update(cx, |input, cx| {
                assert_eq!(input.value, "AP\u{65e5}\u{672c}\u{1f642}");
                input.undo(&Undo, window, cx);
                assert_eq!(input.value, "AP");
                input.undo(&Undo, window, cx);
                assert_eq!(input.value, "A");
            })
        });
    }
    #[gpui::test]
    fn pointer_selection_is_rebased_before_following_text(cx: &mut TestAppContext) {
        let (entity, cx) = input(cx, "abc");
        cx.update(|window, cx| {
            entity.update(cx, |input, cx| {
                slow_paste(input, "P", window, cx);
                input.move_to(0, cx);
                input.replace_text_in_range(None, "X", window, cx);
                assert_eq!(
                    input.pending.as_ref().expect("pending").pointer,
                    Some((0..0, false))
                );
            })
        });
        finish(cx);
        entity.read_with(cx, |input, _| assert_eq!(input.value, "XabcP"));
    }
    #[gpui::test]
    fn repeated_text_rebases_clicks_through_exact_undo_and_redo_ranges(cx: &mut TestAppContext) {
        let (entity, cx) = input(cx, "aaa");
        cx.update(|window, cx| {
            entity.update(cx, |input, cx| {
                input.move_to(0, cx);
                slow_paste(input, "a", window, cx);
                input.undo(&Undo, window, cx);
                input.redo(&Redo, window, cx);
                input.move_to(1, cx);
                input.replace_text_in_range(None, "X", window, cx);
            })
        });
        finish(cx);
        entity.read_with(cx, |input, _| assert_eq!(input.value, "aaXaa"));
    }

    #[gpui::test]
    fn queued_copy_is_used_by_a_later_paste(cx: &mut TestAppContext) {
        let (entity, cx) = input(cx, "A");
        cx.update(|window, cx| {
            entity.update(cx, |input, cx| {
                slow_paste(input, "P", window, cx);
                input.select_all(&SelectAll, window, cx);
                input.copy(&Copy, window, cx);
                input.end(&End, window, cx);
                input.queue_paste(
                    gpui::Task::ready(Some("stale clipboard".to_owned())),
                    window,
                    cx,
                );
            })
        });
        finish(cx);
        entity.read_with(cx, |input, _| assert_eq!(input.value, "APAP"));
    }
    #[gpui::test]
    fn empty_reads_and_authoritative_changes_do_not_erase_text(cx: &mut TestAppContext) {
        let (entity, cx) = input(cx, "original");
        cx.update(|window, cx| {
            entity.update(cx, |input, cx| {
                input.select_all(&SelectAll, window, cx);
                slow_paste(input, "", window, cx);
            })
        });
        finish(cx);
        entity.read_with(cx, |input, _| {
            assert_eq!(input.value, "original");
            assert!(input.undo.is_empty());
        });
        cx.update(|window, cx| {
            entity.update(cx, |input, cx| {
                slow_paste(input, "stale", window, cx);
                input.replace_text_in_range(None, "queued", window, cx);
                input.set_value("reset", cx);
            })
        });
        finish(cx);
        entity.read_with(cx, |input, _| assert_eq!(input.value, "reset"));
        cx.update(|window, cx| {
            entity.update(cx, |input, cx| {
                slow_paste(input, "stale", window, cx);
                input.set_disabled(true, cx);
            })
        });
        finish(cx);
        entity.read_with(cx, |input, _| assert_eq!(input.value, "reset"));
    }
}
