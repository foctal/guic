# WebView

GUIC provides experimental WebView support through the separate
`guic-webview` crate.

## Why separate?

The main toolkit is native-first and should not require WebView support in
default builds. Keeping WebView optional preserves that product direction while
still supporting practical hybrid integration needs.

## Intended use

- OAuth and login flows
- Embedded documentation
- HTML preview
- Legacy web surface integration

## Caveats

- The platform WebView is rendered above GPUI content within its bounds.
- It should be treated as an isolated surface, not a seamless composited GPUI
  primitive.
- Behavior depends on the underlying platform WebView implementation.
- The current implementation uses upstream `wry`.
- On Windows, the example currently sets `GPUI_DISABLE_DIRECT_COMPOSITION=true`
  before app startup as a workaround for compositor conflicts.

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

The example prints page-load start and finish events to stderr. Check both the
rendered page and input/clipboard behavior; a finished load alone does not
prove that the native child window was painted.

For a repeatable native smoke check, open the included fixture:

```sh
cargo run -p guic-example-webview -- "file://$PWD/examples/webview/tests/fixtures/smoke.html"
```

Check that resizing updates the viewport dimensions, Ctrl+A/C/X/V and Tab move
text between the two inputs, the increment button updates its counter, links
scroll to the destination and back, and closing the window exits successfully.
Run this from X11 and from a Wayland session with XWayland. The automated
`linux_startup` tests also verify graceful failure when X11 is unavailable.

With GTK 3.24.41 and WebKitGTK 2.52.6, forcing `GDK_SCALE=2` can emit a
`_gdk_frame_clock_freeze` warning inside Wry's foreign-window `show_all` call.
Rendering, input, resizing, and normal shutdown have been checked at that scale;
the upstream GTK/Wry warning remains a known limitation. Do not enable
`G_DEBUG=fatal-criticals` for that configuration.

The Linux host must initialize GTK before constructing the WebView, attach the
WebView to the actual GPUI parent window, and process GTK events on the main
thread alongside GPUI's event loop. The example's `src/linux.rs` demonstrates
these steps, including adapting GPUI's XCB window ID to the Xlib handle required
by Wry. Creating an unattached `gtk::Fixed` does not embed a WebView in GPUI.

Keep the WebView's lifetime inside the parent window's lifetime. A native
Wayland integration would require a GTK-owned host container and additional
platform integration; initializing GTK alone does not provide that container.
