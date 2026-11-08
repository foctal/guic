# Markdown

Enable the `markdown` feature on `guic`.

Render Markdown documents and lightweight embedded HTML previews inside GUIC
layouts.

```rust
use guic::prelude::{HtmlFragment, Markdown};

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

The renderer supports headings, paragraphs, lists, tables, block quotes,
thematic rules, fenced code blocks,
styled strong/emphasized/struck text, monospace inline code, clickable links,
and lightweight HTML fragments. Escaped Markdown punctuation remains literal.
Set `.id(...)` when rendering multiple documents in one view. Use
`.on_link_click(...)` to route links inside the application; the default handler
opens only HTTP, HTTPS, and mail links. Other schemes remain inert.

## HTML Boundaries

`Markdown` and `HtmlFragment` do not embed a browser or execute HTML. HTML is
converted into inert preview text: tags and attributes are stripped, common and
numeric entities are decoded, and unknown entities are preserved as text.
Content inside `script`, `style`, `iframe`, `object`, `embed`, `svg`, and `math`
elements is omitted entirely. Use `guic-webview` for trusted rich HTML that must
retain browser semantics.
