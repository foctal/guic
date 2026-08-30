# NOTICE

GUIC
Copyright 2026 foctal

Licensed under the Apache License, Version 2.0. The full license text is
available in `LICENSE`.

GUIC is an original project that uses `gpui-component` as a technical reference
for architecture, GPUI usage patterns, and component scope research.

## Referenced projects

- `gpui-component`
  - Repository: <https://github.com/longbridge/gpui-component>
  - Purpose: reference for GPUI component implementation patterns and workspace
    organization tradeoffs
  - Additional note: its optional `gpui-wry` crate informed GUIC's separate
    optional WebView support strategy
- `guic-gpui`
  - Repository: <https://github.com/foctal/guic-gpui>
  - Purpose: rendering and application foundation
  - Origin: independent fork of Zed's GPUI
  - Bundled fonts: IBM Plex Sans and Lilex under SIL Open Font License 1.1

## Attribution policy

- Project branding, product identity, and documentation wording are not copied.
- Public API decisions are intentionally redesigned for GUIC.
- If code is copied or closely adapted in future changes, the relevant file must
  preserve the required license notice and this document must be updated with a
  precise attribution note.
