//! Shared component gallery application for GUIC.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod app;

use gpui::{
    AppContext as _, Bounds, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Point, Render, SharedString, Styled as _, Window, WindowBounds,
    WindowOptions, div, point, px,
};
use guic::prelude::{
    Accordion, AccordionSection, Alert, AreaChart, AutoComplete, Avatar, AvatarStatus, Badge,
    BarChart, Breadcrumb, BreadcrumbItem, Button, ButtonVariant, Card, CascadeOption,
    CascadeSelect, ChartDataset, ChartOptions, ChartPoint, Checkbox, Chip, CodeEditor,
    CodeEditorOptions, ColorPicker, ColorSwatch, CommandPalette, CommandPaletteItem, ComponentSize,
    ConfirmDialog, ConfirmPopup, ContextMenu, DataColumn, DataColumnPin, DataColumnResize, DataRow,
    DataTable, DataTableColumnViewport, DataTableNavigation, DataTableState, DataTableViewport,
    DataView, DataViewItem, DataViewLayout, DatePicker, Dialog, Dock, DockCommand, DockLayout,
    DockNode, DockTab, DockTabs, Drawer, DrawerSide, EditorBuffer, EditorEdit, EditorSession,
    Fieldset, FilePicker, Form, FormField, FormSummary, HtmlFragment, IconButton, IconName, Image,
    ImageFit, InputNumber, InputOtp, Label, LineChart, Listbox, ListboxSelectionMode, Markdown,
    Menu, MenuItem, Menubar, MenubarActivation, MenubarMenu, Message, MessageVariant, MetricCard,
    MultiSelect, Paginator, Panel, PanelMenu, PasswordInput, PickList, PieChart, Popover, Progress,
    PropertyItem, PropertyList, Radio, ScrollArea, SearchInput, Select, SelectItem, Separator,
    Slider, SortDirection, Spinner, Splitter, SplitterAxis, Step, Stepper, Switch, TabItem,
    TabMenu, TableSort, Tabs, Tag, TagVariant, Terminal, TerminalModel, TerminalOptions, TextArea,
    TextInput, ThemeContextExt, ThemeRegistry, TieredMenu, Timeline, TimelineEvent, Toast,
    ToastStack, ToastVariant, Toolbar, Tooltip, TreeNavigation, TreeNode, TreeSelect,
    TreeSelectNode, TreeTable, TreeTableColumn, TreeTableRow, TreeView, TreeViewport,
    ValidationIssue, ValidationSeverity, VirtualList,
};
use guic::tokens::Theme;

struct StoryRoot {
    active_theme: &'static str,
    progress: f32,
    alert_open: bool,
    checkbox_checked: bool,
    selected_radio: usize,
    switch_on: bool,
    active_tab: usize,
    select_expanded: bool,
    selected_option: Option<usize>,
    selected_table_row_id: &'static str,
    table_sort: TableSort,
    status_column_width: f32,
    selected_tree_node: &'static str,
    crates_expanded: bool,
    samples_expanded: bool,
    future_expanded: bool,
    popover_open: bool,
    dialog_open: bool,
    filters_collapsed: bool,
    accordion_open: [bool; 3],
    breadcrumb_index: usize,
    wizard_step: usize,
    paginator_page: usize,
    menubar_open: Option<usize>,
    menu_active: Option<usize>,
    menu_log: SharedString,
    panel_menu_selected: SharedString,
    tiered_menu_collapsed: Vec<SharedString>,
    context_open: bool,
    context_anchor: Point<Pixels>,
    drawer_open: bool,
    confirm_open: bool,
    confirm_popup_open: bool,
    confirm_log: SharedString,
    toasts: Vec<u32>,
    toast_seq: u32,
    input_number_value: f64,
    otp_value: SharedString,
    date_value: SharedString,
    date_open: bool,
    color_value: SharedString,
    file_picker_files: Vec<SharedString>,
    autocomplete_selected: SharedString,
    multi_select_open: bool,
    multi_select_selected: Vec<usize>,
    listbox_selected: Vec<usize>,
    pick_list_selected: Vec<usize>,
    data_view_selected: SharedString,
    cascade_open: bool,
    cascade_path: Vec<usize>,
    tree_select_open: bool,
    tree_select_selected: SharedString,
    tree_table_selected: SharedString,
    tree_table_docs_expanded: bool,
    editor_session: EditorSession,
    dock_layout: DockLayout,
    dock_focus: FocusHandle,
    table_focus: FocusHandle,
    tree_focus: FocusHandle,
    menu_focus: FocusHandle,
    input_number_focus: FocusHandle,
    otp_focus: FocusHandle,
    editor_focus: FocusHandle,
    autocomplete: Entity<AutoComplete>,
    command_palette: Entity<CommandPalette>,
    slider: Entity<Slider>,
    text_input: Entity<TextInput>,
    search_input: Entity<SearchInput>,
    password_input: Entity<PasswordInput>,
    text_area: Entity<TextArea>,
}

impl StoryRoot {
    fn new(cx: &mut Context<Self>) -> Self {
        let autocomplete_view = cx.entity();
        let command_palette_view = cx.entity();
        Self {
            active_theme: Theme::DEFAULT_DARK_NAME,
            progress: 42.0,
            alert_open: true,
            checkbox_checked: true,
            selected_radio: 0,
            switch_on: true,
            active_tab: 0,
            select_expanded: false,
            selected_option: Some(0),
            selected_table_row_id: "runtime",
            table_sort: TableSort::new("area", SortDirection::Ascending),
            status_column_width: 140.0,
            selected_tree_node: "guic-components",
            crates_expanded: true,
            samples_expanded: true,
            future_expanded: true,
            popover_open: false,
            dialog_open: false,
            filters_collapsed: false,
            accordion_open: [true, false, false],
            breadcrumb_index: 2,
            wizard_step: 1,
            paginator_page: 0,
            menubar_open: None,
            menu_active: None,
            menu_log: SharedString::from("No menu command yet"),
            panel_menu_selected: SharedString::from("overview"),
            tiered_menu_collapsed: Vec::new(),
            context_open: false,
            context_anchor: point(px(0.0), px(0.0)),
            drawer_open: false,
            confirm_open: false,
            confirm_popup_open: false,
            confirm_log: SharedString::from("No confirmation yet"),
            toasts: Vec::new(),
            toast_seq: 0,
            input_number_value: 4.0,
            otp_value: SharedString::from("1206"),
            date_value: SharedString::from("2026-06-28"),
            date_open: false,
            color_value: SharedString::from("#3b82f6"),
            file_picker_files: vec![
                SharedString::from("roadmap.md"),
                SharedString::from("screenshot.png"),
            ],
            autocomplete_selected: SharedString::from("No suggestion selected"),
            multi_select_open: false,
            multi_select_selected: vec![0, 2],
            listbox_selected: vec![0, 2],
            pick_list_selected: vec![1],
            data_view_selected: SharedString::from("runtime"),
            cascade_open: false,
            cascade_path: vec![0, 1],
            tree_select_open: false,
            tree_select_selected: SharedString::from("editor"),
            tree_table_selected: SharedString::from("components"),
            tree_table_docs_expanded: true,
            editor_session: EditorSession::new(EditorBuffer::from_text(
                "fn main() {\n    let status = \"ready\";\n    println!(\"{status}\");\n}",
            )),
            dock_layout: Self::initial_dock_layout(),
            dock_focus: cx.focus_handle(),
            table_focus: cx.focus_handle(),
            tree_focus: cx.focus_handle(),
            menu_focus: cx.focus_handle(),
            input_number_focus: cx.focus_handle(),
            otp_focus: cx.focus_handle(),
            editor_focus: cx.focus_handle(),
            autocomplete: cx.new(|cx| {
                AutoComplete::new("gallery-autocomplete", cx)
                    .items(vec![
                        SelectItem::new("accordion", "Accordion"),
                        SelectItem::new("cascade", "CascadeSelect"),
                        SelectItem::new("charts", "Charts"),
                        SelectItem::new("terminal", "Terminal"),
                        SelectItem::new("tree-table", "TreeTable"),
                    ])
                    .on_select(move |item, _window, app| {
                        autocomplete_view.update(app, |this, cx| {
                            this.autocomplete_selected =
                                SharedString::from(format!("Selected {}", item.label));
                            cx.notify();
                        });
                    })
            }),
            command_palette: cx.new(|cx| {
                CommandPalette::new("gallery-command-palette", cx)
                    .items(vec![
                        CommandPaletteItem::new("file.open", "Open file")
                            .shortcut("⌘O")
                            .keywords(["load", "document"]),
                        CommandPaletteItem::new("settings.open", "Open settings")
                            .shortcut("⌘,"),
                        CommandPaletteItem::new("terminal.toggle", "Toggle terminal")
                            .shortcut("⌃`"),
                    ])
                    .on_activate(move |item, _window, app| {
                        command_palette_view.update(app, |this, cx| {
                            this.menu_log =
                                SharedString::from(format!("command → {}", item.id()));
                            cx.notify();
                        });
                    })
            }),
            slider: cx.new(|cx| {
                Slider::new("gallery-slider", cx)
                    .range(0.0, 100.0)
                    .step(5.0)
                    .value(40.0)
            }),
            text_input: cx.new(|cx| {
                TextInput::new("story-text-input", cx)
                    .placeholder("Project name")
                    .value("Production Readiness")
            }),
            search_input: cx.new(|cx| {
                SearchInput::search("story-search-input", cx)
                    .placeholder("Search components")
                    .value("input")
            }),
            password_input: cx.new(|cx| {
                PasswordInput::password("story-password-input", cx)
                    .placeholder("Production token")
            }),
            text_area: cx.new(|cx| {
                TextArea::text_area("story-text-area", cx)
                    .placeholder("Describe the next milestone")
                    .value(
                        "Finish the remaining review fixes.\nVerify the workspace.\nPrepare release notes.",
                    )
            }),
        }
    }

    fn set_theme(&mut self, name: &'static str, cx: &mut Context<Self>) {
        if let Some(theme) = ThemeRegistry::global(cx).get(name).cloned() {
            self.active_theme = name;
            cx.set_theme(theme);
            cx.notify();
        }
    }

    fn theme_label(name: &'static str) -> &'static str {
        match name {
            Theme::DEFAULT_DARK_NAME => "Default Dark",
            Theme::DEFAULT_LIGHT_NAME => "Default Light",
            Theme::HIGH_CONTRAST_DARK_NAME => "High Contrast Dark",
            Theme::HIGH_CONTRAST_LIGHT_NAME => "High Contrast Light",
            _ => name,
        }
    }

    fn advance_progress(&mut self, cx: &mut Context<Self>) {
        self.progress = if self.progress >= 100.0 {
            10.0
        } else {
            (self.progress + 22.0).min(100.0)
        };
        cx.notify();
    }

    fn set_radio(&mut self, selected_radio: usize, cx: &mut Context<Self>) {
        self.selected_radio = selected_radio;
        cx.notify();
    }

    fn set_select_expanded(&mut self, expanded: bool, cx: &mut Context<Self>) {
        self.select_expanded = expanded;
        cx.notify();
    }

    fn select_option(&mut self, selected: usize, cx: &mut Context<Self>) {
        self.selected_option = Some(selected);
        self.select_expanded = false;
        cx.notify();
    }

    fn toggle_popover(&mut self, cx: &mut Context<Self>) {
        self.popover_open = !self.popover_open;
        cx.notify();
    }

    fn set_table_row(&mut self, row_id: &str, cx: &mut Context<Self>) {
        self.selected_table_row_id = match row_id {
            "runtime" => "runtime",
            "advanced" => "advanced",
            "cross-platform" => "cross-platform",
            _ => self.selected_table_row_id,
        };
        cx.notify();
    }

    fn set_table_sort(&mut self, sort: &TableSort, cx: &mut Context<Self>) {
        self.table_sort = sort.clone();
        cx.notify();
    }

    fn release_rows(&self) -> Vec<DataRow> {
        let mut rows = vec![
            DataRow::new(
                "runtime",
                vec![
                    "Runtime",
                    "Ready",
                    "Theme, focus, assets, and overlays are wired.",
                ],
            ),
            DataRow::new(
                "advanced",
                vec![
                    "Advanced",
                    "In progress",
                    "Table, tree, and markdown are now represented.",
                ],
            ),
            DataRow::new(
                "cross-platform",
                vec![
                    "Cross-platform",
                    "Review",
                    "Linux-only gpui_platform flags no longer leak to macOS and Windows.",
                ],
            ),
        ];
        rows.sort_by(|left, right| {
            let column_index = match self.table_sort.column_id().as_ref() {
                "status" => 1,
                "notes" => 2,
                _ => 0,
            };
            let left_value = left
                .cells()
                .get(column_index)
                .map(AsRef::as_ref)
                .unwrap_or("");
            let right_value = right
                .cells()
                .get(column_index)
                .map(AsRef::as_ref)
                .unwrap_or("");
            match self.table_sort.direction() {
                SortDirection::Ascending => left_value.cmp(right_value),
                SortDirection::Descending => right_value.cmp(left_value),
            }
        });
        rows.into_iter()
            .map(|row| {
                let selected = row.id().as_ref() == self.selected_table_row_id;
                row.selected(selected)
            })
            .collect()
    }

    fn release_table_model(&self) -> DataTable {
        DataTable::new("story-data-table-model")
            .columns(vec![
                DataColumn::new("area", "Area")
                    .width(180.0)
                    .sortable(true)
                    .pin(DataColumnPin::Start),
                DataColumn::new("status", "Status")
                    .width(140.0)
                    .sortable(true),
                DataColumn::new("notes", "Notes")
                    .width(360.0)
                    .sortable(true),
            ])
            .rows(self.release_rows())
            .sort(self.table_sort.clone())
            .max_height(220.0)
            .state(DataTableState::Ready)
            .apply_column_resize(guic::components::DataColumnResize::new(
                "status",
                self.status_column_width,
            ))
    }

    fn navigate_table(&mut self, navigation: DataTableNavigation, cx: &mut Context<Self>) {
        if let guic::components::DataTableNavigationOutcome::Select(row_id) = self
            .release_table_model()
            .navigation_outcome(self.selected_table_row_id, navigation)
        {
            let row_id = row_id.to_string();
            self.set_table_row(&row_id, cx);
        }
    }

    fn resize_status_column(&mut self, delta: f32, cx: &mut Context<Self>) {
        if let Some(resize) = self
            .release_table_model()
            .resized_column("status", self.status_column_width + delta)
        {
            self.status_column_width = resize.width();
            cx.notify();
        }
    }

    fn select_tree_node(&mut self, node_id: &str, cx: &mut Context<Self>) {
        self.selected_tree_node = match node_id {
            "crates" => "crates",
            "guic" => "guic",
            "guic-components" => "guic-components",
            "samples" => "samples",
            "gallery" => "gallery",
            "future" => "future",
            _ => self.selected_tree_node,
        };
        cx.notify();
    }

    fn toggle_tree_node(&mut self, node_id: &str, cx: &mut Context<Self>) {
        match node_id {
            "crates" => self.crates_expanded = !self.crates_expanded,
            "samples" => self.samples_expanded = !self.samples_expanded,
            "future" => self.future_expanded = !self.future_expanded,
            _ => {}
        }
        cx.notify();
    }

    fn workspace_tree_model(&self) -> TreeView {
        TreeView::new("story-tree-model").nodes(vec![
            TreeNode::new("crates", "crates")
                .expanded(self.crates_expanded)
                .selected(self.selected_tree_node == "crates")
                .children(vec![
                    TreeNode::new("guic", "guic")
                        .detail("umbrella crate")
                        .selected(self.selected_tree_node == "guic"),
                    TreeNode::new("guic-components", "guic-components")
                        .detail("baseline widgets")
                        .selected(self.selected_tree_node == "guic-components"),
                ]),
            TreeNode::new("samples", "samples")
                .expanded(self.samples_expanded)
                .selected(self.selected_tree_node == "samples")
                .children(vec![
                    TreeNode::new("gallery", "component-gallery")
                        .selected(self.selected_tree_node == "gallery"),
                ]),
            TreeNode::new("future", "future subsystems")
                .selected(self.selected_tree_node == "future")
                .loading(true)
                .expanded(self.future_expanded),
        ])
    }

    fn initial_dock_layout() -> DockLayout {
        DockLayout::new(DockNode::horizontal(
            DockNode::Tabs(
                DockTabs::new(
                    "sidebar",
                    vec![
                        DockTab::new("files", "Files", "Project files and folders").badge("12"),
                        DockTab::new("search", "Search", "Workspace-wide search results"),
                    ],
                )
                .active_tab(0),
            ),
            DockNode::vertical(
                DockNode::Tabs(
                    DockTabs::new(
                        "editor",
                        vec![
                            DockTab::new(
                                "main",
                                "main.rs",
                                "fn main() {\n    println!(\"guic\");\n}",
                            ),
                            DockTab::new(
                                "todo",
                                "TODO.md",
                                "Release checklist and validation notes.",
                            ),
                        ],
                    )
                    .active_tab(1),
                ),
                DockNode::Tabs(
                    DockTabs::new(
                        "console",
                        vec![
                            DockTab::new("logs", "Logs", "cargo check\ncargo test\ncargo clippy"),
                            DockTab::new("problems", "Problems", "No critical issues detected."),
                        ],
                    )
                    .active_tab(0),
                ),
                720,
            ),
            280,
        ))
    }

    fn apply_dock_command(&mut self, command: &DockCommand, cx: &mut Context<Self>) {
        if self.dock_layout.apply(command) {
            cx.notify();
        }
    }

    fn navigate_tree(&mut self, navigation: TreeNavigation, cx: &mut Context<Self>) {
        match self
            .workspace_tree_model()
            .navigation_outcome(self.selected_tree_node, navigation)
        {
            guic::components::TreeNavigationOutcome::Select(node_id) => {
                let node_id = node_id.to_string();
                self.select_tree_node(&node_id, cx);
            }
            guic::components::TreeNavigationOutcome::Toggle(node_id) => {
                let node_id = node_id.to_string();
                self.toggle_tree_node(&node_id, cx);
            }
            guic::components::TreeNavigationOutcome::Noop => {}
        }
    }

    fn set_dialog_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.dialog_open = open;
        cx.notify();
    }

    /// Renders the identity, layout, navigation, and feedback component catalog.
    fn render_catalog(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let identity_row = div()
            .flex()
            .gap_3()
            .flex_wrap()
            .items_center()
            .child(Avatar::new("Ada Lovelace").status(AvatarStatus::Online))
            .child(Avatar::new("Grace Hopper").status(AvatarStatus::Busy))
            .child(Avatar::new("guic").size(ComponentSize::Small))
            .child(Tag::new("stable").variant(TagVariant::Success).dot(true))
            .child(Tag::new("preview").variant(TagVariant::Warning).dot(true))
            .child(
                Tag::new("backend")
                    .variant(TagVariant::Info)
                    .removable(true),
            );

        let toolbar = Toolbar::new()
            .child(Button::new("New").size(ComponentSize::Small))
            .child(Button::new("Open").secondary().size(ComponentSize::Small))
            .separator()
            .child(Button::new("Save").secondary().size(ComponentSize::Small))
            .spacer()
            .child(IconButton::new(IconName::Search).variant(ButtonVariant::Ghost));

        let card = Card::new()
            .title("Workspace usage")
            .subtitle("Last 30 days")
            .header_actions(Badge::new("live").success())
            .child(Label::new("1,204 sessions").muted(true))
            .child(Progress::new(self.progress).id("catalog-workspace-usage"))
            .footer(Button::new("View report").size(ComponentSize::Small));

        let panel = Panel::new("catalog-filters", "Filters")
            .collapsible(true)
            .collapsed(self.filters_collapsed)
            .actions(Badge::new("3"))
            .on_toggle(cx.listener(|this, collapsed, _, cx| {
                this.filters_collapsed = *collapsed;
                cx.notify();
            }))
            .child(Checkbox::new("catalog-filter-open").label("Open issues"))
            .child(Checkbox::new("catalog-filter-mine").label("Assigned to me"));

        let breadcrumb = Breadcrumb::new("catalog-breadcrumb")
            .items(vec![
                BreadcrumbItem::new("home", "Home"),
                BreadcrumbItem::new("workspace", "Workspace"),
                BreadcrumbItem::new("components", "Components"),
            ])
            .on_select(cx.listener(|this, index, _, cx| {
                this.breadcrumb_index = *index;
                cx.notify();
            }));

        let stepper = Stepper::new().active(self.wizard_step).steps(vec![
            Step::new("Account").description("Identity"),
            Step::new("Profile").description("Details"),
            Step::new("Review").description("Confirm"),
        ]);

        let tab_menu = TabMenu::new("catalog-tab-menu")
            .items(vec![
                TabItem::new("overview", "Overview"),
                TabItem::new("activity", "Activity"),
                TabItem::new("settings", "Settings"),
            ])
            .selected(self.active_tab)
            .on_select(cx.listener(|this, index, _, cx| {
                this.active_tab = *index;
                cx.notify();
            }));

        let timeline = Timeline::new().events(vec![
            TimelineEvent::new("Release candidate built")
                .description("All component checks completed successfully.")
                .timestamp("10:32"),
            TimelineEvent::new("Documentation updated")
                .description("Feature flags and migration notes were refreshed.")
                .timestamp("09:45"),
            TimelineEvent::new("Workspace created").timestamp("Yesterday"),
        ]);

        let pick_list = PickList::new("catalog-pick-list")
            .items(vec![
                SelectItem::new("ada", "Ada Lovelace"),
                SelectItem::new("grace", "Grace Hopper"),
                SelectItem::new("margaret", "Margaret Hamilton"),
            ])
            .selected(self.pick_list_selected.clone())
            .available_label("Available reviewers")
            .selected_label("Assigned reviewers")
            .on_change(cx.listener(|this, selected: &Vec<usize>, _, cx| {
                this.pick_list_selected = selected.clone();
                cx.notify();
            }));

        let splitter = Splitter::new(
            "catalog-splitter",
            div()
                .size_full()
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(Label::new("Navigator").muted(true))
                .child(Label::new("Components\nThemes\nExamples")),
            div()
                .size_full()
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(Label::new("Preview").muted(true))
                .child(Label::new("A controlled two-pane layout surface.")),
        )
        .axis(SplitterAxis::Horizontal)
        .fraction(0.36);

        let stepper_controls = div()
            .flex()
            .gap_2()
            .items_center()
            .child(
                Button::new("Back")
                    .secondary()
                    .size(ComponentSize::Small)
                    .disabled(self.wizard_step == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.wizard_step = this.wizard_step.saturating_sub(1);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("Next")
                    .size(ComponentSize::Small)
                    .disabled(self.wizard_step >= 2)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.wizard_step = (this.wizard_step + 1).min(2);
                        cx.notify();
                    })),
            );

        let accordion = Accordion::new("catalog-accordion")
            .section(
                AccordionSection::new("General", Label::new("Theme, language, and startup."))
                    .expanded(self.accordion_open[0]),
            )
            .section(
                AccordionSection::new("Notifications", Label::new("Email and in-app alerts."))
                    .expanded(self.accordion_open[1]),
            )
            .section(
                AccordionSection::new("Advanced", Label::new("Experimental feature flags."))
                    .expanded(self.accordion_open[2]),
            )
            .on_toggle(cx.listener(|this, index: &usize, _, cx| {
                this.accordion_open[*index] = !this.accordion_open[*index];
                cx.notify();
            }));

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(Label::new("Catalog"))
            .child(identity_row)
            .child(toolbar)
            .child(
                div()
                    .flex()
                    .gap_4()
                    .flex_wrap()
                    .items_start()
                    .child(div().w_full().max_w(px(320.0)).child(card))
                    .child(div().w_full().max_w(px(320.0)).child(panel)),
            )
            .child(breadcrumb)
            .child(tab_menu)
            .child(
                div()
                    .w_full()
                    .max_w(px(520.0))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(stepper)
                    .child(stepper_controls),
            )
            .child(div().w_full().max_w(px(420.0)).child(accordion))
            .child(div().w_full().max_w(px(640.0)).h(px(140.0)).child(splitter))
            .child(div().w_full().max_w(px(520.0)).child(timeline))
            .child(div().w_full().max_w(px(520.0)).child(pick_list))
            .child(
                Paginator::new("catalog-paginator")
                    .page_count(8)
                    .page(self.paginator_page)
                    .on_select(cx.listener(|this, page: &usize, _, cx| {
                        this.paginator_page = *page;
                        cx.notify();
                    })),
            )
    }

    /// Renders the menu family demo: a `Menubar`, an embedded `Menu`, and a
    /// right-click `ContextMenu`, all host-managed.
    fn render_menus(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let context_close_view = cx.entity();
        let menubar = Menubar::new("gallery-menubar")
            .open(self.menubar_open)
            .menus(vec![
                MenubarMenu::new(
                    "File",
                    vec![
                        MenuItem::new("new", "New")
                            .icon(IconName::Plus)
                            .shortcut("⌘N"),
                        MenuItem::new("open", "Open").shortcut("⌘O"),
                        MenuItem::separator(),
                        MenuItem::new("close", "Close").danger(true).shortcut("⌘W"),
                    ],
                ),
                MenubarMenu::new(
                    "View",
                    vec![
                        MenuItem::header("Panels"),
                        MenuItem::new("sidebar", "Toggle Sidebar"),
                        MenuItem::new("terminal", "Toggle Terminal").disabled(true),
                    ],
                ),
            ])
            .on_open(cx.listener(|this, next: &Option<usize>, _, cx| {
                this.menubar_open = *next;
                cx.notify();
            }))
            .on_activate(cx.listener(|this, activation: &MenubarActivation, _, cx| {
                this.menu_log = SharedString::from(format!(
                    "menubar[{}] → {}",
                    activation.menu, activation.item
                ));
                this.menubar_open = None;
                cx.notify();
            }));

        let standalone = Menu::new("gallery-menu")
            .focusable(self.menu_focus.clone())
            .active_index(self.menu_active)
            .min_width(200.0)
            .items(vec![
                MenuItem::new("rename", "Rename"),
                MenuItem::new("duplicate", "Duplicate"),
                MenuItem::separator(),
                MenuItem::new("delete", "Delete").danger(true),
            ])
            .on_highlight(cx.listener(|this, index: &usize, _, cx| {
                this.menu_active = Some(*index);
                cx.notify();
            }))
            .on_activate(cx.listener(|this, id: &SharedString, _, cx| {
                this.menu_log = SharedString::from(format!("menu → {id}"));
                cx.notify();
            }));

        let panel_menu = PanelMenu::new("gallery-panel-menu")
            .items(vec![
                MenuItem::header("Workspace"),
                MenuItem::new("overview", "Overview"),
                MenuItem::new("activity", "Activity"),
                MenuItem::separator(),
                MenuItem::header("Management"),
                MenuItem::new("members", "Members"),
                MenuItem::new("billing", "Billing").disabled(true),
            ])
            .selected(Some(self.panel_menu_selected.clone()))
            .on_select(cx.listener(|this, id: &SharedString, _, cx| {
                this.panel_menu_selected = id.clone();
                this.menu_log = SharedString::from(format!("panel → {id}"));
                cx.notify();
            }));

        let tiered_menu = TieredMenu::new("gallery-tiered-menu")
            .min_width(240.0)
            .collapsed(self.tiered_menu_collapsed.clone())
            .items(vec![
                MenuItem::new("insert", "Insert").children(vec![
                    MenuItem::new("insert-chart", "Chart"),
                    MenuItem::new("insert-table", "Table"),
                    MenuItem::new("insert-terminal", "Terminal"),
                ]),
                MenuItem::new("format", "Format").children(vec![
                    MenuItem::new("format-code", "Code"),
                    MenuItem::new("format-markdown", "Markdown"),
                ]),
                MenuItem::separator(),
                MenuItem::new("publish", "Publish").disabled(true),
            ])
            .on_activate(cx.listener(|this, id: &SharedString, _, cx| {
                this.menu_log = SharedString::from(format!("tiered → {id}"));
                cx.notify();
            }))
            .on_toggle(cx.listener(|this, id: &SharedString, _, cx| {
                if let Some(index) = this
                    .tiered_menu_collapsed
                    .iter()
                    .position(|collapsed| collapsed == id)
                {
                    this.tiered_menu_collapsed.remove(index);
                } else {
                    this.tiered_menu_collapsed.push(id.clone());
                }
                this.menu_log = SharedString::from(format!("tiered branch → {id}"));
                cx.notify();
            }));

        let context_target = div()
            .w_full()
            .max_w(px(260.0))
            .px(px(16.0))
            .py(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .child(Label::new("Right-click for a context menu").muted(true));

        let context = ContextMenu::new("gallery-context", context_target)
            .open(self.context_open)
            .anchor(self.context_anchor)
            .items(vec![
                MenuItem::new("inspect", "Inspect"),
                MenuItem::new("copy", "Copy path").shortcut("⌘C"),
                MenuItem::separator(),
                MenuItem::new("remove", "Remove").danger(true),
            ])
            .on_request(cx.listener(|this, position: &Point<Pixels>, _, cx| {
                this.context_anchor = *position;
                this.context_open = true;
                cx.notify();
            }))
            .on_activate(cx.listener(|this, id: &SharedString, _, cx| {
                this.menu_log = SharedString::from(format!("context → {id}"));
                this.context_open = false;
                cx.notify();
            }))
            .on_close(move |_window, app| {
                context_close_view.update(app, |this, cx| {
                    this.context_open = false;
                    cx.notify();
                });
            });

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new("Menus"))
            .child(menubar)
            .child(self.command_palette.clone())
            .child(
                div()
                    .flex()
                    .gap_4()
                    .flex_wrap()
                    .items_start()
                    .child(div().max_w(px(220.0)).child(standalone))
                    .child(div().w(px(220.0)).child(panel_menu))
                    .child(div().w(px(260.0)).child(tiered_menu))
                    .child(div().rounded(px(8.0)).border_1().child(context)),
            )
            .child(Label::new(self.menu_log.clone()).muted(true))
    }

    /// Renders the feedback and overlay demo: inline `Message`s, a `Drawer`, a
    /// `ConfirmDialog`, and a `ToastStack`.
    fn render_feedback(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let messages = div()
            .flex()
            .flex_col()
            .gap_2()
            .max_w(px(420.0))
            .child(Message::new("Saved 2 minutes ago").variant(MessageVariant::Info))
            .child(Message::new("Connection restored").variant(MessageVariant::Success))
            .child(Message::new("Token expires soon").variant(MessageVariant::Warning))
            .child(Message::new("Name is required").variant(MessageVariant::Danger));

        let triggers = div()
            .flex()
            .gap_3()
            .flex_wrap()
            .items_center()
            .child(
                Button::new("Open Drawer")
                    .secondary()
                    .size(ComponentSize::Small)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.drawer_open = true;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("Delete project")
                    .danger()
                    .size(ComponentSize::Small)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.confirm_open = true;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("Push toast")
                    .size(ComponentSize::Small)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toast_seq += 1;
                        this.toasts.push(this.toast_seq);
                        cx.notify();
                    })),
            )
            .child(
                ConfirmPopup::new(
                    "gallery-confirm-popup",
                    Button::new("Archive project")
                        .secondary()
                        .size(ComponentSize::Small),
                )
                .open(self.confirm_popup_open)
                .message("Archive this project? You can restore it later.")
                .confirm_label("Archive")
                .on_confirm(cx.listener(|this, _, _, cx| {
                    this.confirm_popup_open = false;
                    this.confirm_log = SharedString::from("Project archived");
                    cx.notify();
                }))
                .on_cancel(cx.listener(|this, _, _, cx| {
                    this.confirm_popup_open = false;
                    cx.notify();
                })),
            )
            .child(
                Button::new("Confirm archive")
                    .secondary()
                    .size(ComponentSize::Small)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.confirm_popup_open = !this.confirm_popup_open;
                        cx.notify();
                    })),
            )
            .child(Label::new(self.confirm_log.clone()).muted(true));

        let drawer = Drawer::new("gallery-drawer")
            .open(self.drawer_open)
            .side(DrawerSide::Right)
            .title("Inspector")
            .on_close(cx.listener(|this, _, _, cx| {
                this.drawer_open = false;
                cx.notify();
            }))
            .child(Label::new("Drawer content lives here.").muted(true))
            .child(Message::new("Edits apply immediately").variant(MessageVariant::Info))
            .footer(
                Button::new("Done")
                    .primary()
                    .size(ComponentSize::Small)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.drawer_open = false;
                        cx.notify();
                    })),
            );

        let confirm = ConfirmDialog::new("gallery-confirm")
            .open(self.confirm_open)
            .title("Delete project?")
            .message("This permanently removes the workspace and cannot be undone.")
            .confirm_label("Delete")
            .cancel_label("Keep")
            .danger(true)
            .on_confirm(cx.listener(|this, _, _, cx| {
                this.confirm_open = false;
                this.confirm_log = SharedString::from("Project deleted");
                cx.notify();
            }))
            .on_cancel(cx.listener(|this, _, _, cx| {
                this.confirm_open = false;
                this.confirm_log = SharedString::from("Deletion cancelled");
                cx.notify();
            }));

        let toasts = self
            .toasts
            .iter()
            .map(|seq| {
                let seq = *seq;
                Toast::new(format!("toast-{seq}"), format!("Notification #{seq}"))
                    .variant(ToastVariant::Success)
                    .description("Click the close button to dismiss.")
                    .on_close(cx.listener(move |this, _, _, cx| {
                        this.toasts.retain(|id| *id != seq);
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();
        let toast_stack = ToastStack::new("gallery-toasts").toasts(toasts);

        div()
            .relative()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new("Feedback & Overlays"))
            .child(messages)
            .child(triggers)
            .child(drawer)
            .child(confirm)
            .child(toast_stack)
    }

    /// Renders the numeric input demo: a draggable `Slider` and a keyboard-aware
    /// `InputNumber` stepper.
    fn render_inputs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let slider_value = self.slider.read(cx).current_value();

        let slider_row = div()
            .flex()
            .flex_col()
            .gap_2()
            .max_w(px(420.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(Label::new("Volume").muted(true))
                    .child(Label::new(format!("{}", slider_value as i32))),
            )
            .child(self.slider.clone());

        let input_number = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new("Quantity").muted(true))
            .child(
                InputNumber::new("gallery-input-number")
                    .value(self.input_number_value)
                    .range(0.0, 20.0)
                    .step(1.0)
                    .suffix("items")
                    .focusable(self.input_number_focus.clone())
                    .on_change(cx.listener(|this, value: &f64, _, cx| {
                        this.input_number_value = *value;
                        cx.notify();
                    })),
            );

        let multi_select = div()
            .flex()
            .flex_col()
            .gap_2()
            .max_w(px(320.0))
            .child(Label::new("Labels").muted(true))
            .child(
                MultiSelect::new("gallery-multi-select")
                    .placeholder("Select labels")
                    .items(vec![
                        SelectItem::new("bug", "Bug"),
                        SelectItem::new("docs", "Docs"),
                        SelectItem::new("perf", "Performance"),
                        SelectItem::new("ui", "UI"),
                    ])
                    .selected(self.multi_select_selected.clone())
                    .expanded(self.multi_select_open)
                    .on_toggle(cx.listener(|this, expanded, _, cx| {
                        this.multi_select_open = *expanded;
                        cx.notify();
                    }))
                    .on_select(cx.listener(|this, index: &usize, _, cx| {
                        if let Some(pos) =
                            this.multi_select_selected.iter().position(|i| i == index)
                        {
                            this.multi_select_selected.remove(pos);
                        } else {
                            this.multi_select_selected.push(*index);
                        }
                        cx.notify();
                    })),
            );

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new("Numeric Inputs"))
            .child(slider_row)
            .child(input_number)
            .child(multi_select)
    }

    /// Renders picker-style inputs that are host-managed but need live state in
    /// the gallery.
    fn render_forms_and_pickers(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let file_picker_view = cx.entity();
        let file_remove_view = file_picker_view.clone();
        let date_picker = DatePicker::new("gallery-date-picker")
            .value(self.date_value.clone())
            .open(self.date_open)
            .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                this.date_open = *open;
                cx.notify();
            }))
            .on_change(cx.listener(|this, value: &SharedString, _, cx| {
                this.date_value = value.clone();
                this.date_open = false;
                cx.notify();
            }));

        let input_otp = InputOtp::new("gallery-otp")
            .length(6)
            .value(self.otp_value.clone())
            .focusable(self.otp_focus.clone())
            .on_change(cx.listener(|this, value: &SharedString, _, cx| {
                this.otp_value = value.clone();
                cx.notify();
            }));

        let color_picker = ColorPicker::new("gallery-color-picker")
            .value(self.color_value.clone())
            .swatches(vec![
                ColorSwatch::new("#3b82f6", "Blue"),
                ColorSwatch::new("#22c55e", "Green"),
                ColorSwatch::new("#f59e0b", "Amber"),
                ColorSwatch::new("#ef4444", "Red"),
            ])
            .on_change(cx.listener(|this, value: &SharedString, _, cx| {
                this.color_value = value.clone();
                cx.notify();
            }));

        let file_picker = FilePicker::new("gallery-file-picker")
            .label("Attach files")
            .files(self.file_picker_files.clone())
            .on_request_pick(move |_window, app| {
                file_picker_view.update(app, |this, cx| {
                    if !this
                        .file_picker_files
                        .iter()
                        .any(|file| file.as_ref() == "release-notes.md")
                    {
                        this.file_picker_files
                            .push(SharedString::from("release-notes.md"));
                    }
                    cx.notify();
                });
            })
            .on_remove(move |file, _window, app| {
                file_remove_view.update(app, |this, cx| {
                    this.file_picker_files.retain(|candidate| candidate != file);
                    cx.notify();
                });
            });

        let chips = div()
            .flex()
            .gap_2()
            .flex_wrap()
            .child(Chip::new("Native").selected(true))
            .child(Chip::new("Accessible").selected(true).removable(true))
            .child(Chip::new("Blocked").disabled(true));

        let fieldset = Fieldset::new()
            .legend("Preferences")
            .description("Grouped controls share one labelled surface.")
            .child(
                Checkbox::new("forms-fieldset-notifications")
                    .label("Enable notifications")
                    .checked(true),
            )
            .child(
                Radio::new("forms-fieldset-native")
                    .label("Native renderer")
                    .checked(true),
            )
            .child(
                Switch::new("forms-fieldset-preview")
                    .label("Preview mode")
                    .checked(true),
            );

        let form = Form::new("gallery-form")
            .label("Account form")
            .child(FormSummary::new(
                "gallery-form-summary",
                vec![ValidationIssue::error(
                    "notifications",
                    "Notifications",
                    "Choose a delivery channel",
                )],
            ))
            .child(
                FormField::new(
                    "notifications",
                    "Notifications",
                    Checkbox::new("gallery-form-notifications")
                        .label("Send release notifications")
                        .checked(false),
                )
                .required(true)
                .description("Controls product release alerts")
                .validation(ValidationSeverity::Error, "Choose a delivery channel"),
            )
            .action(Button::new("Save"));

        let image = Image::new("gallery-preview.png")
            .alt("Gallery image fallback")
            .fit(ImageFit::Cover)
            .width(220.0)
            .height(120.0);

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new("Forms & Pickers"))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .flex_wrap()
                    .items_start()
                    .child(
                        div()
                            .w(px(320.0))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(Label::new("Date").muted(true))
                            .child(date_picker)
                            .child(Label::new("One-time code").muted(true))
                            .child(input_otp)
                            .child(Label::new("Color").muted(true))
                            .child(color_picker),
                    )
                    .child(
                        div()
                            .w(px(360.0))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(Label::new("Autocomplete").muted(true))
                            .child(self.autocomplete.clone())
                            .child(Label::new(self.autocomplete_selected.clone()).muted(true))
                            .child(file_picker)
                            .child(fieldset)
                            .child(form)
                            .child(Label::new("Chips").muted(true))
                            .child(chips)
                            .child(Label::new("Image fallback").muted(true))
                            .child(image),
                    ),
            )
    }

    /// Renders the collection component catalog.
    fn render_collections(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let data_view = DataView::new("gallery-data-view")
            .layout(DataViewLayout::Grid)
            .selected(self.data_view_selected.clone())
            .items(vec![
                DataViewItem::new("runtime", "Runtime")
                    .description("Core focus, overlays, assets, and themes")
                    .metadata("Ready")
                    .badge("Core"),
                DataViewItem::new("components", "Components")
                    .description("Reusable controls and collection widgets")
                    .metadata("Expanded")
                    .badge("UI"),
                DataViewItem::new("subsystems", "Subsystems")
                    .description("Charts, editor, and terminal crates")
                    .metadata("Preview")
                    .badge("New"),
            ])
            .on_select(cx.listener(|this, id: &SharedString, _, cx| {
                this.data_view_selected = id.clone();
                cx.notify();
            }));

        let listbox = Listbox::new("gallery-listbox")
            .selection_mode(ListboxSelectionMode::Multiple)
            .items(vec![
                SelectItem::new("rust", "Rust"),
                SelectItem::new("gpui", "GPUI"),
                SelectItem::new("accesskit", "AccessKit"),
                SelectItem::new("blocked", "Blocked").disabled(true),
            ])
            .selected(self.listbox_selected.clone())
            .on_selection_change(cx.listener(|this, selected: &Vec<usize>, _, cx| {
                this.listbox_selected = selected.clone();
                cx.notify();
            }));

        let cascade = CascadeSelect::new("gallery-cascade-select")
            .expanded(self.cascade_open)
            .path(self.cascade_path.clone())
            .options(vec![
                CascadeOption::new("platform", "Platform").children(vec![
                    CascadeOption::new("macos", "macOS"),
                    CascadeOption::new("linux", "Linux"),
                    CascadeOption::new("windows", "Windows"),
                ]),
                CascadeOption::new("subsystem", "Subsystem").children(vec![
                    CascadeOption::new("charts", "Charts"),
                    CascadeOption::new("editor", "Editor"),
                    CascadeOption::new("terminal", "Terminal"),
                ]),
            ])
            .on_toggle(cx.listener(|this, expanded, _, cx| {
                this.cascade_open = *expanded;
                cx.notify();
            }))
            .on_select(cx.listener(|this, path: &Vec<usize>, _, cx| {
                this.cascade_path = path.clone();
                cx.notify();
            }));

        let tree_select = TreeSelect::new("gallery-tree-select")
            .expanded(self.tree_select_open)
            .selected(self.tree_select_selected.clone())
            .nodes(vec![
                TreeSelectNode::new("foundation", "Foundation")
                    .expanded(true)
                    .children(vec![
                        TreeSelectNode::new("core", "Core"),
                        TreeSelectNode::new("components", "Components"),
                    ]),
                TreeSelectNode::new("subsystems", "Subsystems")
                    .expanded(true)
                    .children(vec![
                        TreeSelectNode::new("charts", "Charts"),
                        TreeSelectNode::new("editor", "Editor"),
                        TreeSelectNode::new("terminal", "Terminal"),
                    ]),
            ])
            .on_toggle(cx.listener(|this, expanded, _, cx| {
                this.tree_select_open = *expanded;
                cx.notify();
            }))
            .on_select(cx.listener(|this, id: &SharedString, _, cx| {
                this.tree_select_selected = id.clone();
                cx.notify();
            }));

        let tree_table = TreeTable::new("gallery-tree-table")
            .columns(vec![
                TreeTableColumn::new("name", "Name").width(220),
                TreeTableColumn::new("status", "Status").width(120),
                TreeTableColumn::new("owner", "Owner"),
            ])
            .rows(vec![
                TreeTableRow::new("components", vec!["Components", "Ready", "UI"])
                    .expanded(true)
                    .selected(self.tree_table_selected == "components")
                    .children(vec![
                        TreeTableRow::new("inputs", vec!["Inputs", "Ready", "Forms"])
                            .selected(self.tree_table_selected == "inputs"),
                        TreeTableRow::new("collections", vec!["Collections", "Ready", "Data"])
                            .selected(self.tree_table_selected == "collections"),
                    ]),
                TreeTableRow::new("docs", vec!["Documentation", "Review", "DX"])
                    .expanded(self.tree_table_docs_expanded)
                    .selected(self.tree_table_selected == "docs")
                    .children(vec![
                        TreeTableRow::new("guides", vec!["Guides", "Updated", "DX"])
                            .selected(self.tree_table_selected == "guides"),
                    ]),
            ])
            .on_select(cx.listener(|this, id: &SharedString, _, cx| {
                this.tree_table_selected = id.clone();
                cx.notify();
            }))
            .on_toggle(cx.listener(|this, id: &SharedString, _, cx| {
                if id.as_ref() == "docs" {
                    this.tree_table_docs_expanded = !this.tree_table_docs_expanded;
                }
                cx.notify();
            }));

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new("Collections & Hierarchies"))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .flex_wrap()
                    .items_start()
                    .child(div().w(px(520.0)).child(data_view))
                    .child(div().w(px(320.0)).child(listbox)),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .flex_wrap()
                    .items_start()
                    .child(div().w(px(360.0)).child(cascade))
                    .child(div().w(px(360.0)).child(tree_select)),
            )
            .child(div().max_w(px(760.0)).child(tree_table))
    }

    /// Renders dedicated subsystem crates inside the gallery.
    fn render_subsystems(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let chart_points = vec![
            ChartPoint::category("Jan", 12.0),
            ChartPoint::category("Feb", 18.0),
            ChartPoint::category("Mar", 14.0),
            ChartPoint::category("Apr", 26.0),
        ];
        let actual = ChartDataset::new("actual", "Actual").points(chart_points.clone());
        let forecast = ChartDataset::new("forecast", "Forecast").points(vec![
            ChartPoint::category("Jan", 10.0),
            ChartPoint::category("Feb", 16.0),
            ChartPoint::category("Mar", 20.0),
            ChartPoint::category("Apr", 30.0),
        ]);
        let chart_options = ChartOptions::default().height(180.0).values(true);

        let editor = CodeEditor::new("gallery-code-editor", self.editor_session.buffer().clone())
            .selections(self.editor_session.selections().to_vec())
            .focusable(self.editor_focus.clone())
            .options(CodeEditorOptions::default().visible_lines(8))
            .on_edit(cx.listener(|this, edit: &EditorEdit, _, cx| {
                this.editor_session.apply(edit.clone());
                cx.notify();
            }));

        let mut terminal_model = TerminalModel::new(48, 6);
        terminal_model.write("\u{1b}[32mguic\u{1b}[0m check\n");
        terminal_model.write("charts: ready\neditor: ready\nterminal: ready\n");
        let terminal = Terminal::new("gallery-terminal", terminal_model)
            .options(TerminalOptions::default().visible_scrollback(1));

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new("Subsystem Crates"))
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap_4()
                    .child(
                        LineChart::new("gallery-line-chart")
                            .options(chart_options.clone().title("Revenue"))
                            .datasets(vec![actual.clone(), forecast]),
                    )
                    .child(
                        BarChart::new("gallery-bar-chart")
                            .options(chart_options.clone().title("Builds"))
                            .datasets(vec![actual.clone()]),
                    )
                    .child(
                        AreaChart::new("gallery-area-chart")
                            .options(chart_options.clone().title("Coverage"))
                            .datasets(vec![actual.clone()]),
                    )
                    .child(
                        PieChart::new("gallery-pie-chart")
                            .options(chart_options.title("Subsystem Share"))
                            .datasets(vec![ChartDataset::new("share", "Share").points(vec![
                                ChartPoint::category("Components", 52.0),
                                ChartPoint::category("Charts", 18.0),
                                ChartPoint::category("Editor", 16.0),
                                ChartPoint::category("Terminal", 14.0),
                            ])]),
                    ),
            )
            .child(div().max_w(px(760.0)).child(editor))
            .child(div().max_w(px(760.0)).child(terminal))
    }
}

impl Render for StoryRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let themes = [
            Theme::DEFAULT_DARK_NAME,
            Theme::DEFAULT_LIGHT_NAME,
            Theme::HIGH_CONTRAST_DARK_NAME,
            Theme::HIGH_CONTRAST_LIGHT_NAME,
        ];

        let mut theme_row = div().flex().gap_2().flex_wrap();
        for name in themes {
            let selected = self.active_theme == name;
            let button = if selected {
                Button::new(Self::theme_label(name))
                    .primary()
                    .size(ComponentSize::Small)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_theme(name, cx)))
            } else {
                Button::new(Self::theme_label(name))
                    .secondary()
                    .size(ComponentSize::Small)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_theme(name, cx)))
            };
            theme_row = theme_row.child(button);
        }

        let alert = if self.alert_open {
            Alert::new("Browse the components included in this preview release.")
                .title("Gallery status")
                .info()
                .closable(true)
                .on_close(cx.listener(|this, _, _, cx| {
                    this.alert_open = false;
                    cx.notify();
                }))
                .into_any_element()
        } else {
            div().into_any_element()
        };

        let controls = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new("Selection Controls").muted(true))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .flex_wrap()
                    .items_center()
                    .child(
                        Checkbox::new("story-checkbox")
                            .label("Enable release checklist")
                            .checked(self.checkbox_checked)
                            .on_toggle(cx.listener(|this, checked, _, cx| {
                                this.checkbox_checked = *checked;
                                cx.notify();
                            })),
                    )
                    .child(
                        Radio::new("story-radio-a")
                            .label("Alpha")
                            .checked(self.selected_radio == 0)
                            .on_select(cx.listener(|this, _, _, cx| this.set_radio(0, cx))),
                    )
                    .child(
                        Radio::new("story-radio-b")
                            .label("Beta")
                            .checked(self.selected_radio == 1)
                            .on_select(cx.listener(|this, _, _, cx| this.set_radio(1, cx))),
                    )
                    .child(
                        Switch::new("story-switch")
                            .label("Feature flag")
                            .checked(self.switch_on)
                            .on_toggle(cx.listener(|this, checked, _, cx| {
                                this.switch_on = *checked;
                                cx.notify();
                            })),
                    ),
            );

        let tabs = Tabs::new("story-tabs")
            .selected(self.active_tab)
            .items(vec![
                TabItem::new("overview", "Overview"),
                TabItem::new("runtime", "Runtime"),
                TabItem::new("components", "Components"),
                TabItem::new("blocked", "Blocked").disabled(true),
            ])
            .on_select(cx.listener(|this, index, _, cx| {
                this.active_tab = *index;
                cx.notify();
            }));

        let select = Select::new("story-select")
            .placeholder("Select a milestone")
            .selected(self.selected_option)
            .expanded(self.select_expanded)
            .items(vec![
                SelectItem::new("alpha", "Alpha"),
                SelectItem::new("beta", "Beta"),
                SelectItem::new("stable", "Stable"),
            ])
            .on_toggle(cx.listener(|this, expanded, _, cx| {
                this.set_select_expanded(*expanded, cx);
            }))
            .on_select(cx.listener(|this, selected, _, cx| this.select_option(*selected, cx)));

        let popover = Popover::new(
            "story-popover",
            Button::new("Toggle Popover")
                .secondary()
                .size(ComponentSize::Small)
                .on_click(cx.listener(|this, _, _, cx| this.toggle_popover(cx))),
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(Label::new("Popover content").muted(true))
                .child(Label::new("Use this surface for contextual actions.")),
        )
        .open(self.popover_open)
        .width(260.0);

        let dialog = Dialog::new("story-dialog")
            .open(self.dialog_open)
            .title("Ship the remaining review fixes?")
            .description(
                "This dialog represents the confirmation surface used by higher-level flows.",
            )
            .content(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Label::new(
                        "The component set now includes inputs and overlay surfaces.",
                    ))
                    .child(
                        Label::new(
                            "The final readiness call still depends on deeper verification.",
                        )
                        .muted(true),
                    ),
            )
            .secondary_label("Cancel")
            .primary_label("Continue")
            .on_cancel(cx.listener(|this, _, _, cx| this.set_dialog_open(false, cx)))
            .on_confirm(cx.listener(|this, _, _, cx| this.set_dialog_open(false, cx)));

        let scroll_demo = ScrollArea::new(
            "story-scroll-area",
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children((0..12).map(|index| {
                    div()
                        .px_3()
                        .py_2()
                        .rounded(px(8.0))
                        .bg(gpui::black().opacity(0.08))
                        .child(format!("Scrollable row {}", index + 1))
                })),
        )
        .vertical(true)
        .horizontal(false);

        let list = VirtualList::new("story-virtual-list", 100, move |range, _window, _cx| {
            range
                .map(|index| {
                    div()
                        .id(index)
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(gpui::black().opacity(0.08))
                        .child(format!("Virtual item {}", index + 1))
                        .into_any_element()
                })
                .collect()
        })
        .height(px(180.0));

        let metrics = div()
            .flex()
            .gap_3()
            .flex_wrap()
            .child(MetricCard::new("Components", "Preview").detail("Interactive catalog"))
            .child(MetricCard::new("Themes", "4").detail("Built-in and switchable"))
            .child(MetricCard::new("Workspace", "Ready").detail("check, test, clippy clean"));

        let property_list = PropertyList::new("Runtime").items(vec![
            PropertyItem::new("Theme", Self::theme_label(self.active_theme))
                .badge("active", guic::components::BadgeVariant::Success),
            PropertyItem::new("Overlay count", if self.dialog_open { "1" } else { "0" }),
            PropertyItem::new("Progress", format!("{}%", self.progress as i32)),
        ]);

        let rows = self.release_rows();

        let table = self
            .release_table_model()
            .title("Release Checklist")
            .rows(rows)
            .focusable(self.table_focus.clone())
            .on_sort(cx.listener(|this, sort, _, cx| this.set_table_sort(sort, cx)))
            .on_column_resize(cx.listener(|this, resize: &DataColumnResize, _, cx| {
                if resize.column_id().as_ref() == "status" {
                    this.status_column_width = resize.width();
                    cx.notify();
                }
            }))
            .on_row_select(cx.listener(|this, row_id: &SharedString, _, cx| {
                this.set_table_row(row_id.as_ref(), cx)
            }))
            .render_cell(|cell| {
                if cell.column_id().as_ref() == "status" {
                    let badge = match cell.value().as_ref() {
                        "Ready" => Badge::new(cell.value().clone()).success(),
                        "Review" => Badge::new(cell.value().clone()).warning(),
                        _ => Badge::new(cell.value().clone()),
                    };
                    badge.into_any_element()
                } else {
                    Label::new(cell.value().clone()).into_any_element()
                }
            })
            .render_row_actions(|_| {
                Button::new("Inspect")
                    .secondary()
                    .size(ComponentSize::Small)
                    .into_any_element()
            })
            .max_height(220.0);

        let virtualized_table = DataTable::new("story-data-table-virtualized")
            .title("Large Dataset Preview")
            .columns(vec![
                DataColumn::new("index", "Row")
                    .width(88.0)
                    .pin(DataColumnPin::Start),
                DataColumn::new("owner", "Owner").width(180.0),
                DataColumn::new("status", "Status").width(140.0),
                DataColumn::new("notes", "Notes")
                    .width(240.0)
                    .pin(DataColumnPin::End),
            ])
            .rows(
                (0..200)
                    .map(|index| {
                        DataRow::new(
                            format!("build-{index}"),
                            vec![
                                format!("#{}", index + 1),
                                format!("Team {}", index % 8 + 1),
                                if index % 5 == 0 { "Review" } else { "Ready" }.to_string(),
                                format!("Virtualized row {}", index + 1),
                            ],
                        )
                    })
                    .collect(),
            )
            .row_height(36.0)
            .viewport(DataTableViewport::new(540.0, 180.0).overscan(2))
            .column_viewport(DataTableColumnViewport::new(80.0, 260.0).overscan(40.0))
            .max_height(180.0)
            .state(DataTableState::Ready);

        let tree_model = self.workspace_tree_model();
        let visible_tree_count = tree_model.visible_node_ids().len();
        let tree = tree_model
            .title("Workspace")
            .focusable(self.tree_focus.clone())
            .on_select(cx.listener(|this, node_id: &SharedString, _, cx| {
                this.select_tree_node(node_id.as_ref(), cx);
            }))
            .on_toggle(cx.listener(|this, node_id: &SharedString, _, cx| {
                this.toggle_tree_node(node_id.as_ref(), cx);
            }));
        let virtualized_tree = TreeView::new("story-tree-virtualized")
            .title("Large Workspace Preview")
            .nodes(
                (0..10_000)
                    .map(|index| {
                        TreeNode::new(
                            format!("generated-node-{index}"),
                            format!("Generated node {index}"),
                        )
                        .detail(format!("virtual item {index}"))
                    })
                    .collect(),
            )
            .row_height(36.0)
            .viewport(TreeViewport::new(90_000.0, 216.0).overscan(2))
            .max_height(216.0);

        let table_navigation = div()
            .max_w(px(760.0))
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new("Release Checklist Navigation").muted(true))
            .child(
                Label::new(format!(
                    "Selected row: {} • Visible rows: {}",
                    self.selected_table_row_id,
                    self.release_table_model().visible_row_ids().len()
                ))
                .muted(true),
            )
            .child(
                Label::new(format!(
                    "Status column width: {:.0}px",
                    self.release_table_model()
                        .column_width("status")
                        .unwrap_or(self.status_column_width)
                ))
                .muted(true),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("Home")
                            .secondary()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_table(DataTableNavigation::Home, cx);
                            })),
                    )
                    .child(
                        Button::new("Up")
                            .secondary()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_table(DataTableNavigation::Up, cx);
                            })),
                    )
                    .child(
                        Button::new("Down")
                            .secondary()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_table(DataTableNavigation::Down, cx);
                            })),
                    )
                    .child(
                        Button::new("Page Up")
                            .secondary()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_table(DataTableNavigation::PageUp, cx);
                            })),
                    )
                    .child(
                        Button::new("Page Down")
                            .secondary()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_table(DataTableNavigation::PageDown, cx);
                            })),
                    )
                    .child(
                        Button::new("End")
                            .secondary()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_table(DataTableNavigation::End, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("Shrink Status")
                            .ghost()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.resize_status_column(-24.0, cx);
                            })),
                    )
                    .child(
                        Button::new("Grow Status")
                            .ghost()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.resize_status_column(24.0, cx);
                            })),
                    ),
            );

        let tree_navigation = div()
            .min_w(px(280.0))
            .max_w(px(320.0))
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new("Workspace Navigation").muted(true))
            .child(
                Label::new(format!(
                    "Selected node: {} • Visible nodes: {}",
                    self.selected_tree_node, visible_tree_count
                ))
                .muted(true),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("Home")
                            .secondary()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_tree(TreeNavigation::Home, cx);
                            })),
                    )
                    .child(
                        Button::new("Up")
                            .secondary()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_tree(TreeNavigation::Up, cx);
                            })),
                    )
                    .child(
                        Button::new("Down")
                            .secondary()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_tree(TreeNavigation::Down, cx);
                            })),
                    )
                    .child(
                        Button::new("Left")
                            .secondary()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_tree(TreeNavigation::Left, cx);
                            })),
                    )
                    .child(
                        Button::new("Right")
                            .secondary()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_tree(TreeNavigation::Right, cx);
                            })),
                    )
                    .child(
                        Button::new("End")
                            .secondary()
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_tree(TreeNavigation::End, cx);
                            })),
                    ),
            );

        let markdown = Markdown::new(
            "# Component subsystems\n\n\
            GUIC ships optional component subsystems from the canonical component crate.\n\n\
            - Data table surface\n\
            - Hierarchical tree view\n\
            - Markdown and HTML preview blocks\n\n\
            > Remaining roadmap items still need deeper interaction and performance validation.\n\n\
            ```rust\n\
            let status = \"production-oriented\";\n\
            ```",
        );

        let html_preview = HtmlFragment::new(
            "<section><strong>Embedded HTML</strong> keeps preview-oriented rendering lightweight.</section>",
        );
        let dock_leaf_count = self.dock_layout.leaf_count();
        let dock_tab_count = self.dock_layout.tab_count();
        let dock = Dock::new("story-dock", self.dock_layout.clone())
            .title("Workspace Dock")
            .focusable(self.dock_focus.clone())
            .on_command(cx.listener(|this, command: &DockCommand, _, cx| {
                this.apply_dock_command(command, cx);
            }));

        let catalog = self.render_catalog(cx);
        let menus = self.render_menus(cx);
        let feedback = self.render_feedback(cx);
        let inputs = self.render_inputs(cx);
        let forms_and_pickers = self.render_forms_and_pickers(cx);
        let collections = self.render_collections(cx);
        let subsystems = self.render_subsystems(cx);

        let content = div()
            .w_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Label::new("GUIC Component Gallery")
                            .secondary("Interactive reference for GUIC components"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(Badge::new("sample").primary())
                            .child(Badge::new("workspace"))
                            .child(Badge::new(Self::theme_label(self.active_theme)).success()),
                    ),
            )
            .child(alert)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new("Themes"))
                    .child(theme_row),
            )
            .child(Separator::new())
            .child(catalog)
            .child(Separator::new())
            .child(menus)
            .child(Separator::new())
            .child(feedback)
            .child(Separator::new())
            .child(inputs)
            .child(Separator::new())
            .child(forms_and_pickers)
            .child(Separator::new())
            .child(collections)
            .child(Separator::new())
            .child(subsystems)
            .child(Separator::new())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new("Buttons"))
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .flex_wrap()
                            .items_center()
                            .child(
                                Button::new("Primary Action").primary().on_click(
                                    cx.listener(|this, _, _, cx| this.advance_progress(cx)),
                                ),
                            )
                            .child(Button::new("Secondary").secondary())
                            .child(Button::new("Ghost").ghost())
                            .child(Button::new("Danger").danger())
                            .child(Button::new("Disabled").disabled(true))
                            .child(
                                IconButton::new(IconName::Search).variant(ButtonVariant::Secondary),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new("Selection Controls"))
                    .child(controls),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new("Inputs"))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(520.0))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(self.text_input.clone())
                            .child(self.search_input.clone())
                            .child(self.password_input.clone())
                            .child(self.text_area.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new("Progress"))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(420.0))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(Progress::new(self.progress).id("gallery-progress"))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Button::new("Advance").size(ComponentSize::Small).on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.advance_progress(cx);
                                            }),
                                        ),
                                    )
                                    .child(
                                        Label::new(format!("{}%", self.progress as i32))
                                            .muted(true),
                                    ),
                            )
                            .child(
                                div().w_full().max_w(px(200.0)).child(
                                    Progress::new(0.0)
                                        .id("gallery-indeterminate-progress")
                                        .size(ComponentSize::Small)
                                        .indeterminate(true),
                                ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new("Tabs"))
                    .child(tabs)
                    .child(
                        Label::new(match self.active_tab {
                            0 => "Overview content",
                            1 => "Runtime content",
                            2 => "Components content",
                            _ => "Unavailable content",
                        })
                        .muted(true),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new("Overlay Surfaces"))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(520.0))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(select)
                            .child(popover)
                            .child(
                                div()
                                    .flex()
                                    .gap_3()
                                    .items_center()
                                    .child(Tooltip::new(
                                        Button::new("Hover for Tooltip")
                                            .ghost()
                                            .size(ComponentSize::Small),
                                        "Tooltips use GPUI's native tooltip lifecycle.",
                                    ))
                                    .child(
                                        Button::new("Open Dialog")
                                            .secondary()
                                            .size(ComponentSize::Small)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.set_dialog_open(true, cx);
                                            })),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new("Advanced Widgets"))
                    .child(metrics)
                    .child(div().max_w(px(520.0)).child(property_list))
                    .child(
                        div()
                            .max_w(px(860.0))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(Label::new("Dock Layout").muted(true))
                            .child(
                                Label::new(format!(
                                    "Leaf stacks: {} • Tabs: {} • Persistent JSON-ready layout",
                                    dock_leaf_count, dock_tab_count
                                ))
                                .muted(true),
                            )
                            .child(div().h(px(360.0)).child(dock)),
                    )
                    .child(div().max_w(px(760.0)).child(table))
                    .child(table_navigation)
                    .child(div().max_w(px(760.0)).child(virtualized_table))
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .flex_wrap()
                            .items_start()
                            .child(
                                div()
                                    .min_w(px(280.0))
                                    .max_w(px(320.0))
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .child(tree)
                                    .child(tree_navigation)
                                    .child(virtualized_tree),
                            )
                            .child(
                                div()
                                    .min_w(px(320.0))
                                    .max_w(px(420.0))
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .child(markdown)
                                    .child(html_preview),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new("Scrolling"))
                    .child(div().h(px(140.0)).max_w(px(320.0)).child(scroll_demo)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new("Virtual List"))
                    .child(div().max_w(px(320.0)).child(list)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new("Status"))
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .items_center()
                            .child(Spinner::new())
                            .child(Label::new("Loading design tokens...").muted(true)),
                    ),
            );

        div()
            .size_full()
            .child(
                ScrollArea::new("gallery-scroll-area", content)
                    .vertical(true)
                    .horizontal(false),
            )
            .child(dialog)
    }
}

/// Starts the shared GUIC component gallery application.
pub fn run() {
    gpui_platform::application().run(|cx: &mut gpui::App| {
        guic::init(cx);
        app::init(cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                gpui::size(gpui::px(1200.), gpui::px(900.)),
                cx,
            ))),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("GUIC Component Gallery".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let view = cx.new(StoryRoot::new);
            cx.new(|cx| guic::core::Root::new(view, window, cx))
        })
        .expect("failed to open GUIC component gallery window");
    })
}
