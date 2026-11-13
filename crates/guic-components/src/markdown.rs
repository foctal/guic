use crate::Label;
use std::{ops::Range, rc::Rc};
type LinkHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;
use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, div, px,
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
    id: SharedString,
    on_link: Option<LinkHandler>,
}

impl Markdown {
    /// Creates a rendered Markdown surface from a source string.
    #[must_use]
    pub fn new(source: impl Into<SharedString>) -> Self {
        Self {
            source: source.into(),
            id: "guic-markdown".into(),
            on_link: None,
        }
    }
    /// Sets an identity when rendering multiple documents in the same view.
    #[must_use]
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = id.into();
        self
    }
    /// Handles link activation. Without a handler, only HTTP, HTTPS and mail links open.
    #[must_use]
    pub fn on_link_click(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_link = Some(Rc::new(handler));
        self
    }
}

#[derive(Default)]
struct MarkdownCache {
    source: SharedString,
    blocks: Rc<[MarkdownBlock]>,
    colors: Option<(gpui::Hsla, gpui::Hsla)>,
    inlines: Vec<Rc<InlineContent>>,
}
impl MarkdownCache {
    fn update(&mut self, source: SharedString, theme: &Theme) {
        let colors = (theme.primary(), theme.secondary());
        if self.source != source {
            self.blocks = parse_markdown(&source).into();
            self.source = source;
            self.inlines.clear();
        }
        if self.colors != Some(colors) {
            self.colors = Some(colors);
            self.inlines.clear();
        }
    }
}

impl RenderOnce for Markdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let cache = window.use_keyed_state(format!("{}-document", self.id), cx, |_, _| {
            MarkdownCache::default()
        });
        let theme = Theme::global(cx).clone();
        let children = cache.update(cx, |cache, _| {
            cache.update(self.source, &theme);
            let mut inline_id = 0;
            cache
                .blocks
                .clone()
                .iter()
                .map(|block| {
                    render_block(
                        block,
                        &theme,
                        &self.on_link,
                        &mut inline_id,
                        &mut cache.inlines,
                    )
                })
                .collect::<Vec<_>>()
        });

        div()
            .id(self.id)
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
    List {
        start: Option<u64>,
        items: Vec<Vec<MarkdownBlock>>,
    },
    Quote(Vec<MarkdownBlock>),
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

fn render_block(
    block: &MarkdownBlock,
    theme: &Theme,
    on_link: &Option<LinkHandler>,
    inline_id: &mut usize,
    inlines: &mut Vec<Rc<InlineContent>>,
) -> AnyElement {
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
                .child(render_inline(text, theme, on_link, inline_id, inlines))
                .into_any_element()
        }
        MarkdownBlock::Paragraph(text) => render_inline(text, theme, on_link, inline_id, inlines),
        MarkdownBlock::List { start, items } => {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children(items.iter().enumerate().map(|(index, item)| {
                    let marker = start.map_or_else(
                        || "-".to_owned(),
                        |start| format!("{}.", start.saturating_add(index as u64)),
                    );
                    div()
                        .flex()
                        .gap_2()
                        .items_start()
                        .child(div().text_color(theme.primary()).child(marker))
                        .child(div().flex().flex_col().gap_2().children(
                            item.iter().map(|block| {
                                render_block(block, theme, on_link, inline_id, inlines)
                            }),
                        ))
                        .into_any_element()
                }))
                .into_any_element()
        }
        MarkdownBlock::Quote(blocks) => div()
            .pl_3()
            .border_l_2()
            .border_color(theme.primary())
            .flex()
            .flex_col()
            .gap_2()
            .children(
                blocks
                    .iter()
                    .map(|block| render_block(block, theme, on_link, inline_id, inlines)),
            )
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
                root = root.child(Label::new(language.clone()).muted(true));
            }
            root.child(div().text_color(theme.foreground()).child(code.clone()))
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
                table = table.child(render_table_row(
                    headers, true, theme, on_link, inline_id, inlines,
                ));
            }

            for row in rows {
                table = table.child(render_table_row(
                    row, false, theme, on_link, inline_id, inlines,
                ));
            }

            table.into_any_element()
        }
        MarkdownBlock::Html(html) => div()
            .rounded(px(theme.radius.md))
            .bg(theme.secondary().opacity(0.12))
            .p_3()
            .child(Label::new(html_preview_text(html)).muted(true))
            .into_any_element(),
    }
}

fn render_table_row(
    cells: &[String],
    header: bool,
    theme: &Theme,
    on_link: &Option<LinkHandler>,
    inline_id: &mut usize,
    inlines: &mut Vec<Rc<InlineContent>>,
) -> AnyElement {
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
        .children(cells.iter().map(|cell| {
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
                        .child(render_inline(cell, theme, on_link, inline_id, inlines))
                        .into_any_element()
                } else {
                    render_inline(cell, theme, on_link, inline_id, inlines)
                })
                .into_any_element()
        }))
        .into_any_element()
}

#[derive(Default)]
struct InlineContent {
    text: SharedString,
    highlights: Vec<(Range<usize>, gpui::HighlightStyle)>,
    links: Vec<(Range<usize>, String)>,
    fonts: Vec<(Range<usize>, SharedString)>,
}

fn inline_content(source: &str, theme: &Theme) -> InlineContent {
    let mut result = InlineContent::default();
    let mut rendered = String::new();
    let (mut strong, mut emphasis, mut strike) = (0usize, 0usize, 0usize);
    let mut link: Option<(usize, String)> = None;
    for event in Parser::new_ext(source, Options::ENABLE_STRIKETHROUGH) {
        match event {
            Event::Start(Tag::Strong) => strong += 1,
            Event::End(TagEnd::Strong) => strong = strong.saturating_sub(1),
            Event::Start(Tag::Emphasis) => emphasis += 1,
            Event::End(TagEnd::Emphasis) => emphasis = emphasis.saturating_sub(1),
            Event::Start(Tag::Strikethrough) => strike += 1,
            Event::End(TagEnd::Strikethrough) => strike = strike.saturating_sub(1),
            Event::Start(Tag::Link { dest_url, .. }) => {
                link = Some((rendered.len(), dest_url.into_string()))
            }
            Event::End(TagEnd::Link) => {
                if let Some((start, url)) = link.take() {
                    result.links.push((start..rendered.len(), url));
                }
            }
            Event::Code(text) => {
                let start = rendered.len();
                rendered.push_str(&text);
                let range = start..rendered.len();
                result.fonts.push((range.clone(), "monospace".into()));
                result.highlights.push((
                    range,
                    gpui::HighlightStyle {
                        background_color: Some(theme.secondary()),
                        ..Default::default()
                    },
                ));
            }
            Event::Text(text) => {
                let start = rendered.len();
                rendered.push_str(&text);
                let highlight = gpui::HighlightStyle {
                    font_weight: (strong > 0).then_some(gpui::FontWeight::BOLD),
                    font_style: (emphasis > 0).then_some(gpui::FontStyle::Italic),
                    strikethrough: (strike > 0).then_some(gpui::StrikethroughStyle {
                        thickness: px(1.0),
                        color: None,
                    }),
                    underline: link.as_ref().map(|_| gpui::UnderlineStyle {
                        thickness: px(1.0),
                        color: None,
                        wavy: false,
                    }),
                    color: link.as_ref().map(|_| theme.primary()),
                    ..Default::default()
                };
                result.highlights.push((start..rendered.len(), highlight));
            }
            Event::SoftBreak | Event::HardBreak => rendered.push('\n'),
            _ => {}
        }
    }
    result.text = rendered.into();
    result
}

fn render_inline(
    source: &str,
    theme: &Theme,
    on_link: &Option<LinkHandler>,
    inline_id: &mut usize,
    inlines: &mut Vec<Rc<InlineContent>>,
) -> AnyElement {
    if *inline_id == inlines.len() {
        inlines.push(Rc::new(inline_content(source, theme)));
    }
    let content = inlines[*inline_id].clone();
    let text = gpui::StyledText::new(content.text.clone())
        .with_highlights(content.highlights.clone())
        .with_font_family_overrides(content.fonts.clone());
    let id = format!("inline-{}", *inline_id);
    *inline_id += 1;
    let ranges = content
        .links
        .iter()
        .map(|(range, _)| range.clone())
        .collect();
    let handler = on_link.clone();
    gpui::InteractiveText::new(id, text)
        .on_click(ranges, move |index, window, cx| {
            let url = &content.links[index].1;
            if let Some(handler) = &handler {
                handler(url, window, cx);
            } else if matches!(
                url.split_once(':')
                    .map(|(scheme, _)| scheme.to_ascii_lowercase())
                    .as_deref(),
                Some("http" | "https" | "mailto")
            ) {
                cx.open_url(url);
            }
        })
        .into_any_element()
}

fn parse_markdown(source: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut current_text = String::new();
    enum Container {
        Quote {
            parent: Vec<MarkdownBlock>,
        },
        List {
            parent: Vec<MarkdownBlock>,
            start: Option<u64>,
            items: Vec<Vec<MarkdownBlock>>,
        },
    }
    let mut containers = Vec::new();
    let mut flattened_depth = 0usize;
    let mut table_headers = Vec::new();
    let mut table_rows = Vec::new();
    let mut current_table_row = Vec::new();
    let mut current_html = String::new();
    let mut current_code = String::new();
    let mut code_language = None;
    let mut link_destinations = Vec::new();
    let mut in_heading = None;
    let mut in_html = false;
    let mut inline_html_depth = 0;
    let mut in_code = false;
    let mut in_table = false;
    let mut in_table_head = false;

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH);

    for event in Parser::new_ext(source, options) {
        // Bound the recursive render tree even for attacker-controlled Markdown.
        // Excess nesting keeps its text without creating more layout containers.
        if flattened_depth > 0 {
            match event {
                Event::Start(Tag::List(_) | Tag::BlockQuote(_)) => flattened_depth += 1,
                Event::End(TagEnd::List(_) | TagEnd::BlockQuote(_)) => flattened_depth -= 1,
                Event::InlineHtml(html) => update_suppressed_html_depth(
                    html.trim_start_matches('<').trim_end_matches('>'),
                    &mut inline_html_depth,
                ),
                Event::Text(text) | Event::Code(text) if inline_html_depth == 0 => {
                    for ch in text.chars() {
                        if "\\`*_{}[]()#+-.!|>~".contains(ch) {
                            current_text.push('\\');
                        }
                        current_text.push(ch);
                    }
                }
                Event::End(TagEnd::Paragraph | TagEnd::Item)
                | Event::SoftBreak
                | Event::HardBreak => current_text.push('\n'),
                _ => {}
            }
            continue;
        }
        if containers.len() == 64
            && matches!(event, Event::Start(Tag::List(_) | Tag::BlockQuote(_)))
        {
            flattened_depth = 1;
            continue;
        }
        match event {
            Event::Start(Tag::Paragraph) if !in_table => {
                current_text.clear();
            }
            Event::End(TagEnd::Paragraph) if !in_table => {
                push_text_block(&mut blocks, &mut current_text);
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
                push_text_block(&mut blocks, &mut current_text);
                containers.push(Container::Quote {
                    parent: std::mem::take(&mut blocks),
                });
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                push_text_block(&mut blocks, &mut current_text);
                if let Some(Container::Quote { parent }) = containers.pop() {
                    let quoted = std::mem::replace(&mut blocks, parent);
                    blocks.push(MarkdownBlock::Quote(quoted));
                }
            }
            Event::Start(Tag::List(start)) => {
                push_text_block(&mut blocks, &mut current_text);
                containers.push(Container::List {
                    parent: std::mem::take(&mut blocks),
                    start,
                    items: Vec::new(),
                });
            }
            Event::End(TagEnd::List(_)) => {
                if let Some(Container::List {
                    parent,
                    start,
                    items,
                }) = containers.pop()
                {
                    blocks = parent;
                    blocks.push(MarkdownBlock::List { start, items });
                }
            }
            Event::End(TagEnd::Item) => {
                push_text_block(&mut blocks, &mut current_text);
                if let Some(Container::List { items, .. }) = containers.last_mut() {
                    items.push(std::mem::take(&mut blocks));
                }
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
                update_suppressed_html_depth(
                    html.trim_start_matches('<').trim_end_matches('>'),
                    &mut inline_html_depth,
                );
                if inline_html_depth == 0 {
                    current_text.push_str(html_preview_text(html.as_ref()).as_ref());
                }
            }
            Event::End(TagEnd::HtmlBlock) if in_html => {
                blocks.push(MarkdownBlock::Html(current_html.trim().to_owned()));
                current_html.clear();
                in_html = false;
            }
            Event::Text(text) => {
                if in_code {
                    current_code.push_str(text.as_ref());
                } else if inline_html_depth == 0 {
                    for ch in text.chars() {
                        if "\\`*_{}[]()#+-.!|>~".contains(ch) {
                            current_text.push('\\');
                        }
                        current_text.push(ch);
                    }
                }
            }
            Event::Code(code) if inline_html_depth == 0 => {
                let fence =
                    "`".repeat(code.split(|ch| ch != '`').map(str::len).max().unwrap_or(0) + 1);
                let padding = !code.chars().all(|ch| ch == ' ');
                current_text.push_str(&fence);
                if padding {
                    current_text.push(' ');
                }
                current_text.push_str(code.as_ref());
                if padding {
                    current_text.push(' ');
                }
                current_text.push_str(&fence);
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
                link_destinations.push(dest_url.into_string());
            }
            Event::End(TagEnd::Link) => {
                current_text.push_str("](<");
                if let Some(destination) = link_destinations.pop() {
                    current_text.push_str(&destination.replace('>', "%3E").replace('<', "%3C"));
                }
                current_text.push_str(">)");
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
            _ => {}
        }
    }

    if !current_html.trim().is_empty() {
        blocks.push(MarkdownBlock::Html(current_html.trim().to_owned()));
    }

    blocks
}

fn push_text_block(blocks: &mut Vec<MarkdownBlock>, current_text: &mut String) {
    if !current_text.trim().is_empty() {
        blocks.push(MarkdownBlock::Paragraph(current_text.trim().to_owned()));
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
        if in_entity && !(ch.is_ascii_alphanumeric() || matches!(ch, '#' | ';')) {
            if suppressed_depth == 0 {
                text.push('&');
                text.push_str(&entity);
            }
            in_entity = false;
        }
        match ch {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                update_suppressed_html_depth(&tag, &mut suppressed_depth);
                if suppressed_depth == 0
                    && matches!(
                        html_tag_name(&tag).as_str(),
                        "p" | "div" | "br" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                    )
                {
                    text.push(' ');
                }
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

    if in_entity && suppressed_depth == 0 {
        text.push('&');
        text.push_str(&entity);
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
        assert!(matches!(
            blocks.get(1),
            Some(MarkdownBlock::List { start: None, .. })
        ));
        assert!(matches!(
            blocks.get(2),
            Some(MarkdownBlock::CodeBlock { .. })
        ));
    }

    #[test]
    fn excessive_nesting_keeps_text_in_a_bounded_tree() {
        let source = format!("{}leaf<script>hidden</script>", "> ".repeat(1_000));
        let blocks = parse_markdown(&source);
        let mut current = blocks.as_slice();
        for _ in 0..64 {
            let [MarkdownBlock::Quote(children)] = current else {
                panic!("quote container");
            };
            current = children;
        }
        assert_eq!(current, &[MarkdownBlock::Paragraph("leaf".into())]);
    }

    #[test]
    fn inline_code_preserves_whitespace_only_content() {
        let blocks = parse_markdown("`   `");
        let MarkdownBlock::Paragraph(source) = &blocks[0] else {
            panic!("paragraph");
        };
        let content = super::inline_content(source, &guic_tokens::Theme::dark());
        assert_eq!(content.text, "   ");
    }

    #[test]
    fn inline_html_suppresses_script_contents() {
        let blocks = parse_markdown("Before <script>hidden</script> after");
        assert_eq!(
            blocks,
            vec![MarkdownBlock::Paragraph("Before  after".into())]
        );
    }

    #[test]
    fn quotes_and_loose_lists_retain_paragraphs() {
        assert_eq!(
            parse_markdown("> First\n>\n> Second"),
            vec![MarkdownBlock::Quote(vec![
                MarkdownBlock::Paragraph("First".into()),
                MarkdownBlock::Paragraph("Second".into()),
            ]),]
        );
        assert_eq!(
            parse_markdown("- First\n\n- Second"),
            vec![MarkdownBlock::List {
                start: None,
                items: vec![
                    vec![MarkdownBlock::Paragraph("First".into())],
                    vec![MarkdownBlock::Paragraph("Second".into())],
                ]
            },]
        );
    }

    #[test]
    fn nested_lists_preserve_parent_text_and_start_number() {
        assert_eq!(
            parse_markdown("3. Parent\n   - Child\n4. Last"),
            vec![MarkdownBlock::List {
                start: Some(3),
                items: vec![
                    vec![
                        MarkdownBlock::Paragraph("Parent".into()),
                        MarkdownBlock::List {
                            start: None,
                            items: vec![vec![MarkdownBlock::Paragraph("Child".into())],]
                        }
                    ],
                    vec![MarkdownBlock::Paragraph("Last".into())],
                ]
            },]
        );
    }

    #[test]
    fn html_preview_preserves_literal_ampersands_and_separates_blocks() {
        assert_eq!(html_preview_text("<p>R&D</p><p>AT&T</p>"), "R&D AT&T");
        assert_eq!(html_preview_text("a &unfinished"), "a &unfinished");
    }

    #[test]
    fn retained_document_reuses_parsing_and_invalidates_colors() {
        use super::{MarkdownCache, inline_content};
        use std::rc::Rc;
        let mut cache = MarkdownCache::default();
        let theme = guic_tokens::Theme::dark();
        cache.update("**Ready**".into(), &theme);
        let blocks = cache.blocks.clone();
        cache
            .inlines
            .push(Rc::new(inline_content("**Ready**", &theme)));
        let inline = cache.inlines[0].clone();
        cache.update("**Ready**".into(), &theme);
        assert!(Rc::ptr_eq(&blocks, &cache.blocks));
        assert!(Rc::ptr_eq(&inline, &cache.inlines[0]));
        cache.update("**Ready**".into(), &guic_tokens::Theme::light());
        assert!(Rc::ptr_eq(&blocks, &cache.blocks));
        assert!(cache.inlines.is_empty());
        cache.update("Changed".into(), &theme);
        assert!(!Rc::ptr_eq(&blocks, &cache.blocks));
    }

    #[test]
    fn rich_inline_preserves_escapes_unicode_and_nested_styles() {
        let blocks = parse_markdown(
            r"**Bold and *italic*** with \*literal\* and `a` and [Guide](https://example.com).",
        );
        let MarkdownBlock::Paragraph(source) = &blocks[0] else {
            panic!("paragraph");
        };
        let content = super::inline_content(source, &guic_tokens::Theme::dark());
        assert_eq!(
            content.text,
            "Bold and italic with *literal* and a and Guide."
        );
        assert!(
            content
                .highlights
                .iter()
                .any(|(_, style)| style.font_weight.is_some() && style.font_style.is_some())
        );
        assert_eq!(content.links.len(), 1);
        assert_eq!(&content.text[content.links[0].0.clone()], "Guide");
        assert_eq!(content.links[0].1, "https://example.com");
        assert_eq!(&content.text[content.fonts[0].0.clone()], "a");
        let content = super::inline_content("**\u{65e5}\u{672c}**", &guic_tokens::Theme::light());
        assert_eq!(content.highlights[0].0, 0..6);
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
                    "Intro with **bold** and *emphasis* and ~~strike~~\\.".to_owned()
                ),
                MarkdownBlock::Rule,
                MarkdownBlock::Paragraph("[Guide](<https://example.com>)".to_owned()),
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
                MarkdownBlock::Paragraph("Before preview after\\.".to_owned()),
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
