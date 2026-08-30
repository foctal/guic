//! General-purpose GUIC components.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use gpui::{App, ClickEvent, KeyDownEvent, Window};
use std::rc::Rc;

fn handle_roving_focus_key(
    event: &KeyDownEvent,
    position: usize,
    item_count: usize,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let moves = match event.keystroke.key.as_str() {
        "left" | "up" => -(position.min(1) as isize),
        "right" | "down" => usize::from(position + 1 < item_count) as isize,
        "home" => -(position as isize),
        "end" => item_count.saturating_sub(position + 1) as isize,
        _ => return false,
    };
    move_roving_focus(moves, window, cx);
    true
}

fn move_roving_focus(moves: isize, window: &mut Window, cx: &mut App) {
    for _ in 0..moves.unsigned_abs() {
        if moves < 0 {
            window.focus_prev(cx);
        } else {
            window.focus_next(cx);
        }
    }
}

mod accordion;
mod alert;
mod auto_complete;
mod avatar;
mod badge;
mod breadcrumb;
mod button;
mod card;
mod cascade_select;
mod checkbox;
mod chip;
mod color_picker;
mod command_palette;
mod confirm_dialog;
mod confirm_popup;
mod data_view;
mod date_picker;
mod dialog;
mod drawer;
mod fieldset;
mod file_picker;
mod form;
mod icon_button;
mod image;
mod input_number;
mod input_otp;
mod label;
mod listbox;
mod menu;
mod message;
mod metric_card;
mod multi_select;
mod paginator;
mod panel;
mod pick_list;
mod popover;
mod progress;
mod property_list;
mod radio;
mod scroll_area;
mod select;
mod separator;
mod size;
mod slider;
mod spinner;
mod splitter;
mod stepper;
mod switch;
mod tab_menu;
mod tabs;
mod tag;
mod text_input;
mod timeline;
mod toast;
mod toolbar;
mod tooltip;
mod tree_select;
mod tree_table;
mod virtual_list;

#[cfg(feature = "data-table")]
mod data_table;
#[cfg(feature = "dock")]
mod dock;
#[cfg(feature = "markdown")]
mod markdown;
#[cfg(feature = "tree")]
mod tree;

pub use accordion::{Accordion, AccordionSection};
pub use alert::{Alert, AlertVariant};
pub use auto_complete::AutoComplete;
pub use avatar::{Avatar, AvatarShape, AvatarStatus};
pub use badge::{Badge, BadgeVariant};
pub use breadcrumb::{Breadcrumb, BreadcrumbItem};
pub use button::{Button, ButtonVariant};
pub use card::Card;
pub use cascade_select::{CascadeOption, CascadeSelect};
pub use checkbox::Checkbox;
pub use chip::Chip;
pub use color_picker::{ColorPicker, ColorSwatch};
pub use command_palette::{CommandPalette, CommandPaletteItem};
pub use confirm_dialog::ConfirmDialog;
pub use confirm_popup::ConfirmPopup;
pub use data_view::{DataView, DataViewItem, DataViewLayout};
pub use date_picker::DatePicker;
pub use dialog::Dialog;
pub use drawer::{Drawer, DrawerSide};
pub use fieldset::Fieldset;
pub use file_picker::FilePicker;
pub use form::{Form, FormField, FormSummary, ValidationIssue, ValidationSeverity};
pub use icon_button::IconButton;
pub use image::{Image, ImageFit};
pub use input_number::InputNumber;
pub use input_otp::InputOtp;
pub use label::Label;
pub use listbox::{Listbox, ListboxSelectionMode};
pub use menu::{
    ContextMenu, Menu, MenuItem, Menubar, MenubarActivation, MenubarMenu, PanelMenu, TieredMenu,
};
pub use message::{Message, MessageVariant};
pub use metric_card::MetricCard;
pub use multi_select::MultiSelect;
pub use paginator::Paginator;
pub use panel::Panel;
pub use pick_list::PickList;
pub use popover::Popover;
pub use progress::Progress;
pub use property_list::{PropertyItem, PropertyList};
pub use radio::Radio;
pub use scroll_area::ScrollArea;
pub use select::{Select, SelectItem};
pub use separator::{Separator, SeparatorAxis};
pub use size::ComponentSize;
pub use slider::Slider;
pub use spinner::Spinner;
pub use splitter::{Splitter, SplitterAxis};
pub use stepper::{Step, Stepper};
pub use switch::Switch;
pub use tab_menu::TabMenu;
pub use tabs::{TabItem, Tabs};
pub use tag::{Tag, TagVariant};
pub use text_input::{PasswordInput, SearchInput, TextArea, TextInput, TextInputKind};
pub use timeline::{Timeline, TimelineEvent};
pub use toast::{Toast, ToastPlacement, ToastStack, ToastVariant};
pub use toolbar::Toolbar;
pub use tooltip::Tooltip;
pub use tree_select::{TreeSelect, TreeSelectNode};
pub use tree_table::{TreeTable, TreeTableColumn, TreeTableRow};
pub use virtual_list::{VirtualList, VirtualListMetrics};

#[cfg(feature = "data-table")]
pub use data_table::{
    DataColumn, DataColumnAlign, DataColumnPin, DataColumnResize, DataRow, DataTable,
    DataTableCell, DataTableColumnViewport, DataTableNavigation, DataTableNavigationOutcome,
    DataTableSelection, DataTableSelectionIntent, DataTableSelectionMode, DataTableState,
    DataTableViewport, SortDirection, TableSort, VisibleDataColumn, VisibleDataRow,
};
#[cfg(feature = "dock")]
pub use dock::{
    Dock, DockAxis, DockCommand, DockDragPayload, DockDropTarget, DockDropZone, DockLayout,
    DockNode, DockPlacement, DockSplitResize, DockStackSelection, DockTab, DockTabSelection,
    DockTabs,
};
#[cfg(feature = "markdown")]
pub use markdown::{HtmlFragment, Markdown};
#[cfg(feature = "tree")]
pub use tree::{
    TreeMutation, TreeMutationError, TreeNavigation, TreeNavigationOutcome, TreeNode,
    TreeSelection, TreeSelectionIntent, TreeSelectionMode, TreeView, TreeViewport, VisibleTreeNode,
};

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type BoolHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;
type IndexHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;
type TextChangeHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// Initializes component-level registrations.
pub fn init(cx: &mut gpui::App) {
    text_input::init_key_bindings(cx);
}

#[cfg(test)]
mod catalog_keyboard_interaction_tests {
    use super::{DatePicker, Listbox, MultiSelect, SelectItem, TabItem, TabMenu, Tabs};
    use gpui::{
        AppContext as _, Context, IntoElement as _, Keystroke, ParentElement as _, Render,
        SharedString, Styled as _, TestAppContext, VisualContext as _, Window, div,
    };

    #[derive(Clone, Copy)]
    enum Surface {
        Tabs,
        TabMenu,
        DatePicker,
        Listbox,
        MultiSelect,
    }

    struct Harness {
        surface: Surface,
        selected: usize,
        date: SharedString,
        open: bool,
        multi_selected: Vec<usize>,
    }

    impl Harness {
        fn new(surface: Surface) -> Self {
            Self {
                surface,
                selected: 0,
                date: "2026-08-01".into(),
                open: true,
                multi_selected: Vec::new(),
            }
        }
    }

    impl Render for Harness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            let items = || {
                vec![
                    SelectItem::new("a", "Alpha"),
                    SelectItem::new("b", "Beta"),
                    SelectItem::new("c", "Charlie"),
                ]
            };
            let tabs = || {
                vec![
                    TabItem::new("a", "Alpha"),
                    TabItem::new("b", "Beta"),
                    TabItem::new("c", "Charlie"),
                ]
            };
            let child = match self.surface {
                Surface::Tabs => Tabs::new("keyboard-tabs")
                    .items(tabs())
                    .selected(self.selected)
                    .on_select(cx.listener(|this, selected, _, cx| {
                        this.selected = *selected;
                        cx.notify();
                    }))
                    .into_any_element(),
                Surface::TabMenu => TabMenu::new("keyboard-tab-menu")
                    .items(tabs())
                    .selected(self.selected)
                    .on_select(cx.listener(|this, selected, _, cx| {
                        this.selected = *selected;
                        cx.notify();
                    }))
                    .into_any_element(),
                Surface::DatePicker => DatePicker::new("keyboard-date")
                    .value(self.date.clone())
                    .open(self.open)
                    .on_open_change(cx.listener(|this, open, _, cx| {
                        this.open = *open;
                        cx.notify();
                    }))
                    .on_change(cx.listener(|this, date: &SharedString, _, cx| {
                        this.date = date.clone();
                        cx.notify();
                    }))
                    .into_any_element(),
                Surface::Listbox => Listbox::new("keyboard-listbox")
                    .items(items())
                    .selected(vec![self.selected])
                    .on_selection_change(cx.listener(|this, selected: &Vec<usize>, _, cx| {
                        this.selected = selected[0];
                        cx.notify();
                    }))
                    .into_any_element(),
                Surface::MultiSelect => MultiSelect::new("keyboard-multi-select")
                    .items(items())
                    .selected(self.multi_selected.clone())
                    .expanded(true)
                    .on_toggle(|_, _, _| {})
                    .on_select(cx.listener(|this, selected: &usize, _, cx| {
                        if let Some(position) = this
                            .multi_selected
                            .iter()
                            .position(|index| index == selected)
                        {
                            this.multi_selected.remove(position);
                        } else {
                            this.multi_selected.push(*selected);
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            };
            div().size_full().p_4().child(child).into_any_element()
        }
    }

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
    }

    fn focus_next(cx: &mut gpui::VisualTestContext, count: usize) {
        let window = cx.window_handle();
        cx.update_window(window, |_, window, cx| {
            for _ in 0..count {
                window.focus_next(cx);
            }
        })
        .expect("window update should succeed");
    }

    #[gpui::test]
    fn tabs_and_tab_menu_support_directional_keyboard_selection(cx: &mut TestAppContext) {
        init(cx);
        let (tabs, cx) = cx.add_window_view(|_, _| Harness::new(Surface::Tabs));
        focus_next(cx, 1);
        let window = cx.window_handle();
        cx.dispatch_keystroke(window, Keystroke::parse("right").expect("key parses"));
        tabs.update(cx, |view, _| assert_eq!(view.selected, 1));

        let (menu, cx) = cx.add_window_view(|_, _| Harness::new(Surface::TabMenu));
        focus_next(cx, 1);
        let window = cx.window_handle();
        cx.dispatch_keystroke(window, Keystroke::parse("right").expect("key parses"));
        cx.dispatch_keystroke(window, Keystroke::parse("space").expect("key parses"));
        menu.update(cx, |view, _| assert_eq!(view.selected, 1));
    }

    #[gpui::test]
    fn date_picker_calendar_supports_grid_keyboard_navigation(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, _| Harness::new(Surface::DatePicker));
        focus_next(cx, 2);
        let window = cx.window_handle();
        cx.dispatch_keystroke(window, Keystroke::parse("right").expect("key parses"));
        cx.dispatch_keystroke(window, Keystroke::parse("enter").expect("key parses"));
        view.update(cx, |view, _| {
            assert_eq!(view.date.as_ref(), "2026-08-02");
            assert!(!view.open);
        });
    }

    #[gpui::test]
    fn listbox_and_multi_select_support_roving_keyboard_activation(cx: &mut TestAppContext) {
        init(cx);
        let (listbox, cx) = cx.add_window_view(|_, _| Harness::new(Surface::Listbox));
        focus_next(cx, 1);
        let window = cx.window_handle();
        cx.dispatch_keystroke(window, Keystroke::parse("down").expect("key parses"));
        cx.dispatch_keystroke(window, Keystroke::parse("space").expect("key parses"));
        listbox.update(cx, |view, _| assert_eq!(view.selected, 1));

        let (multi, cx) = cx.add_window_view(|_, _| Harness::new(Surface::MultiSelect));
        focus_next(cx, 2);
        let window = cx.window_handle();
        cx.dispatch_keystroke(window, Keystroke::parse("down").expect("key parses"));
        cx.dispatch_keystroke(window, Keystroke::parse("space").expect("key parses"));
        multi.update(cx, |view, _| assert_eq!(view.multi_selected, vec![1]));
    }
}

#[cfg(test)]
mod multi_select_interaction_tests {
    use super::{MultiSelect, SelectItem};
    use gpui::{
        Context, Modifiers, ParentElement as _, Render, Styled as _, TestAppContext, Window, div,
    };

    struct MultiSelectHarness {
        expanded: bool,
        selected: Vec<usize>,
        toggled_to: Option<bool>,
    }

    impl Render for MultiSelectHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div().size_full().p_4().child(
                MultiSelect::new("labels")
                    .items(vec![
                        SelectItem::new("bug", "Bug"),
                        SelectItem::new("docs", "Docs"),
                        SelectItem::new("perf", "Perf"),
                    ])
                    .selected(self.selected.clone())
                    .expanded(self.expanded)
                    .on_toggle(cx.listener(|this, expanded, _, cx| {
                        this.toggled_to = Some(*expanded);
                        this.expanded = *expanded;
                        cx.notify();
                    }))
                    .on_select(cx.listener(|this, index: &usize, _, cx| {
                        if let Some(pos) = this.selected.iter().position(|i| i == index) {
                            this.selected.remove(pos);
                        } else {
                            this.selected.push(*index);
                        }
                        cx.notify();
                    })),
            )
        }
    }

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
    }

    #[gpui::test]
    fn trigger_click_toggles_expansion(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, _| MultiSelectHarness {
            expanded: false,
            selected: vec![0],
            toggled_to: None,
        });

        let trigger = cx
            .debug_bounds("guic-multi-select-trigger-labels")
            .expect("trigger should be present");
        cx.simulate_click(trigger.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.toggled_to, Some(true));
            assert!(view.expanded);
        });
    }

    #[gpui::test]
    fn row_click_toggles_membership(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, _| MultiSelectHarness {
            expanded: true,
            selected: vec![0],
            toggled_to: None,
        });

        // Add "perf" (index 2).
        let perf = cx
            .debug_bounds("guic-multi-select-item-2")
            .expect("perf row should be present");
        cx.simulate_click(perf.center(), Modifiers::none());
        view.update(cx, |view, _| assert_eq!(view.selected, vec![0, 2]));

        // Toggle "Bug" (index 0) back off.
        let bug = cx
            .debug_bounds("guic-multi-select-item-0")
            .expect("bug row should be present");
        cx.simulate_click(bug.center(), Modifiers::none());
        view.update(cx, |view, _| assert_eq!(view.selected, vec![2]));
    }
}

#[cfg(test)]
mod overlay_interaction_tests {
    use super::{ConfirmDialog, Drawer, DrawerSide, Label, Toast, ToastStack};
    use gpui::{
        Context, Modifiers, ParentElement as _, Render, Styled as _, TestAppContext, Window, div,
    };

    struct DrawerHarness {
        open: bool,
        closed: bool,
    }

    impl Render for DrawerHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div().size_full().child(
                Drawer::new("test-drawer")
                    .open(self.open)
                    .side(DrawerSide::Right)
                    .title("Details")
                    .on_close(cx.listener(|this, _, _, cx| {
                        this.closed = true;
                        this.open = false;
                        cx.notify();
                    }))
                    .child(Label::new("Body")),
            )
        }
    }

    struct ConfirmHarness {
        open: bool,
        confirmed: bool,
        cancelled: bool,
    }

    impl Render for ConfirmHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div().size_full().child(
                ConfirmDialog::new("test-confirm")
                    .open(self.open)
                    .title("Delete project?")
                    .message("This cannot be undone.")
                    .confirm_label("Delete")
                    .cancel_label("Keep")
                    .danger(true)
                    .on_confirm(cx.listener(|this, _, _, cx| {
                        this.confirmed = true;
                        this.open = false;
                        cx.notify();
                    }))
                    .on_cancel(cx.listener(|this, _, _, cx| {
                        this.cancelled = true;
                        this.open = false;
                        cx.notify();
                    })),
            )
        }
    }

    struct ToastHarness {
        present: bool,
        closed: bool,
    }

    impl Render for ToastHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            let toasts = if self.present {
                vec![
                    Toast::new("saved", "Saved").on_close(cx.listener(|this, _, _, cx| {
                        this.closed = true;
                        this.present = false;
                        cx.notify();
                    })),
                ]
            } else {
                Vec::new()
            };
            div()
                .size_full()
                .child(ToastStack::new("test-toasts").toasts(toasts))
        }
    }

    #[gpui::test]
    fn drawer_scrim_click_closes(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, _| DrawerHarness {
            open: true,
            closed: false,
        });

        let scrim = cx
            .debug_bounds("guic-drawer-scrim")
            .expect("drawer scrim should be present while open");
        cx.simulate_click(scrim.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert!(view.closed);
            assert!(!view.open);
        });
    }

    #[gpui::test]
    fn confirm_dialog_confirm_and_cancel(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, _| ConfirmHarness {
            open: true,
            confirmed: false,
            cancelled: false,
        });

        let confirm = cx
            .debug_bounds("guic-button-Delete")
            .expect("confirm button should be present");
        cx.simulate_click(confirm.center(), Modifiers::none());
        view.update(cx, |view, _| {
            assert!(view.confirmed);
            assert!(!view.open);
        });

        // Reopen and exercise the cancel path.
        view.update(cx, |view, cx| {
            view.open = true;
            view.confirmed = false;
            cx.notify();
        });
        let cancel = cx
            .debug_bounds("guic-button-Keep")
            .expect("cancel button should be present");
        cx.simulate_click(cancel.center(), Modifiers::none());
        view.update(cx, |view, _| {
            assert!(view.cancelled);
            assert!(!view.open);
        });
    }

    #[gpui::test]
    fn toast_close_button_dismisses(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, _| ToastHarness {
            present: true,
            closed: false,
        });

        let close = cx
            .debug_bounds("guic-icon-button-X")
            .expect("toast close button should be present");
        cx.simulate_click(close.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert!(view.closed);
            assert!(!view.present);
        });
    }
}

#[cfg(test)]
mod catalog_interaction_tests {
    use super::{Accordion, AccordionSection, Breadcrumb, BreadcrumbItem, Button, Label, Panel};
    use gpui::{
        Context, Keystroke, Modifiers, ParentElement as _, Render, Styled as _, TestAppContext,
        VisualContext as _, Window, div,
    };

    struct CatalogHarness {
        panel_collapsed: bool,
        breadcrumb_selected: Option<usize>,
        sections_open: [bool; 2],
        panel_action_clicks: usize,
    }

    impl CatalogHarness {
        fn new() -> Self {
            Self {
                panel_collapsed: false,
                breadcrumb_selected: None,
                sections_open: [true, false],
                panel_action_clicks: 0,
            }
        }
    }

    impl Render for CatalogHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div()
                .size_full()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    Panel::new("catalog-panel", "Filters")
                        .collapsed(self.panel_collapsed)
                        .actions(Button::new("Panel Action").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.panel_action_clicks += 1;
                                cx.notify();
                            },
                        )))
                        .on_toggle(cx.listener(|this, collapsed, _, cx| {
                            this.panel_collapsed = *collapsed;
                            cx.notify();
                        }))
                        .child(Label::new("Body")),
                )
                .child(
                    Breadcrumb::new("catalog-breadcrumb")
                        .items(vec![
                            BreadcrumbItem::new("home", "Home"),
                            BreadcrumbItem::new("settings", "Settings"),
                            BreadcrumbItem::new("profile", "Profile"),
                        ])
                        .on_select(cx.listener(|this, index, _, cx| {
                            this.breadcrumb_selected = Some(*index);
                            cx.notify();
                        })),
                )
                .child(
                    Accordion::new("catalog-accordion")
                        .section(
                            AccordionSection::new("General", Label::new("General body"))
                                .expanded(self.sections_open[0]),
                        )
                        .section(
                            AccordionSection::new("Advanced", Label::new("Advanced body"))
                                .expanded(self.sections_open[1]),
                        )
                        .on_toggle(cx.listener(|this, index: &usize, _, cx| {
                            this.sections_open[*index] = !this.sections_open[*index];
                            cx.notify();
                        })),
                )
        }
    }

    #[gpui::test]
    fn panel_header_click_toggles_collapsed_state(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, _| CatalogHarness::new());

        let header = cx
            .debug_bounds("guic-panel-header-catalog-panel")
            .expect("panel header should be present");
        cx.simulate_click(header.center(), Modifiers::none());

        view.update(cx, |view, _| assert!(view.panel_collapsed));
    }

    #[gpui::test]
    fn panel_header_actions_do_not_toggle_the_panel(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, _| CatalogHarness::new());

        let action = cx
            .debug_bounds("guic-button-Panel Action")
            .expect("panel action should be present");
        cx.simulate_click(action.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.panel_action_clicks, 1);
            assert!(!view.panel_collapsed);
        });
    }

    #[gpui::test]
    fn catalog_disclosures_and_links_activate_from_the_keyboard(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, _| CatalogHarness::new());
        let window = cx.window_handle();

        let panel = cx
            .debug_bounds("guic-panel-header-catalog-panel")
            .expect("panel header should be present");
        cx.simulate_click(panel.center(), Modifiers::none());
        cx.dispatch_keystroke(window, Keystroke::parse("enter").expect("keystroke parses"));

        let crumb = cx
            .debug_bounds("guic-breadcrumb-settings")
            .expect("settings crumb should be present");
        cx.simulate_click(crumb.center(), Modifiers::none());
        cx.dispatch_keystroke(window, Keystroke::parse("space").expect("keystroke parses"));

        let accordion = cx
            .debug_bounds("guic-accordion-catalog-accordion-section-1")
            .expect("accordion header should be present");
        cx.simulate_click(accordion.center(), Modifiers::none());
        cx.dispatch_keystroke(window, Keystroke::parse("enter").expect("keystroke parses"));

        view.update(cx, |view, _| {
            assert!(!view.panel_collapsed);
            assert_eq!(view.breadcrumb_selected, Some(1));
            assert!(!view.sections_open[1]);
        });
    }

    #[gpui::test]
    fn breadcrumb_click_reports_index(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, _| CatalogHarness::new());

        let crumb = cx
            .debug_bounds("guic-breadcrumb-settings")
            .expect("settings crumb should be present");
        cx.simulate_click(crumb.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.breadcrumb_selected, Some(1));
        });
    }

    #[gpui::test]
    fn accordion_header_click_reports_index(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, _| CatalogHarness::new());

        let header = cx
            .debug_bounds("guic-accordion-catalog-accordion-section-1")
            .expect("second accordion header should be present");
        cx.simulate_click(header.center(), Modifiers::none());

        view.update(cx, |view, _| assert!(view.sections_open[1]));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::{
        Button, Checkbox, ComponentSize, Dialog, Label, Popover, Select, SelectItem, TextInput,
    };
    use gpui::{
        AppContext as _, Context, EntityInputHandler as _, FocusHandle, InteractiveElement as _,
        KeyUpEvent, Keystroke, Modifiers, ParentElement as _, Render, Styled as _, TestAppContext,
        VisualContext as _, Window, div,
    };

    struct InteractionHarness {
        clicks: usize,
        checked: bool,
        select_expanded: bool,
        selected_option: Option<usize>,
        select_focus: FocusHandle,
        dialog_open: bool,
        popover_open: bool,
        text_input: gpui::Entity<TextInput>,
        search_input: gpui::Entity<TextInput>,
        password_input: gpui::Entity<TextInput>,
        text_area: gpui::Entity<TextInput>,
    }

    struct KeyboardButtonHarness {
        clicks: usize,
        focus_handle: FocusHandle,
    }

    impl KeyboardButtonHarness {
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                clicks: 0,
                focus_handle: cx.focus_handle(),
            }
        }
    }

    impl Render for KeyboardButtonHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            Button::new("Keyboard Action")
                .focusable(self.focus_handle.clone())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.clicks += 1;
                    cx.notify();
                }))
        }
    }

    impl InteractionHarness {
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                clicks: 0,
                checked: false,
                select_expanded: false,
                selected_option: Some(0),
                select_focus: cx.focus_handle(),
                dialog_open: false,
                popover_open: false,
                text_input: cx.new(|cx| {
                    TextInput::new("integration-text-input", cx).placeholder("Type here")
                }),
                search_input: cx.new(|cx| {
                    TextInput::search("integration-search-input", cx)
                        .placeholder("Search components")
                }),
                password_input: cx.new(|cx| {
                    TextInput::password("integration-password-input", cx)
                        .placeholder("Enter password")
                }),
                text_area: cx.new(|cx| {
                    TextInput::text_area("integration-text-area", cx).placeholder("Write notes")
                }),
            }
        }
    }

    impl Render for InteractionHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div()
                .size_full()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    Button::new("Integration Action")
                        .size(ComponentSize::Small)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.clicks += 1;
                            cx.notify();
                        })),
                )
                .child(
                    Checkbox::new("integration-checkbox")
                        .label("Enable")
                        .checked(self.checked)
                        .on_toggle(cx.listener(|this, checked, _, cx| {
                            this.checked = *checked;
                            cx.notify();
                        })),
                )
                .child(
                    Select::new("integration-select")
                        .size(ComponentSize::Small)
                        .focusable(self.select_focus.clone())
                        .selected(self.selected_option)
                        .expanded(self.select_expanded)
                        .items(vec![
                            SelectItem::new("alpha", "Alpha"),
                            SelectItem::new("beta", "Beta"),
                        ])
                        .on_toggle(cx.listener(|this, expanded, _, cx| {
                            this.select_expanded = *expanded;
                            cx.notify();
                        }))
                        .on_select(cx.listener(|this, selected, _, cx| {
                            this.selected_option = Some(*selected);
                            this.select_expanded = false;
                            cx.notify();
                        })),
                )
                .child(self.text_input.clone())
                .child(self.search_input.clone())
                .child(self.password_input.clone())
                .child(self.text_area.clone())
                .child(
                    Button::new("Open Dialog")
                        .size(ComponentSize::Small)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.dialog_open = true;
                            cx.notify();
                        })),
                )
                .child(
                    Popover::new(
                        "integration-popover",
                        Button::new("Toggle Popover")
                            .size(ComponentSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.popover_open = !this.popover_open;
                                cx.notify();
                            })),
                        Label::new("Popover body"),
                    )
                    .open(self.popover_open),
                )
                .child(
                    Dialog::new("integration-dialog")
                        .open(self.dialog_open)
                        .title("Dialog")
                        .description("Dismiss through the scrim")
                        .secondary_label("Close")
                        .on_cancel(cx.listener(|this, _, _, cx| {
                            this.dialog_open = false;
                            cx.notify();
                        })),
                )
        }
    }

    struct DeferredPopoverHarness {
        open: bool,
    }

    impl DeferredPopoverHarness {
        fn new() -> Self {
            Self { open: false }
        }
    }

    impl Render for DeferredPopoverHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div().size_full().p_4().child(
                div()
                    .id("popover-clip-container")
                    .debug_selector(|| "popover-clip-container".to_owned())
                    .w(gpui::px(120.0))
                    .h(gpui::px(32.0))
                    .overflow_hidden()
                    .child(
                        Popover::new(
                            "deferred-popover",
                            Button::new("Open Nested Popover")
                                .size(ComponentSize::Small)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.open = !this.open;
                                    cx.notify();
                                })),
                            Label::new("Escapes clipping"),
                        )
                        .open(self.open)
                        .width(220.0),
                    ),
            )
        }
    }

    #[gpui::test]
    fn button_and_checkbox_handle_click_interactions(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| InteractionHarness::new(cx));

        let button_bounds = cx
            .debug_bounds("guic-button-Integration Action")
            .expect("button bounds should be present");
        cx.simulate_click(button_bounds.center(), Modifiers::none());

        let checkbox_bounds = cx
            .debug_bounds("guic-checkbox-integration-checkbox")
            .expect("checkbox bounds should be present");
        cx.simulate_click(checkbox_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.clicks, 1);
            assert!(view.checked);
        });
    }

    #[gpui::test]
    fn button_handles_keyboard_activation(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| KeyboardButtonHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.focus_handle.focus(window, cx));
        })
        .expect("window update should succeed");
        let enter = Keystroke::parse("enter").expect("keystroke should parse");
        cx.dispatch_keystroke(window, enter.clone());
        cx.simulate_event(KeyUpEvent { keystroke: enter });

        view.update(cx, |view, _| assert_eq!(view.clicks, 1));
    }

    #[gpui::test]
    fn select_toggle_and_selection_update_state(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| InteractionHarness::new(cx));

        let trigger_bounds = cx
            .debug_bounds("guic-select-trigger-integration-select")
            .expect("select trigger bounds should be present");
        cx.simulate_click(trigger_bounds.center(), Modifiers::none());

        let item_bounds = cx
            .debug_bounds("guic-select-item-1")
            .expect("expanded select item should be present");
        cx.simulate_click(item_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.selected_option, Some(1));
            assert!(!view.select_expanded);
        });
    }

    #[gpui::test]
    fn select_handles_keyboard_navigation_and_dismissal(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| InteractionHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.select_focus.focus(window, cx));
        })
        .expect("window update should succeed");

        cx.dispatch_keystroke(
            window,
            Keystroke::parse("down").expect("keystroke should parse"),
        );
        view.update(cx, |view, _| {
            assert_eq!(view.selected_option, Some(1));
            assert!(!view.select_expanded);
        });

        cx.dispatch_keystroke(
            window,
            Keystroke::parse("enter").expect("keystroke should parse"),
        );
        view.update(cx, |view, _| assert!(view.select_expanded));

        cx.dispatch_keystroke(
            window,
            Keystroke::parse("escape").expect("keystroke should parse"),
        );
        view.update(cx, |view, _| assert!(!view.select_expanded));
    }

    #[gpui::test]
    fn text_input_accepts_keyboard_input(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| InteractionHarness::new(cx));

        let input_bounds = cx
            .debug_bounds("guic-text-input-integration-text-input")
            .expect("text input bounds should be present");
        cx.simulate_click(input_bounds.center(), Modifiers::none());
        let window = cx.window_handle();
        cx.dispatch_keystroke(
            window,
            Keystroke::parse("a").expect("keystroke should parse"),
        );

        view.update(cx, |view, cx| {
            let current = view.text_input.read(cx).current_value().to_owned();
            assert_eq!(current, "a");
        });
    }

    #[gpui::test]
    fn search_and_password_inputs_accept_keyboard_input(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| InteractionHarness::new(cx));

        let search_bounds = cx
            .debug_bounds("guic-text-input-integration-search-input")
            .expect("search input bounds should be present");
        cx.simulate_click(search_bounds.center(), Modifiers::none());
        let window = cx.window_handle();
        cx.dispatch_keystroke(
            window,
            Keystroke::parse("s").expect("keystroke should parse"),
        );

        let password_bounds = cx
            .debug_bounds("guic-text-input-integration-password-input")
            .expect("password input bounds should be present");
        cx.simulate_click(password_bounds.center(), Modifiers::none());
        cx.dispatch_keystroke(
            window,
            Keystroke::parse("p").expect("keystroke should parse"),
        );

        view.update(cx, |view, cx| {
            assert_eq!(view.search_input.read(cx).current_value(), "s");
            assert_eq!(view.password_input.read(cx).current_value(), "p");
        });
    }

    #[gpui::test]
    fn text_area_preserves_newlines(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| InteractionHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.text_area.update(cx, |input, cx| {
                    input.replace_text_in_range(None, "line 1\nline 2", window, cx);
                });
            });
        })
        .expect("window update should succeed");

        view.update(cx, |view, cx| {
            assert_eq!(view.text_area.read(cx).current_value(), "line 1\nline 2");
        });
    }

    #[gpui::test]
    fn dialog_and_popover_toggle_visibility(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| InteractionHarness::new(cx));

        let popover_button_bounds = cx
            .debug_bounds("guic-button-Toggle Popover")
            .expect("popover toggle button should exist");
        cx.simulate_click(popover_button_bounds.center(), Modifiers::none());

        let dialog_button_bounds = cx
            .debug_bounds("guic-button-Open Dialog")
            .expect("dialog button should exist");
        cx.simulate_click(dialog_button_bounds.center(), Modifiers::none());

        let scrim_bounds = cx
            .debug_bounds("guic-dialog-scrim")
            .expect("dialog scrim should exist after opening");
        cx.simulate_click(scrim_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert!(view.popover_open);
            assert!(!view.dialog_open);
        });
    }

    #[gpui::test]
    fn dialog_secondary_action_closes_surface(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| InteractionHarness::new(cx));

        let dialog_button_bounds = cx
            .debug_bounds("guic-button-Open Dialog")
            .expect("dialog button should exist");
        cx.simulate_click(dialog_button_bounds.center(), Modifiers::none());

        let close_button_bounds = cx
            .debug_bounds("guic-button-Close")
            .expect("dialog close button should exist");
        cx.simulate_click(close_button_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert!(!view.dialog_open);
        });
    }

    #[gpui::test]
    fn popover_panel_does_not_expand_clipping_container_layout(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
        let (_view, cx) = cx.add_window_view(|_, _| DeferredPopoverHarness::new());

        let container_before = cx
            .debug_bounds("popover-clip-container")
            .expect("clip container should exist before opening");

        let trigger_bounds = cx
            .debug_bounds("guic-button-Open Nested Popover")
            .expect("nested popover trigger should exist");
        cx.simulate_click(trigger_bounds.center(), Modifiers::none());

        let container_after = cx
            .debug_bounds("popover-clip-container")
            .expect("clip container should still exist after opening");

        assert!(
            container_after.size.height == container_before.size.height,
            "opening the popover should not expand the clipping container layout"
        );
    }
}

#[cfg(test)]
mod binary_input_interaction_tests {
    use super::{Checkbox, Radio, Switch};
    use gpui::{
        AppContext as _, Context, FocusHandle, IntoElement as _, Keystroke, ParentElement as _,
        Render, Styled as _, TestAppContext, VisualContext as _, Window, div,
    };

    struct BinaryInputsHarness {
        checkbox_checked: bool,
        switch_checked: bool,
        radio_selected: bool,
        checkbox_focus: FocusHandle,
        switch_focus: FocusHandle,
        radio_focus: FocusHandle,
    }

    impl BinaryInputsHarness {
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                checkbox_checked: false,
                switch_checked: false,
                radio_selected: false,
                checkbox_focus: cx.focus_handle(),
                switch_focus: cx.focus_handle(),
                radio_focus: cx.focus_handle(),
            }
        }
    }

    impl Render for BinaryInputsHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div().size_full().p_4().children([
                Checkbox::new("keyboard-checkbox")
                    .checked(self.checkbox_checked)
                    .focusable(self.checkbox_focus.clone())
                    .on_toggle(cx.listener(|this, checked: &bool, _, cx| {
                        this.checkbox_checked = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                Switch::new("keyboard-switch")
                    .checked(self.switch_checked)
                    .focusable(self.switch_focus.clone())
                    .on_toggle(cx.listener(|this, checked: &bool, _, cx| {
                        this.switch_checked = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                Radio::new("keyboard-radio")
                    .checked(self.radio_selected)
                    .focusable(self.radio_focus.clone())
                    .on_select(cx.listener(|this, selected: &bool, _, cx| {
                        this.radio_selected = *selected;
                        cx.notify();
                    }))
                    .into_any_element(),
            ])
        }
    }

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            super::init(cx);
        });
    }

    #[gpui::test]
    fn keyboard_activates_checkbox_switch_and_radio(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, cx| BinaryInputsHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.checkbox_focus.focus(window, cx));
        })
        .expect("window update should succeed");
        cx.dispatch_keystroke(window, Keystroke::parse("space").expect("keystroke parses"));
        view.update(cx, |view, _| assert!(view.checkbox_checked));

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.switch_focus.focus(window, cx));
        })
        .expect("window update should succeed");
        cx.dispatch_keystroke(window, Keystroke::parse("enter").expect("keystroke parses"));
        view.update(cx, |view, _| assert!(view.switch_checked));

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.radio_focus.focus(window, cx));
        })
        .expect("window update should succeed");
        cx.dispatch_keystroke(window, Keystroke::parse("space").expect("keystroke parses"));
        view.update(cx, |view, _| assert!(view.radio_selected));
    }
}
