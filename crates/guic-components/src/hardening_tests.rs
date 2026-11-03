use crate::{
    Button, ComponentSize, ContextMenu, IconButton, InputNumber, Select, SelectItem, TextInput,
    Tooltip,
};
use gpui::VisualContext as _;
use gpui::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, Modifiers, MouseButton,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, TestAppContext,
    Window, div, px,
};
use guic_icons::IconName;

struct Harness {
    input: Entity<TextInput>,
    size: ComponentSize,
    open: bool,
    clicks: usize,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .debug_selector(|| "toolbar".into())
                    .flex()
                    .items_center()
                    .child(
                        Tooltip::new(
                            Button::new("Run")
                                .id("run")
                                .size(self.size)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.clicks += 1;
                                    cx.notify();
                                })),
                            "Run action",
                        )
                        .id("run-tip"),
                    )
                    .child(self.input.clone())
                    .child(
                        Select::new("choice")
                            .size(self.size)
                            .width(px(120.))
                            .expanded(self.open)
                            .items(vec![SelectItem::new("one", "One")]),
                    )
                    .child(
                        IconButton::new(IconName::Search)
                            .id("search")
                            .size(self.size),
                    )
                    .child(InputNumber::new("number").size(self.size))
                    .child(
                        crate::Tabs::new("tabs")
                            .size(self.size)
                            .items(vec![crate::TabItem::new("tab", "Tab")]),
                    )
                    .child(
                        crate::TabMenu::new("tabmenu")
                            .size(self.size)
                            .items(vec![crate::TabItem::new("tab", "Tab")]),
                    )
                    .child(crate::MultiSelect::new("multi").size(self.size)),
            )
            .child(
                div().w(px(300.)).h(px(200.)).child(
                    ContextMenu::new(
                        "fill",
                        div().size_full().debug_selector(|| "fill-child".into()),
                    )
                    .open(self.open),
                ),
            )
            .child(
                div().w(px(300.)).h(px(200.)).flex().child(ContextMenu::new(
                    "flex",
                    div()
                        .flex_1()
                        .h_full()
                        .debug_selector(|| "flex-child".into()),
                )),
            )
    }
}

#[gpui::test]
fn hardening_control_bounds_and_wrappers(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    for (size, height) in [
        (ComponentSize::Small, 28.),
        (ComponentSize::Medium, 34.),
        (ComponentSize::Large, 42.),
    ] {
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            input: cx.new(|cx| TextInput::new("input", cx).size(size)),
            size,
            open: false,
            clicks: 0,
        });
        for selector in [
            "guic-button-run",
            "guic-text-input-input",
            "guic-select-trigger-choice",
            "guic-icon-button-search",
            "guic-input-number-number",
            "guic-tabs-tabs",
            "guic-tab_menu-tabmenu",
            "guic-multi-select-trigger-multi",
        ] {
            let bounds = cx.debug_bounds(selector).expect(selector);
            assert_eq!(bounds.size.height, px(height), "{selector}");
        }
        for selector in ["fill-child", "flex-child"] {
            assert_eq!(
                cx.debug_bounds(selector).expect(selector).size,
                gpui::size(px(300.), px(200.))
            );
        }
        let button = cx.debug_bounds("guic-button-run").expect("button");
        cx.simulate_mouse_down(button.center(), MouseButton::Left, Modifiers::none());
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        cx.simulate_mouse_up(button.center(), MouseButton::Left, Modifiers::none());
        view.update(cx, |this, _| assert_eq!(this.clicks, 1));
        let window = cx.window_handle();
        let _ = cx.debug_bounds("guic-button-run");
        for key in ["enter", "space"] {
            cx.update_window(window, |_, window, cx| {
                let keystroke = gpui::Keystroke::parse(key).expect("key");
                window.dispatch_event(
                    gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
                        keystroke: keystroke.clone(),
                        is_held: false,
                        prefer_character_input: false,
                    }),
                    cx,
                );
                window.dispatch_event(
                    gpui::PlatformInput::KeyUp(gpui::KeyUpEvent { keystroke }),
                    cx,
                );
            })
            .expect("window");
        }
        view.update(cx, |this, _| assert_eq!(this.clicks, 3));
        let before = cx.debug_bounds("toolbar").expect("toolbar");
        view.update(cx, |this, cx| {
            this.open = true;
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds("toolbar").expect("toolbar"), before);
        assert_eq!(
            cx.debug_bounds("fill-child").expect("fill").size,
            gpui::size(px(300.), px(200.))
        );
    }
}

#[cfg(feature = "tree")]
struct TreeHarness {
    count: usize,
}
#[cfg(feature = "tree")]
impl Render for TreeHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            crate::TreeView::new("bounded-tree")
                .title("Title")
                .max_height(160.)
                .nodes(
                    (0..self.count)
                        .map(|index| crate::TreeNode::new(format!("node-{index}"), "Node"))
                        .collect(),
                ),
        )
    }
}

#[cfg(feature = "tree")]
#[gpui::test]
fn hardening_tree_outer_maximum(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    let (view, cx) = cx.add_window_view(|_, _| TreeHarness { count: 1 });
    let small = cx.debug_bounds("guic-tree-bounded-tree").expect("tree");
    assert!(small.size.height < px(160.));
    view.update(cx, |this, cx| {
        this.count = 100;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(
        cx.debug_bounds("guic-tree-bounded-tree")
            .expect("tree")
            .size
            .height,
        px(160.)
    );
}

struct FocusHarness {
    open: bool,
    input: Entity<TextInput>,
    previous: gpui::FocusHandle,
}
impl Render for FocusHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui::Focusable as _;
        div()
            .size_full()
            .child(
                Button::new("Launch")
                    .focusable(self.previous.clone())
                    .on_click(|_, _, _| {}),
            )
            .child(
                crate::Dialog::new("focus-dialog")
                    .open(self.open)
                    .autofocus_target(self.input.read(cx).focus_handle(cx))
                    .content(self.input.clone()),
            )
    }
}

#[gpui::test]
fn hardening_dialog_mount_focus_and_restore(cx: &mut TestAppContext) {
    use gpui::Focusable as _;
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    let (view, cx) = cx.add_window_view(|_, cx| FocusHarness {
        open: false,
        input: cx.new(|cx| TextInput::new("focus-input", cx)),
        previous: cx.focus_handle(),
    });
    let window = cx.window_handle();
    cx.update_window(window, |_, window, cx| {
        let previous = view.read(cx).previous.clone();
        previous.focus(window, cx);
    })
    .expect("window");
    view.update(cx, |this, cx| {
        this.open = true;
        cx.notify();
    });
    let _ = cx.debug_bounds("guic-button-Launch");
    cx.update_window(window, |_, window, cx| {
        window.simulate_next_frame(cx);
    })
    .expect("frame");
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        assert!(
            view.read(cx)
                .input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
    })
    .expect("window");
    view.update(cx, |this, cx| {
        this.open = false;
        cx.notify();
    });
    let _ = cx.debug_bounds("guic-button-Launch");
    cx.update_window(window, |_, window, cx| {
        window.simulate_next_frame(cx);
    })
    .expect("frame");
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        assert!(view.read(cx).previous.is_focused(window));
    })
    .expect("window");
}

struct SelectHarness {
    open: bool,
    selected: Option<usize>,
}
impl Render for SelectHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div().absolute().bottom_0().child(
                Select::new("bottom-select")
                    .width(px(160.))
                    .max_menu_height(px(80.))
                    .expanded(self.open)
                    .selected(self.selected)
                    .items(
                        (0..30)
                            .map(|index| {
                                SelectItem::new(format!("item-{index}"), format!("Item {index}"))
                            })
                            .collect(),
                    )
                    .on_toggle(cx.listener(|this, open, _, cx| {
                        this.open = *open;
                        cx.notify();
                    }))
                    .on_select(cx.listener(|this, selected, _, cx| {
                        this.selected = Some(*selected);
                        cx.notify();
                    })),
            ),
        )
    }
}

#[gpui::test]
fn hardening_select_flips_clamps_and_dismisses(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    let (view, cx) = cx.add_window_view(|_, _| SelectHarness {
        open: true,
        selected: None,
    });
    let trigger = cx
        .debug_bounds("guic-select-trigger-bottom-select")
        .expect("trigger");
    let menu = cx
        .debug_bounds("guic-select-menu-bottom-select")
        .expect("menu");
    assert_eq!(menu.size.width, px(160.));
    assert!(menu.size.height <= px(80.));
    assert!(menu.bottom() <= trigger.top());
    cx.simulate_click(gpui::point(px(1.), px(1.)), Modifiers::none());
    view.update(cx, |this, _| assert!(!this.open));
    assert_eq!(
        cx.debug_bounds("guic-select-trigger-bottom-select")
            .expect("trigger"),
        trigger
    );
    view.update(cx, |this, cx| {
        this.open = true;
        cx.notify();
    });
    let item = cx.debug_bounds("guic-select-item-0").expect("item");
    cx.simulate_click(item.center(), Modifiers::none());
    view.update(cx, |this, _| {
        assert_eq!(this.selected, Some(0));
        assert!(!this.open);
    });
}

#[gpui::test]
fn hardening_select_scrolls_selected_option_into_view(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    let (view, cx) = cx.add_window_view(|_, _| SelectHarness {
        open: true,
        selected: Some(25),
    });
    for (index, selector) in [
        (25, "guic-select-item-25"),
        (1, "guic-select-item-1"),
        (29, "guic-select-item-29"),
    ] {
        view.update(cx, |this, cx| {
            this.selected = Some(index);
            cx.notify();
        });
        let _ = cx.debug_bounds("guic-select-menu-bottom-select");
        cx.update_window(cx.window_handle(), |_, window, cx| {
            window.simulate_next_frame(cx)
        })
        .expect("frame");
        cx.run_until_parked();
        let menu = cx
            .debug_bounds("guic-select-menu-bottom-select")
            .expect("menu");
        let row = cx.debug_bounds(selector).expect("selected row");
        assert!(
            row.top() >= menu.top(),
            "selected row above viewport: {row:?} {menu:?}"
        );
        assert!(
            row.bottom() <= menu.bottom(),
            "selected row below viewport: {row:?} {menu:?}"
        );
    }
}

struct TrapHarness {
    first: gpui::FocusHandle,
    last: gpui::FocusHandle,
    outside: gpui::FocusHandle,
}
impl Render for TrapHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                div()
                    .tab_index(0)
                    .track_focus(&self.outside)
                    .child("Outside"),
            )
            .child(guic_core::FocusTrap::new(
                "trap",
                div()
                    .child(
                        div()
                            .tab_index(0)
                            .track_focus(&self.first)
                            .debug_selector(|| "trap-first".into())
                            .child("First"),
                    )
                    .child(div().tab_index(0).track_focus(&self.last).child("Last")),
            ))
    }
}

#[gpui::test]
fn hardening_modal_tab_wraps_both_directions(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| TrapHarness {
        first: cx.focus_handle().tab_stop(true),
        last: cx.focus_handle().tab_stop(true),
        outside: cx.focus_handle().tab_stop(true),
    });
    let _ = cx.debug_bounds("trap-first").expect("first mounted");
    let window = cx.window_handle();
    cx.update_window(window, |_, window, cx| {
        let handle = view.read(cx).first.clone();
        handle.focus(window, cx);
    })
    .expect("focus");
    for (key, last) in [
        ("tab", true),
        ("tab", false),
        ("shift-tab", true),
        ("shift-tab", false),
    ] {
        cx.simulate_keystrokes(key);
        cx.update_window(window, |_, window, cx| {
            let view = view.read(cx);
            assert!(!view.outside.is_focused(window));
            assert!(
                if last {
                    view.last.is_focused(window)
                } else {
                    view.first.is_focused(window)
                },
                "after {key}, expected last={last}, focused={:?}, first={:?}, last={:?}",
                window.focused(cx),
                view.first,
                view.last
            );
        })
        .expect("window");
    }
}

struct ExclusiveHarness {
    first: bool,
    second: bool,
}
impl Render for ExclusiveHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .child(
                Select::new("exclusive-first")
                    .width(px(120.))
                    .expanded(self.first)
                    .on_toggle(cx.listener(|this, open, _, cx| {
                        this.first = *open;
                        cx.notify();
                    })),
            )
            .child(
                Select::new("exclusive-second")
                    .width(px(120.))
                    .expanded(self.second)
                    .on_toggle(cx.listener(|this, open, _, cx| {
                        this.second = *open;
                        cx.notify();
                    })),
            )
    }
}

#[gpui::test]
fn hardening_select_exclusive_group_closes_previous_owner(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    let (view, cx) = cx.add_window_view(|_, _| ExclusiveHarness {
        first: true,
        second: false,
    });
    let _ = cx.debug_bounds("guic-select-menu-exclusive-first");
    view.update(cx, |this, cx| {
        this.second = true;
        cx.notify();
    });
    let _ = cx.debug_bounds("guic-select-menu-exclusive-second");
    cx.run_until_parked();
    view.update(cx, |this, _| {
        assert!(!this.first);
        assert!(this.second);
    });
    view.update(cx, |this, cx| {
        this.first = true;
        cx.notify();
    });
    let _ = cx.debug_bounds("guic-select-menu-exclusive-first");
    cx.run_until_parked();
    view.update(cx, |this, _| {
        assert!(this.first);
        assert!(!this.second);
    });
}

struct MenuFocusHarness {
    open: bool,
    previous: gpui::FocusHandle,
    menu: gpui::FocusHandle,
}
impl Render for MenuFocusHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let on_close = cx.listener(|this, _: &(), _, cx| {
            this.open = false;
            cx.notify();
        });
        div()
            .size_full()
            .child(
                Button::new("Menu launcher")
                    .id("menu-launcher")
                    .focusable(self.previous.clone())
                    .on_click(|_, _, _| {}),
            )
            .child(
                crate::Menu::new("lifecycle-menu")
                    .open(self.open)
                    .autofocus(true)
                    .focusable(self.menu.clone())
                    .items(vec![crate::MenuItem::new("item", "Item")])
                    .on_close(move |window, cx| on_close(&(), window, cx)),
            )
    }
}

#[gpui::test]
fn hardening_menu_mounts_focus_and_restores_after_escape(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    let (view, cx) = cx.add_window_view(|_, cx| MenuFocusHarness {
        open: false,
        previous: cx.focus_handle(),
        menu: cx.focus_handle(),
    });
    let window = cx.window_handle();
    let _ = cx.debug_bounds("guic-button-menu-launcher");
    cx.update_window(window, |_, window, cx| {
        let handle = view.read(cx).previous.clone();
        handle.focus(window, cx);
    })
    .expect("focus");
    view.update(cx, |this, cx| {
        this.open = true;
        cx.notify();
    });
    let _ = cx.debug_bounds("guic-button-menu-launcher");
    cx.update_window(window, |_, window, cx| window.simulate_next_frame(cx))
        .expect("frame");
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        assert!(view.read(cx).menu.is_focused(window))
    })
    .expect("focus");
    cx.simulate_keystrokes("escape");
    view.update(cx, |this, _| assert!(!this.open));
    let _ = cx.debug_bounds("guic-button-menu-launcher");
    cx.update_window(window, |_, window, cx| window.simulate_next_frame(cx))
        .expect("frame");
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        assert!(view.read(cx).previous.is_focused(window))
    })
    .expect("restored");
}

struct CompactSelectHarness {
    size: ComponentSize,
    empty: bool,
}
impl Render for CompactSelectHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().text_size(px(40.)).child(
            Select::new("compact-options")
                .width(px(180.))
                .size(self.size)
                .expanded(true)
                .selected(Some(2))
                .empty_message("Empty")
                .items(if self.empty {
                    vec![]
                } else {
                    vec![
                        SelectItem::new("selected", "Selected"),
                        SelectItem::new("disabled", "Disabled").disabled(true),
                        SelectItem::new(
                            "long",
                            "A long option label that wraps onto several lines",
                        ),
                    ]
                }),
        )
    }
}

#[gpui::test]
fn select_popup_typography_does_not_inherit_large_parent_text(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    for (size, font, padding, trigger) in [
        (ComponentSize::Small, 12., 4., 28.),
        (ComponentSize::Medium, 14., 12., 34.),
    ] {
        let (view, cx) = cx.add_window_view(|_, _| CompactSelectHarness { size, empty: false });
        let trigger_bounds = cx
            .debug_bounds("guic-select-trigger-compact-options")
            .expect("trigger");
        let label_bounds = cx
            .debug_bounds("guic-select-label-compact-options")
            .expect("label");
        assert!(trigger_bounds.contains(&label_bounds.origin));
        assert!(trigger_bounds.contains(&label_bounds.bottom_right()));
        let selected = cx
            .debug_bounds("guic-select-item-0")
            .expect("selected option");
        let disabled = cx
            .debug_bounds("guic-select-item-1")
            .expect("disabled option");
        let long = cx.debug_bounds("guic-select-item-2").expect("long option");
        assert_eq!(selected.size.height, px(font * 1.5 + padding * 2.));
        assert_eq!(disabled.size.height, selected.size.height);
        assert!(long.size.height > selected.size.height);
        assert_eq!(
            cx.debug_bounds("guic-select-trigger-compact-options")
                .expect("trigger")
                .size
                .height,
            px(trigger)
        );
        view.update(cx, |view, cx| {
            view.empty = true;
            cx.notify();
        });
        assert_eq!(
            cx.debug_bounds("guic-select-empty")
                .expect("empty message")
                .size
                .height,
            selected.size.height
        );
    }
}

#[cfg(feature = "tree")]
struct CompactTreeHarness {
    height: f32,
    fill: bool,
    count: usize,
    offset: Option<f32>,
    pitch: f32,
    scroll: gpui::ScrollHandle,
    title: bool,
}
#[cfg(feature = "tree")]
impl Render for CompactTreeHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut tree = crate::TreeView::new("compact-tree")
            .body_only(true)
            .row_height(self.pitch)
            .track_scroll(self.scroll.clone())
            .nodes(
                (0..self.count)
                    .map(|i| {
                        crate::TreeNode::new(
                            format!("compact-{i}"),
                            "A very long inspector label with enough text to exceed a narrow panel",
                        )
                    })
                    .collect(),
            );
        if self.title {
            tree = tree.title("Inspector");
        }
        if let Some(offset) = self.offset {
            tree = tree.viewport(crate::TreeViewport::new(offset, self.height).overscan(1));
        }
        tree = if self.fill {
            tree.fill_height()
        } else {
            tree.height(self.height)
        };
        div()
            .size_full()
            .child(div().w(px(180.)).h(px(self.height)).child(tree))
    }
}

#[cfg(feature = "tree")]
#[gpui::test]
fn tree_fixed_and_filled_panels_have_exact_row_pitch(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    for fill in [false, true] {
        for height in [80., 160., 320.] {
            let (view, cx) = cx.add_window_view(|_, _| CompactTreeHarness {
                height,
                fill,
                count: 2,
                offset: None,
                pitch: 24.,
                scroll: gpui::ScrollHandle::new(),
                title: false,
            });
            for count in [0, 2, 1000] {
                view.update(cx, |view, cx| {
                    view.count = count;
                    cx.notify();
                });
                let outer = cx.debug_bounds("guic-tree-compact-tree").expect("tree");
                let body = cx
                    .debug_bounds("guic-tree-body-compact-tree")
                    .expect("body");
                assert_eq!(outer.size.height, px(height));
                assert_eq!(body.size.height, px(height));
                if count > 0 {
                    let first = cx.debug_bounds("guic-tree-row-compact-0").expect("first");
                    let next = cx.debug_bounds("guic-tree-row-compact-1").expect("next");
                    assert_eq!(first.size.height, px(24.));
                    assert_eq!(next.top() - first.top(), px(24.));
                    assert_eq!(first.left(), body.left());
                }
            }
        }
    }
}

#[cfg(feature = "tree")]
#[gpui::test]
fn virtual_tree_spacers_match_row_offsets_at_start_middle_and_end(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    for pitch in [24., 36.] {
        let (view, cx) = cx.add_window_view(|_, _| CompactTreeHarness {
            height: 144.,
            fill: false,
            count: 1000,
            offset: Some(0.),
            pitch,
            scroll: gpui::ScrollHandle::new(),
            title: false,
        });
        for (offset, index, selector) in [
            (0., 0., "guic-tree-row-compact-0"),
            (500. * pitch, 500., "guic-tree-row-compact-500"),
            (1000. * pitch - 144., 999., "guic-tree-row-compact-999"),
        ] {
            view.update(cx, |view, cx| {
                view.offset = Some(offset);
                cx.notify();
            });
            let body = cx
                .debug_bounds("guic-tree-body-compact-tree")
                .expect("body");
            let row = cx.debug_bounds(selector).expect("rendered row");
            assert_eq!(row.top() - body.top(), px(index * pitch));
            assert_eq!(row.size.height, px(pitch));
        }
    }
}

struct ModalRegressionHarness {
    confirmation: bool,
    dismissible: bool,
    long: bool,
    open: bool,
    underlying_clicks: usize,
    cancellations: usize,
    confirmations: usize,
}
impl Render for ModalRegressionHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let message = if self.long {
            "Long dialog content. ".repeat(300)
        } else {
            "Short content".into()
        };
        let cancel = cx.listener(|this, _, _, cx| {
            this.cancellations += 1;
            this.open = false;
            cx.notify();
        });
        let confirm = cx.listener(|this, _, _, cx| {
            this.confirmations += 1;
            this.open = false;
            cx.notify();
        });
        let modal = if self.confirmation {
            crate::ConfirmDialog::new("regression-modal")
                .open(self.open)
                .dismissible(self.dismissible)
                .title("Confirmation")
                .message(message)
                .on_cancel(cancel)
                .on_confirm(confirm)
                .into_any_element()
        } else {
            crate::Dialog::new("regression-modal")
                .open(self.open)
                .dismissible(self.dismissible)
                .title("Dialog")
                .content(div().child(message))
                .secondary_label("Cancel")
                .primary_label("Confirm")
                .on_cancel(cancel)
                .on_confirm(confirm)
                .into_any_element()
        };
        div()
            .size_full()
            .child(
                div()
                    .id("background-control")
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|this, _, _, _| this.underlying_clicks += 1)),
            )
            .child(
                div()
                    .ml(px(70.))
                    .mt(px(50.))
                    .w(px(80.))
                    .h(px(60.))
                    .child(modal),
            )
    }
}

#[gpui::test]
fn nested_modals_occlude_cards_and_scrims_and_respect_dismissal(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    for confirmation in [false, true] {
        for dismissible in [false, true] {
            let (view, cx) = cx.add_window_view(|_, _| ModalRegressionHarness {
                confirmation,
                dismissible,
                long: false,
                open: true,
                underlying_clicks: 0,
                cancellations: 0,
                confirmations: 0,
            });
            cx.simulate_resize(gpui::size(px(360.), px(260.)));
            let card = cx
                .debug_bounds(if confirmation {
                    "guic-confirm-dialog-card"
                } else {
                    "guic-dialog-card"
                })
                .expect("card");
            let scrim = cx
                .debug_bounds(if confirmation {
                    "guic-confirm-dialog-scrim"
                } else {
                    "guic-dialog-scrim"
                })
                .expect("scrim");
            assert_eq!(scrim.origin, gpui::point(px(0.), px(0.)));
            assert_eq!(scrim.size, gpui::size(px(360.), px(260.)));
            assert!(card.left() >= px(16.) && card.right() <= px(344.));
            assert!(card.top() >= px(16.) && card.bottom() <= px(244.));
            cx.simulate_click(
                card.origin + gpui::point(px(10.), px(10.)),
                Modifiers::none(),
            );
            view.update(cx, |view, _| {
                assert!(view.open);
                assert_eq!(view.underlying_clicks, 0);
                assert_eq!(view.cancellations, 0);
            });
            cx.simulate_click(gpui::point(px(4.), px(4.)), Modifiers::none());
            view.update(cx, |view, _| {
                assert_eq!(view.open, !dismissible);
                assert_eq!(view.cancellations, usize::from(dismissible));
                assert_eq!(view.underlying_clicks, 0);
            });
        }
    }
}

#[gpui::test]
fn long_modals_scroll_actions_into_small_viewports(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    for confirmation in [false, true] {
        let (view, cx) = cx.add_window_view(|_, _| ModalRegressionHarness {
            confirmation,
            dismissible: true,
            long: true,
            open: true,
            underlying_clicks: 0,
            cancellations: 0,
            confirmations: 0,
        });
        cx.simulate_resize(gpui::size(px(320.), px(220.)));
        let card = cx
            .debug_bounds(if confirmation {
                "guic-confirm-dialog-card"
            } else {
                "guic-dialog-card"
            })
            .expect("card");
        assert!(card.top() >= px(16.) && card.bottom() <= px(204.));
        cx.simulate_event(gpui::ScrollWheelEvent {
            position: card.center(),
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-100000.))),
            modifiers: Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
        });
        let confirm = cx
            .debug_bounds("guic-button-Confirm")
            .expect("confirm action");
        assert!(
            card.contains(&confirm.origin) && card.contains(&confirm.bottom_right()),
            "{card:?}: {confirm:?}"
        );
        cx.simulate_click(confirm.center(), Modifiers::none());
        view.update(cx, |view, _| {
            assert_eq!(view.confirmations, 1);
            assert_eq!(view.cancellations, 0);
            assert_eq!(view.underlying_clicks, 0);
        });
    }
}

#[cfg(feature = "tree")]
#[gpui::test]
fn virtual_tree_scroll_handle_keeps_title_fixed_and_visible_rows_aligned(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
        crate::init(cx);
    });
    let scroll = gpui::ScrollHandle::new();
    let (view, cx) = cx.add_window_view(|_, _| CompactTreeHarness {
        height: 160.,
        fill: false,
        count: 1000,
        offset: Some(0.),
        pitch: 24.,
        scroll: scroll.clone(),
        title: true,
    });
    let title = cx
        .debug_bounds("guic-tree-title-compact-tree")
        .expect("title");
    for (index, selector) in [
        (0., "guic-tree-row-compact-0"),
        (500., "guic-tree-row-compact-500"),
        (995., "guic-tree-row-compact-995"),
    ] {
        let offset = index * 24.;
        scroll.set_offset(gpui::point(px(0.), px(-offset)));
        view.update(cx, |view, cx| {
            view.offset = Some(offset);
            cx.notify();
        });
        let body = cx
            .debug_bounds("guic-tree-body-compact-tree")
            .expect("body");
        let row = cx.debug_bounds(selector).expect("visible row");
        assert_eq!(
            cx.debug_bounds("guic-tree-title-compact-tree")
                .expect("title"),
            title
        );
        assert_eq!(row.top() - body.top(), px(index * 24.) + scroll.offset().y);
        assert!(row.bottom() > body.top() && row.top() < body.bottom());
    }
}
