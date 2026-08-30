use crate::{Button, ButtonVariant, ComponentSize, Label, Tag, TagVariant};
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, div, px,
};
use guic_tokens::Theme;
use std::rc::Rc;

type FileRequestHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type FileRemoveHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A controlled file import workflow surface.
///
/// `FilePicker` does not open native dialogs itself. Instead it emits
/// `on_request_pick`, allowing the host to use its platform file-dialog layer
/// and then pass selected file names back through [`FilePicker::files`].
#[derive(gpui::IntoElement)]
pub struct FilePicker {
    id: SharedString,
    files: Vec<SharedString>,
    label: SharedString,
    disabled: bool,
    on_request_pick: Option<FileRequestHandler>,
    on_remove: Option<FileRemoveHandler>,
}

impl FilePicker {
    /// Creates an empty file picker.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            files: Vec::new(),
            label: SharedString::from("Choose files"),
            disabled: false,
            on_request_pick: None,
            on_remove: None,
        }
    }

    /// Replaces selected file labels.
    #[must_use]
    pub fn files(mut self, files: Vec<impl Into<SharedString>>) -> Self {
        self.files = files.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the picker button label.
    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Sets whether the surface is disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Registers a handler for opening the host-owned native file dialog.
    #[must_use]
    pub fn on_request_pick(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_request_pick = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for removing a selected file.
    #[must_use]
    pub fn on_remove(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for FilePicker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let mut root = div()
            .id(self.id)
            .w_full()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .opacity(if self.disabled { 0.55 } else { 1.0 });

        let button = Button::new(self.label)
            .variant(ButtonVariant::Secondary)
            .size(ComponentSize::Small);
        root = if !self.disabled {
            if let Some(handler) = self.on_request_pick {
                root.child(button.on_click(move |event: &ClickEvent, window, cx| {
                    let _ = event;
                    handler(window, cx);
                }))
            } else {
                root.child(button)
            }
        } else {
            root.child(button.disabled(true))
        };

        if self.files.is_empty() {
            root = root.child(Label::new("No files selected").muted(true));
        } else {
            let mut chips = div().flex().flex_wrap().gap_2();
            for file in self.files {
                let mut tag = Tag::new(file.clone()).variant(TagVariant::Info);
                if !self.disabled
                    && let Some(on_remove) = self.on_remove.clone()
                {
                    tag = tag.on_remove(move |event: &ClickEvent, window, cx| {
                        let _ = event;
                        on_remove(&file, window, cx);
                    });
                }
                chips = chips.child(tag);
            }
            root = root.child(chips);
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::FilePicker;

    #[test]
    fn file_picker_builder_tracks_files() {
        let picker = FilePicker::new("import")
            .files(vec!["a.txt", "b.txt"])
            .label("Import")
            .disabled(true);
        assert_eq!(picker.files.len(), 2);
        assert_eq!(picker.label, "Import");
        assert!(picker.disabled);
    }
}
