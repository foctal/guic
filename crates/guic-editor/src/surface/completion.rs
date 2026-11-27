//! Explicit completion requests anchored to the primary caret.
use super::*;
use gpui::StatefulInteractiveElement as _;

pub(super) struct CompletionMenu {
    items: Vec<EditorCompletion>,
    selected: usize,
    replace: Range<usize>,
}

impl EditorSurface {
    pub(super) fn dismiss_completion(&mut self, cx: &mut Context<Self>) {
        self.completion_job = None;
        if self.completion.take().is_some() {
            cx.notify();
        }
    }

    fn request_completion(&mut self, cx: &mut Context<Self>) {
        self.dismiss_completion(cx);
        let Some(provider) = self.props.completion_provider.clone() else {
            return;
        };
        let source = self.edit.buffer.clone();
        let selection = self.edit.selections[0].clone();
        let head = selection.head;
        let mut bytes = self.selection_bytes();
        if bytes.is_empty() {
            let line = source.line(head.line).map_or("", |line| line.as_ref());
            let end = source.lines.column_byte(head.line, head.column);
            let prefix = &line[..end];
            for (index, grapheme) in prefix.grapheme_indices(true).rev() {
                if !grapheme
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    break;
                }
                bytes.start = source.lines.line_byte(head.line) + index;
            }
        }
        let replace = source.utf16_range(bytes);
        let worker = cx
            .background_executor()
            .spawn(async move { provider.completions(&source, head) });
        self.completion_job = Some(cx.spawn(async move |entity, cx| {
            let items = worker.await;
            let _ = entity.update(cx, |state, cx| {
                state.completion_job = None;
                if !items.is_empty() {
                    state.completion = Some(CompletionMenu {
                        items,
                        selected: 0,
                        replace,
                    });
                    cx.notify();
                }
            });
        }));
    }

    fn accept_completion(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.completion.take() else {
            return;
        };
        let Some(item) = menu.items.get(index) else {
            return;
        };
        self.replace_text_in_range(Some(menu.replace), item.insert_text.as_ref(), window, cx);
        cx.notify();
    }

    pub(super) fn completion_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let key = event.keystroke.key.as_str();
        let mods = event.keystroke.modifiers;
        if key == "space"
            && mods.control
            && !mods.alt
            && !mods.platform
            && !mods.shift
            && self.props.completion_provider.is_some()
            && self.editable()
            && self.edit.selections.len() == 1
            && self.pending_edit.is_none()
            && self.marked.is_none()
        {
            self.request_completion(cx);
            return true;
        }
        if key == "escape" && (self.completion.is_some() || self.completion_job.is_some()) {
            self.dismiss_completion(cx);
            return true;
        }
        let Some(menu) = &mut self.completion else {
            return false;
        };
        if mods == gpui::Modifiers::none() {
            match key {
                "up" => menu.selected = menu.selected.saturating_sub(1),
                "down" => menu.selected = (menu.selected + 1).min(menu.items.len() - 1),
                "enter" | "tab" => {
                    let index = menu.selected;
                    self.accept_completion(index, window, cx);
                    return true;
                }
                _ => {
                    self.dismiss_completion(cx);
                    return false;
                }
            }
            cx.notify();
            return true;
        }
        self.dismiss_completion(cx);
        false
    }

    pub(super) fn completion_element(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let menu = self.completion.as_ref()?;
        let layout = self.layout.as_ref()?;
        let caret = layout.caret?;
        let available = layout.bounds;
        let line = px(LINE_HEIGHT);
        let below = available.bottom() - caret.bottom();
        let above = caret.top() - available.top();
        let count = ((below.max(above) / line).floor() as usize)
            .clamp(1, 8)
            .min(menu.items.len());
        let height = line * count as f32;
        let width = px(320.).min(available.size.width);
        let left = (caret.left() - available.left()).clamp(px(0.), available.size.width - width);
        let top = if below >= height {
            caret.bottom() - available.top()
        } else {
            (caret.top() - available.top() - height).max(px(0.))
        };
        let theme = Theme::global(cx);
        let start = (menu.selected + 1).saturating_sub(count);
        let mut list = div()
            .id("completions")
            .absolute()
            .left(left)
            .top(top)
            .w(width)
            .overflow_hidden()
            .role(gpui::Role::ListBox)
            .aria_label("Completions")
            .rounded(px(theme.radius.sm))
            .bg(theme.background())
            .shadow_md()
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            });
        for (offset, item) in menu.items[start..start + count].iter().enumerate() {
            let index = start + offset;
            let selected = index == menu.selected;
            let mut row = div()
                .id(("completion", index))
                .debug_selector(move || format!("guic-editor-completion-{index}"))
                .role(gpui::Role::ListBoxOption)
                .aria_selected(selected)
                .aria_label(item.label.clone())
                .h(line)
                .px_2()
                .flex()
                .gap_2()
                .items_center()
                .cursor_pointer()
                .bg(if selected {
                    theme.primary().opacity(0.18)
                } else {
                    theme.background()
                })
                .text_color(theme.foreground())
                .child(div().min_w_0().truncate().child(item.label.clone()));
            if let Some(detail) = &item.detail {
                row = row.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(theme.muted_foreground())
                        .child(detail.clone()),
                );
            }
            list = list.child(row.on_click(cx.listener(move |editor, _, window, cx| {
                cx.stop_propagation();
                editor.accept_completion(index, window, cx);
            })));
        }
        Some(list)
    }
}
