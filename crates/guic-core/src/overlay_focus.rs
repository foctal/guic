use gpui::{
    AnyElement, App, Bounds, Element, ElementId, FocusHandle, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, SharedString, Window,
};
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
struct Lifecycle {
    open: bool,
    previous: Option<FocusHandle>,
    generation: Rc<Cell<u64>>,
}

/// Mount-aware focus for a controlled overlay, without adding layout geometry.
/// Keep this wrapper rendered with `open(false)` when closing to restore focus.
/// Pending focus is canceled if the wrapper closes or is removed before mounting.
pub struct OverlayFocus {
    id: SharedString,
    child: AnyElement,
    open: bool,
    target: Option<FocusHandle>,
    restore: bool,
}

impl OverlayFocus {
    /// Wraps overlay content in a stable focus lifecycle.
    pub fn new(id: impl Into<SharedString>, child: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            child: child.into_any_element(),
            open: true,
            target: None,
            restore: true,
        }
    }
    /// Sets controlled visibility for focus lifecycle transitions.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    /// Requests focus after the child has mounted.
    #[must_use]
    pub fn autofocus(mut self, target: Option<FocusHandle>) -> Self {
        self.target = target;
        self
    }
    /// Enables or disables restoration to focus captured on opening.
    #[must_use]
    pub fn restore_focus(mut self, restore: bool) -> Self {
        self.restore = restore;
        self
    }
}

impl IntoElement for OverlayFocus {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for OverlayFocus {
    type RequestLayoutState = ();
    type PrepaintState = Option<(FocusHandle, Rc<Cell<u64>>, u64)>;
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone().into())
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let pending = id.and_then(|id| {
            window.with_element_state(id, |state: Option<Lifecycle>, window| {
                let mut state = state.unwrap_or_default();
                let target = if state.open != self.open {
                    state.generation.set(state.generation.get().wrapping_add(1));
                    state.open = self.open;
                    if self.open {
                        state.previous = if self.restore {
                            window.focused(cx)
                        } else {
                            None
                        };
                        self.target.clone()
                    } else if self.restore {
                        state.previous.take()
                    } else {
                        state.previous = None;
                        None
                    }
                } else {
                    None
                };
                let pending =
                    target.map(|target| (target, state.generation.clone(), state.generation.get()));
                (pending, state)
            })
        });
        self.child.prepaint(window, cx);
        pending
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        pending: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
        if let Some((target, generation, expected)) = pending.take() {
            let generation = Rc::downgrade(&generation);
            window.on_next_frame(move |window, cx| {
                if generation
                    .upgrade()
                    .is_some_and(|generation| generation.get() == expected)
                {
                    target.focus(window, cx);
                }
            });
        }
    }
}
