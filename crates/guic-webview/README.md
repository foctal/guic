# guic-webview

Experimental WebView support for GUIC, based on [Wry](https://github.com/tauri-apps/wry).

## Status

This crate is optional and non-default. It is intended for cases where an
application needs embedded web content without making WebView a core dependency
of GUIC itself.

- The native WebView is layered above GPUI content within its bounds.
- It is best suited for separate windows, dialogs, sheets, or isolated panels.
- It should be treated as an integration utility, not as the primary UI model.
- The implementation uses the upstream `wry` crate.

The current wrapper relies on:

- `WebView::set_bounds`
- `WebView::set_visible`
- `WebView::focus_parent`
- `WebView::evaluate_script`
- `WebView::load_url`

Validate WebView behavior on every target platform used by the application.

On Linux, the GPUI example uses X11 (or XWayland on a Wayland desktop), initializes
GTK, and pumps its event loop on the main thread. Native Wayland embedding into
a GPUI-owned window is not supported by the example. See
[Linux host integration](../../docs/webview.md#linux-example).

## Typical use cases

- OAuth or SSO flows
- Embedded documentation
- HTML preview
- Transitional hybrid application surfaces
