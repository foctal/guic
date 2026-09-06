//! Interaction without an additional layout box.
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId, Hitbox,
    InspectorElementId, InteractiveElement, Interactivity, IntoElement, LayoutId, Pixels, Window,
};

type OverlayBuilder = Box<dyn FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> AnyElement>;

pub(crate) struct BehavioralTrigger {
    child: AnyElement,
    overlay: Option<AnyElement>,
    overlay_builder: Option<OverlayBuilder>,
    interactivity: Interactivity,
}

impl BehavioralTrigger {
    pub(crate) fn new(child: impl IntoElement) -> Self {
        Self {
            child: child.into_any_element(),
            overlay: None,
            overlay_builder: None,
            interactivity: Interactivity::new(),
        }
    }

    pub(crate) fn overlay(mut self, overlay: Option<AnyElement>) -> Self {
        self.overlay = overlay;
        self
    }
    pub(crate) fn anchored_overlay(
        mut self,
        builder: impl FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.overlay_builder = Some(Box::new(builder));
        self
    }
}

impl InteractiveElement for BehavioralTrigger {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

impl IntoElement for BehavioralTrigger {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for BehavioralTrigger {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;
    fn id(&self) -> Option<ElementId> {
        self.interactivity.element_id.clone()
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let child = &mut self.child;
        let layout =
            self.interactivity
                .request_layout(id, inspector, window, cx, |_, window, cx| {
                    child.request_layout(window, cx)
                });
        (layout, ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Hitbox> {
        if let Some(builder) = self.overlay_builder.take() {
            self.overlay = Some(builder(bounds, window, cx));
        }
        let child = &mut self.child;
        let overlay = &mut self.overlay;
        self.interactivity.prepaint(
            id,
            inspector,
            bounds,
            bounds.size,
            window,
            cx,
            |_, _, hitbox, window, cx| {
                child.prepaint(window, cx);
                if let Some(overlay) = overlay {
                    // The overlay owns viewport coordinates and never enters the trigger's layout tree.
                    overlay.layout_as_root(
                        window.viewport_size().map(AvailableSpace::Definite),
                        window,
                        cx,
                    );
                    overlay.prepaint_at(Default::default(), window, cx);
                }
                hitbox
            },
        )
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let child = &mut self.child;
        let overlay = &mut self.overlay;
        self.interactivity.paint(
            id,
            inspector,
            bounds,
            hitbox.as_ref(),
            window,
            cx,
            |_, window, cx| {
                child.paint(window, cx);
                if let Some(overlay) = overlay {
                    overlay.paint(window, cx);
                }
            },
        );
    }
}
