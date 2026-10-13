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
    InstanceDisposition, JsonStore, LoadSource, NativeCapabilities, NativeCapability,
    NativeCommand, NativeMenuItem, NativeServices, NotificationRequest, NotificationService,
    NotificationUrgency, OverlayId, OverlayKind, PersistenceError, Recovered, Root, SecretValue,
    ServiceError, ServiceErrorKind, ServiceFuture, SingleInstanceService, TrayDefinition,
    TrayService, UpdateInfo, UpdateService,
};
pub use guic_tokens::{Theme, ThemeContextExt, ThemeRegistry};

#[cfg(feature = "icons")]
pub use guic_icons::{Icon, IconName};

#[cfg(feature = "charts")]
pub use guic_charts::{
    AreaChart, BarChart, BubbleChart, ChartAxis, ChartAxisSide, ChartCategoryTick, ChartDataset,
    ChartDomainFormatter, ChartDomainTick, ChartDomainValue, ChartEasing, ChartHit,
    ChartInteractionCommand, ChartInteractionState, ChartKind, ChartLabelCollisionPolicy,
    ChartOptions, ChartPoint, ChartScale, ChartSeries, ChartTooltipMode, ChartTransition,
    ChartValueAxis, ChartValueFormatter, DoughnutChart, HorizontalBarChart, LineChart, MixedChart,
    PieChart, ScatterChart,
};

#[cfg(feature = "editor")]
pub use guic_editor::{
    CodeEditor, CodeEditorOptions, DiagnosticSeverity, EditorBuffer, EditorCommand,
    EditorCompletion, EditorDiagnostic, EditorEdit, EditorLanguageAdapter, EditorPosition,
    EditorSearchMatch, EditorSelection, EditorSession, SyntaxToken, SyntaxTokenKind,
};

#[cfg(feature = "components")]
pub use guic_components::{
    Accordion, AccordionSection, Alert, AlertVariant, AutoComplete, Avatar, AvatarShape,
    AvatarStatus, Badge, BadgeVariant, Breadcrumb, BreadcrumbItem, Button, ButtonVariant, Card,
    CascadeOption, CascadeSelect, Checkbox, Chip, ColorPicker, ColorSwatch, CommandPalette,
    CommandPaletteItem, ComponentSize, ConfirmDialog, ConfirmPopup, ContextMenu, DataView,
    DataViewItem, DataViewLayout, DatePicker, Dialog, Drawer, DrawerSide, Fieldset, FilePicker,
    Form, FormField, FormSummary, IconButton, Image, ImageFit, InputGroup, InputGroupAddon,
    InputNumber, InputOtp, Label, Listbox, ListboxSelectionMode, Menu, MenuItem, Menubar,
    MenubarActivation, MenubarMenu, Message, MessageVariant, MetricCard, MultiSelect, Paginator,
    Panel, PanelMenu, PasswordInput, PickList, Popover, Progress, PropertyItem, PropertyList,
    Radio, ScrollArea, SearchInput, Select, SelectItem, Separator, Skeleton, Slider, Spinner,
    Splitter, SplitterAxis, Step, Stepper, Switch, TabItem, TabMenu, Tabs, Tag, TagVariant,
    TextArea, TextInput, TextInputKind, TieredMenu, Timeline, TimelineEvent, Toast, ToastPlacement,
    ToastStack, ToastVariant, Toolbar, Tooltip, TreeSelect, TreeSelectNode, TreeTable,
    TreeTableColumn, TreeTableRow, ValidationIssue, ValidationSeverity, VirtualList,
    VirtualListMetrics,
};

#[cfg(feature = "data-table")]
pub use guic_components::{
    DataColumn, DataColumnAlign, DataColumnPin, DataColumnResize, DataRow, DataTable,
    DataTableCell, DataTableColumnViewport, DataTableNavigation, DataTableNavigationOutcome,
    DataTableSelection, DataTableSelectionIntent, DataTableSelectionMode, DataTableState,
    DataTableViewport, SortDirection, TableSort, VisibleDataColumn, VisibleDataRow,
};
#[cfg(feature = "dock")]
pub use guic_components::{
    Dock, DockAxis, DockCommand, DockDragPayload, DockDropTarget, DockDropZone, DockLayout,
    DockNode, DockPlacement, DockSplitResize, DockStackSelection, DockTab, DockTabSelection,
    DockTabs,
};
#[cfg(feature = "markdown")]
pub use guic_components::{HtmlFragment, Markdown};
#[cfg(feature = "tree")]
pub use guic_components::{
    TreeMutation, TreeMutationError, TreeNavigation, TreeNavigationOutcome, TreeNode,
    TreeSelection, TreeSelectionIntent, TreeSelectionMode, TreeView, TreeViewport, VisibleTreeNode,
};
