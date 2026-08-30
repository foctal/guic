# Getting Started

Follow the [installation guide](installation.md), then initialize GUIC before
opening the first window. Application views should be hosted in
[`guic::core::Root`](https://docs.rs/guic/latest/guic/core/struct.Root.html) so
theme, focus, overlay, and command services share the window lifecycle.

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
