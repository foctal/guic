# Markdown

## Purpose

Render Markdown documents and lightweight embedded HTML previews inside GUIC
layouts.

## Import

```rust
use guic::prelude::{HtmlFragment, Markdown};
```

## Basic Usage

```rust
Markdown::new("# Release Notes\n\n- Stable tokens\n- Component gallery")
```

```rust
HtmlFragment::new("<p><strong>Preview</strong> content.</p>")
```

## Mixed Content

```rust
Markdown::new(
    "Status with **strong text** and [guide](https://example.com)\n\n\
     | Area | State |\n\
     | --- | --- |\n\
     | Tokens | Stable |\n\n\
     ---\n\n\
     <div>Embedded HTML preview</div>",
)
```

## Notes

The current renderer focuses on production-oriented preview use cases such as
release notes, status panes, and embedded help content. It covers headings,
paragraphs, lists, tables, block quotes, thematic rules, fenced code blocks,
inline formatting markers, and lightweight HTML fragments.

## HTML Boundaries

`Markdown` and `HtmlFragment` do not embed a browser or execute HTML. HTML is
converted into inert preview text: tags and attributes are stripped, common and
numeric entities are decoded, and unknown entities are preserved as text.
Content inside `script`, `style`, `iframe`, `object`, `embed`, `svg`, and `math`
elements is omitted entirely. Use `guic-webview` for trusted rich HTML that must
retain browser semantics.
