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
- The current implementation uses the `lb-wry` crate package for compatibility
  with the reference GPUI ecosystem. GUIC does not currently depend on a
  fork-only public API, so upstream `wry` remains a future migration target
  once compatibility is confirmed.
- On Windows, the example currently sets `GPUI_DISABLE_DIRECT_COMPOSITION=true`
  before app startup as a workaround for compositor conflicts.
