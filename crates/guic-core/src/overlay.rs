use gpui::{AnyElement, App, FocusHandle, Global, IntoElement, Window, WindowId, deferred};
use std::collections::BTreeMap;
use std::rc::Rc;

type OverlayRenderer = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// Standard deferred projection priorities used by GUIC overlays.
pub struct OverlayPriority;

impl OverlayPriority {
    /// Floating anchored content such as popovers and dropdown menus.
    pub const FLOATING: usize = 1;
    /// Window-blocking surfaces such as dialogs, drawers, and context menus.
    pub const MODAL: usize = 2;
    /// Non-blocking notifications that should appear above other projections.
    pub const NOTIFICATION: usize = 3;
}

/// Projects an element into GPUI's deferred overlay drawing pass.
///
/// Components should use this helper instead of calling `gpui::deferred`
/// directly so overlay layering stays centralized in `guic-core`.
#[must_use]
pub fn overlay_portal(content: impl IntoElement, priority: usize) -> AnyElement {
    deferred(content).priority(priority).into_any_element()
}

/// Shared overlay manager state.
#[derive(Default)]
pub struct OverlayManager {
    next_id: u64,
    overlays: Vec<OverlayId>,
    entries: BTreeMap<OverlayId, OverlayEntry>,
}

impl Global for OverlayManager {}

impl OverlayManager {
    /// Returns the registered overlay manager.
    #[must_use]
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// Returns the registered overlay manager mutably.
    #[must_use]
    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Opens an overlay and returns its identifier.
    #[must_use]
    pub fn open(&mut self, kind: OverlayKind) -> OverlayId {
        self.open_with_options(kind, OverlayOptions::default())
    }

    /// Opens an overlay with explicit runtime options.
    #[must_use]
    pub fn open_with_options(&mut self, kind: OverlayKind, options: OverlayOptions) -> OverlayId {
        self.open_entry(kind, options, None)
    }

    /// Opens a renderable overlay and returns its identifier.
    ///
    /// The renderer is called by [`crate::Root`] while rendering the overlay
    /// host. It should return the complete portal element, including any scrim
    /// or positioning container needed by the overlay surface.
    #[must_use]
    pub fn open_rendered<F, E>(
        &mut self,
        kind: OverlayKind,
        options: OverlayOptions,
        renderer: F,
    ) -> OverlayId
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        let renderer = Rc::new(move |window: &mut Window, cx: &mut App| {
            renderer(window, cx).into_any_element()
        });
        self.open_entry(kind, options, Some(renderer))
    }

    fn open_entry(
        &mut self,
        kind: OverlayKind,
        options: OverlayOptions,
        renderer: Option<OverlayRenderer>,
    ) -> OverlayId {
        let id = OverlayId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.overlays.push(id);
        self.entries.insert(
            id,
            OverlayEntry {
                id,
                kind,
                priority: options.priority,
                dismissible: options.dismissible,
                traps_focus: options.traps_focus,
                focus_trap: options.focus_trap,
                restore_focus_to: options.restore_focus_to,
                window_id: options.window_id,
                renderer,
            },
        );
        id
    }

    /// Closes the top-most overlay.
    pub fn close_top(&mut self, reason: CloseReason) -> Option<ClosedOverlay> {
        let id = self.overlays.pop()?;
        self.entries
            .remove(&id)
            .map(|entry| ClosedOverlay { entry, reason })
    }

    /// Closes the top-most overlay and restores focus when configured.
    pub fn close_top_and_restore_focus(
        &mut self,
        reason: CloseReason,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<ClosedOverlay> {
        let closed = self.close_top(reason)?;
        closed.restore_focus(window, cx);
        Some(closed)
    }

    /// Dismisses the top-most overlay if it allows dismissal.
    pub fn dismiss_top(&mut self, reason: CloseReason) -> Option<ClosedOverlay> {
        let id = *self
            .overlays
            .iter()
            .rev()
            .find(|id| self.entries.get(id).is_some_and(|entry| entry.dismissible))?;
        self.close(id, reason)
    }

    /// Dismisses the top-most dismissible overlay and restores focus when configured.
    pub fn dismiss_top_and_restore_focus(
        &mut self,
        reason: CloseReason,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<ClosedOverlay> {
        let closed = self.dismiss_top(reason)?;
        closed.restore_focus(window, cx);
        Some(closed)
    }

    /// Closes a specific overlay.
    pub fn close(&mut self, id: OverlayId, reason: CloseReason) -> Option<ClosedOverlay> {
        self.overlays.retain(|overlay_id| *overlay_id != id);
        self.entries
            .remove(&id)
            .map(|entry| ClosedOverlay { entry, reason })
    }

    /// Closes a specific overlay and restores focus when configured.
    pub fn close_and_restore_focus(
        &mut self,
        id: OverlayId,
        reason: CloseReason,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<ClosedOverlay> {
        let closed = self.close(id, reason)?;
        closed.restore_focus(window, cx);
        Some(closed)
    }

    /// Closes all overlays associated with a GPUI window.
    pub fn close_window(&mut self, window_id: WindowId) -> Vec<ClosedOverlay> {
        let ids = self
            .overlays
            .iter()
            .copied()
            .filter(|id| {
                self.entries
                    .get(id)
                    .is_some_and(|entry| entry.window_id == Some(window_id))
            })
            .collect::<Vec<_>>();

        ids.into_iter()
            .filter_map(|id| self.close(id, CloseReason::WindowClosed))
            .collect()
    }

    /// Returns the top-most overlay that declares a focus trap.
    #[must_use]
    pub fn active_focus_trap(&self) -> Option<&OverlayEntry> {
        self.overlays
            .iter()
            .rev()
            .find_map(|id| self.entries.get(id).filter(|entry| entry.traps_focus))
    }

    /// Returns the top-most overlay entry.
    #[must_use]
    pub fn top(&self) -> Option<&OverlayEntry> {
        self.overlays.last().and_then(|id| self.entries.get(id))
    }

    /// Returns the open overlays from bottom to top.
    pub fn entries(&self) -> impl Iterator<Item = &OverlayEntry> {
        self.overlays.iter().filter_map(|id| self.entries.get(id))
    }

    /// Returns renderable overlay entries in bottom-to-top projection order.
    #[must_use]
    pub fn render_entries(&self) -> Vec<OverlayEntry> {
        let mut entries = self
            .entries()
            .filter(|entry| entry.is_renderable())
            .cloned()
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.priority);
        entries
    }

    /// Returns the overlay count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.overlays.len()
    }

    /// Returns whether no overlays are currently open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.overlays.is_empty()
    }
}

/// A stable overlay identifier.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct OverlayId(u64);

/// Runtime options for an overlay entry.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OverlayOptions {
    priority: i16,
    dismissible: bool,
    traps_focus: bool,
    focus_trap: Option<FocusHandle>,
    restore_focus_to: Option<FocusHandle>,
    window_id: Option<WindowId>,
}

impl OverlayOptions {
    /// Creates default overlay options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the projection priority. Higher values render above lower values.
    #[must_use]
    pub fn priority(mut self, priority: i16) -> Self {
        self.priority = priority;
        self
    }

    /// Sets whether the overlay can be dismissed by generic outside actions.
    #[must_use]
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    /// Sets whether the overlay should be treated as a focus trap.
    #[must_use]
    pub fn traps_focus(mut self, traps_focus: bool) -> Self {
        self.traps_focus = traps_focus;
        self
    }

    /// Sets the focus handle that owns keyboard focus while the overlay is open.
    ///
    /// This also marks the overlay as a focus trap.
    #[must_use]
    pub fn focus_trap(mut self, focus_handle: FocusHandle) -> Self {
        self.traps_focus = true;
        self.focus_trap = Some(focus_handle);
        self
    }

    /// Sets the focus handle that should be restored after the overlay closes.
    #[must_use]
    pub fn restore_focus_to(mut self, focus_handle: FocusHandle) -> Self {
        self.restore_focus_to = Some(focus_handle);
        self
    }

    /// Associates the overlay with a GPUI window for cleanup on window close.
    #[must_use]
    pub fn window_id(mut self, window_id: WindowId) -> Self {
        self.window_id = Some(window_id);
        self
    }
}

/// Metadata tracked for an open overlay.
#[derive(Clone)]
pub struct OverlayEntry {
    /// Stable overlay identifier.
    pub id: OverlayId,
    /// Logical overlay kind.
    pub kind: OverlayKind,
    priority: i16,
    dismissible: bool,
    traps_focus: bool,
    focus_trap: Option<FocusHandle>,
    restore_focus_to: Option<FocusHandle>,
    window_id: Option<WindowId>,
    renderer: Option<OverlayRenderer>,
}

impl OverlayEntry {
    /// Returns the projection priority. Higher values render above lower values.
    #[must_use]
    pub fn priority(&self) -> i16 {
        self.priority
    }

    /// Returns whether this overlay can be dismissed by generic outside actions.
    #[must_use]
    pub fn is_dismissible(&self) -> bool {
        self.dismissible
    }

    /// Returns whether this overlay is intended to trap focus.
    #[must_use]
    pub fn traps_focus(&self) -> bool {
        self.traps_focus
    }

    /// Returns the focus handle that owns keyboard focus while this overlay is open.
    #[must_use]
    pub fn focus_trap(&self) -> Option<FocusHandle> {
        self.focus_trap.clone()
    }

    /// Returns the focus handle to restore after closure, if one was provided.
    #[must_use]
    pub fn restore_focus_to(&self) -> Option<FocusHandle> {
        self.restore_focus_to.clone()
    }

    /// Returns the associated GPUI window identifier, if one was provided.
    #[must_use]
    pub fn window_id(&self) -> Option<WindowId> {
        self.window_id
    }

    /// Returns whether this entry has renderable portal content.
    #[must_use]
    pub fn is_renderable(&self) -> bool {
        self.renderer.is_some()
    }

    /// Renders this overlay entry, if it owns portal content.
    #[must_use]
    pub fn render(&self, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        self.renderer.as_ref().map(|renderer| renderer(window, cx))
    }
}

/// Information returned when an overlay closes.
#[derive(Clone)]
pub struct ClosedOverlay {
    /// The overlay that was closed.
    pub entry: OverlayEntry,
    /// The close reason that produced the closure.
    pub reason: CloseReason,
}

impl ClosedOverlay {
    /// Restores focus to the configured handle, if this overlay captured one.
    pub fn restore_focus(&self, window: &mut Window, cx: &mut App) -> bool {
        let Some(handle) = self.entry.restore_focus_to() else {
            return false;
        };
        handle.focus(window, cx);
        true
    }
}

/// Supported overlay categories.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayKind {
    /// Tooltip overlay.
    Tooltip,
    /// Hover card overlay.
    HoverCard,
    /// Dropdown overlay.
    Dropdown,
    /// Popover overlay.
    Popover,
    /// Sheet overlay.
    Sheet,
    /// Dialog overlay.
    Dialog,
    /// Notification overlay.
    Notification,
}

/// Reasons an overlay may close.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseReason {
    /// Closed by confirming an action.
    Confirm,
    /// Closed by explicit cancellation.
    Cancel,
    /// Closed by the escape key.
    Escape,
    /// Closed by clicking outside.
    OutsideClick,
    /// Closed programmatically.
    Programmatic,
    /// Closed because its window was closed.
    WindowClosed,
}

#[cfg(test)]
mod tests {
    use super::{CloseReason, OverlayKind, OverlayManager, OverlayOptions};
    use gpui::{WindowId, div};

    #[test]
    fn manages_overlay_stack_order() {
        let mut manager = OverlayManager::default();
        let first = manager.open(OverlayKind::Popover);
        let second = manager.open(OverlayKind::Dialog);

        assert_eq!(manager.len(), 2);
        assert_eq!(
            manager.top().map(|entry| entry.kind),
            Some(OverlayKind::Dialog)
        );

        let closed = manager
            .close(second, CloseReason::Escape)
            .expect("overlay should close");
        assert_eq!(closed.reason, CloseReason::Escape);
        assert_eq!(closed.entry.kind, OverlayKind::Dialog);
        assert_eq!(manager.top().map(|entry| entry.id), Some(first));
    }

    #[test]
    fn dismisses_topmost_dismissible_overlay() {
        let mut manager = OverlayManager::default();
        let first = manager.open_with_options(
            OverlayKind::Popover,
            OverlayOptions::new().dismissible(true),
        );
        let second = manager.open(OverlayKind::Dialog);

        let closed = manager
            .dismiss_top(CloseReason::OutsideClick)
            .expect("dismissible overlay should close");

        assert_eq!(closed.entry.id, first);
        assert_eq!(manager.top().map(|entry| entry.id), Some(second));
    }

    #[test]
    fn returns_renderable_entries_in_priority_order() {
        let mut manager = OverlayManager::default();
        let high = manager.open_rendered(
            OverlayKind::Dialog,
            OverlayOptions::new().priority(10),
            |_, _| div(),
        );
        let low = manager.open_rendered(
            OverlayKind::Tooltip,
            OverlayOptions::new().priority(-1),
            |_, _| div(),
        );

        let entries = manager.render_entries();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, low);
        assert_eq!(entries[1].id, high);
    }

    #[test]
    fn closes_overlays_for_window() {
        let mut manager = OverlayManager::default();
        let window_id = WindowId::from(42);
        let scoped = manager.open_with_options(
            OverlayKind::Popover,
            OverlayOptions::new().window_id(window_id),
        );
        let other = manager.open(OverlayKind::Notification);

        let closed = manager.close_window(window_id);

        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].entry.id, scoped);
        assert_eq!(closed[0].reason, CloseReason::WindowClosed);
        assert_eq!(manager.top().map(|entry| entry.id), Some(other));
    }

    #[test]
    fn reports_topmost_focus_trap() {
        let mut manager = OverlayManager::default();
        let first = manager.open_with_options(
            OverlayKind::Popover,
            OverlayOptions::new().traps_focus(true),
        );
        let second = manager.open(OverlayKind::Notification);

        assert_eq!(
            manager.active_focus_trap().map(|entry| entry.id),
            Some(first)
        );

        manager.close(second, CloseReason::Programmatic);
        assert_eq!(
            manager.active_focus_trap().map(|entry| entry.id),
            Some(first)
        );
    }
}
