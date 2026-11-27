//! Common GUIC imports for application code.
//!
//! # Example
//!
//! ```no_run
//! use guic::prelude::*;
//! use gpui::{ParentElement as _, div};
//!
//! struct Dashboard;
//!
//! impl Render for Dashboard {
//!     fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
//!         div().child("Dashboard")
//!     }
//! }
//! ```

pub use gpui::{App, Context, Entity, IntoElement, PathPromptOptions, Render, Window};
pub use guic_core::{
    ApplicationMenuService, CancellationToken, CloseReason, CommandRegistrationError,
    CommandRouter, CommandScope, CommandSpec, CredentialService, DeepLink, DeepLinkService,
    DroppedItem, FileDialogFilter, FileDialogRequest, FileDialogService, FocusDirection,
    InstanceDisposition, JsonStore, LoadSource, Localization, NativeCapabilities, NativeCapability,
    NativeCommand, NativeMenuItem, NativeServices, NotificationRequest, NotificationService,
    NotificationUrgency, OverlayId, OverlayKind, PersistenceError, Recovered, Root, SecretValue,
    ServiceError, ServiceErrorKind, ServiceFuture, SingleInstanceService, TrayDefinition,
    TrayService, UpdateInfo, UpdateService, set_locale, translate,
};
pub use guic_tokens::{Theme, ThemeContextExt, ThemeRegistry};
pub use guic_tokens::{ThemeController, ThemePreference};

#[cfg(feature = "icons")]
pub use guic_icons::{Icon, IconName};

#[cfg(feature = "charts")]
pub use guic_charts::{
    AreaChart, BarChart, BubbleChart, CandlestickChart, ChartAxis, ChartAxisSide,
    ChartCategoryTick, ChartDataset, ChartDomainFormatter, ChartDomainTick, ChartDomainValue,
    ChartEasing, ChartHit, ChartInteractionCommand, ChartInteractionState, ChartKind,
    ChartLabelCollisionPolicy, ChartOhlc, ChartOptions, ChartPoint, ChartScale, ChartSeries,
    ChartTooltipMode, ChartTransition, ChartValueAxis, ChartValueFormatter, DoughnutChart,
    HorizontalBarChart, LineChart, MixedChart, PieChart, RadarChart, SankeyChart, SankeyDiagram,
    SankeyError, SankeyHit, SankeyLink, ScatterChart,
};

#[cfg(feature = "editor-tree-sitter")]
pub use guic_editor::EditorGrammar;

#[cfg(feature = "editor")]
pub use guic_editor::{
    CodeEditor, CodeEditorOptions, DiagnosticSeverity, EditorBuffer, EditorCommand,
    EditorCompletion, EditorDiagnostic, EditorEdit, EditorLanguageAdapter, EditorPosition,
    EditorSearchMatch, EditorSelection, EditorSession, SyntaxToken, SyntaxTokenKind,
};

#[cfg(feature = "components")]
pub use guic_components::{
    Accordion, AccordionSection, Alert, AlertVariant, AutoComplete, Avatar, AvatarShape,
    AvatarStatus, Badge, BadgeVariant, Breadcrumb, BreadcrumbItem, Button, ButtonVariant,
    CalendarDate, CalendarWeekday, Card, CascadeOption, CascadeSelect, Checkbox, Chip, ColorPicker,
    ColorSwatch, CommandPalette, CommandPaletteItem, ComponentSize, ConfirmDialog, ConfirmPopup,
    ContextMenu, CopyButton, DataView, DataViewItem, DataViewLayout, DatePicker,
    DateRangeSelection, Dialog, Drawer, DrawerSide, Fieldset, FilePicker, Form, FormField,
    FormSummary, IconButton, Image, ImageFit, InputGroup, InputGroupAddon, InputNumber, InputOtp,
    Label, Listbox, ListboxSelectionMode, Menu, MenuItem, Menubar, MenubarActivation, MenubarMenu,
    Message, MessageVariant, MetricCard, MultiSelect, Paginator, Panel, PanelMenu, PasswordInput,
    PickList, Popover, Progress, PropertyItem, PropertyList, Radio, Rating, ScrollArea,
    SearchInput, Select, SelectItem, Separator, Skeleton, Slider, Spinner, Splitter, SplitterAxis,
    Step, Stepper, Switch, TabItem, TabMenu, Tabs, Tag, TagVariant, TextArea, TextInput,
    TextInputKind, TieredMenu, Timeline, TimelineEvent, Toast, ToastPlacement, ToastStack,
    ToastVariant, Toolbar, Tooltip, TreeSelect, TreeSelectNode, TreeTable, TreeTableColumn,
    TreeTableRow, ValidationIssue, ValidationSeverity, VariableList, VirtualList,
    VirtualListMetrics,
};

#[cfg(feature = "data-table")]
pub use guic_components::{
    DataColumn, DataColumnAlign, DataColumnPin, DataColumnResize, DataRow, DataTable,
    DataTableCell, DataTableColumnViewport, DataTableNavigation, DataTableNavigationOutcome,
    DataTableRows, DataTableSelection, DataTableSelectionIntent, DataTableSelectionMode,
    DataTableState, DataTableViewport, SortDirection, TableSort, VisibleDataColumn, VisibleDataRow,
};
#[cfg(feature = "diff")]
pub use guic_components::{
    DiffDocument, DiffFileChange, DiffLayout, DiffLine, DiffLineKind, DiffPatchError,
    DiffPatchFile, DiffPatchHunk, DiffView, MergeChoice, MergeConflict, MergeConflictView,
    MergeDocument, MergeParseError,
};
#[cfg(feature = "dock")]
pub use guic_components::{
    Dock, DockAxis, DockCommand, DockDensity, DockDragPayload, DockDropTarget, DockDropZone,
    DockFocusIndicator, DockLayout, DockNode, DockPlacement, DockSplitResize, DockStackSelection,
    DockTab, DockTabSelection, DockTabs,
};
#[cfg(feature = "markdown")]
pub use guic_components::{HtmlFragment, Markdown};
#[cfg(feature = "tree")]
pub use guic_components::{
    TreeMutation, TreeMutationError, TreeNavigation, TreeNavigationOutcome, TreeNode,
    TreeSelection, TreeSelectionIntent, TreeSelectionMode, TreeView, TreeViewNodes, TreeViewport,
    VisibleTreeNode,
};
