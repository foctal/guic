//! Experimental WebView support for GUIC.
//!
//! The WebView is rendered by the native platform WebView implementation through
//! `wry`, so it is visually layered above GPUI content within its bounds.
//! This makes it best suited for separate windows, modal surfaces, or isolated
//! overlay regions.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::rc::Rc;

use gpui::{
    App, Bounds, ContentMask, DismissEvent, Element, ElementId, Entity, EventEmitter, FocusHandle,
    Focusable, GlobalElementId, Hitbox, InteractiveElement, IntoElement, LayoutId, MouseDownEvent,
    ParentElement as _, Pixels, Render, Size, Style, Styled as _, Window, canvas, div,
};
use wry::{
    Rect,
    dpi::{self, LogicalSize},
};

/// Errors returned by GUIC's WebView wrapper.
#[derive(Debug, thiserror::Error)]
pub enum WebViewError {
    /// An operation on the underlying `wry::WebView` failed.
    #[error(transparent)]
    Wry(#[from] wry::Error),
}

/// A WebView entity backed by `wry`.
///
/// The API is intentionally small so platform WebView behavior remains isolated
/// from GUIC's native component stack.
pub struct WebView {
    focus_handle: FocusHandle,
    webview: Rc<wry::WebView>,
    visible: bool,
    bounds: Bounds<Pixels>,
}

impl Drop for WebView {
    fn drop(&mut self) {
        if let Err(error) = self.hide() {
            tracing::warn!("failed to hide webview during drop: {error}");
        }
    }
}

impl WebView {
    /// Creates a new WebView from a native `wry::WebView`.
    pub fn new(webview: wry::WebView, cx: &mut App) -> Self {
        if let Err(error) = webview.set_bounds(Rect::default()) {
            tracing::warn!("failed to initialize webview bounds: {error}");
        }

        Self {
            focus_handle: cx.focus_handle(),
            visible: true,
            bounds: Bounds::default(),
            webview: Rc::new(webview),
        }
    }

    /// Shows the WebView.
    pub fn show(&mut self) -> Result<(), WebViewError> {
        self.webview.set_visible(true)?;
        self.visible = true;
        Ok(())
    }

    /// Hides the WebView.
    pub fn hide(&mut self) -> Result<(), WebViewError> {
        self.webview.focus_parent()?;
        self.webview.set_visible(false)?;
        self.visible = false;
        Ok(())
    }

    /// Returns whether the WebView is visible.
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Returns the current layout bounds of the WebView.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    /// Navigates back in history.
    pub fn back(&mut self) -> Result<(), WebViewError> {
        Ok(self.webview.evaluate_script("history.back();")?)
    }

    /// Loads a URL in the WebView.
    pub fn load_url(&mut self, url: &str) -> Result<(), WebViewError> {
        self.webview.load_url(url)?;
        Ok(())
    }
}

impl Focusable for WebView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<DismissEvent> for WebView {}

impl Render for WebView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();

        div()
            .track_focus(&self.focus_handle)
            .size_full()
            .child({
                let view = cx.entity().clone();
                canvas(
                    move |bounds, _, cx| view.update(cx, |this, _| this.bounds = bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full()
            })
            .child(WebViewElement::new(self.webview.clone(), view, window, cx))
    }
}

/// An element that positions a platform WebView over GPUI content.
pub struct WebViewElement {
    parent: Entity<WebView>,
    view: Rc<wry::WebView>,
}

impl WebViewElement {
    /// Creates a new WebView element from a native `wry::WebView`.
    pub fn new(
        view: Rc<wry::WebView>,
        parent: Entity<WebView>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self {
        Self { view, parent }
    }
}

impl IntoElement for WebViewElement {
    type Element = WebViewElement;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for WebViewElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::View(self.parent.entity_id()))
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = Style {
            size: Size::full(),
            flex_shrink: 1.,
            ..Default::default()
        };

        let id = window.request_layout(style, [], cx);
        (id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        if !self.parent.read(cx).visible() {
            return None;
        }

        if let Err(error) = self.view.set_bounds(Rect {
            size: dpi::Size::Logical(LogicalSize {
                width: bounds.size.width.into(),
                height: bounds.size.height.into(),
            }),
            position: dpi::Position::Logical(dpi::LogicalPosition::new(
                bounds.origin.x.into(),
                bounds.origin.y.into(),
            )),
        }) {
            tracing::warn!("failed to update webview bounds: {error}");
        }

        Some(window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal))
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        _: &mut App,
    ) {
        let bounds = hitbox.clone().map(|hitbox| hitbox.bounds).unwrap_or(bounds);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            let webview = self.view.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, _, _, _| {
                if !bounds.contains(&event.position)
                    && let Err(error) = webview.focus_parent()
                {
                    tracing::warn!("failed to blur webview focus: {error}");
                }
            });
        });
    }
}
