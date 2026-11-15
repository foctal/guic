# Platform Support

GUIC targets macOS, Windows, and Linux. The preview APIs and platform
limitations are described in [API stability](api-stability.md).

## Continuous integration

| Runner | Checks |
| --- | --- |
| macOS | Workspace library tests and CLI/build-helper tests |
| Windows | Workspace library tests, CLI/build-helper and sidecar cleanup tests, and packaging example builds |
| Ubuntu | Workspace tests, formatting, strict full/minimal rustdoc, documentation examples, and theme schema |

The [CI workflow](../.github/workflows/ci.yml) defines the exact commands.

## Linux

Follow the [Linux installation instructions](installation.md#linux) for native
dependencies. Native applications can use X11 or Wayland; the WebView example
requires X11 or XWayland. See [WebView host integration](webview.md#linux-example)
for details.
