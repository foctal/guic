//! Ordered background edits with an independent native composition context.
use super::*;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) const ASYNC_BYTES: usize = 64 * 1024;
pub(super) const ASYNC_SELECTIONS: usize = 128;

struct PreparedEdit {
    edit: EditorEdit,
    guard: EditorEdit,
    text_changed: bool,
    selection_changed: bool,
    clipboard: Option<(usize, SharedString, String)>,
    replacements: Vec<(Range<usize>, usize)>,
}

// A completed background task may be cancelled before its foreground waiter
// runs. Its result must also be safe to drop on the foreground thread.
struct PreparedResult {
    data: Option<PreparedEdit>,
    executor: gpui::BackgroundExecutor,
}
impl Drop for PreparedResult {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            self.executor
                .spawn(async move {
                    drop(data);
                })
                .detach();
        }
    }
}

pub(super) enum Work {
    Insert {
        text: String,
        primary: bool,
    },
    Native {
        text: String,
        range: Option<Range<usize>>,
        bytes: Option<Range<usize>>,
    },
    DraftCommit {
        draft: String,
        bytes: Range<usize>,
        text: String,
    },
    EditKey(KeyDownEvent),
    Clipboard {
        sequence: usize,
        cut: bool,
    },
    Paste {
        read: gpui::Task<Option<String>>,
        copied: Option<usize>,
    },
    Select {
        selections: Vec<(usize, usize)>,
    },
    Key(KeyDownEvent),
}

#[derive(Default)]
pub(super) struct Draft {
    pub text: String,
    pub selected: Range<usize>,
    pub marked: Option<Range<usize>>,
}
impl Draft {
    pub fn preview(&self) -> String {
        let end = self.text.floor_char_boundary(self.text.len().min(4096));
        let graphemes = self.text[..end].grapheme_indices(true).take(128);
        graphemes
            .take_while(|(byte, g)| end == self.text.len() || byte + g.len() < end)
            .map(|(_, g)| g)
            .collect::<String>()
            .replace('\n', " ")
    }
    pub fn units(&self, bytes: Range<usize>) -> Range<usize> {
        self.text[..bytes.start].encode_utf16().count()
            ..self.text[..bytes.end].encode_utf16().count()
    }
    pub fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        marked: bool,
    ) {
        let range = range
            .map(|r| native_range(&self.text, r))
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone());
        let replacement = normalize_newlines(text);
        self.text.replace_range(range.clone(), &replacement);
        self.selected = selected
            .map(|r| {
                let raw = native_range(text, r);
                range.start + normalize_newlines(&text[..raw.start]).len()
                    ..range.start + normalize_newlines(&text[..raw.end]).len()
            })
            .unwrap_or(range.start + replacement.len()..range.start + replacement.len());
        self.marked = marked.then_some(range.start..range.start + replacement.len());
    }
}

struct Transition {
    before: LineStore,
    after: LineStore,
    replacements: Vec<(Range<usize>, usize)>,
}

pub(super) struct PendingEdit {
    pub queue: VecDeque<Work>,
    pub draft: Draft,
    pub working: bool,
    pub reveal: bool,
    pub waiting_host: bool,
    pub accept_host_result: bool,
    pub replaying: bool,
    pub clipboard_sequence: usize,
    pub pointer: Option<Vec<(usize, usize)>>,
    transitions: Vec<Transition>,
    clipboards: BTreeMap<usize, SharedString>,
    pub cancelled: Arc<AtomicBool>,
    task: Option<gpui::Task<()>>,
    executor: gpui::BackgroundExecutor,
}
impl PendingEdit {
    fn new(executor: gpui::BackgroundExecutor) -> Self {
        Self {
            queue: VecDeque::new(),
            draft: Draft::default(),
            working: false,
            reveal: true,
            waiting_host: false,
            accept_host_result: false,
            replaying: false,
            clipboard_sequence: 0,
            pointer: None,
            transitions: Vec::new(),
            clipboards: BTreeMap::new(),
            cancelled: Arc::new(AtomicBool::new(false)),
            task: None,
            executor,
        }
    }
}
impl PendingEdit {
    pub(super) fn rebase_history(&mut self, before: &EditorBuffer, after: &EditorBuffer) {
        // History restores shared snapshots. Find a path through this pending
        // batch's transactions, including inverse changes for queued Undo.
        let mut frontier = VecDeque::from([(before.lines.clone(), Vec::new())]);
        let mut visited = vec![before.lines.clone()];
        while let Some((snapshot, path)) = frontier.pop_front() {
            if snapshot.same_snapshot(&after.lines) {
                let changes: Vec<_> = path
                    .into_iter()
                    .flat_map(|(index, forward): (usize, bool)| {
                        let changes = &self.transitions[index].replacements;
                        if forward {
                            changes.clone()
                        } else {
                            changes
                                .iter()
                                .rev()
                                .map(|(range, length)| {
                                    (range.start..range.start + length, range.len())
                                })
                                .collect()
                        }
                    })
                    .collect();
                for (range, length) in changes {
                    self.rebase_pointers(&range, length);
                }
                return;
            }
            for (index, transition) in self.transitions.iter().enumerate() {
                let next = if snapshot.same_snapshot(&transition.before) {
                    Some((&transition.after, true))
                } else if snapshot.same_snapshot(&transition.after) {
                    Some((&transition.before, false))
                } else {
                    None
                };
                if let Some((next, forward)) = next
                    && !visited.iter().any(|seen| seen.same_snapshot(next))
                {
                    visited.push(next.clone());
                    let mut path = path.clone();
                    path.push((index, forward));
                    frontier.push_back((next.clone(), path));
                }
            }
        }
        // History outside this batch has no position map. Preserve the host's
        // restored selection instead of applying stale offsets to unrelated text.
        self.pointer = None;
        self.queue
            .retain(|work| !matches!(work, Work::Select { .. }));
    }

    fn rebase_pointers(&mut self, range: &Range<usize>, length: usize) {
        let rebase = |offset: &mut usize| {
            if *offset >= range.end {
                *offset = *offset - range.len() + length;
            } else if *offset > range.start {
                *offset = range.start + length;
            }
        };
        if let Some(selections) = &mut self.pointer {
            for (anchor, head) in selections {
                rebase(anchor);
                rebase(head);
            }
        }
        for work in &mut self.queue {
            if let Work::Select { selections } = work {
                for (anchor, head) in selections {
                    rebase(anchor);
                    rebase(head);
                }
            }
        }
    }
}

impl Drop for PendingEdit {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        let payload = (
            std::mem::take(&mut self.queue),
            std::mem::take(&mut self.clipboards),
            std::mem::take(&mut self.draft),
            std::mem::take(&mut self.transitions),
        );
        self.executor
            .spawn(async move {
                drop(payload);
            })
            .detach();
    }
}

impl EditorSurface {
    pub(super) fn pointer_selection(&self) -> EditorSelection {
        self.pending_edit
            .as_ref()
            .and_then(|p| p.pointer.as_ref())
            .and_then(|selections| selections.first().copied())
            .map_or_else(
                || self.edit.selections[0].clone(),
                |(anchor, head)| {
                    EditorSelection::new(
                        byte_position(&self.edit.buffer, anchor),
                        byte_position(&self.edit.buffer, head),
                    )
                },
            )
    }

    pub(super) fn select_pointer(
        &mut self,
        selection: EditorSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_pointers(vec![selection], window, cx);
    }

    pub(super) fn pointer_selections(&self) -> std::borrow::Cow<'_, [EditorSelection]> {
        if let Some(selections) = self.pending_edit.as_ref().and_then(|p| p.pointer.as_ref()) {
            selections
                .iter()
                .map(|&(anchor, head)| {
                    EditorSelection::new(
                        byte_position(&self.edit.buffer, anchor),
                        byte_position(&self.edit.buffer, head),
                    )
                })
                .collect::<Vec<_>>()
                .into()
        } else {
            self.edit.selections.as_slice().into()
        }
    }

    pub(super) fn select_pointers(
        &mut self,
        selections: Vec<EditorSelection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(pending) = &mut self.pending_edit {
            let selections = selections
                .iter()
                .map(|selection| {
                    (
                        position_byte(&self.edit.buffer, selection.anchor),
                        position_byte(&self.edit.buffer, selection.head),
                    )
                })
                .collect::<Vec<_>>();
            pending.pointer = Some(selections.clone());
            // Coalesce drag updates only when no intervening input depends on
            // the previous selection. Preserve typing/copy/history order.
            if matches!(pending.queue.back(), Some(Work::Select { .. })) {
                pending.queue.pop_back();
            }
            self.enqueue_edit(Work::Select { selections }, window, cx);
        } else {
            self.apply(
                EditorEdit::new(self.edit.buffer.clone(), selections),
                window,
                cx,
            );
        }
    }

    fn edit_frame(
        window: &Window,
        cx: &Context<Self>,
        callback: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let entity = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = entity.update(cx, |state, cx| callback(state, window, cx));
        });
    }

    pub(super) fn cancel_edits(&mut self) {
        self.edit_generation = self.edit_generation.wrapping_add(1);
        self.pending_edit = None;
    }

    pub(super) fn enqueue_edit(&mut self, work: Work, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_edit.is_none() {
            self.edit_generation = self.edit_generation.wrapping_add(1);
            self.pending_edit = Some(PendingEdit::new(cx.background_executor().clone()));
            self.end_pointer_selection();
        }
        let pending = self.pending_edit.as_mut().expect("pending edits");
        pending.reveal = true;
        pending.queue.push_back(work);
        self.drain_edits(window, cx);
        cx.notify();
    }

    pub(super) fn commit_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pending) = &mut self.pending_edit else {
            return;
        };
        pending.reveal = true;
        let draft = std::mem::take(&mut pending.draft);
        if !draft.text.is_empty() {
            pending.queue.push_back(Work::Insert {
                text: draft.text,
                primary: true,
            });
        }
        self.drain_edits(window, cx);
        cx.notify();
    }

    pub(super) fn drain_edits(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pending) = &mut self.pending_edit else {
            return;
        };
        if pending.working || pending.waiting_host {
            return;
        }
        let Some(work) = pending.queue.pop_front() else {
            if pending.draft.marked.is_none() {
                self.pending_edit = None;
            }
            cx.notify();
            return;
        };
        if let Work::Select { selections } = work {
            if !pending
                .queue
                .iter()
                .any(|work| matches!(work, Work::Select { .. }))
            {
                pending.pointer = None;
            }
            let selections = selections
                .into_iter()
                .map(|(anchor, head)| {
                    EditorSelection::new(
                        byte_position(&self.edit.buffer, anchor),
                        byte_position(&self.edit.buffer, head),
                    )
                })
                .collect();
            self.apply(
                EditorEdit::new(self.edit.buffer.clone(), selections),
                window,
                cx,
            );
            self.drain_edits(window, cx);
            return;
        }
        if let Work::Key(event) = work {
            let history = editor_key_command(&event).is_some() && self.props.on_command.is_some();
            let generation = self.edit_generation;
            if history {
                pending.waiting_host = true;
                // First let the host acknowledge the preceding edit. Then issue
                // the command and wait for a full render of its result. A command
                // that changes nothing must not restore obsolete host props.
                window.refresh();
                Self::edit_frame(window, cx, move |_, window, cx| {
                    Self::edit_frame(window, cx, move |state, window, cx| {
                        if state.edit_generation != generation {
                            return;
                        }
                        if let Some(pending) = &mut state.pending_edit {
                            pending.replaying = true;
                            pending.accept_host_result = true;
                        }
                        state.key_down(&event, window, cx);
                        if let Some(pending) = &mut state.pending_edit {
                            pending.replaying = false;
                        }
                        window.refresh();
                        Self::edit_frame(window, cx, move |state, window, cx| {
                            if state.edit_generation != generation {
                                return;
                            }
                            if let Some(pending) = &mut state.pending_edit {
                                pending.waiting_host = false;
                                pending.accept_host_result = false;
                            }
                            state.drain_edits(window, cx);
                        });
                    });
                    window.refresh();
                });
            } else {
                pending.replaying = true;
                self.key_down(&event, window, cx);
                if let Some(pending) = &mut self.pending_edit {
                    pending.replaying = false;
                }
                Self::edit_frame(window, cx, move |state, window, cx| {
                    if state.edit_generation == generation {
                        state.drain_edits(window, cx);
                    }
                });
            }
            return;
        }
        let copied_text = match &work {
            Work::Paste {
                copied: Some(sequence),
                ..
            } => pending
                .clipboards
                .range(..=*sequence)
                .next_back()
                .map(|(_, text)| text.clone()),
            _ => None,
        };
        pending.working = true;
        let cancelled = pending.cancelled.clone();
        let generation = self.edit_generation;
        let mut source = self.edit.clone();
        let guard = source.clone();
        let committed = self.composition.clone().unwrap_or_else(|| source.clone());
        let host_snapshot = self.props.buffer.clone();
        let result_executor = cx.background_executor().clone();
        let worker = cx.background_executor().spawn(async move {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            let baseline = source.buffer.clone();
            source.buffer.replacements = Some(Vec::new());
            let mut clipboard = None;
            let mut edit = match work {
                Work::Insert { text, primary } => {
                    let selections = if primary {
                        &source.selections[..1]
                    } else {
                        &source.selections
                    };
                    edit_replacing_selections(&source.buffer, selections, &text)
                }
                Work::DraftCommit {
                    mut draft,
                    bytes,
                    text,
                } => {
                    draft.replace_range(bytes, &normalize_newlines(&text));
                    edit_replacing_selections(&source.buffer, &source.selections[..1], &draft)
                }
                Work::Paste { read, .. } => {
                    let text = if let Some(text) = copied_text {
                        Some(text.to_string())
                    } else {
                        read.await
                    };
                    if cancelled.load(Ordering::Relaxed) {
                        return None;
                    }
                    if let Some(text) = text.filter(|text| !text.is_empty()) {
                        edit_replacing_selections(&source.buffer, &source.selections, &text)
                    } else {
                        source.clone()
                    }
                }
                Work::Clipboard { sequence, cut } => {
                    let text = source
                        .selections
                        .iter()
                        .filter(|s| !s.is_empty())
                        .map(|s| source.buffer.text_in_range(s.ordered()))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let edit = if cut && !text.is_empty() {
                        edit_replacing_selections(&source.buffer, &source.selections, "")
                    } else {
                        source.clone()
                    };
                    if !text.is_empty() {
                        clipboard = Some((sequence, SharedString::from(text.clone()), text));
                    }
                    edit
                }
                Work::EditKey(event) => match event.keystroke.key.as_str() {
                    "backspace" => edit_deleting(&source.buffer, &source.selections, true),
                    "delete" => edit_deleting(&source.buffer, &source.selections, false),
                    "enter" => edit_replacing_selections(&source.buffer, &source.selections, "\n"),
                    "tab" => edit_indenting_lines(
                        &source.buffer,
                        &source.selections,
                        "    ",
                        event.keystroke.modifiers.shift,
                    ),
                    _ => unreachable!("queued text editing key"),
                },
                Work::Native { text, range, bytes } => {
                    let mut buffer = source.buffer;
                    let range = range
                        .map(|r| buffer.native_range(r))
                        .or(bytes)
                        .unwrap_or_else(|| {
                            let r = source.selections[0].ordered();
                            position_byte(&buffer, r.start)..position_byte(&buffer, r.end)
                        });
                    let replacement = normalize_newlines(&text);
                    buffer.replace_bytes(range.clone(), &replacement);
                    let cursor = byte_position(&buffer, range.start + replacement.len());
                    EditorEdit::new(buffer, vec![EditorSelection::cursor(cursor)])
                }
                Work::Key(_) | Work::Select { .. } => {
                    unreachable!("navigation is handled on the foreground thread")
                }
            };
            let replacements = edit.buffer.replacements.take().unwrap_or_default();
            // Prepare position queries before publication, including long interior
            // lines of multiline pastes. This also keeps queued navigation cheap.
            for line in 0..edit.buffer.line_count() {
                if cancelled.load(Ordering::Relaxed) {
                    return None;
                }
                let _ = edit.buffer.line_len(line);
            }
            if !edit
                .buffer
                .lines
                .compare_and_remember_difference(&baseline.lines)
            {
                edit.buffer = baseline;
            }
            if cancelled.load(Ordering::Relaxed) {
                None
            } else {
                let text_changed = edit
                    .buffer
                    .lines
                    .compare_and_remember_difference(&committed.buffer.lines);
                edit.buffer
                    .lines
                    .compare_and_remember_difference(&host_snapshot.lines);
                let selection_changed = edit.selections != committed.selections;
                Some(PreparedResult {
                    data: Some(PreparedEdit {
                        edit,
                        text_changed,
                        selection_changed,
                        clipboard,
                        replacements,
                        guard,
                    }),
                    executor: result_executor,
                })
            }
        });
        let task = cx.spawn_in(window, async move |entity, cx| {
            if let Some(mut prepared) = worker.await {
                let _ = entity.update_in(cx, |state, window, cx| {
                    let PreparedEdit {
                        edit,
                        text_changed,
                        selection_changed,
                        clipboard,
                        replacements,
                        guard,
                    } = prepared.data.take().expect("prepared edit");
                    if state.edit_generation != generation
                        || !state.edit.buffer.lines.same_snapshot(&guard.buffer.lines)
                        || state.edit.selections != guard.selections
                        || !state.editable()
                    {
                        cx.background_executor()
                            .spawn(async move {
                                drop((edit, guard, clipboard, replacements));
                            })
                            .detach();
                        return;
                    }
                    for (range, length) in &replacements {
                        state.rebase_rectangle_anchor(range, *length);
                    }
                    if let Some(pending) = &mut state.pending_edit {
                        pending.working = false;
                        for (range, length) in &replacements {
                            pending.rebase_pointers(range, *length);
                        }
                        if !replacements.is_empty() {
                            pending.transitions.push(Transition {
                                before: guard.buffer.lines.clone(),
                                after: edit.buffer.lines.clone(),
                                replacements,
                            });
                        }
                        if let Some((sequence, cached, text)) = clipboard {
                            pending.clipboards.insert(sequence, cached);
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                        }
                    }
                    let before = std::mem::replace(&mut state.edit, edit);
                    let composition = state.composition.take();
                    state.marked = None;
                    state.composing_selection = None;
                    state.preferred_x = None;
                    state.upstream = false;
                    state.reveal = state.pending_edit.as_ref().is_some_and(|p| p.reveal);
                    state.max_width = px(0.0);
                    state.publish_changes(text_changed, selection_changed, window, cx);
                    cx.background_executor()
                        .spawn(async move {
                            drop((before, composition, guard));
                        })
                        .detach();
                    cx.notify();
                    state.drain_edits(window, cx);
                });
            }
        });
        self.pending_edit.as_mut().expect("pending edit").task = Some(task);
    }
}

impl EditorSurface {
    pub(super) fn draft_bounds(
        &self,
        range: Range<usize>,
        window: &Window,
        cx: &App,
    ) -> Option<Bounds<Pixels>> {
        let pending = self.pending_edit.as_ref()?;
        let viewport = self.layout.as_ref()?;
        let byte = utf16_byte(&pending.draft.text, range.start);
        let preview = pending.draft.preview();
        if byte > preview.len() {
            return None;
        }
        let pasting = guic_core::translate(cx, "input.pasting", "Pasting...");
        let text = SharedString::from(format!("{pasting} {preview}"));
        let run = TextRun {
            len: text.len(),
            font: viewport.font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window
            .text_system()
            .shape_line(text, px(FONT_SIZE), &[run], None);
        let x = viewport.bounds.left() + px(PADDING) + line.x_for_index(pasting.len() + 1 + byte);
        if x > viewport.bounds.right() - px(PADDING) {
            return None;
        }
        Some(Bounds::new(
            point(x, viewport.bounds.bottom() - px(LINE_HEIGHT)),
            size(px(1.0), px(LINE_HEIGHT)),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn discarded_ready_result_releases_snapshots_on_the_executor(cx: &mut gpui::TestAppContext) {
        let buffer = EditorBuffer::from_text("日本🙂".repeat(20_000));
        let analysis = buffer
            .lines
            .analyze(0, &AtomicBool::new(false))
            .expect("analysis");
        let weak = Arc::downgrade(&analysis);
        drop(analysis);
        let edit = EditorEdit::new(
            buffer,
            vec![EditorSelection::cursor(EditorPosition::default())],
        );
        let prepared = PreparedResult {
            data: Some(PreparedEdit {
                guard: edit.clone(),
                edit,
                text_changed: true,
                selection_changed: true,
                clipboard: None,
                replacements: Vec::new(),
            }),
            executor: cx.background_executor.clone(),
        };
        drop(prepared);
        assert!(weak.upgrade().is_some());
        cx.run_until_parked();
        assert!(weak.upgrade().is_none());
    }
}
