use crate::{FocusManager, FocusScopeId, GlobalState, OverlayId, OverlayManager};
use gpui::{
    AnyView, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Window, div,
};
use guic_tokens::Theme;

/// The top-level GUIC root view.
///
/// `Root` connects application content to shared GUIC systems such as theming,
/// overlays, and focus management, then renders the wrapped application view.
pub struct Root {
    view: AnyView,
    focus_handle: FocusHandle,
    focus_scope: FocusScopeId,
    active_focus_trap: Option<OverlayId>,
}

impl Root {
    /// Creates a new root wrapper around an application view.
    pub fn new<V>(view: Entity<V>, window: &mut Window, cx: &mut Context<Self>) -> Self
    where
        V: Render,
    {
        let focus_handle = cx.focus_handle();
        let focus_scope = {
            let manager = cx.global_mut::<FocusManager>();
            let scope = manager.allocate_scope();
            manager.register_handle(scope, focus_handle.clone());
            scope
        };
        let window_id = window.window_handle().window_id();

        let _ = cx.on_release(move |_, cx| {
            FocusManager::global_mut(cx).unregister_scope(focus_scope);
            OverlayManager::global_mut(cx).close_window(window_id);
        });
        cx.global_mut::<GlobalState>().mount_root();
        Self {
            view: view.into(),
            focus_handle,
            focus_scope,
            active_focus_trap: None,
        }
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        let theme_name = theme.name.clone();
        let background = theme.background();
        let foreground = theme.foreground();
        let (overlay_entries, focus_trap) = {
            let manager = OverlayManager::global(cx);
            (
                manager.render_entries(),
                manager
                    .active_focus_trap()
                    .map(|entry| (entry.id, entry.focus_trap())),
            )
        };
        let overlay_count = overlay_entries.len();
        let overlay_elements = overlay_entries
            .iter()
            .filter_map(|entry| entry.render(_window, cx))
            .collect::<Vec<_>>();
        cx.global_mut::<GlobalState>()
            .set_active_theme_name(theme_name);
        FocusManager::global_mut(cx).set_active_scope(self.focus_scope);
        match focus_trap {
            Some((id, Some(handle))) if self.active_focus_trap != Some(id) => {
                handle.focus(_window, cx);
                self.active_focus_trap = Some(id);
            }
            Some((id, _)) => {
                self.active_focus_trap = Some(id);
            }
            None => {
                self.active_focus_trap = None;
            }
        }

        div()
            .size_full()
            .bg(background)
            .text_color(foreground)
            .track_focus(&self.focus_handle)
            .child(self.view.clone())
            .child(
                div()
                    .id("guic-overlay-host")
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .opacity(if overlay_count == 0 { 0.0 } else { 1.0 })
                    .children(overlay_elements),
            )
            .child(
                div()
                    .id("guic-notification-host")
                    .absolute()
                    .top_0()
                    .right_0(),
            )
    }
}
