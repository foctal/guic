# Getting Started

Follow the [installation guide](installation.md), then use `guic::application()`
and `guic::open_window()` to create the application and its first window.
The window helper initializes GUIC and hosts the view in
[`guic::core::Root`](https://docs.rs/guic/latest/guic/core/struct.Root.html), so
theme, focus, overlay, and command services share the window lifecycle.
Applications opening GPUI windows directly must initialize GUIC and construct
`Root` themselves.

A complete minimal application is available in
[`examples/hello_world`](../examples/hello_world/src/main.rs).

```bash
cargo run -p guic-example-hello-world
```

To explore the component set and themes, run the gallery:

```bash
cargo run -p guic-sample-component-gallery
```

Next, see [Components](components.md), [Theming](theming.md), and
[Application Commands](commands.md).
