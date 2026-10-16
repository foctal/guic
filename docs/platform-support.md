# Platform Support

GUIC targets native desktop applications on macOS, Windows, and Linux. Platform
support is release-blocking for the umbrella crate, core runtime, tokens,
components, assets, icons, macros, examples, and samples.

## Supported Matrix

| Platform | CI validation | Manual release validation | Notes |
| --- | --- | --- | --- |
| macOS latest | `cargo test --workspace --all-features --lib` | Build, launch, keyboard, mouse, focus, overlay, text input, clipboard, high-contrast review where available | Uses the pinned `guic-gpui` release. |
| Windows latest | `cargo test --workspace --all-features --lib` | Build, launch, keyboard, mouse, focus, overlay, text input, clipboard, high-contrast review where available | Uses the pinned `guic-gpui` release. |
| Ubuntu latest | `cargo fmt`, `cargo test --workspace --all-features`, `cargo doc --workspace --all-features --no-deps`, theme schema check | Build, launch, keyboard, mouse, focus, overlay, text input, clipboard, font fallback, Linux desktop backend review | Linux development dependencies are installed by `scripts/install-linux-deps.sh`. |

## Linux Desktop Assumptions

Linux support assumes a modern desktop session with the GPUI runtime
dependencies installed. The repository helper installs the packages currently
needed for local development and CI smoke validation:

```sh
./scripts/install-linux-deps.sh
```

Applications that enable `guic-webview` also require the WebKitGTK packages
installed by that helper.

## WebView Support

`guic-webview` is optional and depends on upstream `wry`. WebView behavior must
be manually reviewed on macOS, Windows, and Linux before any stable release.
The Linux WebView example requires X11 or XWayland; it does not embed into a
native Wayland GPUI window. See [WebView](webview.md#linux-example) for the host
initialization and event-loop requirements.
Release notes should record the tested platforms, the pinned `guic-gpui` release, and
the `wry` version.

## Release Bar

Before a release is considered platform-ready, maintainers should validate:

- Workspace build and tests on the supported matrix.
- Keyboard input, focus transitions, mouse input, and overlay dismissal.
- Text editing basics, clipboard interoperability, and IME behavior.
- Font fallback and text measurement in representative UI surfaces.
- Accessibility and high-contrast behavior where platform hooks allow.
- Optional WebView examples on every supported platform when `webview` is in
  scope for the release.

Use the [platform smoke checklist](platform-smoke.md) and retain results in the
release record. Automated CI does not replace this physical-system release gate.
