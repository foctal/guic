# Platform Support

GUIC targets macOS, Windows, and Linux. The preview APIs and platform
limitations are described in [API stability](api-stability.md).

## Continuous integration

| Runner | Checks |
| --- | --- |
| macOS | Workspace library tests and CLI/build-helper tests |
| Windows | Workspace library tests, CLI/build-helper tests, and packaging example builds |
| Ubuntu | Workspace tests, formatting, strict full/minimal rustdoc, documentation examples, and theme schema |

The [CI workflow](../.github/workflows/ci.yml) defines the exact commands.
Native input, accessibility, rendering, and installed application behavior
require the [platform smoke checks](platform-smoke.md).

## Linux

Install the development dependencies from the repository root:

```sh
./scripts/install-linux-deps.sh
```

The helper includes WebKitGTK packages for the optional WebView integration.
Applications without `webview` do not need WebKit at runtime. Native applications
can use X11 or Wayland; the WebView example requires X11 or XWayland.
See [WebView host integration](webview.md#linux-example) for details.
