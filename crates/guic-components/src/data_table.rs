use crate::{Label, ScrollArea, Separator};
use gpui::{
    AnyElement, App, AppContext as _, Bounds, ClickEvent, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, Keystroke, MouseMoveEvent, ParentElement as _, Pixels, RenderOnce,
    ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _,
    UniformListScrollHandle, Window, canvas, div, px, uniform_list,
};
use guic_tokens::Theme;
use std::{cell::Cell, collections::HashMap, rc::Rc};

use crate::virtual_list::VirtualListMetrics;

type SharedStringHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type SortHandler = Rc<dyn Fn(&TableSort, &mut Window, &mut App)>;
type SelectionHandler = Rc<dyn Fn(&DataTableSelection, &mut Window, &mut App)>;
type ColumnResizeHandler = Rc<dyn Fn(&DataColumnResize, &mut Window, &mut App)>;
type CellRenderer = Rc<dyn Fn(&DataTableCell) -> AnyElement>;
type RowActionsRenderer = Rc<dyn Fn(&DataRow) -> AnyElement>;

type OrderHandler = Rc<dyn Fn(&[SharedString], &mut Window, &mut App)>;

#[derive(Clone)]
struct ColumnDrag {
    owner: gpui::EntityId,
    id: SharedString,
    title: SharedString,
}

impl gpui::Render for ColumnDrag {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        div()
            .px_3()
            .py_2()
            .bg(theme.background())
            .text_color(theme.foreground())
            .child(self.title.clone())
    }
}

struct ColumnMoves {
    owner: gpui::EntityId,
    order: Vec<(SharedString, DataColumnPin)>,
    change: OrderHandler,
}

impl ColumnMoves {
    fn apply(&self, from: usize, to: usize, window: &mut Window, cx: &mut App) {
        if from == to || self.order[from].1 != self.order[to].1 {
            return;
        }
        let mut ids: Vec<_> = self.order.iter().map(|(id, _)| id.clone()).collect();
        let id = ids.remove(from);
        ids.insert(to, id);
        (self.change)(&ids, window, cx);
    }
}

/// Pin placement for a table column.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DataColumnPin {
    /// Scroll and virtualize the column normally.
    #[default]
    None,
    /// Keep the column at the leading edge.
    Start,
    /// Keep the column at the trailing edge.
    End,
}

/// Context passed to a custom table cell renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataTableCell {
    row_id: SharedString,
    column_id: SharedString,
    row_index: usize,
    column_index: usize,
    value: SharedString,
    selected: bool,
}

impl DataTableCell {
    /// Returns the row identifier.
    #[must_use]
    pub fn row_id(&self) -> &SharedString {
        &self.row_id
    }
    /// Returns the column identifier.
    #[must_use]
    pub fn column_id(&self) -> &SharedString {
        &self.column_id
    }
    /// Returns the source row index.
    #[must_use]
    pub fn row_index(&self) -> usize {
        self.row_index
    }
    /// Returns the source column index.
    #[must_use]
    pub fn column_index(&self) -> usize {
        self.column_index
    }
    /// Returns the cell value.
    #[must_use]
    pub fn value(&self) -> &SharedString {
        &self.value
    }
    /// Returns whether the row is selected.
    #[must_use]
    pub fn is_selected(&self) -> bool {
        self.selected
    }
}

/// External horizontal viewport metadata for column virtualization.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DataTableColumnViewport {
    scroll_offset: f32,
    viewport_width: f32,
    overscan: f32,
}

impl DataTableColumnViewport {
    /// Creates a horizontal viewport descriptor.
    #[must_use]
    pub fn new(scroll_offset: f32, viewport_width: f32) -> Self {
        Self {
            scroll_offset: if scroll_offset.is_finite() {
                scroll_offset.max(0.0)
            } else {
                0.0
            },
            viewport_width: if viewport_width.is_finite() {
                viewport_width.max(0.0)
            } else {
                0.0
            },
            overscan: 160.0,
        }
    }
    /// Sets overscan width in logical pixels.
    #[must_use]
    pub fn overscan(mut self, overscan: f32) -> Self {
        if overscan.is_finite() {
            self.overscan = overscan.max(0.0);
        }
        self
    }
    /// Returns horizontal scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }
    /// Returns viewport width.
    #[must_use]
    pub fn viewport_width(&self) -> f32 {
        self.viewport_width
    }
    /// Returns overscan width.
    #[must_use]
    pub fn overscan_width(&self) -> f32 {
        self.overscan
    }
}

/// Flattened visible row metadata for host-managed table interactions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibleDataRow {
    id: SharedString,
    index: usize,
    cells: Vec<SharedString>,
    selected: bool,
}

impl VisibleDataRow {
    fn new(row: &DataRow, index: usize) -> Self {
        Self {
            id: row.id.clone(),
            index,
            cells: row.cells.clone(),
            selected: row.selected,
        }
    }

    /// Returns the stable row identifier.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns the source row index.
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns the immutable visible cell slice.
    #[must_use]
    pub fn cells(&self) -> &[SharedString] {
        &self.cells
    }

    /// Returns whether the row is currently selected.
    #[must_use]
    pub fn is_selected(&self) -> bool {
        self.selected
    }
}

/// Directional intents for host-managed table keyboard traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataTableNavigation {
    /// Move to the previous row.
    Up,
    /// Move to the next row.
    Down,
    /// Move to the first row.
    Home,
    /// Move to the last row.
    End,
    /// Move up by roughly one viewport of rows.
    PageUp,
    /// Move down by roughly one viewport of rows.
    PageDown,
}

/// A host-applied result from [`DataTable::navigation_outcome`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataTableNavigationOutcome {
    /// Select the provided row identifier.
    Select(SharedString),
    /// No action is required for the requested navigation intent.
    Noop,
}

/// Row selection behavior for [`DataTable`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DataTableSelectionMode {
    /// Replace the current selection with one focused row.
    #[default]
    Single,
    /// Allow host-managed toggle and range selection.
    Multiple,
}

/// Selection intent emitted by pointer and keyboard interactions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataTableSelectionIntent {
    /// Replace the current selection with one row.
    Replace,
    /// Toggle one row in the current selection.
    Toggle,
    /// Select the range between the anchor row and focused row.
    Extend,
}

/// Host-applied row selection update for [`DataTable`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataTableSelection {
    intent: DataTableSelectionIntent,
    anchor_id: SharedString,
    focused_id: SharedString,
    selected_ids: Vec<SharedString>,
}

impl DataTableSelection {
    fn new(
        intent: DataTableSelectionIntent,
        anchor_id: SharedString,
        focused_id: SharedString,
        selected_ids: Vec<SharedString>,
    ) -> Self {
        Self {
            intent,
            anchor_id,
            focused_id,
            selected_ids,
        }
    }

    /// Returns the originating selection intent.
    #[must_use]
    pub fn intent(&self) -> DataTableSelectionIntent {
        self.intent
    }

    /// Returns the range anchor row identifier.
    #[must_use]
    pub fn anchor_id(&self) -> &SharedString {
        &self.anchor_id
    }

    /// Returns the focused row identifier.
    #[must_use]
    pub fn focused_id(&self) -> &SharedString {
        &self.focused_id
    }

    /// Returns selected row identifiers in table order.
    #[must_use]
    pub fn selected_ids(&self) -> &[SharedString] {
        &self.selected_ids
    }
}

/// Flattened visible column metadata for host-managed sizing interactions.
#[derive(Clone, Debug, PartialEq)]
pub struct VisibleDataColumn {
    id: SharedString,
    title: SharedString,
    width: Option<f32>,
    min_width: f32,
    align: DataColumnAlign,
    sortable: bool,
    pin: DataColumnPin,
}

impl VisibleDataColumn {
    fn new(column: &DataColumn) -> Self {
        Self {
            id: column.id.clone(),
            title: column.title.clone(),
            width: column.width,
            min_width: column.min_width,
            align: column.align,
            sortable: column.sortable,
            pin: column.pin,
        }
    }

    /// Returns the stable column identifier.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns the display title.
    #[must_use]
    pub fn title(&self) -> &SharedString {
        &self.title
    }

    /// Returns the explicit width, if one is set.
    #[must_use]
    pub fn width(&self) -> Option<f32> {
        self.width
    }

    /// Returns the enforced minimum width.
    #[must_use]
    pub fn min_width(&self) -> f32 {
        self.min_width
    }

    /// Returns whether the column participates in sorting.
    #[must_use]
    pub fn is_sortable(&self) -> bool {
        self.sortable
    }

    /// Returns the column alignment.
    #[must_use]
    pub fn align(&self) -> DataColumnAlign {
        self.align
    }

    /// Returns the column pin placement.
    #[must_use]
    pub fn pin(&self) -> DataColumnPin {
        self.pin
    }
}

/// A host-applied width update for a [`DataTable`] column.
#[derive(Clone, Debug, PartialEq)]
pub struct DataColumnResize {
    column_id: SharedString,
    width: f32,
}

impl DataColumnResize {
    /// Creates a new resize descriptor.
    #[must_use]
    pub fn new(column_id: impl Into<SharedString>, width: f32) -> Self {
        Self {
            column_id: column_id.into(),
            width: if width.is_finite() {
                width.max(48.0)
            } else {
                48.0
            },
        }
    }

    /// Returns the resized column identifier.
    #[must_use]
    pub fn column_id(&self) -> &SharedString {
        &self.column_id
    }

    /// Returns the clamped target width.
    #[must_use]
    pub fn width(&self) -> f32 {
        self.width
    }
}

/// Sort direction metadata for [`DataTable`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortDirection {
    /// Ascending order.
    Ascending,
    /// Descending order.
    Descending,
}

impl SortDirection {
    fn indicator(self) -> &'static str {
        match self {
            Self::Ascending => "ASC",
            Self::Descending => "DESC",
        }
    }

    fn toggle(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

/// Active sort information for a [`DataTable`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableSort {
    column_id: SharedString,
    direction: SortDirection,
}

impl TableSort {
    /// Creates a sort descriptor for the given column.
    #[must_use]
    pub fn new(column_id: impl Into<SharedString>, direction: SortDirection) -> Self {
        Self {
            column_id: column_id.into(),
            direction,
        }
    }

    /// Returns the sorted column identifier.
    #[must_use]
    pub fn column_id(&self) -> &SharedString {
        &self.column_id
    }

    /// Returns the active direction.
    #[must_use]
    pub fn direction(&self) -> SortDirection {
        self.direction
    }

    fn next_for(&self, column_id: &SharedString) -> Self {
        if self.column_id == *column_id {
            Self::new(column_id.clone(), self.direction.toggle())
        } else {
            Self::new(column_id.clone(), SortDirection::Ascending)
        }
    }
}

/// Column alignment in a [`DataTable`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DataColumnAlign {
    /// Left-align cell content.
    #[default]
    Start,
    /// Center-align cell content.
    Center,
    /// Right-align cell content.
    End,
}

/// Immutable table column metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct DataColumn {
    id: SharedString,
    title: SharedString,
    width: Option<f32>,
    min_width: f32,
    align: DataColumnAlign,
    sortable: bool,
    pin: DataColumnPin,
}

impl DataColumn {
    /// Creates a new table column.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            width: None,
            min_width: 48.0,
            align: DataColumnAlign::Start,
            sortable: false,
            pin: DataColumnPin::None,
        }
    }

    /// Sets an explicit column width in logical pixels.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        if width.is_finite() {
            self.width = Some(width.max(self.min_width));
        }
        self
    }

    /// Sets the minimum column width in logical pixels.
    #[must_use]
    pub fn min_width(mut self, min_width: f32) -> Self {
        if min_width.is_finite() {
            self.min_width = min_width.max(24.0);
            if let Some(width) = self.width {
                self.width = Some(width.max(self.min_width));
            }
        }
        self
    }

    /// Sets the cell alignment.
    #[must_use]
    pub fn align(mut self, align: DataColumnAlign) -> Self {
        self.align = align;
        self
    }

    /// Marks the column as sortable.
    #[must_use]
    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Pins the column to an edge outside the virtualized middle region.
    #[must_use]
    pub fn pin(mut self, pin: DataColumnPin) -> Self {
        self.pin = pin;
        self
    }

    /// Returns the stable column identifier.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns the explicit width, if one is set.
    #[must_use]
    pub fn width_value(&self) -> Option<f32> {
        self.width
    }

    /// Returns the enforced minimum width.
    #[must_use]
    pub fn min_width_value(&self) -> f32 {
        self.min_width
    }

    /// Returns the pin placement.
    #[must_use]
    pub fn pin_value(&self) -> DataColumnPin {
        self.pin
    }

    fn effective_width(&self) -> f32 {
        self.width.unwrap_or(self.min_width.max(120.0))
    }
}

/// External viewport metadata for virtualized [`DataTable`] rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DataTableViewport {
    scroll_offset: f32,
    viewport_height: f32,
    overscan: usize,
}

impl DataTableViewport {
    /// Creates a new viewport descriptor.
    #[must_use]
    pub fn new(scroll_offset: f32, viewport_height: f32) -> Self {
        Self {
            scroll_offset: if scroll_offset.is_finite() {
                scroll_offset.max(0.0)
            } else {
                0.0
            },
            viewport_height: if viewport_height.is_finite() {
                viewport_height.max(0.0)
            } else {
                0.0
            },
            overscan: 4,
        }
    }

    /// Sets the overscan row count.
    #[must_use]
    pub fn overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    /// Returns the scroll offset in logical pixels.
    #[must_use]
    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    /// Returns the viewport height in logical pixels.
    #[must_use]
    pub fn viewport_height(&self) -> f32 {
        self.viewport_height
    }

    /// Returns the overscan row count.
    #[must_use]
    pub fn overscan_rows(&self) -> usize {
        self.overscan
    }
}

/// Immutable row data for a [`DataTable`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataRow {
    id: SharedString,
    cells: Vec<SharedString>,
    selected: bool,
}

impl DataRow {
    /// Creates a new row.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, cells: Vec<impl Into<SharedString>>) -> Self {
        Self {
            id: id.into(),
            cells: cells.into_iter().map(Into::into).collect(),
            selected: false,
        }
    }

    /// Marks the row as selected.
    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Returns the immutable cell slice for the row.
    #[must_use]
    pub fn cells(&self) -> &[SharedString] {
        &self.cells
    }

    /// Returns the stable identifier for the row.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns whether the row is currently selected.
    #[must_use]
    pub fn is_selected(&self) -> bool {
        self.selected
    }
}

/// Visual state for a [`DataTable`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum DataTableState {
    /// Render data rows normally.
    #[default]
    Ready,
    /// Render a loading placeholder.
    Loading {
        /// Optional supporting detail.
        detail: Option<SharedString>,
    },
    /// Render an empty-state placeholder.
    Empty {
        /// User-facing empty-state message.
        message: SharedString,
    },
    /// Render an error-state placeholder.
    Error {
        /// User-facing error-state message.
        message: SharedString,
    },
}

/// Immutable row data and selection metadata shared between table frames.
///
/// Build this once when the dataset changes, then pass cheap clones to
/// [`DataTable::shared_rows`]. Rendering does not scan or clone offscreen rows.
#[derive(Clone, Debug, Default)]
pub struct DataTableRows {
    rows: Rc<[DataRow]>,
    ids: Rc<[SharedString]>,
    positions: Rc<HashMap<SharedString, usize>>,
    selected: Rc<[SharedString]>,
}

impl DataTableRows {
    /// Indexes a dataset once for reuse across renders.
    #[must_use]
    pub fn new(rows: Vec<DataRow>) -> Self {
        let mut positions = HashMap::with_capacity(rows.len());
        for (index, row) in rows.iter().enumerate() {
            positions.entry(row.id.clone()).or_insert(index);
        }
        let ids = rows.iter().map(|row| row.id.clone()).collect();
        let selected = rows
            .iter()
            .filter(|row| row.selected)
            .map(|row| row.id.clone())
            .collect();
        Self {
            rows: rows.into(),
            positions: Rc::new(positions),
            ids,
            selected,
        }
    }
    /// Looks up a stable row identifier in display order. If IDs repeat, the first wins.
    #[must_use]
    pub fn row_index(&self, id: &str) -> Option<usize> {
        self.positions.get(id).copied()
    }

    /// Returns the immutable rows in display order.
    #[must_use]
    pub fn as_slice(&self) -> &[DataRow] {
        &self.rows
    }
}
impl std::ops::Deref for DataTableRows {
    type Target = [DataRow];
    fn deref(&self) -> &Self::Target {
        &self.rows
    }
}

/// A controlled table for structured row and column data.
///
/// The current implementation focuses on stable layout, selection styling,
/// explicit loading, empty, and error states, plus externally managed row
/// virtualization for large datasets.
///
/// # Example
///
/// ```no_run
/// use guic_components::{DataColumn, DataRow, DataTable, SortDirection, TableSort};
///
/// let table = DataTable::new("releases")
///     .columns(vec![
///         DataColumn::new("name", "Name"),
///         DataColumn::new("status", "Status"),
///     ])
///     .rows(vec![
///         DataRow::new("v0.1", vec!["v0.1", "Shipped"]).selected(true),
///         DataRow::new("v0.2", vec!["v0.2", "Planned"]),
///     ])
///     .sort(TableSort::new("name", SortDirection::Ascending));
/// ```
#[derive(gpui::IntoElement)]
pub struct DataTable {
    id: SharedString,
    title: Option<SharedString>,
    columns: Vec<DataColumn>,
    column_order: Vec<SharedString>,
    on_column_order_change: Option<OrderHandler>,
    rows: DataTableRows,
    state: DataTableState,
    sort: Option<TableSort>,
    on_sort: Option<SortHandler>,
    on_row_select: Option<SharedStringHandler>,
    on_row_selection: Option<SelectionHandler>,
    selection_mode: DataTableSelectionMode,
    selection_cursor: Option<(SharedString, SharedString)>,
    max_height: Option<f32>,
    row_height: f32,
    viewport: Option<DataTableViewport>,
    scroll_handle: Option<ScrollHandle>,
    virtual_scroll: Option<UniformListScrollHandle>,
    column_viewport: Option<DataTableColumnViewport>,
    striped: bool,
    focus_handle: Option<FocusHandle>,
    on_column_resize: Option<ColumnResizeHandler>,
    cell_renderer: Option<CellRenderer>,
    row_actions_renderer: Option<RowActionsRenderer>,
    row_actions_width: f32,
}

impl DataTable {
    /// Creates a new table.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: None,
            columns: Vec::new(),
            column_order: Vec::new(),
            on_column_order_change: None,
            rows: DataTableRows::default(),
            state: DataTableState::Ready,
            sort: None,
            on_sort: None,
            on_row_select: None,
            on_row_selection: None,
            selection_mode: DataTableSelectionMode::Single,
            selection_cursor: None,
            max_height: None,
            row_height: 40.0,
            viewport: None,
            scroll_handle: None,
            virtual_scroll: None,
            column_viewport: None,
            striped: true,
            focus_handle: None,
            on_column_resize: None,
            cell_renderer: None,
            row_actions_renderer: None,
            row_actions_width: 112.0,
        }
    }

    /// Sets an optional title above the table.
    #[must_use]
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Replaces the column definitions.
    #[must_use]
    pub fn columns(mut self, columns: Vec<DataColumn>) -> Self {
        self.columns = columns;
        self
    }

    /// Sets a presentation order by column ID. Unknown and repeated IDs are ignored.
    /// Unlisted columns retain declaration order. Row cells always use declaration order.
    #[must_use]
    pub fn column_order(mut self, ids: Vec<SharedString>) -> Self {
        self.column_order = ids;
        self
    }

    /// Enables header dragging and Alt+Left/Right moves within each pin group.
    /// Store the emitted full order and pass it back through `column_order`.
    #[must_use]
    pub fn on_column_order_change(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_column_order_change = Some(Rc::new(handler));
        self
    }

    /// Replaces the row data.
    #[must_use]
    pub fn rows(mut self, rows: Vec<DataRow>) -> Self {
        self.rows = DataTableRows::new(rows);
        self
    }

    /// Uses an indexed dataset without cloning rows on every frame.
    #[must_use]
    pub fn shared_rows(mut self, rows: DataTableRows) -> Self {
        self.rows = rows;
        self
    }

    /// Sets the table surface state.
    #[must_use]
    pub fn state(mut self, state: DataTableState) -> Self {
        self.state = state;
        self
    }

    /// Adds sort metadata to the header row.
    #[must_use]
    pub fn sort(mut self, sort: TableSort) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Invokes a callback when a sortable column header is clicked.
    #[must_use]
    pub fn on_sort(
        mut self,
        handler: impl Fn(&TableSort, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_sort = Some(Rc::new(handler));
        self
    }

    /// Invokes a callback when a row is selected.
    #[must_use]
    pub fn on_row_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_row_select = Some(Rc::new(handler));
        self
    }

    /// Sets the row selection mode.
    #[must_use]
    pub fn selection_mode(mut self, selection_mode: DataTableSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    /// Invokes a callback when a row selection update is requested.
    ///
    /// This callback is the richer selection counterpart to
    /// [`Self::on_row_select`]. It emits table-ordered selected row identifiers
    /// for replace, toggle, and range selection intents.
    #[must_use]
    pub fn on_row_selection(
        mut self,
        handler: impl Fn(&DataTableSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_row_selection = Some(Rc::new(handler));
        self
    }

    /// Restores the anchor and focused row from a previous selection callback.
    /// This preserves the direction of a keyboard range when the host redraws.
    /// Removed rows fall back to the first selected row.
    #[must_use]
    pub fn selection_cursor(mut self, selection: &DataTableSelection) -> Self {
        self.selection_cursor = Some((selection.anchor_id.clone(), selection.focused_id.clone()));
        self
    }

    /// Caps the scrollable table body height in logical pixels.
    #[must_use]
    pub fn max_height(mut self, height: f32) -> Self {
        if height.is_finite() {
            self.max_height = Some(height.max(120.0));
        }
        self
    }

    /// Sets the expected row height used for virtualization math.
    #[must_use]
    pub fn row_height(mut self, row_height: f32) -> Self {
        if row_height.is_finite() {
            self.row_height = row_height.max(24.0);
        }
        self
    }

    /// Applies externally managed viewport metadata for row virtualization.
    #[must_use]
    pub fn viewport(mut self, viewport: DataTableViewport) -> Self {
        self.virtual_scroll = None;
        self.viewport = Some(viewport);
        self
    }

    /// Renders fixed-height rows on demand inside a scrollable body of the given height.
    /// Retain the handle to reveal rows from application code. The runtime determines
    /// the visible range during layout, including after scrolling or resizing.
    #[must_use]
    pub fn virtualized(mut self, height: f32, handle: UniformListScrollHandle) -> Self {
        if height.is_finite() && height > 0.0 {
            self.max_height = Some(height);
            self.viewport = None;
            self.virtual_scroll = Some(handle);
        }
        self
    }

    /// Tracks the body scroll position when [`Self::max_height`] is set.
    ///
    /// For row virtualization, pass the negated vertical offset to
    /// [`DataTableViewport::new`] on each render.
    #[must_use]
    pub fn track_scroll(mut self, handle: ScrollHandle) -> Self {
        self.scroll_handle = Some(handle);
        self
    }

    /// Applies externally managed horizontal viewport metadata.
    #[must_use]
    pub fn column_viewport(mut self, viewport: DataTableColumnViewport) -> Self {
        self.column_viewport = Some(viewport);
        self
    }

    /// Handles interactive column resize requests.
    #[must_use]
    pub fn on_column_resize(
        mut self,
        handler: impl Fn(&DataColumnResize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_column_resize = Some(Rc::new(handler));
        self
    }

    /// Sets a custom renderer for body cells.
    #[must_use]
    pub fn render_cell(
        mut self,
        renderer: impl Fn(&DataTableCell) -> AnyElement + 'static,
    ) -> Self {
        self.cell_renderer = Some(Rc::new(renderer));
        self
    }

    /// Sets a trailing renderer for row-specific actions.
    #[must_use]
    pub fn render_row_actions(
        mut self,
        renderer: impl Fn(&DataRow) -> AnyElement + 'static,
    ) -> Self {
        self.row_actions_renderer = Some(Rc::new(renderer));
        self
    }

    /// Sets the fixed width reserved for the trailing row actions region.
    #[must_use]
    pub fn row_actions_width(mut self, width: f32) -> Self {
        if width.is_finite() {
            self.row_actions_width = width.max(48.0);
        }
        self
    }

    /// Enables or disables striped row backgrounds.
    #[must_use]
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }

    /// Makes the table body keyboard-focusable so that arrow, page, and
    /// `Home`/`End` keys move selection through [`Self::on_row_select`] or
    /// [`Self::on_row_selection`].
    ///
    /// The host owns the [`FocusHandle`] (typically created once with
    /// `cx.focus_handle()`) so focus survives across re-renders. Keyboard
    /// navigation is only wired up when both a focus handle and a row-selection
    /// handler are present.
    #[must_use]
    pub fn focusable(mut self, focus_handle: FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle);
        self
    }

    /// Returns visible column metadata in render order.
    #[must_use]
    pub fn visible_column_models(&self) -> Vec<VisibleDataColumn> {
        self.visible_columns()
            .indices
            .into_iter()
            .map(|index| VisibleDataColumn::new(&self.columns[index]))
            .collect()
    }

    /// Returns the explicit width of a column, if one is set.
    #[must_use]
    pub fn column_width(&self, column_id: &str) -> Option<f32> {
        self.columns
            .iter()
            .find(|column| column.id.as_ref() == column_id)
            .and_then(|column| column.width)
    }

    /// Returns a host-applied resize descriptor with minimum-width clamping.
    #[must_use]
    pub fn resized_column(&self, column_id: &str, proposed_width: f32) -> Option<DataColumnResize> {
        let column = self
            .columns
            .iter()
            .find(|column| column.id.as_ref() == column_id)?;
        Some(DataColumnResize::new(
            column.id.clone(),
            proposed_width.max(column.min_width),
        ))
    }

    /// Applies a host-managed column width update to the table model.
    #[must_use]
    pub fn apply_column_resize(mut self, resize: DataColumnResize) -> Self {
        for column in &mut self.columns {
            if column.id == *resize.column_id() {
                column.width = Some(resize.width().max(column.min_width));
            }
        }
        self
    }

    fn state_message(&self) -> Option<(SharedString, bool)> {
        match &self.state {
            DataTableState::Ready => None,
            DataTableState::Loading { detail } => Some((
                detail
                    .clone()
                    .unwrap_or_else(|| SharedString::from("Loading rows...")),
                false,
            )),
            DataTableState::Empty { message } => Some((message.clone(), false)),
            DataTableState::Error { message } => Some((message.clone(), true)),
        }
    }

    fn visible_rows(&self) -> (usize, usize, f32, f32) {
        let viewport = self.viewport.or_else(|| {
            let (offset, height) = crate::virtual_list::tracked_viewport(
                self.virtual_scroll.as_ref()?,
                self.max_height?,
                self.row_height,
                self.rows.len(),
            );
            Some(DataTableViewport::new(offset, height).overscan(0))
        });
        let Some(viewport) = viewport else {
            return (0, self.rows.len(), 0.0, 0.0);
        };

        let metrics = VirtualListMetrics::new(
            self.row_height,
            viewport.viewport_height(),
            viewport.overscan_rows(),
            self.rows.len(),
        );
        let range = metrics.visible_range(viewport.scroll_offset());
        let start = range.start.min(self.rows.len());
        let end = range.end.min(self.rows.len());
        let top_spacer = start as f32 * self.row_height;
        let bottom_spacer = (metrics.total_height() - end as f32 * self.row_height).max(0.0);
        (start, end, top_spacer, bottom_spacer)
    }

    /// Returns the selected row identifier, if any.
    #[must_use]
    pub fn selected_row_id(&self) -> Option<SharedString> {
        self.rows.selected.first().cloned()
    }

    /// Returns selected row identifiers in table order.
    #[must_use]
    pub fn selected_row_ids(&self) -> Vec<SharedString> {
        self.rows.selected.to_vec()
    }

    /// Returns visible row metadata in render order.
    #[must_use]
    pub fn visible_row_models(&self) -> Vec<VisibleDataRow> {
        let (start, end, _, _) = self.visible_rows();
        self.rows
            .iter()
            .enumerate()
            .skip(start)
            .take(end.saturating_sub(start))
            .map(|(index, row)| VisibleDataRow::new(row, index))
            .collect()
    }

    /// Returns the visible row identifiers in render order.
    #[must_use]
    pub fn visible_row_ids(&self) -> Vec<SharedString> {
        let (start, end, _, _) = self.visible_rows();
        self.rows.ids[start..end].to_vec()
    }

    /// Returns the next row identifier after the provided one.
    #[must_use]
    pub fn next_row_id(&self, current: &str) -> Option<SharedString> {
        row_navigation_target(&self.rows, current, DataTableNavigation::Down, 1)
    }

    /// Returns the previous row identifier before the provided one.
    #[must_use]
    pub fn previous_row_id(&self, current: &str) -> Option<SharedString> {
        row_navigation_target(&self.rows, current, DataTableNavigation::Up, 1)
    }

    /// Returns the host-applied outcome for a directional navigation intent.
    #[must_use]
    pub fn navigation_outcome(
        &self,
        current: &str,
        navigation: DataTableNavigation,
    ) -> DataTableNavigationOutcome {
        row_navigation_target(&self.rows, current, navigation, self.page_step())
            .map_or(DataTableNavigationOutcome::Noop, |id| {
                DataTableNavigationOutcome::Select(id)
            })
    }

    /// Returns the row identifiers between two rows, inclusive, in table order.
    #[must_use]
    pub fn row_range_ids(&self, anchor_id: &str, focused_id: &str) -> Vec<SharedString> {
        row_range_ids(&self.rows, anchor_id, focused_id)
    }

    /// Returns a host-applied selection update for a row interaction.
    #[must_use]
    pub fn selection_change(
        &self,
        row_id: &str,
        intent: DataTableSelectionIntent,
    ) -> Option<DataTableSelection> {
        selection_change_for(
            &self.rows,
            self.anchor(),
            row_id,
            intent,
            self.selection_mode,
        )
    }

    fn anchor(&self) -> Option<&SharedString> {
        self.selection_cursor
            .as_ref()
            .map(|(anchor, _)| anchor)
            .filter(|id| self.rows.row_index(id).is_some())
            .or_else(|| self.rows.selected.first())
    }

    fn page_step(&self) -> usize {
        self.viewport
            .map(|viewport| viewport.viewport_height())
            .or_else(|| self.virtual_scroll.as_ref().and(self.max_height))
            .map_or(10, |height| {
                ((height / self.row_height).floor() as usize).max(1)
            })
    }

    fn visible_columns(&self) -> VisibleColumns {
        let mut order: Vec<_> = (0..self.columns.len()).collect();
        if !self.column_order.is_empty() {
            let mut priorities = HashMap::new();
            for (i, id) in self.column_order.iter().enumerate() {
                priorities.entry(id).or_insert(i);
            }
            order.sort_by_key(|&i| {
                priorities
                    .get(&self.columns[i].id)
                    .copied()
                    .unwrap_or(usize::MAX)
            });
        }
        order.sort_by_key(|&i| match self.columns[i].pin {
            DataColumnPin::Start => 0,
            DataColumnPin::None => 1,
            DataColumnPin::End => 2,
        });
        let mut positions = vec![0; order.len()];
        let mut indices = Vec::new();
        let mut first_middle_offset = 0.;
        let mut middle_offset = 0.;
        let mut first_middle = true;
        for (position, &index) in order.iter().enumerate() {
            positions[index] = position;
            let column = &self.columns[index];
            match column.pin {
                DataColumnPin::Start | DataColumnPin::End => indices.push(index),
                DataColumnPin::None => {
                    let column_start = middle_offset;
                    let column_end = column_start + column.effective_width();
                    middle_offset = column_end;
                    let visible = self.column_viewport.is_none_or(|viewport| {
                        let viewport_start = (viewport.scroll_offset - viewport.overscan).max(0.0);
                        let viewport_end =
                            viewport.scroll_offset + viewport.viewport_width + viewport.overscan;
                        column_end > viewport_start && column_start < viewport_end
                    });
                    if visible {
                        if first_middle && let Some(viewport) = self.column_viewport {
                            first_middle_offset = column_start - viewport.scroll_offset;
                        }
                        first_middle = false;
                        indices.push(index);
                    }
                }
            }
        }

        let clipped = self.column_viewport.is_some()
            || self.columns.iter().any(|c| c.pin != DataColumnPin::None);
        VisibleColumns {
            order,
            positions,
            indices,
            first_middle_offset,
            clipped,
        }
    }
}

struct VisibleColumns {
    order: Vec<usize>,
    positions: Vec<usize>,
    indices: Vec<usize>,
    first_middle_offset: f32,
    clipped: bool,
}

impl VisibleColumns {
    fn middle(&self, cells: gpui::Div) -> gpui::Div {
        if self.first_middle_offset != 0.0 {
            div()
                .min_w_0()
                .flex_1()
                .overflow_hidden()
                .flex()
                .child(cells.ml(px(self.first_middle_offset)))
        } else if self.clipped {
            cells.overflow_hidden()
        } else {
            cells
        }
    }
}

impl DataTable {
    fn render_row(
        &self,
        row_index: usize,
        columns: &VisibleColumns,
        theme: &Theme,
    ) -> impl IntoElement + use<> {
        let row = &self.rows[row_index];
        let selected = row.selected;
        let stripe = self.striped && row_index % 2 == 1;
        let row_id = row.id.clone();
        let mut row_view = div()
            .id(row.id.clone())
            .role(gpui::Role::Row)
            .aria_row_index(row_index + 1)
            .aria_selected(selected)
            .debug_selector({
                let row_id = row_id.clone();
                move || format!("guic-data-table-row-{row_id}")
            })
            .w_full()
            .min_h(px(self.row_height))
            .flex()
            .items_center()
            .border_b_1()
            .border_color(theme.border().opacity(0.55))
            .bg(if selected {
                theme.primary().opacity(0.12)
            } else if stripe {
                theme.secondary().opacity(0.08)
            } else {
                theme.background()
            });
        if self.viewport.is_some() || self.virtual_scroll.is_some() {
            row_view = row_view
                .h(px(self.row_height))
                .flex_shrink_0()
                .overflow_hidden();
        }
        let mut row_start = div().flex().flex_shrink_0();
        let mut row_middle = div().min_w_0().flex_1().flex();
        let mut row_end = div().flex().flex_shrink_0();

        for &column_index in &columns.indices {
            let column = &self.columns[column_index];
            let cell = row
                .cells
                .get(column_index)
                .cloned()
                .unwrap_or_else(|| SharedString::from(""));
            let width = if self.column_viewport.is_some() || column.pin != DataColumnPin::None {
                Some(column.effective_width())
            } else {
                column.width
            };
            let content = self.cell_renderer.as_ref().map(|renderer| {
                renderer(&DataTableCell {
                    row_id: row.id.clone(),
                    column_id: column.id.clone(),
                    row_index,
                    column_index,
                    value: cell.clone(),
                    selected,
                })
            });
            let cell = render_body_cell(cell, content, width, column.align, selected, theme)
                .id(("cell", column_index))
                .role(gpui::Role::Cell)
                .aria_column_index(columns.positions[column_index])
                .into_any_element();
            match column.pin {
                DataColumnPin::Start => row_start = row_start.child(cell),
                DataColumnPin::None => row_middle = row_middle.child(cell),
                DataColumnPin::End => row_end = row_end.child(cell),
            }
        }

        if let Some(renderer) = &self.row_actions_renderer {
            row_end = row_end.child(
                div()
                    .id("actions")
                    .role(gpui::Role::Cell)
                    .aria_column_index(self.columns.len())
                    .w(px(self.row_actions_width))
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .justify_end()
                    .child(renderer(row)),
            );
        }
        row_view = row_view
            .child(row_start)
            .child(columns.middle(row_middle))
            .child(row_end);

        if self.on_row_select.is_some() || self.on_row_selection.is_some() {
            let row_handler = self.on_row_select.clone();
            let selection_handler = self.on_row_selection.clone();
            let rows = self.rows.clone();
            let anchor = self.anchor().cloned();
            let selection_mode = self.selection_mode;
            row_view = row_view
                .cursor_pointer()
                .on_click(move |event: &ClickEvent, window, cx| {
                    let modifiers = event.modifiers();
                    let intent = if modifiers.shift {
                        DataTableSelectionIntent::Extend
                    } else if modifiers.platform || modifiers.control {
                        DataTableSelectionIntent::Toggle
                    } else {
                        DataTableSelectionIntent::Replace
                    };
                    emit_row_selection(
                        &row_id,
                        intent,
                        &rows,
                        anchor.as_ref(),
                        selection_mode,
                        row_handler.as_ref(),
                        selection_handler.as_ref(),
                        window,
                        cx,
                    );
                });
        }

        row_view
    }
}

impl RenderOnce for DataTable {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx).clone();
        let state_message = self.state_message();
        let sort = self.sort.clone();
        let columns = self.visible_columns();
        let moves = self.on_column_order_change.as_ref().map(|change| {
            Rc::new(ColumnMoves {
                owner: _window
                    .use_keyed_state(format!("{}-column-order", self.id), cx, |_, _| ())
                    .entity_id(),
                order: columns
                    .order
                    .iter()
                    .map(|&i| (self.columns[i].id.clone(), self.columns[i].pin))
                    .collect(),
                change: change.clone(),
            })
        });
        let (start, end, top_spacer, bottom_spacer) = self.visible_rows();
        let focus_handle = self.focus_handle.clone();
        let mut root = div()
            .id(self.id.clone())
            .role(gpui::Role::Table)
            .aria_row_count(self.rows.len().saturating_add(1))
            .aria_column_count(
                self.columns.len() + usize::from(self.row_actions_renderer.is_some()),
            )
            .w_full()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .flex()
            .flex_col()
            .overflow_hidden();

        if let Some(handle) = focus_handle {
            root = root.key_context("GuicDataTable").track_focus(&handle);
        }

        if self.focus_handle.is_some()
            && (self.on_row_select.is_some() || self.on_row_selection.is_some())
        {
            let rows = self.rows.clone();
            let focused = self
                .selection_cursor
                .as_ref()
                .map(|(_, focus)| focus)
                .filter(|id| rows.row_index(id).is_some())
                .cloned()
                .or_else(|| self.selected_row_id());
            let anchor = self.anchor().cloned();
            let scroll = self.virtual_scroll.clone();
            let page_step = self.page_step();
            let selection_mode = self.selection_mode;
            let row_handler = self.on_row_select.clone();
            let selection_handler = self.on_row_selection.clone();
            root = root.on_key_down(move |event: &KeyDownEvent, window, cx| {
                let Some(navigation) = data_table_navigation_for(&event.keystroke) else {
                    return;
                };
                let target = if let Some(current) = &focused {
                    row_navigation_target(&rows, current, navigation, page_step)
                } else {
                    match navigation {
                        DataTableNavigation::Up
                        | DataTableNavigation::End
                        | DataTableNavigation::PageUp => rows.ids.last().cloned(),
                        _ => rows.ids.first().cloned(),
                    }
                };
                if let Some(target) = target {
                    if let Some(handle) = &scroll
                        && let Some(index) = rows.row_index(&target)
                    {
                        handle.scroll_to_item(index, gpui::ScrollStrategy::Nearest);
                    }
                    let intent = if focused.is_some() && event.keystroke.modifiers.shift {
                        DataTableSelectionIntent::Extend
                    } else {
                        DataTableSelectionIntent::Replace
                    };
                    emit_row_selection(
                        &target,
                        intent,
                        &rows,
                        anchor.as_ref(),
                        selection_mode,
                        row_handler.as_ref(),
                        selection_handler.as_ref(),
                        window,
                        cx,
                    );
                    cx.stop_propagation();
                }
            });
        }

        if let Some(title) = self.title.clone() {
            root = root.aria_label(title.clone());
            root = root.child(
                div()
                    .px_4()
                    .py_3()
                    .bg(theme.secondary().opacity(0.18))
                    .child(Label::new(title).muted(true)),
            );
        }

        let mut header = div()
            .id("header")
            .role(gpui::Role::Row)
            .aria_row_index(0)
            .w_full()
            .flex()
            .items_center()
            .bg(theme.secondary().opacity(0.26))
            .border_b_1()
            .border_color(theme.border());
        let mut header_start = div().flex().flex_shrink_0();
        let mut header_middle = div().min_w_0().flex_1().flex();
        let mut header_end = div().flex().flex_shrink_0();

        for &column_index in &columns.indices {
            let column = &self.columns[column_index];
            let mut title = column.title.to_string();
            if let Some(active_sort) = &sort
                && active_sort.column_id == column.id
            {
                title = format!("{title} ({})", active_sort.direction.indicator());
            }

            let next_sort = sort.as_ref().map_or_else(
                || TableSort::new(column.id.clone(), SortDirection::Ascending),
                |active_sort| active_sort.next_for(&column.id),
            );
            let cell = self.render_header_cell(
                &title,
                column_index,
                columns.positions[column_index],
                next_sort,
                moves.as_ref(),
                &theme,
            );
            match column.pin {
                DataColumnPin::Start => header_start = header_start.child(cell),
                DataColumnPin::None => header_middle = header_middle.child(cell),
                DataColumnPin::End => header_end = header_end.child(cell),
            }
        }

        if self.row_actions_renderer.is_some() {
            header_end = header_end.child(
                render_cell_base(
                    Some(self.row_actions_width),
                    DataColumnAlign::End,
                    true,
                    false,
                    &theme,
                )
                .id("actions-header")
                .role(gpui::Role::ColumnHeader)
                .aria_column_index(self.columns.len())
                .aria_label("Actions")
                .child("Actions")
                .into_any_element(),
            );
        }
        header = header
            .child(header_start)
            .child(columns.middle(header_middle))
            .child(header_end);

        let body = if let Some((message, is_error)) = state_message {
            div()
                .w_full()
                .min_h(px(120.0))
                .px_4()
                .py_6()
                .flex()
                .items_center()
                .justify_center()
                .child(Label::new(message).muted(!is_error).into_any_element())
                .into_any_element()
        } else if self.rows.is_empty() {
            div()
                .w_full()
                .min_h(px(120.0))
                .px_4()
                .py_6()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Label::new(guic_core::translate(cx, "table.empty", "No rows available"))
                        .muted(true),
                )
                .into_any_element()
        } else if let Some(handle) = self.virtual_scroll.clone() {
            let id = self.id.clone();
            let height = self
                .max_height
                .expect("virtualized tables have a body height");
            let row_height = self.row_height;
            let list = uniform_list("body-rows", self.rows.len(), move |range, _, _| {
                range
                    .map(|index| self.render_row(index, &columns, &theme))
                    .collect()
            })
            .with_item_size(gpui::size(px(0.0), px(row_height)))
            .track_scroll(&handle)
            .size_full();
            div()
                .debug_selector(move || format!("guic-data-table-body-{id}"))
                .w_full()
                .h(px(height))
                .child(list)
                .into_any_element()
        } else {
            let mut rows = div().w_full().flex().flex_col();
            if top_spacer > 0.0 {
                rows = rows.child(div().w_full().h(px(top_spacer)).flex_shrink_0());
            }

            for row_index in start..end {
                rows = rows.child(self.render_row(row_index, &columns, &theme));
            }

            if bottom_spacer > 0.0 {
                rows = rows.child(div().w_full().h(px(bottom_spacer)).flex_shrink_0());
            }

            let mut body = div().w_full().child(rows);
            if let Some(max_height) = self.max_height {
                let mut scroll = ScrollArea::new("guic-data-table-scroll", body)
                    .vertical(true)
                    .horizontal(true);
                if let Some(handle) = self.scroll_handle {
                    scroll = scroll.track_scroll(handle);
                }
                body = div()
                    .debug_selector(|| format!("guic-data-table-body-{}", self.id))
                    .w_full()
                    .h(px(max_height))
                    .child(scroll);
            }
            body.into_any_element()
        };

        root.child(header).child(Separator::new()).child(body)
    }
}

impl DataTable {
    fn render_header_cell(
        &self,
        value: &str,
        column_index: usize,
        position: usize,
        next_sort: TableSort,
        moves: Option<&Rc<ColumnMoves>>,
        theme: &Theme,
    ) -> gpui::AnyElement {
        let column = &self.columns[column_index];
        let width = if self.column_viewport.is_some() || column.pin != DataColumnPin::None {
            Some(column.effective_width())
        } else {
            column.width
        };
        let mut content = render_cell_base(width, column.align, true, false, theme)
            .id(("column-header", column_index))
            .role(gpui::Role::ColumnHeader)
            .aria_label(value.to_owned())
            .aria_column_index(position)
            .child(value.to_owned());
        if let Some(handler) = self.on_sort.clone().filter(|_| column.sortable) {
            content = content
                .debug_selector({
                    let column_id = column.id.clone();
                    move || format!("guic-data-table-header-{column_id}")
                })
                .cursor_pointer()
                .on_click(move |_: &ClickEvent, window, cx| handler(&next_sort, window, cx));
        }

        if let Some(moves) = moves {
            let source = ColumnDrag {
                owner: moves.owner,
                id: column.id.clone(),
                title: column.title.clone(),
            };
            let drop_moves = moves.clone();
            let key_moves = moves.clone();
            let id = column.id.clone();
            content = content
                .tab_index(0)
                .debug_selector(move || format!("guic-data-table-header-{id}"))
                .focus(|style| style.border_1().border_color(theme.primary()))
                .on_drag(source, |payload, _, _, cx| cx.new(|_| payload.clone()))
                .on_drop(move |payload: &ColumnDrag, window, cx| {
                    if payload.owner == drop_moves.owner
                        && let Some(from) = drop_moves
                            .order
                            .iter()
                            .position(|(id, _)| id == &payload.id)
                    {
                        drop_moves.apply(from, position, window, cx);
                    }
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.modifiers.alt {
                        let to = match event.keystroke.key.as_str() {
                            "left" => position.saturating_sub(1),
                            "right" => (position + 1).min(key_moves.order.len() - 1),
                            _ => return,
                        };
                        key_moves.apply(position, to, window, cx);
                        cx.stop_propagation();
                    }
                });
        }

        let Some(resize_handler) = self.on_column_resize.clone() else {
            return content.into_any_element();
        };

        let bounds = Rc::new(Cell::new(None::<Bounds<Pixels>>));
        let bounds_sink = bounds.clone();
        let bounds_canvas = canvas(
            move |cell_bounds, _window, _cx| bounds_sink.set(Some(cell_bounds)),
            |_bounds, _state, _window, _cx| {},
        )
        .absolute()
        .inset_0();
        let column_id = column.id.clone();
        let min_width = column.min_width;
        let current_width = column.effective_width();
        let selector = format!("guic-data-table-resize-{}", column.id);
        let handle = div()
            .id(selector.clone())
            .debug_selector(move || selector.clone())
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(px(8.0))
            .cursor_col_resize()
            .bg(gpui::transparent_black())
            .hover(|style: gpui::StyleRefinement| style.bg(gpui::black().opacity(0.08)))
            .on_click({
                let column_id = column_id.clone();
                let resize_handler = resize_handler.clone();
                move |_: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    resize_handler(
                        &DataColumnResize::new(column_id.clone(), current_width + 24.0),
                        window,
                        cx,
                    );
                }
            })
            .on_drag_update(move |_, event: &MouseMoveEvent, window, cx| {
                let Some(cell_bounds) = bounds.get() else {
                    return;
                };
                let proposed = f32::from(event.position.x - cell_bounds.origin.x).max(min_width);
                resize_handler(
                    &DataColumnResize::new(column_id.clone(), proposed),
                    window,
                    cx,
                );
            });

        div()
            .relative()
            .flex()
            .w(px(width.unwrap_or_else(|| column.effective_width())))
            .flex_shrink_0()
            .child(bounds_canvas)
            .child(div().w_full().child(content))
            .child(handle)
            .into_any_element()
    }
}

fn render_body_cell(
    value: SharedString,
    content: Option<AnyElement>,
    width: Option<f32>,
    align: DataColumnAlign,
    selected: bool,
    theme: &Theme,
) -> gpui::Div {
    let cell = render_cell_base(width, align, false, selected, theme);
    if let Some(content) = content {
        cell.child(content)
    } else {
        cell.child(value)
    }
}

fn render_cell_base(
    width: Option<f32>,
    align: DataColumnAlign,
    header: bool,
    selected: bool,
    theme: &Theme,
) -> gpui::Div {
    let mut cell = div()
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .text_color(if selected && !header {
            theme.foreground()
        } else if header {
            theme.muted_foreground()
        } else {
            theme.foreground()
        })
        .text_size(px(if header {
            theme.typography.text_sm
        } else {
            theme.typography.text_md
        }));

    cell = match align {
        DataColumnAlign::Start => cell.justify_start(),
        DataColumnAlign::Center => cell.justify_center(),
        DataColumnAlign::End => cell.justify_end(),
    };

    cell = if let Some(width) = width {
        cell.w(px(width)).flex_shrink_0()
    } else {
        cell.flex_1()
    };

    cell
}

fn row_navigation_target(
    rows: &DataTableRows,
    current: &str,
    navigation: DataTableNavigation,
    page_step: usize,
) -> Option<SharedString> {
    let ids = &rows.ids;
    let index = rows.row_index(current)?;
    let target_index = match navigation {
        DataTableNavigation::Up => index.checked_sub(1),
        DataTableNavigation::Down => index.checked_add(1),
        DataTableNavigation::Home => Some(0),
        DataTableNavigation::End => ids.len().checked_sub(1),
        DataTableNavigation::PageUp => Some(index.saturating_sub(page_step)),
        DataTableNavigation::PageDown => Some(
            index
                .saturating_add(page_step)
                .min(ids.len().saturating_sub(1)),
        ),
    };
    target_index.and_then(|index| ids.get(index)).cloned()
}

fn row_range_ids(rows: &DataTableRows, anchor_id: &str, focused_id: &str) -> Vec<SharedString> {
    let Some(anchor_index) = rows.row_index(anchor_id) else {
        return Vec::new();
    };
    let Some(focused_index) = rows.row_index(focused_id) else {
        return Vec::new();
    };
    let start = anchor_index.min(focused_index);
    let end = anchor_index.max(focused_index);
    rows.ids[start..=end].to_vec()
}

fn selection_change_for(
    rows: &DataTableRows,
    anchor: Option<&SharedString>,
    row_id: &str,
    intent: DataTableSelectionIntent,
    selection_mode: DataTableSelectionMode,
) -> Option<DataTableSelection> {
    let index = rows.row_index(row_id)?;
    let focused_id = rows.ids[index].clone();
    let effective_intent = match selection_mode {
        DataTableSelectionMode::Single => DataTableSelectionIntent::Replace,
        DataTableSelectionMode::Multiple => intent,
    };
    let anchor_id = if effective_intent == DataTableSelectionIntent::Extend {
        anchor.unwrap_or(&focused_id).clone()
    } else {
        focused_id.clone()
    };
    let selected_ids = match effective_intent {
        DataTableSelectionIntent::Replace => vec![focused_id.clone()],
        DataTableSelectionIntent::Extend => row_range_ids(rows, &anchor_id, row_id),
        DataTableSelectionIntent::Toggle => {
            let mut selected = rows.selected.to_vec();
            let insertion = selected.partition_point(|id| rows.positions[id] < index);
            if selected.get(insertion) == Some(&focused_id) {
                selected.remove(insertion);
            } else {
                selected.insert(insertion, focused_id.clone());
            }
            selected
        }
    };

    Some(DataTableSelection::new(
        effective_intent,
        anchor_id,
        focused_id,
        selected_ids,
    ))
}

#[allow(clippy::too_many_arguments)]
fn emit_row_selection(
    row_id: &SharedString,
    intent: DataTableSelectionIntent,
    rows: &DataTableRows,
    anchor: Option<&SharedString>,
    selection_mode: DataTableSelectionMode,
    row_handler: Option<&SharedStringHandler>,
    selection_handler: Option<&SelectionHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(handler) = selection_handler
        && let Some(selection) =
            selection_change_for(rows, anchor, row_id.as_ref(), intent, selection_mode)
    {
        handler(&selection, window, cx);
        return;
    }

    if let Some(handler) = row_handler {
        handler(row_id, window, cx);
    }
}

fn data_table_navigation_for(keystroke: &Keystroke) -> Option<DataTableNavigation> {
    match keystroke.key.as_str() {
        "up" => Some(DataTableNavigation::Up),
        "down" => Some(DataTableNavigation::Down),
        "home" => Some(DataTableNavigation::Home),
        "end" => Some(DataTableNavigation::End),
        "pageup" => Some(DataTableNavigation::PageUp),
        "pagedown" => Some(DataTableNavigation::PageDown),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DataColumn, DataColumnPin, DataColumnResize, DataRow, DataTable, DataTableColumnViewport,
        DataTableNavigation, DataTableNavigationOutcome, DataTableSelection,
        DataTableSelectionIntent, DataTableSelectionMode, DataTableState, DataTableViewport,
        SortDirection, TableSort,
    };
    use gpui::{
        AppContext as _, Context, FocusHandle, Keystroke, Modifiers, ParentElement as _, Render,
        SharedString, Styled as _, TestAppContext, VisualContext as _, Window, div,
    };

    struct DataTableHarness {
        selected_row: Option<String>,
        sort: TableSort,
        status_width: f32,
        focus_handle: FocusHandle,
    }

    impl DataTableHarness {
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                selected_row: Some("runtime".to_owned()),
                sort: TableSort::new("area", SortDirection::Ascending),
                status_width: 120.0,
                focus_handle: cx.focus_handle(),
            }
        }
    }

    struct DataTableSelectionHarness {
        selected_rows: Vec<String>,
        cursor: Option<DataTableSelection>,
        scroll: gpui::UniformListScrollHandle,
        focus_handle: FocusHandle,
    }

    impl DataTableSelectionHarness {
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                cursor: None,
                scroll: Default::default(),
                selected_rows: vec!["row-1".to_owned()],
                focus_handle: cx.focus_handle(),
            }
        }
    }

    impl Render for DataTableSelectionHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            let rows = (0..40)
                .map(|index| {
                    let id = format!("row-{index}");
                    let selected = self.selected_rows.iter().any(|selected| selected == &id);
                    DataRow::new(id, vec![format!("Row {index}")]).selected(selected)
                })
                .collect::<Vec<_>>();

            let mut table =
                DataTable::new("selection-data-table-test").virtualized(120.0, self.scroll.clone());
            if let Some(cursor) = &self.cursor {
                table = table.selection_cursor(cursor);
            }
            div().size_full().p_4().child(
                table
                    .columns(vec![DataColumn::new("name", "Name")])
                    .rows(rows)
                    .selection_mode(DataTableSelectionMode::Multiple)
                    .focusable(self.focus_handle.clone())
                    .on_row_selection(cx.listener(
                        |this, selection: &DataTableSelection, _, cx| {
                            this.cursor = Some(selection.clone());
                            this.selected_rows = selection
                                .selected_ids()
                                .iter()
                                .map(ToString::to_string)
                                .collect();
                            cx.notify();
                        },
                    )),
            )
        }
    }

    fn selection_ids(selection: &DataTableSelection) -> Vec<String> {
        selection
            .selected_ids()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    impl Render for DataTableHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            let rows = vec![
                DataRow::new("runtime", vec!["Runtime", "Ready"]),
                DataRow::new("advanced", vec!["Advanced", "In progress"]),
            ]
            .into_iter()
            .map(|row| {
                let selected = self.selected_row.as_deref() == Some(row.id().as_ref());
                row.selected(selected)
            })
            .collect::<Vec<_>>();

            div().size_full().p_4().child(
                DataTable::new("advanced-data-table-test")
                    .columns(vec![
                        DataColumn::new("area", "Area").sortable(true),
                        DataColumn::new("status", "Status")
                            .width(self.status_width)
                            .sortable(true),
                    ])
                    .rows(rows)
                    .focusable(self.focus_handle.clone())
                    .sort(self.sort.clone())
                    .on_sort(cx.listener(|this, sort: &TableSort, _, cx| {
                        this.sort = sort.clone();
                        cx.notify();
                    }))
                    .on_column_resize(cx.listener(|this, resize: &DataColumnResize, _, cx| {
                        if resize.column_id().as_ref() == "status" {
                            this.status_width = resize.width();
                            cx.notify();
                        }
                    }))
                    .on_row_select(cx.listener(|this, row_id: &SharedString, _, cx| {
                        this.selected_row = Some(row_id.to_string());
                        cx.notify();
                    })),
            )
        }
    }

    #[gpui::test]
    fn header_drag_reorders_cells_without_reindexing_the_row(cx: &mut TestAppContext) {
        use gpui::{InteractiveElement as _, IntoElement, MouseButton, px};
        struct Grid(Vec<SharedString>);
        impl Render for Grid {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                DataTable::new("order")
                    .columns(vec![
                        DataColumn::new("a", "A").width(90.),
                        DataColumn::new("b", "B").width(90.),
                        DataColumn::new("c", "C").width(90.),
                    ])
                    .rows(vec![DataRow::new("r", vec!["one", "two", "three"])])
                    .column_order(self.0.clone())
                    .render_cell(|cell| {
                        let name = format!("{}-{}", cell.column_id(), cell.value());
                        div()
                            .debug_selector(move || name.clone())
                            .child(cell.value().clone())
                            .into_any_element()
                    })
                    .on_column_order_change(cx.listener(|v, ids: &[SharedString], _, cx| {
                        v.0 = ids.to_vec();
                        cx.notify();
                    }))
            }
        }
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
        });
        let (view, cx) =
            cx.add_window_view(|_, _| Grid(vec!["unknown".into(), "a".into(), "a".into()]));
        let from = cx
            .debug_bounds("guic-data-table-header-a")
            .expect("source header")
            .center();
        let to = cx
            .debug_bounds("guic-data-table-header-c")
            .expect("target header")
            .center();
        cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            from + gpui::point(px(10.), px(0.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        view.read_with(cx, |v, _| {
            assert_eq!(
                v.0.iter().map(|id| id.as_ref()).collect::<Vec<_>>(),
                ["b", "c", "a"]
            )
        });
        let b = cx.debug_bounds("b-two").expect("original second value");
        let a = cx.debug_bounds("a-one").expect("original first value");
        assert!(b.right() <= a.left());
        let header = cx
            .debug_bounds("guic-data-table-header-a")
            .expect("moved header");
        cx.simulate_click(header.center(), Modifiers::none());
        cx.simulate_keystrokes("alt-left");
        view.read_with(cx, |v, _| {
            assert_eq!(
                v.0.iter().map(|id| id.as_ref()).collect::<Vec<_>>(),
                ["b", "a", "c"]
            )
        });
    }

    #[test]
    fn row_builder_marks_selection() {
        let row = DataRow::new("release", vec!["v0.1", "Stable"]).selected(true);
        assert!(row.selected);
        assert_eq!(row.cells.len(), 2);
    }

    #[test]
    fn table_state_preserves_messages() {
        let state = DataTableState::Error {
            message: "Failed to load".into(),
        };
        let table = DataTable::new("errors").state(state.clone());
        assert_eq!(table.state_message(), Some(("Failed to load".into(), true)));
        assert!(matches!(state, DataTableState::Error { .. }));
    }

    #[test]
    fn sort_metadata_tracks_column_and_direction() {
        let sort = TableSort::new("status", SortDirection::Descending);
        let column = DataColumn::new("status", "Status");
        assert_eq!(sort.column_id(), &column.id);
        assert_eq!(sort.direction(), SortDirection::Descending);
    }

    #[test]
    fn sort_metadata_toggles_active_column_direction() {
        let sort = TableSort::new("status", SortDirection::Ascending);
        let next = sort.next_for(&"status".into());
        let reset = sort.next_for(&"name".into());
        assert_eq!(next.direction(), SortDirection::Descending);
        assert_eq!(reset.column_id().as_ref(), "name");
        assert_eq!(reset.direction(), SortDirection::Ascending);
    }

    #[test]
    fn viewport_limits_visible_rows() {
        let rows = (0..50)
            .map(|index| DataRow::new(index.to_string(), vec![format!("Row {index}")]))
            .collect::<Vec<_>>();
        let table = DataTable::new("virtualized")
            .rows(rows)
            .row_height(32.0)
            .viewport(DataTableViewport::new(320.0, 96.0).overscan(1));

        let (start, end, top_spacer, bottom_spacer) = table.visible_rows();
        assert_eq!((start, end), (9, 14));
        assert_eq!(top_spacer, 288.0);
        assert_eq!(bottom_spacer, 1152.0);
    }

    #[test]
    fn non_finite_layout_configuration_is_bounded() {
        let row_viewport = DataTableViewport::new(f32::INFINITY, f32::NAN);
        assert_eq!(row_viewport.scroll_offset(), 0.0);
        assert_eq!(row_viewport.viewport_height(), 0.0);

        let column_viewport =
            DataTableColumnViewport::new(f32::NAN, f32::INFINITY).overscan(f32::INFINITY);
        assert_eq!(column_viewport.scroll_offset(), 0.0);
        assert_eq!(column_viewport.viewport_width(), 0.0);
        assert_eq!(column_viewport.overscan_width(), 160.0);

        let column = DataColumn::new("name", "Name")
            .width(f32::INFINITY)
            .min_width(f32::NAN);
        assert_eq!(column.width, None);
        assert_eq!(column.min_width, 48.0);
        assert_eq!(DataColumnResize::new("name", f32::NAN).width(), 48.0);

        let table = DataTable::new("table")
            .row_height(f32::INFINITY)
            .max_height(f32::NAN)
            .row_actions_width(f32::INFINITY);
        assert_eq!(table.row_height, 40.0);
        assert_eq!(table.max_height, None);
        assert_eq!(table.row_actions_width, 112.0);
    }

    #[test]
    fn table_reports_selected_and_adjacent_rows() {
        let table = DataTable::new("selection").rows(vec![
            DataRow::new("alpha", vec!["Alpha"]).selected(true),
            DataRow::new("beta", vec!["Beta"]),
            DataRow::new("gamma", vec!["Gamma"]),
        ]);

        assert_eq!(table.selected_row_id().as_deref(), Some("alpha"));
        assert_eq!(table.next_row_id("alpha").as_deref(), Some("beta"));
        assert_eq!(table.previous_row_id("beta").as_deref(), Some("alpha"));
        assert_eq!(table.previous_row_id("alpha"), None);
    }

    #[test]
    fn table_reports_all_selected_rows() {
        let table = DataTable::new("selection").rows(vec![
            DataRow::new("alpha", vec!["Alpha"]).selected(true),
            DataRow::new("beta", vec!["Beta"]),
            DataRow::new("gamma", vec!["Gamma"]).selected(true),
        ]);

        assert_eq!(table.selected_row_ids(), vec!["alpha", "gamma"]);
    }

    #[test]
    fn table_reports_visible_row_models() {
        let rows = (0..8)
            .map(|index| DataRow::new(format!("row-{index}"), vec![format!("Row {index}")]))
            .collect::<Vec<_>>();
        let table = DataTable::new("visible")
            .rows(rows)
            .row_height(40.0)
            .viewport(DataTableViewport::new(80.0, 120.0).overscan(0));

        let visible = table.visible_row_models();
        assert_eq!(visible.len(), 3);
        assert_eq!(visible[0].id().as_ref(), "row-2");
        assert_eq!(visible[0].index(), 2);
        assert_eq!(visible[2].id().as_ref(), "row-4");
    }

    #[test]
    fn column_resize_clamps_to_minimum_width() {
        let table = DataTable::new("widths").columns(vec![
            DataColumn::new("name", "Name")
                .width(180.0)
                .min_width(120.0),
            DataColumn::new("status", "Status").width(96.0),
        ]);

        let resize = table
            .resized_column("name", 80.0)
            .expect("column should exist");
        assert_eq!(resize.column_id().as_ref(), "name");
        assert_eq!(resize.width(), 120.0);
    }

    #[test]
    fn table_applies_column_resize_updates() {
        let table = DataTable::new("widths").columns(vec![
            DataColumn::new("name", "Name").width(180.0),
            DataColumn::new("status", "Status").width(96.0),
        ]);
        let resized = table.apply_column_resize(DataColumnResize::new("status", 140.0));

        assert_eq!(resized.column_width("status"), Some(140.0));
        assert_eq!(resized.column_width("name"), Some(180.0));
    }

    #[test]
    fn table_reports_visible_column_models() {
        let table = DataTable::new("columns").columns(vec![
            DataColumn::new("name", "Name")
                .width(180.0)
                .min_width(120.0),
            DataColumn::new("status", "Status").sortable(true),
        ]);

        let columns = table.visible_column_models();
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].id().as_ref(), "name");
        assert_eq!(columns[0].width(), Some(180.0));
        assert_eq!(columns[0].min_width(), 120.0);
        assert!(columns[1].is_sortable());
    }

    #[test]
    fn table_virtualizes_middle_columns_and_preserves_pinned_edges() {
        let table = DataTable::new("columns")
            .columns(vec![
                DataColumn::new("start", "Start")
                    .width(80.0)
                    .pin(DataColumnPin::Start),
                DataColumn::new("middle-0", "Middle 0").width(100.0),
                DataColumn::new("middle-1", "Middle 1").width(100.0),
                DataColumn::new("middle-2", "Middle 2").width(100.0),
                DataColumn::new("middle-3", "Middle 3").width(100.0),
                DataColumn::new("middle-4", "Middle 4").width(100.0),
                DataColumn::new("end", "End")
                    .width(80.0)
                    .pin(DataColumnPin::End),
            ])
            .column_viewport(DataTableColumnViewport::new(250.0, 100.0).overscan(0.0));

        let ids = table
            .visible_column_models()
            .into_iter()
            .map(|column| column.id().to_string())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["start", "middle-2", "middle-3", "end"]);
    }

    #[test]
    fn table_large_dataset_keeps_render_models_bounded() {
        let rows = (0..100_000)
            .map(|index| DataRow::new(format!("row-{index}"), vec![index.to_string()]))
            .collect::<Vec<_>>();
        let columns = (0..1_000)
            .map(|index| DataColumn::new(format!("column-{index}"), index.to_string()).width(80.0))
            .collect::<Vec<_>>();
        let table = DataTable::new("stress")
            .rows(rows)
            .columns(columns)
            .row_height(32.0)
            .viewport(DataTableViewport::new(1_600_000.0, 320.0).overscan(2))
            .column_viewport(DataTableColumnViewport::new(40_000.0, 640.0).overscan(80.0));

        let visible_rows = table.visible_row_models();
        let visible_columns = table.visible_column_models();
        assert!(visible_rows.len() <= 14);
        assert!(visible_columns.len() <= 11);
        assert_eq!(visible_rows[0].index(), 49_998);
        assert_eq!(visible_columns[0].id().as_ref(), "column-499");
    }

    #[test]
    fn table_produces_navigation_outcomes() {
        let rows = (0..10)
            .map(|index| {
                DataRow::new(
                    format!("row-{index}"),
                    vec![format!("Row {index}"), format!("Value {index}")],
                )
            })
            .collect::<Vec<_>>();
        let table = DataTable::new("nav")
            .rows(rows)
            .row_height(40.0)
            .viewport(DataTableViewport::new(0.0, 120.0).overscan(0));

        assert_eq!(
            table.navigation_outcome("row-4", DataTableNavigation::Up),
            DataTableNavigationOutcome::Select("row-3".into())
        );
        assert_eq!(
            table.navigation_outcome("row-4", DataTableNavigation::Down),
            DataTableNavigationOutcome::Select("row-5".into())
        );
        assert_eq!(
            table.navigation_outcome("row-4", DataTableNavigation::Home),
            DataTableNavigationOutcome::Select("row-0".into())
        );
        assert_eq!(
            table.navigation_outcome("row-4", DataTableNavigation::End),
            DataTableNavigationOutcome::Select("row-9".into())
        );
        assert_eq!(
            table.navigation_outcome("row-4", DataTableNavigation::PageUp),
            DataTableNavigationOutcome::Select("row-1".into())
        );
        assert_eq!(
            table.navigation_outcome("row-4", DataTableNavigation::PageDown),
            DataTableNavigationOutcome::Select("row-7".into())
        );
        assert_eq!(
            table.navigation_outcome("missing", DataTableNavigation::Down),
            DataTableNavigationOutcome::Noop
        );
    }

    #[test]
    fn table_produces_multi_selection_changes() {
        let table = DataTable::new("selection")
            .selection_mode(DataTableSelectionMode::Multiple)
            .rows(vec![
                DataRow::new("alpha", vec!["Alpha"]).selected(true),
                DataRow::new("beta", vec!["Beta"]),
                DataRow::new("gamma", vec!["Gamma"]),
                DataRow::new("delta", vec!["Delta"]),
            ]);

        let toggled = table
            .selection_change("gamma", DataTableSelectionIntent::Toggle)
            .expect("selection should exist");
        assert_eq!(toggled.intent(), DataTableSelectionIntent::Toggle);
        assert_eq!(selection_ids(&toggled), vec!["alpha", "gamma"]);

        let range = table
            .selection_change("delta", DataTableSelectionIntent::Extend)
            .expect("selection should exist");
        assert_eq!(range.anchor_id().as_ref(), "alpha");
        assert_eq!(
            selection_ids(&range),
            vec!["alpha", "beta", "gamma", "delta"]
        );
    }

    #[test]
    fn single_selection_mode_coerces_toggle_to_replace() {
        let table = DataTable::new("selection").rows(vec![
            DataRow::new("alpha", vec!["Alpha"]).selected(true),
            DataRow::new("beta", vec!["Beta"]),
        ]);

        let selection = table
            .selection_change("beta", DataTableSelectionIntent::Toggle)
            .expect("selection should exist");
        assert_eq!(selection.intent(), DataTableSelectionIntent::Replace);
        assert_eq!(selection_ids(&selection), vec!["beta"]);
    }

    #[gpui::test]
    fn table_header_sort_and_row_selection_handle_clicks(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| DataTableHarness::new(cx));

        let header_bounds = cx
            .debug_bounds("guic-data-table-header-status")
            .expect("sortable status header should exist");
        cx.simulate_click(header_bounds.center(), Modifiers::none());

        let row_bounds = cx
            .debug_bounds("guic-data-table-row-advanced")
            .expect("advanced row should exist");
        cx.simulate_click(row_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.sort.column_id().as_ref(), "status");
            assert_eq!(view.sort.direction(), SortDirection::Ascending);
            assert_eq!(view.selected_row.as_deref(), Some("advanced"));
        });
    }

    #[gpui::test]
    fn table_column_resize_handle_emits_width_update(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| DataTableHarness::new(cx));

        let resize_bounds = cx
            .debug_bounds("guic-data-table-resize-status")
            .expect("status resize handle should exist");
        cx.simulate_click(resize_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.status_width, 144.0);
            assert_eq!(view.sort.column_id().as_ref(), "area");
        });
    }

    #[gpui::test]
    fn horizontal_viewport_scrolls_headers_and_cells_between_pinned_edges(cx: &mut TestAppContext) {
        use gpui::{InteractiveElement as _, IntoElement as _, px};
        struct Grid(f32);
        impl Render for Grid {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
                let mut columns = Vec::new();
                for id in ["left", "one", "two", "three", "four", "right"] {
                    let pin = match id {
                        "left" => DataColumnPin::Start,
                        "right" => DataColumnPin::End,
                        _ => DataColumnPin::None,
                    };
                    columns.push(DataColumn::new(id, id).width(100.).pin(pin).sortable(true));
                }
                div().w(px(402.)).child(
                    DataTable::new("wide")
                        .columns(columns)
                        .rows(vec![DataRow::new("row", vec!["v"; 6])])
                        .column_viewport(DataTableColumnViewport::new(self.0, 200.).overscan(100.))
                        .on_sort(|_, _, _| {})
                        .render_cell(|cell| {
                            div()
                                .debug_selector(|| format!("value-{}", cell.column_id()))
                                .child("v")
                                .into_any_element()
                        }),
                )
            }
        }
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
        });
        let (grid, cx) = cx.add_window_view(|_, _| Grid(0.));
        let header = cx
            .debug_bounds("guic-data-table-header-two")
            .expect("header");
        let value = cx.debug_bounds("value-two").expect("value");
        let pinned = cx
            .debug_bounds("guic-data-table-header-right")
            .expect("pinned");
        for offset in [35., 135.] {
            grid.update(cx, |grid, cx| {
                grid.0 = offset;
                cx.notify();
            });
            cx.run_until_parked();
            assert_eq!(
                cx.debug_bounds("guic-data-table-header-two")
                    .expect("header")
                    .left(),
                header.left() - px(offset)
            );
            assert_eq!(
                cx.debug_bounds("value-two").expect("value").left(),
                value.left() - px(offset)
            );
            assert_eq!(
                cx.debug_bounds("guic-data-table-header-right"),
                Some(pinned)
            );
        }
    }

    #[gpui::test]
    fn table_keyboard_navigation_moves_selection(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| DataTableHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.focus_handle.focus(window, cx));
        })
        .expect("window update should succeed");

        cx.dispatch_keystroke(window, Keystroke::parse("down").expect("keystroke parses"));
        view.update(cx, |view, _| {
            assert_eq!(view.selected_row.as_deref(), Some("advanced"));
        });

        cx.dispatch_keystroke(window, Keystroke::parse("up").expect("keystroke parses"));
        view.update(cx, |view, _| {
            assert_eq!(view.selected_row.as_deref(), Some("runtime"));
        });

        cx.dispatch_keystroke(window, Keystroke::parse("end").expect("keystroke parses"));
        view.update(cx, |view, _| {
            assert_eq!(view.selected_row.as_deref(), Some("advanced"));
        });
    }

    #[gpui::test]
    fn table_keyboard_range_selection_extends_selection(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| DataTableSelectionHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.focus_handle.focus(window, cx));
        })
        .expect("window update should succeed");

        cx.dispatch_keystroke(
            window,
            Keystroke::parse("shift-down").expect("keystroke parses"),
        );
        view.update(cx, |view, _| {
            assert_eq!(view.selected_rows, vec!["row-1", "row-2"]);
        });
        for (key, expected) in [
            ("shift-down", vec!["row-1", "row-2", "row-3"]),
            ("shift-up", vec!["row-1", "row-2"]),
            ("end", vec!["row-39"]),
        ] {
            cx.dispatch_keystroke(window, Keystroke::parse(key).expect("valid keystroke"));
            view.read_with(cx, |view, _| assert_eq!(view.selected_rows, expected));
        }
        assert!(cx.debug_bounds("guic-data-table-row-row-39").is_some());
        assert!(cx.debug_bounds("guic-data-table-row-row-0").is_none());
        cx.update(|window, _| window.enable_a11y_for_testing());
        cx.run_until_parked();
        let tree = cx.update(|window, _| window.debug_a11y_tree_json().expect("accessible table"));
        let tree: serde_json::Value = serde_json::from_str(&tree).expect("accessibility JSON");
        let nodes = tree["nodes"].as_object().expect("nodes");
        assert!(nodes.values().any(|node| node["aria"]["row_count"] == 41));
        assert!(nodes.values().any(|node| node["aria"]["row_index"] == 40));
        assert!(
            nodes
                .values()
                .any(|node| node["aria"]["role"] == "ColumnHeader")
        );
    }
}

#[cfg(test)]
mod shared_rows_tests {
    use super::*;
    #[test]
    fn sharing_a_dataset_retains_indexes_and_selection() {
        let rows = DataTableRows::new(vec![
            DataRow::new("a", vec!["A"]),
            DataRow::new("b", vec!["B"]).selected(true),
        ]);
        let table = DataTable::new("table").shared_rows(rows.clone());
        assert!(Rc::ptr_eq(&rows.rows, &table.rows.rows));
        assert!(Rc::ptr_eq(&rows.ids, &table.rows.ids));
        assert_eq!(table.selected_row_id().as_deref(), Some("b"));
        assert_eq!(table.selected_row_ids(), vec![SharedString::from("b")]);
        assert_eq!(table.rows.as_slice().len(), 2);
    }
}
