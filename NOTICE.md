# NOTICE

GUIC
Copyright 2026 foctal

Licensed under the Apache License, Version 2.0. The full license text is
available in `LICENSE`.

## Packaging fork

The `guic-packager`, `guic-packager-updater`, `guic-packager-utils`, and
`guic-packager-resource-resolver` crates are derived from
[cargo-packager](https://github.com/crabnebula-dev/cargo-packager), by CrabNebula
Ltd. and contributors, including code from Tauri Programme within The Commons
Conservancy. These crates retain Apache-2.0 OR MIT licensing. Original copyright
notices and both license texts are included in each crate; `UPSTREAM.md` records
provenance.

## Referenced projects

[gpui-component](https://github.com/longbridge/gpui-component) served as a
reference for component implementation and workspace organization. Its optional
`gpui-wry` crate informed GUIC's WebView integration.

[guic-gpui](https://github.com/foctal/guic-gpui) is GUIC's rendering and
application runtime, an independent fork of Zed's GPUI. It bundles IBM Plex Sans
and Lilex under the SIL Open Font License 1.1.

Code copied or adapted from other projects must retain its required license
notices and attribution. Dependency notices are also included in
`crates/cargo-guic/THIRD-PARTY-LICENSES.txt` and
`crates/guic-native/THIRD-PARTY-LICENSES.txt`.
