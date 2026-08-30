use crate::Label;
use gpui::{
    AnyElement, App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _,
    Window, div, px,
};
use guic_tokens::Theme;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// A rendered Markdown document surface.
///
/// The current implementation prioritizes predictable block rendering for
/// headings, paragraphs, lists, block quotes, fenced code, and inline HTML
/// snippets commonly used in release notes and embedded previews.
///
/// # Example
///
/// ```no_run
/// use guic_components::Markdown;
///
/// let doc = Markdown::new("# GUIC\n\n- Tokens\n- Components\n- Runtime");
/// ```
#[derive(gpui::IntoElement)]
pub struct Markdown {
    source: SharedString,
}

impl Markdown {
    /// Creates a rendered Markdown surface from a source string.
    #[must_use]
    pub fn new(source: impl Into<SharedString>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

impl RenderOnce for Markdown {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let blocks = parse_markdown(self.source.as_ref());
        let children = blocks
            .into_iter()
            .map(|block| render_block(block, theme))
            .collect::<Vec<_>>();

        div()
            .w_full()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .children(children)
    }
}

/// A lightweight HTML fragment surface intended for embedded previews.
///
/// # Example
///
/// ```no_run
/// use guic_components::HtmlFragment;
///
/// let fragment = HtmlFragment::new("<p><strong>Ready</strong> for preview.</p>");
/// ```
#[derive(gpui::IntoElement)]
pub struct HtmlFragment {
    source: SharedString,
}

impl HtmlFragment {
    /// Creates a new HTML fragment preview surface.
    #[must_use]
    pub fn new(source: impl Into<SharedString>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

impl RenderOnce for HtmlFragment {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        div()
            .w_full()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.secondary().opacity(0.08))
            .p_4()
            .child(Label::new(html_preview_text(self.source.as_ref())).muted(true))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MarkdownBlock {
    Heading {
        level: u8,
        text: String,
    },
    Paragraph(String),
    BulletList(Vec<String>),
    OrderedList(Vec<String>),
    Quote(String),
    Rule,
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Html(String),
}

fn render_block(block: MarkdownBlock, theme: &Theme) -> AnyElement {
    match block {
        MarkdownBlock::Heading { level, text } => {
            let size = match level {
                1 => theme.typography.text_lg + 6.0,
                2 => theme.typography.text_lg + 2.0,
                _ => theme.typography.text_lg,
            };

            div()
                .text_size(px(size))
                .text_color(theme.foreground())
                .child(text)
                .into_any_element()
        }
        MarkdownBlock::Paragraph(text) => Label::new(text).into_any_element(),
        MarkdownBlock::BulletList(items) => div()
            .flex()
            .flex_col()
            .gap_2()
            .children(items.into_iter().map(|item| {
                div()
                    .flex()
                    .gap_2()
                    .items_start()
                    .child(div().pt_1().text_color(theme.primary()).child("-"))
                    .child(Label::new(item))
                    .into_any_element()
            }))
            .into_any_element(),
        MarkdownBlock::OrderedList(items) => div()
            .flex()
            .flex_col()
            .gap_2()
            .children(items.into_iter().enumerate().map(|(index, item)| {
                div()
                    .flex()
                    .gap_2()
                    .items_start()
                    .child(
                        div()
                            .text_color(theme.primary())
                            .child(format!("{}.", index + 1)),
                    )
                    .child(Label::new(item))
                    .into_any_element()
            }))
            .into_any_element(),
        MarkdownBlock::Quote(text) => div()
            .pl_3()
            .border_l_2()
            .border_color(theme.primary())
            .child(Label::new(text).muted(true))
            .into_any_element(),
        MarkdownBlock::Rule => div()
            .w_full()
            .h(px(1.0))
            .bg(theme.border())
            .into_any_element(),
        MarkdownBlock::CodeBlock { language, code } => {
            let mut root = div()
                .rounded(px(theme.radius.md))
                .border_1()
                .border_color(theme.border())
                .bg(theme.secondary().opacity(0.18))
                .p_3()
                .flex()
                .flex_col()
                .gap_2();
            if let Some(language) = language
                && !language.is_empty()
            {
                root = root.child(Label::new(language).muted(true));
            }
            root.child(div().text_color(theme.foreground()).child(code))
                .into_any_element()
        }
        MarkdownBlock::Table { headers, rows } => {
            let mut table = div()
                .w_full()
                .rounded(px(theme.radius.md))
                .border_1()
                .border_color(theme.border())
                .overflow_hidden()
                .flex()
                .flex_col();

            if !headers.is_empty() {
                table = table.child(render_table_row(headers, true, theme));
            }

            for row in rows {
                table = table.child(render_table_row(row, false, theme));
            }

            table.into_any_element()
        }
        MarkdownBlock::Html(html) => div()
            .rounded(px(theme.radius.md))
            .bg(theme.secondary().opacity(0.12))
            .p_3()
            .child(Label::new(html_preview_text(&html)).muted(true))
            .into_any_element(),
    }
}

fn render_table_row(cells: Vec<String>, header: bool, theme: &Theme) -> AnyElement {
    div()
        .w_full()
        .flex()
        .border_b_1()
        .border_color(theme.border())
        .bg(if header {
            theme.secondary().opacity(0.22)
        } else {
            theme.background()
        })
        .children(cells.into_iter().map(|cell| {
            div()
                .flex_1()
                .min_w(px(96.0))
                .px_3()
                .py_2()
                .border_r_1()
                .border_color(theme.border())
                .child(if header {
                    div()
                        .text_color(theme.foreground())
                        .child(cell)
                        .into_any_element()
                } else {
                    Label::new(cell).into_any_element()
                })
                .into_any_element()
        }))
        .into_any_element()
}

fn parse_markdown(source: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut current_text = String::new();
    let mut current_items = Vec::new();
    let mut table_headers = Vec::new();
    let mut table_rows = Vec::new();
    let mut current_table_row = Vec::new();
    let mut current_html = String::new();
    let mut current_code = String::new();
    let mut code_language = None;
    let mut in_paragraph = false;
    let mut in_heading = None;
    let mut in_quote = false;
    let mut list_kind = None;
    let mut in_html = false;
    let mut in_code = false;
    let mut in_table = false;
    let mut in_table_head = false;

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);

    for event in Parser::new_ext(source, options) {
        match event {
            Event::Start(Tag::Paragraph) => {
                if !in_table {
                    current_text.clear();
                    in_paragraph = true;
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if !in_table {
                    push_text_block(&mut blocks, &mut current_text, MarkdownContainer::Paragraph);
                    in_paragraph = false;
                }
            }
            Event::Start(Tag::Heading { level, .. }) => {
                current_text.clear();
                in_heading = Some(match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                });
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(level) = in_heading.take() {
                    blocks.push(MarkdownBlock::Heading {
                        level,
                        text: current_text.trim().to_owned(),
                    });
                }
                current_text.clear();
            }
            Event::Start(Tag::BlockQuote(_)) => {
                current_text.clear();
                in_quote = true;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                blocks.push(MarkdownBlock::Quote(current_text.trim().to_owned()));
                current_text.clear();
                in_quote = false;
            }
            Event::Start(Tag::List(start)) => {
                current_items.clear();
                list_kind = Some(start.is_some());
            }
            Event::End(TagEnd::List(_)) => {
                if let Some(ordered) = list_kind.take() {
                    if ordered {
                        blocks.push(MarkdownBlock::OrderedList(current_items.clone()));
                    } else {
                        blocks.push(MarkdownBlock::BulletList(current_items.clone()));
                    }
                }
                current_items.clear();
            }
            Event::Start(Tag::Item) => {
                current_text.clear();
            }
            Event::End(TagEnd::Item) => {
                current_items.push(current_text.trim().to_owned());
                current_text.clear();
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                current_code.clear();
                code_language = match kind {
                    CodeBlockKind::Indented => None,
                    CodeBlockKind::Fenced(language) => Some(language.to_string()),
                };
                in_code = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                blocks.push(MarkdownBlock::CodeBlock {
                    language: code_language.take(),
                    code: current_code.trim_end().to_owned(),
                });
                current_code.clear();
                in_code = false;
            }
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                in_table_head = false;
                table_headers.clear();
                table_rows.clear();
                current_table_row.clear();
                current_text.clear();
            }
            Event::End(TagEnd::Table) => {
                blocks.push(MarkdownBlock::Table {
                    headers: table_headers.clone(),
                    rows: table_rows.clone(),
                });
                in_table = false;
                in_table_head = false;
                table_headers.clear();
                table_rows.clear();
                current_table_row.clear();
                current_text.clear();
            }
            Event::Start(Tag::TableHead) => {
                in_table_head = true;
                current_table_row.clear();
            }
            Event::End(TagEnd::TableHead) => {
                if !current_table_row.is_empty() {
                    table_headers = current_table_row.clone();
                    current_table_row.clear();
                }
                in_table_head = false;
            }
            Event::Start(Tag::TableRow) => {
                current_table_row.clear();
            }
            Event::End(TagEnd::TableRow) => {
                if in_table_head {
                    table_headers = current_table_row.clone();
                } else if !current_table_row.is_empty() {
                    table_rows.push(current_table_row.clone());
                }
                current_table_row.clear();
            }
            Event::Start(Tag::TableCell) => {
                current_text.clear();
            }
            Event::End(TagEnd::TableCell) => {
                current_table_row.push(current_text.trim().to_owned());
                current_text.clear();
            }
            Event::Html(html) => {
                current_html.push_str(html.as_ref());
                current_html.push('\n');
                in_html = true;
            }
            Event::InlineHtml(html) => {
                current_text.push_str(html_preview_text(html.as_ref()).as_ref());
            }
            Event::End(TagEnd::HtmlBlock) => {
                if in_html {
                    blocks.push(MarkdownBlock::Html(current_html.trim().to_owned()));
                    current_html.clear();
                    in_html = false;
                }
            }
            Event::Text(text) => {
                if in_code {
                    current_code.push_str(text.as_ref());
                } else {
                    current_text.push_str(text.as_ref());
                }
            }
            Event::Code(code) => {
                current_text.push('`');
                current_text.push_str(code.as_ref());
                current_text.push('`');
            }
            Event::InlineMath(math) => {
                current_text.push('$');
                current_text.push_str(math.as_ref());
                current_text.push('$');
            }
            Event::DisplayMath(math) => {
                current_text.push_str("\n$$\n");
                current_text.push_str(math.as_ref());
                current_text.push_str("\n$$\n");
            }
            Event::Start(Tag::Emphasis) => current_text.push('*'),
            Event::End(TagEnd::Emphasis) => current_text.push('*'),
            Event::Start(Tag::Strong) => current_text.push_str("**"),
            Event::End(TagEnd::Strong) => current_text.push_str("**"),
            Event::Start(Tag::Strikethrough) => current_text.push_str("~~"),
            Event::End(TagEnd::Strikethrough) => current_text.push_str("~~"),
            Event::Start(Tag::Link { dest_url, .. }) => {
                current_text.push('[');
                current_text.push_str(dest_url.as_ref());
                current_text.push_str("] ");
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                current_text.push_str("[image ");
                current_text.push_str(dest_url.as_ref());
                current_text.push_str(": ");
            }
            Event::End(TagEnd::Image) => current_text.push(']'),
            Event::Rule => blocks.push(MarkdownBlock::Rule),
            Event::FootnoteReference(reference) => {
                current_text.push('[');
                current_text.push_str(reference.as_ref());
                current_text.push(']');
            }
            Event::SoftBreak | Event::HardBreak => current_text.push('\n'),
            _ => {
                if in_paragraph || in_heading.is_some() || in_quote || list_kind.is_some() {
                    continue;
                }
            }
        }
    }

    if !current_html.trim().is_empty() {
        blocks.push(MarkdownBlock::Html(current_html.trim().to_owned()));
    }

    blocks
}

enum MarkdownContainer {
    Paragraph,
}

fn push_text_block(
    blocks: &mut Vec<MarkdownBlock>,
    current_text: &mut String,
    container: MarkdownContainer,
) {
    let text = current_text.trim();
    if text.is_empty() {
        current_text.clear();
        return;
    }

    match container {
        MarkdownContainer::Paragraph => blocks.push(MarkdownBlock::Paragraph(text.to_owned())),
    }

    current_text.clear();
}

fn html_preview_text(source: &str) -> SharedString {
    let mut text = String::with_capacity(source.len());
    let mut in_tag = false;
    let mut tag = String::new();
    let mut entity = String::new();
    let mut in_entity = false;
    let mut suppressed_depth = 0usize;

    for ch in source.chars() {
        match ch {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                update_suppressed_html_depth(&tag, &mut suppressed_depth);
                in_tag = false;
                tag.clear();
            }
            _ if in_tag => tag.push(ch),
            '&' if !in_tag => {
                in_entity = true;
                entity.clear();
            }
            ';' if in_entity => {
                if suppressed_depth == 0 {
                    text.push_str(&decode_html_entity(&entity));
                }
                in_entity = false;
            }
            _ if in_entity => entity.push(ch),
            _ if !in_tag && suppressed_depth == 0 => text.push(ch),
            _ => {}
        }
    }

    text.split_whitespace().collect::<Vec<_>>().join(" ").into()
}

fn update_suppressed_html_depth(tag: &str, suppressed_depth: &mut usize) {
    let trimmed = tag.trim();
    let closing = trimmed.starts_with('/');
    let tag_name = html_tag_name(trimmed);
    if !is_suppressed_html_tag(tag_name.as_ref()) {
        return;
    }

    if closing {
        *suppressed_depth = suppressed_depth.saturating_sub(1);
    } else if !trimmed.ends_with('/') {
        *suppressed_depth = suppressed_depth.saturating_add(1);
    }
}

fn html_tag_name(tag: &str) -> String {
    tag.trim_start_matches('/')
        .split(|ch: char| ch.is_whitespace() || ch == '/' || ch == '>')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn is_suppressed_html_tag(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "script" | "style" | "iframe" | "object" | "embed" | "svg" | "math"
    )
}

fn decode_html_entity(entity: &str) -> String {
    match entity {
        "amp" => "&".to_owned(),
        "lt" => "<".to_owned(),
        "gt" => ">".to_owned(),
        "quot" => "\"".to_owned(),
        "apos" => "'".to_owned(),
        "nbsp" => " ".to_owned(),
        _ if entity.starts_with("#x") || entity.starts_with("#X") => {
            u32::from_str_radix(entity.trim_start_matches("#x").trim_start_matches("#X"), 16)
                .ok()
                .and_then(char::from_u32)
                .map_or_else(|| format!("&{entity};"), |ch| ch.to_string())
        }
        _ if entity.starts_with('#') => entity
            .trim_start_matches('#')
            .parse::<u32>()
            .ok()
            .and_then(char::from_u32)
            .map_or_else(|| format!("&{entity};"), |ch| ch.to_string()),
        _ => format!("&{entity};"),
    }
}

#[cfg(test)]
mod tests {
    use super::{MarkdownBlock, html_preview_text, parse_markdown};

    #[test]
    fn markdown_parser_handles_mixed_blocks() {
        let blocks = parse_markdown("# Title\n\n- Alpha\n- Beta\n\n```rust\nfn main() {}\n```");
        assert!(matches!(
            blocks.first(),
            Some(MarkdownBlock::Heading { .. })
        ));
        assert!(matches!(blocks.get(1), Some(MarkdownBlock::BulletList(_))));
        assert!(matches!(
            blocks.get(2),
            Some(MarkdownBlock::CodeBlock { .. })
        ));
    }

    #[test]
    fn markdown_parser_handles_tables() {
        let blocks = parse_markdown(
            "| Area | Status |\n| --- | --- |\n| Tokens | Stable |\n| Runtime | Preview |",
        );

        assert_eq!(
            blocks,
            vec![MarkdownBlock::Table {
                headers: vec!["Area".to_owned(), "Status".to_owned()],
                rows: vec![
                    vec!["Tokens".to_owned(), "Stable".to_owned()],
                    vec!["Runtime".to_owned(), "Preview".to_owned()],
                ],
            }]
        );
    }

    #[test]
    fn html_fragment_strips_tags() {
        assert_eq!(
            html_preview_text("<p><strong>Ready</strong> now.</p>"),
            "Ready now."
        );
    }

    #[test]
    fn markdown_parser_preserves_inline_markers_and_rules() {
        let blocks = parse_markdown(
            "Intro with **bold** and *emphasis* and ~~strike~~.\n\n---\n\n[Guide](https://example.com)",
        );

        assert_eq!(
            blocks,
            vec![
                MarkdownBlock::Paragraph(
                    "Intro with **bold** and *emphasis* and ~~strike~~.".to_owned()
                ),
                MarkdownBlock::Rule,
                MarkdownBlock::Paragraph("[https://example.com] Guide".to_owned()),
            ]
        );
    }

    #[test]
    fn markdown_parser_handles_html_and_images_in_paragraphs() {
        let blocks = parse_markdown(
            "Before <em>preview</em> after.\n\n![Logo](asset://logo.svg)\n\n<div>Inline <strong>HTML</strong></div>",
        );

        assert_eq!(
            blocks,
            vec![
                MarkdownBlock::Paragraph("Before preview after.".to_owned()),
                MarkdownBlock::Paragraph("[image asset://logo.svg: Logo]".to_owned()),
                MarkdownBlock::Html("<div>Inline <strong>HTML</strong></div>".to_owned()),
            ]
        );
    }

    #[test]
    fn html_fragment_decodes_basic_entities() {
        assert_eq!(
            html_preview_text("<p>Fish &amp; Chips &lt;Menu&gt;</p>"),
            "Fish & Chips <Menu>"
        );
    }

    #[test]
    fn html_preview_text_suppresses_unsafe_element_contents() {
        assert_eq!(
            html_preview_text(
                "<p>Visible</p><script>alert('x')</script><style>.x{}</style><iframe>hidden</iframe>",
            ),
            "Visible"
        );
    }

    #[test]
    fn html_preview_text_decodes_numeric_entities_and_preserves_unknown_entities() {
        assert_eq!(
            html_preview_text("<p>&#9731; &#x2603; &custom;</p>"),
            "☃ ☃ &custom;"
        );
    }
}
