use crate::{
    Button, ComponentSize, ContextMenu, IconButton, InputNumber, Select, SelectItem, TextInput,
    Tooltip,
};
use gpui::VisualContext as _;
use gpui::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, Modifiers, MouseButton,
    ParentElement as _, Render, Styled as _, TestAppContext, Window, div, px,
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
