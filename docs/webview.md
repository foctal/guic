# WebView

GUIC provides experimental WebView support through the separate
`guic-webview` crate.

Enable the `webview` feature on `guic` for login flows, embedded documentation,
HTML previews, or other content that needs a browser engine.

## Caveats

- The platform WebView is rendered above GPUI content within its bounds.
- It should be treated as an isolated surface, not a seamless composited GPUI
  primitive.
- Behavior depends on the underlying platform WebView implementation.
- On Windows, the example sets `WindowOptions::windows_composition` to
  `gpui::WindowsCompositionMode::RedirectionBitmap` for the WebView host window.
  Other application windows can retain their default rendering mode.

## Linux example

Run `cargo run -p guic-example-webview` from an X11 desktop or a Wayland desktop
with XWayland enabled. The example uses X11 for both GPUI and GTK, including
when launched from a Wayland session or with `GDK_BACKEND=wayland` in the parent
environment. `DISPLAY` must point to a working X server.
Native Wayland embedding into a GPUI window is not supported by this example.
Other GUIC examples continue to select their normal desktop backend.

The sample defaults `WEBKIT_DISABLE_DMABUF_RENDERER` to `1` on Linux because
WebKit's DMA-BUF renderer can produce a blank foreign child window on virtual
GPUs. An explicit environment value is preserved; use `0` to test DMA-BUF on
your hardware. This setting affects only the example process, not the library.

An optional URL argument lets you test local content without relying on a
network connection:

```sh
cargo run -p guic-example-webview -- file:///absolute/path/to/page.html
```

With GTK 3.24.41 and WebKitGTK 2.52.6, forcing `GDK_SCALE=2` can emit a
`_gdk_frame_clock_freeze` warning inside Wry's foreign-window `show_all` call.
This is a known GTK/Wry limitation. Do not enable
`G_DEBUG=fatal-criticals` for that configuration.

The Linux host must initialize GTK before constructing the WebView, attach the
WebView to the actual GPUI parent window, and process GTK events on the main
thread alongside GPUI's event loop. The example's `src/linux.rs` demonstrates
these steps, including adapting GPUI's XCB window ID to the Xlib handle required
by Wry. Creating an unattached `gtk::Fixed` does not embed a WebView in GPUI.

Keep the WebView's lifetime inside the parent window's lifetime. A native
Wayland integration would require a GTK-owned host container and additional
platform integration; initializing GTK alone does not provide that container.
