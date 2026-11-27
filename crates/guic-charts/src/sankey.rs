//! Weighted flows through a directed acyclic graph.

use super::*;
use std::{collections::VecDeque, sync::Arc};

/// A directed flow between two zero-based node indices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SankeyLink {
    /// Node sending the flow.
    pub source: usize,
    /// Node receiving the flow.
    pub target: usize,
    /// Positive, finite quantity transported by the flow.
    pub value: f64,
}

/// Invalid input detected when preparing a flow diagram.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SankeyError {
    /// The given link references a missing endpoint.
    MissingEndpoint(usize),
    /// The given link has an invalid weight or would overflow a node total.
    InvalidWeight(usize),
    /// The links contain a directed cycle.
    Cycle,
}

impl std::fmt::Display for SankeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEndpoint(i) => write!(f, "missing endpoint on link {i}"),
            Self::InvalidWeight(i) => write!(f, "invalid flow weight on link {i}"),
            Self::Cycle => f.write_str("flow graph contains a directed cycle"),
        }
    }
}

impl std::error::Error for SankeyError {}

struct Graph {
    names: Vec<SharedString>,
    links: Vec<SankeyLink>,
    values: Vec<f64>,
    layers: Vec<Vec<usize>>,
}

/// A validated, shareable flow model. Clones retain topology and totals.
#[derive(Clone)]
pub struct SankeyDiagram(Arc<Graph>);

impl SankeyDiagram {
    /// Prepares a graph in input order, using the longest path to assign each node to a layer.
    /// Disconnected components and isolated nodes are supported.
    pub fn new(names: Vec<SharedString>, links: Vec<SankeyLink>) -> Result<Self, SankeyError> {
        let n = names.len();
        let mut outgoing = vec![Vec::new(); n];
        let mut degrees = vec![0usize; n];
        let mut sent = vec![0.; n];
        let mut received = vec![0.; n];
        for (i, link) in links.iter().enumerate() {
            if link.source >= n || link.target >= n {
                return Err(SankeyError::MissingEndpoint(i));
            }
            sent[link.source] += link.value;
            received[link.target] += link.value;
            if link.value <= 0.
                || !sent[link.source].is_finite()
                || !received[link.target].is_finite()
            {
                return Err(SankeyError::InvalidWeight(i));
            }
            outgoing[link.source].push(link.target);
            degrees[link.target] += 1;
        }
        let mut queue: VecDeque<_> = (0..n).filter(|&i| degrees[i] == 0).collect();
        let mut depths = vec![0; n];
        let mut visited = 0;
        while let Some(source) = queue.pop_front() {
            visited += 1;
            for &target in &outgoing[source] {
                depths[target] = depths[target].max(depths[source] + 1);
                degrees[target] -= 1;
                if degrees[target] == 0 {
                    queue.push_back(target);
                }
            }
        }
        if visited != n {
            return Err(SankeyError::Cycle);
        }
        let mut layers = vec![Vec::new(); depths.iter().max().map_or(0, |d| d + 1)];
        for (i, depth) in depths.into_iter().enumerate() {
            layers[depth].push(i);
        }
        let values = sent
            .into_iter()
            .zip(received)
            .map(|(out, inc)| out.max(inc))
            .collect();
        Ok(Self(Arc::new(Graph {
            names,
            links,
            values,
            layers,
        })))
    }

    /// Returns the larger of incoming and outgoing flow at the given node.
    pub fn node_value(&self, index: usize) -> Option<f64> {
        self.0.values.get(index).copied()
    }

    fn describe(&self, item: SankeyHit) -> (SharedString, f64) {
        let graph = &self.0;
        match item {
            SankeyHit::Node(i) => (graph.names[i].clone(), graph.values[i]),
            SankeyHit::Link(i) => {
                let link = graph.links[i];
                (
                    format!(
                        "{} → {}",
                        graph.names[link.source], graph.names[link.target]
                    )
                    .into(),
                    link.value,
                )
            }
        }
    }
}

/// Identifies a node or ribbon by its original input index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SankeyHit {
    /// A node rectangle.
    Node(usize),
    /// A weighted ribbon.
    Link(usize),
}

struct Ribbon {
    from: Point<Pixels>,
    to: Point<Pixels>,
    width: Pixels,
}

impl Ribbon {
    fn contains(&self, p: Point<Pixels>) -> bool {
        if p.x < self.from.x || p.x > self.to.x || self.to.x <= self.from.x {
            return false;
        }
        let t = (p.x - self.from.x) / (self.to.x - self.from.x);
        let y = self.from.y + (self.to.y - self.from.y) * (t * t * (3. - 2. * t));
        p.y >= y && p.y <= y + self.width
    }
    fn paint(&self, color: Hsla, window: &mut Window) {
        let step = (self.to.x - self.from.x) / 3.;
        let bottom = point(px(0.), self.width);
        let a = point(self.from.x + step, self.from.y);
        let b = point(self.to.x - step, self.to.y);
        let mut path = PathBuilder::fill();
        path.move_to(self.from);
        path.cubic_bezier_to(self.to, a, b);
        path.line_to(self.to + bottom);
        path.cubic_bezier_to(self.from + bottom, b + bottom, a + bottom);
        path.close();
        if let Ok(p) = path.build() {
            window.paint_path(p, color);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct NodeMetrics {
    width: f32,
    spacing: f32,
}

impl Default for NodeMetrics {
    fn default() -> Self {
        Self {
            width: 14.,
            spacing: 12.,
        }
    }
}

struct CachedFlow {
    graph: Arc<Graph>,
    metrics: NodeMetrics,
    layout: FlowLayout,
}

struct FlowLayout {
    bounds: Bounds<Pixels>,
    nodes: Vec<Bounds<Pixels>>,
    ribbons: Vec<Ribbon>,
}

impl FlowLayout {
    fn new(bounds: Bounds<Pixels>, graph: &Graph, metrics: NodeMetrics) -> Self {
        let num_layers = graph.layers.len();
        let largest = graph.values.iter().copied().fold(0., f64::max);
        let normalized = |value: f64| if largest > 0. { value / largest } else { 0. };
        let height = f64::from(f32::from(bounds.size.height.max(px(0.))));
        let max_nodes = graph.layers.iter().map(Vec::len).max().unwrap_or(1);
        let gap = (height / (2 * max_nodes) as f64).min(f64::from(metrics.spacing));
        let min_height = (height / (2 * max_nodes) as f64).min(2.);
        let scale = graph
            .layers
            .iter()
            .filter_map(|layer| {
                let sum: f64 = layer.iter().map(|&i| normalized(graph.values[i])).sum();
                let space = height
                    - gap * layer.len().saturating_sub(1) as f64
                    - min_height * layer.len() as f64;
                (sum > 0.).then(|| space.max(0.) / sum)
            })
            .fold(height, f64::min);
        let node_height = |i: usize| px((normalized(graph.values[i]) * scale + min_height) as f32);
        let node_width = px(metrics.width).min(bounds.size.width / (2 * num_layers.max(1)) as f32);
        let mut nodes = vec![Bounds::default(); graph.names.len()];
        for (depth, layer) in graph.layers.iter().enumerate() {
            let x = if num_layers == 1 {
                bounds.center().x - node_width / 2.
            } else {
                bounds.left()
                    + (bounds.size.width - node_width) * (depth as f32 / (num_layers - 1) as f32)
            };
            let total = layer
                .iter()
                .map(|&i| f32::from(node_height(i)))
                .sum::<f32>()
                + gap as f32 * layer.len().saturating_sub(1) as f32;
            let mut y = bounds.top() + (bounds.size.height - px(total)) / 2.;
            for &i in layer {
                nodes[i] = Bounds::new(point(x, y), gpui::size(node_width, node_height(i)));
                y += node_height(i) + px(gap as f32);
            }
        }
        let mut from_y: Vec<_> = nodes
            .iter()
            .map(|b| b.top() + px(min_height as f32 / 2.))
            .collect();
        let mut to_y = from_y.clone();
        let ribbons = graph
            .links
            .iter()
            .map(|link| {
                let width = px((normalized(link.value) * scale) as f32);
                let from = point(nodes[link.source].right(), from_y[link.source]);
                let to = point(nodes[link.target].left(), to_y[link.target]);
                from_y[link.source] += width;
                to_y[link.target] += width;
                Ribbon { from, to, width }
            })
            .collect();
        Self {
            bounds,
            nodes,
            ribbons,
        }
    }

    fn hit(&self, p: Point<Pixels>) -> Option<SankeyHit> {
        self.nodes
            .iter()
            .rposition(|b| b.contains(&p))
            .map(SankeyHit::Node)
            .or_else(|| {
                self.ribbons
                    .iter()
                    .rposition(|r| r.contains(p))
                    .map(SankeyHit::Link)
            })
    }
}

#[derive(Default)]
struct FlowTip {
    content: Option<(SharedString, String)>,
    hovered: bool,
}
impl Render for FlowTip {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        let Some((name, value)) = &self.content else {
            return div().hidden();
        };
        div()
            .debug_selector(|| "guic-sankey-tooltip".into())
            .w((window.viewport_size().width - px(12.))
                .max(px(0.))
                .min(px(200.)))
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .text_color(theme.foreground())
            .overflow_hidden()
            .p_2()
            .child(div().truncate().child(name.clone()))
            .child(div().truncate().child(value.clone()))
    }
}

type FlowSelect = Rc<dyn Fn(&SankeyHit, &mut Window, &mut App)>;

/// A weighted flow diagram with node and ribbon selection and pointer-following tooltips.
#[derive(gpui::IntoElement)]
pub struct SankeyChart {
    id: SharedString,
    diagram: SankeyDiagram,
    options: ChartOptions,
    on_select: Option<FlowSelect>,
    metrics: NodeMetrics,
    opacity: f32,
    label_gap: f32,
}

impl SankeyChart {
    /// Creates a view of a retained graph.
    pub fn new(id: impl Into<SharedString>, diagram: SankeyDiagram) -> Self {
        Self {
            id: id.into(),
            diagram,
            options: Default::default(),
            on_select: None,
            metrics: Default::default(),
            opacity: 0.35,
            label_gap: 4.,
        }
    }
    /// Sets the title, height, tooltips, value formatter, and empty message.
    pub fn options(mut self, options: ChartOptions) -> Self {
        self.options = options;
        self
    }
    /// Sets the requested node width and vertical gap in logical pixels.
    /// Crowded layers reduce these values to fit the plot.
    #[must_use]
    pub fn node_layout(mut self, width: f32, spacing: f32) -> Self {
        if width.is_finite() {
            self.metrics.width = width.max(1.);
        }
        if spacing.is_finite() {
            self.metrics.spacing = spacing.max(0.);
        }
        self
    }
    /// Sets the opacity of flow ribbons.
    #[must_use]
    pub fn link_opacity(mut self, opacity: f32) -> Self {
        if opacity.is_finite() {
            self.opacity = opacity.clamp(0., 1.);
        }
        self
    }
    /// Sets the gap between a node and its label.
    #[must_use]
    pub fn label_gap(mut self, pixels: f32) -> Self {
        if pixels.is_finite() {
            self.label_gap = pixels.max(0.);
        }
        self
    }

    /// Handles activation of a node or ribbon.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SankeyHit, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SankeyChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let mut root = div()
            .id(self.id.clone())
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .text_color(theme.foreground())
            .role(gpui::Role::Group)
            .aria_label(
                self.options
                    .title
                    .clone()
                    .unwrap_or_else(|| "Flow diagram".into()),
            );
        if let Some(title) = self.options.title {
            root = root.child(title);
        }
        if self.diagram.0.names.is_empty() {
            return root.child(
                self.options
                    .empty_message
                    .unwrap_or_else(|| "No data".into()),
            );
        }
        let graph = self.diagram.clone();
        let summary = graph
            .0
            .links
            .iter()
            .take(20)
            .map(|l| {
                format!(
                    "{} to {}: {}",
                    graph.0.names[l.source],
                    graph.0.names[l.target],
                    self.options.value_formatter.format(l.value)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let cache = window
            .use_keyed_state(format!("{}-layout", self.id), cx, |_, _| {
                Rc::new(RefCell::new(None::<Rc<CachedFlow>>))
            })
            .read(cx)
            .clone();
        let layout_cache = cache.clone();
        let layout_graph = graph.clone();
        let tip = self.options.show_tooltip.then(|| {
            window.use_keyed_state(format!("{}-flow-tip", self.id), cx, |_, _| {
                FlowTip::default()
            })
        });
        let layout_tip = tip.clone();
        let layout_formatter = self.options.value_formatter.clone();
        let mut plot = div()
            .id("plot")
            .debug_selector(|| "guic-sankey-plot".into())
            .relative()
            .w_full()
            .h(px(self.options.height))
            .flex_shrink_0()
            .overflow_hidden()
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let mut slot = layout_cache.borrow_mut();
                        if slot.as_ref().is_none_or(|old| {
                            old.layout.bounds != bounds
                                || old.metrics != self.metrics
                                || !Arc::ptr_eq(&old.graph, &layout_graph.0)
                        }) {
                            *slot = Some(Rc::new(CachedFlow {
                                graph: layout_graph.0.clone(),
                                metrics: self.metrics,
                                layout: FlowLayout::new(bounds, &layout_graph.0, self.metrics),
                            }));
                        }
                        let cached = slot.as_ref().expect("prepared flow layout").clone();
                        if let Some(tip) = &layout_tip
                            && tip.read(cx).hovered
                        {
                            let content = cached.layout.hit(window.mouse_position()).map(|hit| {
                                let (name, value) = layout_graph.describe(hit);
                                (name, layout_formatter.format(value))
                            });
                            tip.update(cx, |tip, _| tip.content = content);
                        }
                        cached
                    },
                    move |bounds, cached, window, cx| {
                        let layout = &cached.layout;
                        let theme = Theme::global(cx);
                        for (ribbon, link) in layout.ribbons.iter().zip(&graph.0.links) {
                            ribbon.paint(palette(link.source, theme).opacity(self.opacity), window);
                        }
                        let font_size = px(theme.typography.text_sm);
                        let text_color = theme.foreground();
                        for (i, rect) in layout.nodes.iter().enumerate() {
                            window.paint_quad(fill(*rect, palette(i, theme)).corner_radii(px(2.)));
                        }
                        for (i, rect) in layout.nodes.iter().enumerate() {
                            let name = &graph.0.names[i];
                            let mut run = window.text_style().to_run(name.len());
                            run.color = text_color;
                            let line = window.text_system().shape_line(
                                name.clone(),
                                font_size,
                                &[run],
                                None,
                            );
                            let x = if rect.center().x <= bounds.center().x {
                                rect.right() + px(self.label_gap)
                            } else {
                                rect.left() - line.width - px(self.label_gap)
                            };
                            let _ = line.paint(
                                point(x, rect.center().y - font_size / 2.),
                                font_size,
                                gpui::TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                        }
                    },
                )
                .absolute()
                .size_full(),
            );
        if let Some(tip) = tip {
            let hover_tip = tip.clone();
            let leave_tip = tip.clone();
            let hover_cache = cache.clone();
            plot = plot
                .on_mouse_move(move |event, _, cx| {
                    let content = hover_cache
                        .borrow()
                        .as_ref()
                        .and_then(|l| l.layout.hit(event.position))
                        .map(|hit| {
                            let (label, value) = self.diagram.describe(hit);
                            (label, self.options.value_formatter.format(value))
                        });
                    hover_tip.update(cx, |tip, cx| {
                        tip.hovered = true;
                        if tip.content != content {
                            tip.content = content;
                            cx.notify();
                        }
                    });
                })
                .on_mouse_exit(move |_, _, cx| {
                    leave_tip.update(cx, |tip, cx| {
                        tip.hovered = false;
                        if tip.content.take().is_some() {
                            cx.notify();
                        }
                    })
                })
                .tooltip_show_delay(Duration::ZERO)
                .cursor_tooltip(move |_, _| tip.clone().into());
        }
        if let Some(handler) = self.on_select {
            plot = plot.on_click(move |event, window, cx| {
                let hit = cache
                    .borrow()
                    .as_ref()
                    .and_then(|l| l.layout.hit(event.position()));
                if let Some(hit) = hit {
                    handler(&hit, window, cx);
                }
            });
        }
        root.aria_description(summary).child(plot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn link(source: usize, target: usize, value: f64) -> SankeyLink {
        SankeyLink {
            source,
            target,
            value,
        }
    }

    #[test]
    fn invalid_graphs_are_rejected_before_layout() {
        let names = vec!["a".into(), "b".into()];
        for (links, expected) in [
            (vec![link(0, 2, 1.)], SankeyError::MissingEndpoint(0)),
            (vec![link(0, 1, f64::NAN)], SankeyError::InvalidWeight(0)),
            (
                vec![link(0, 1, f64::MAX), link(0, 1, f64::MAX)],
                SankeyError::InvalidWeight(1),
            ),
            (vec![link(0, 1, 2.), link(1, 0, 2.)], SankeyError::Cycle),
        ] {
            assert_eq!(
                SankeyDiagram::new(names.clone(), links).err(),
                Some(expected)
            );
        }
    }

    #[test]
    fn unequal_flows_keep_proportional_widths_and_can_be_picked_at_the_curve() {
        let graph = SankeyDiagram::new(
            ["A", "B", "C", "D"].map(Into::into).to_vec(),
            vec![link(0, 1, 1.), link(0, 2, 3.)],
        )
        .expect("valid test fixture");
        let layout = FlowLayout::new(
            Bounds::new(point(px(10.), px(12.)), gpui::size(px(300.), px(180.))),
            &graph.0,
            NodeMetrics::default(),
        );
        assert_eq!(graph.node_value(0), Some(4.));
        assert!((layout.ribbons[1].width / layout.ribbons[0].width - 3.).abs() < 0.001);
        for (i, r) in layout.ribbons.iter().enumerate() {
            let center = point((r.from.x + r.to.x) / 2., (r.from.y + r.to.y + r.width) / 2.);
            assert_eq!(layout.hit(center), Some(SankeyHit::Link(i)));
        }
        for (i, node) in layout.nodes.iter().enumerate() {
            assert!(node.size.height > px(0.));
            assert_eq!(layout.hit(node.center()), Some(SankeyHit::Node(i)));
        }
    }

    #[gpui::test]
    fn hovering_and_clicking_a_ribbon_uses_the_painted_layout(cx: &mut gpui::TestAppContext) {
        use gpui::{Context, Modifiers};
        struct Flow {
            graph: SankeyDiagram,
            selected: Option<SankeyHit>,
        }
        impl Render for Flow {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                SankeyChart::new("flow", self.graph.clone())
                    .on_select(cx.listener(|v, hit: &SankeyHit, _, _| v.selected = Some(*hit)))
            }
        }
        super::super::tests::init_visual_test(cx);
        let (view, cx) = cx.add_window_view(|_, _| Flow {
            graph: SankeyDiagram::new(vec!["A".into(), "B".into()], vec![link(0, 1, 5.)])
                .expect("valid test fixture"),
            selected: None,
        });
        let plot = cx
            .debug_bounds("guic-sankey-plot")
            .expect("valid test fixture");
        cx.simulate_mouse_move(plot.center(), None, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("guic-sankey-tooltip").is_some());
        cx.simulate_click(plot.center(), Modifiers::none());
        view.read_with(cx, |v, _| assert_eq!(v.selected, Some(SankeyHit::Link(0))));
        view.update(cx, |v, cx| {
            v.graph = SankeyDiagram::new(vec!["alone".into()], vec![]).expect("isolated node");
            cx.notify();
        });
        // The node lies in the center. Move to where a ribbon used to be,
        // then restore the link under that stationary pointer.
        let beside = point(plot.left() + plot.size.width * 0.25, plot.center().y);
        cx.simulate_mouse_move(beside, None, Modifiers::none());
        assert!(cx.debug_bounds("guic-sankey-tooltip").is_none());
        view.update(cx, |v, cx| {
            v.graph = SankeyDiagram::new(vec!["a".into(), "b".into()], vec![link(0, 1, 5.)])
                .expect("valid graph");
            cx.notify();
        });
        assert!(cx.debug_bounds("guic-sankey-tooltip").is_some());
    }
}
