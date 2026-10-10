use raw_window_handle::{HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle};
use std::time::Duration;

/// GPUI owns a non-GTK window, so Wry's Linux child-window API requires X11.
/// On Wayland desktops both libraries must connect to the same XWayland server.
pub fn init() -> Result<(), String> {
    if std::env::var_os("DISPLAY").is_none_or(|display| display.is_empty()) {
        return Err("an X11 display is required; enable XWayland on Wayland desktops".into());
    }
    // SAFETY: Called at process startup, before GTK, GPUI, or worker threads start.
    unsafe {
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::set_var("GDK_BACKEND", "x11");
        // The DMA-BUF renderer can leave foreign X11 child windows blank on
        // virtual GPUs. Prefer the compatible path for this example, while
        // preserving an explicit renderer choice from the host environment.
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
    gtk::gdk::set_allowed_backends("x11");
    gtk::init().map_err(|error| format!("could not initialize GTK on X11: {error}"))
}

pub fn build_child(
    builder: wry::WebViewBuilder<'_>,
    window: &gpui::Window,
) -> wry::Result<wry::WebView> {
    let parent = HasWindowHandle::window_handle(window)?;
    let xlib = match parent.as_raw() {
        RawWindowHandle::Xcb(handle) => {
            let mut xlib = XlibWindowHandle::new(handle.window.get().into());
            xlib.visual_id = handle.visual_id.map_or(0, |id| id.get().into());
            xlib
        }
        RawWindowHandle::Xlib(handle) => handle,
        _ => return Err(wry::Error::UnsupportedWindowHandle),
    };
    // SAFETY: XCB and Xlib use the same server-side XID. The borrowed GPUI
    // window stays alive throughout construction; Wry creates its own child
    // and uses the GTK connection to the same DISPLAY initialized above.
    let parent = unsafe { WindowHandle::borrow_raw(RawWindowHandle::Xlib(xlib)) };
    builder.build_as_child(&parent)
}

pub fn run_gtk_loop(cx: &mut gpui::App) {
    // GPUI runs its own event loop. Pump GTK on GPUI's foreground thread so
    // WebKit can load pages, process input, and deliver its asynchronous callbacks.
    cx.spawn(async move |cx| {
        loop {
            // Bound each batch so a busy WebView cannot starve the GPUI loop.
            for _ in 0..64 {
                if !gtk::events_pending() {
                    break;
                }
                gtk::main_iteration_do(false);
            }
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
        }
    })
    .detach();
}
