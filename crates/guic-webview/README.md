# guic-webview

Experimental WebView integration for GUIC, based on
[Wry](https://github.com/tauri-apps/wry). Enable the `webview` feature on `guic`
or depend on this crate directly.

The native WebView is layered above GPUI content within its bounds. Use it for
isolated web content such as login flows, documentation, or HTML previews.
The wrapper supports bounds, visibility, focus, navigation, and script evaluation.

The Linux example requires X11 or XWayland and a GTK event loop on the main
thread. See the [integration guide](https://github.com/foctal/guic/blob/main/docs/webview.md)
for platform requirements and examples.
