use super::*;
use gpui::AppContext as _;
use gpui::{Keystroke, Modifiers, ScrollDelta, TestAppContext, TouchPhase, VisualTestContext};
use std::cell::{Cell, RefCell};

type Edits = Rc<RefCell<Vec<EditorEdit>>>;

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        guic_core::init(cx);
        guic_tokens::init(cx);
    });
}

fn surface<'a>(
    cx: &'a mut TestAppContext,
    text: &str,
    visible: usize,
    edits: Edits,
) -> (Entity<EditorSurface>, &'a mut VisualTestContext) {
    init(cx);
    cx.add_window_view(|window, cx| {
        let props = CodeEditor::new("test", EditorBuffer::from_text(text))
            .focusable(cx.focus_handle().tab_stop(true))
            .options(CodeEditorOptions::default().visible_lines(visible))
            .on_edit(move |edit, _, _| edits.borrow_mut().push(edit.clone()));
        let mut state = EditorSurface::new(props.clone());
        state.configure(props, window, cx);
        state
    })
}

fn focus(entity: &Entity<EditorSurface>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        entity
            .read(cx)
            .props
            .focus_handle
            .clone()
            .expect("focus handle")
            .focus(window, cx)
    });
}

fn key(key: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(key).expect("key"),
        is_held: false,
        prefer_character_input: false,
    }
}

#[gpui::test]
fn ime_updates_are_local_until_commit_and_undo_once(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (state, cx) = surface(cx, "🙂 prefix ", 4, edits.clone());
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.replace_and_mark_text_in_range(None, "n", Some(1..1), window, cx);
            state.replace_and_mark_text_in_range(None, "日本", Some(1..2), window, cx);
            assert_eq!(state.edit.buffer.text(), "🙂 prefix 日本");
            assert_eq!(
                state
                    .selected_text_range(false, window, cx)
                    .expect("selection")
                    .range,
                11..12
            );
            assert_eq!(state.marked_text_range(window, cx), Some(10..12));
            assert!(edits.borrow().is_empty());
            // A host redraw during composition must retain the marked text.
            state.configure(state.props.clone(), window, cx);
            assert_eq!(state.edit.buffer.text(), "🙂 prefix 日本");
            state.replace_text_in_range(None, "日本語", window, cx);
            assert_eq!(edits.borrow().len(), 1);
            assert!(state.marked.is_none());
            let mut session = EditorSession::new(EditorBuffer::from_text("🙂 prefix "));
            session.apply(edits.borrow()[0].clone());
            assert!(session.undo());
            assert_eq!(session.buffer().text(), "🙂 prefix ");
            assert!(!session.undo());
        })
    });
}

#[gpui::test]
fn cancel_unmark_and_authoritative_host_replacement(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (state, cx) = surface(cx, "original", 3, edits.clone());
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.replace_and_mark_text_in_range(None, "n", None, window, cx);
            state.replace_and_mark_text_in_range(None, "", None, window, cx);
            assert_eq!(state.edit.buffer.text(), "original");
            assert!(edits.borrow().is_empty());
            state.replace_and_mark_text_in_range(None, "日本", None, window, cx);
            state.unmark_text(window, cx);
            assert_eq!(edits.borrow().len(), 1);
            state.unmark_text(window, cx);
            assert_eq!(edits.borrow().len(), 1);
            state.replace_and_mark_text_in_range(None, "draft", None, window, cx);
            let props = CodeEditor::new("test", EditorBuffer::from_text("replacement"))
                .focusable(state.props.focus_handle.clone().expect("focus"))
                .on_edit(|_, _, _| {});
            state.configure(props, window, cx);
            assert!(state.marked.is_none());
            assert_eq!(state.edit.buffer.text(), "replacement");
            assert_eq!(edits.borrow().len(), 1);
        })
    });
}

#[gpui::test]
#[allow(
    clippy::reversed_empty_ranges,
    reason = "Exercise malformed native ranges"
)]
fn native_ranges_are_bounded_and_relative_after_newline_normalization(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "🙂", 3, Edits::default());
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.replace_and_mark_text_in_range(None, "a\r\n日本", Some(3..4), window, cx);
            assert_eq!(state.edit.buffer.text(), "🙂a\n日本");
            assert_eq!(
                state
                    .selected_text_range(false, window, cx)
                    .expect("selection")
                    .range,
                4..5
            );
            state.replace_and_mark_text_in_range(None, "🙂", Some(usize::MAX..0), window, cx);
            assert_eq!(
                state
                    .selected_text_range(false, window, cx)
                    .expect("selection")
                    .range,
                4..4
            );
            state.unmark_text(window, cx);
            let mut adjusted = None;
            assert_eq!(
                state.text_for_range(usize::MAX..0, &mut adjusted, window, cx),
                Some(String::new())
            );
            assert_eq!(adjusted, Some(4..4));
            state.replace_text_in_range(Some(usize::MAX..0), "!", window, cx);
            assert_eq!(state.edit.buffer.text(), "🙂🙂!");
        })
    });
}

#[gpui::test]
fn platform_input_inserts_once_and_keystrokes_do_not_bypass_ime(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (state, cx) = surface(cx, "", 3, edits.clone());
    focus(&state, cx);
    cx.simulate_input("日本🙂");
    state.update(cx, |state, _| {
        assert_eq!(state.edit.buffer.text(), "日本🙂")
    });
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            let mut event = key("a");
            event.keystroke.key_char = Some("a".into());
            state.key_down(&event, window, cx);
            assert_eq!(state.edit.buffer.text(), "日本🙂");
        })
    });
    assert_eq!(edits.borrow().len(), 3);
}

#[gpui::test]
fn pointer_hits_graphemes_and_drag_paints_only_selected_text(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "a日本🙂e\u{301}z", 3, Edits::default());
    let (start, end, expected_width) = state.update(cx, |state, _| {
        let row = &state.layout.as_ref().expect("layout").rows[0];
        (
            row.origin + point(row.x_for_column(1), px(10.0)),
            row.origin + point(row.x_for_column(4), px(10.0)),
            row.x_for_column(4) - row.x_for_column(1),
        )
    });
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    state.update(cx, |state, _| {
        assert_eq!(
            state.edit.selections[0],
            EditorSelection::new(EditorPosition::new(0, 1), EditorPosition::new(0, 4))
        );
        let rects = &state.layout.as_ref().expect("layout").selections;
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].size.width, expected_width);
        assert!(!state.selecting);
    });
    cx.simulate_click(start, Modifiers::none());
    let mut modifiers = Modifiers::none();
    modifiers.shift = true;
    cx.simulate_click(end, modifiers);
    state.update(cx, |state, _| {
        assert_eq!(state.edit.selections[0].head.column, 4)
    });
}

#[gpui::test]
fn tabs_use_the_same_mapping_for_painting_and_hit_testing(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "a\t日本", 3, Edits::default());
    let position = state.update(cx, |state, _| {
        let row = &state.layout.as_ref().expect("layout").rows[0];
        assert_eq!(row.line.text.as_ref(), "a   日本");
        row.origin + point(row.x_for_column(2), px(10.0))
    });
    cx.simulate_click(position, Modifiers::none());
    state.update(cx, |state, _| {
        assert_eq!(state.edit.selections[0].head.column, 2)
    });
}

#[gpui::test]
fn scrolling_virtualizes_rows_and_keyboard_reveals_caret(cx: &mut TestAppContext) {
    let text = (0..10_000)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let (state, cx) = surface(cx, &text, 4, Edits::default());
    let position = state.update(cx, |state, _| {
        let viewport = state.layout.as_ref().expect("layout");
        assert!(viewport.rows.len() <= 5);
        viewport.body.center()
    });
    cx.simulate_event(ScrollWheelEvent {
        position,
        delta: ScrollDelta::Lines(point(0.0, -15.0)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    state.update(cx, |state, _| {
        assert!(state.scroll.y > px(0.0));
        assert!(state.layout.as_ref().expect("layout").rows[0].index >= 14);
    });
    focus(&state, cx);
    cx.simulate_keystrokes("left");
    state.update(cx, |state, _| {
        let viewport = state.layout.as_ref().expect("layout");
        let caret = viewport.caret.expect("caret");
        assert!(viewport.body.contains(&caret.center()));
        assert!(viewport.rows.len() <= 5);
        assert!(viewport.rows[0].index > 9_990);
    });
}

#[gpui::test]
fn horizontal_scroll_and_ime_bounds_use_window_coordinates(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, &"wide text ".repeat(200), 3, Edits::default());
    cx.simulate_scale_factor_change(1.5);
    focus(&state, cx);
    cx.simulate_keystrokes("left");
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            assert!(state.scroll.x > px(0.0));
            let viewport = state.layout.as_ref().expect("layout").clone();
            let selection = state
                .selected_text_range(false, window, cx)
                .expect("selection");
            let rect = state
                .bounds_for_range(selection.range, viewport.bounds, window, cx)
                .expect("candidate bounds");
            assert!(viewport.body.contains(&rect.center()));
            let index = state
                .character_index_for_point(rect.center(), window, cx)
                .expect("hit index");
            assert_eq!(index, state.edit.buffer.text().encode_utf16().count() - 1);
        })
    });
}

#[gpui::test]
fn vertical_navigation_preserves_preferred_pixel_column(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "abcdef\nx\nabcdef", 4, Edits::default());
    focus(&state, cx);
    for (keys, line, column) in [
        ("up", 1, 1),
        ("up", 0, 6),
        ("down", 1, 1),
        ("shift-down", 2, 6),
    ] {
        cx.simulate_keystrokes(keys);
        state.update(cx, |state, _| {
            assert_eq!(
                state.edit.selections[0].head,
                EditorPosition::new(line, column)
            )
        });
    }
    state.update(cx, |state, _| {
        assert_eq!(state.edit.selections[0].anchor, EditorPosition::new(1, 1))
    });
}

#[gpui::test]
fn read_only_input_rejects_native_edits_but_allows_selection(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "readonly", 3, Edits::default());
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.props.on_edit = None;
            state.replace_text_in_range(None, "bad", window, cx);
            state.replace_and_mark_text_in_range(None, "bad", None, window, cx);
            assert_eq!(state.edit.buffer.text(), "readonly");
            assert!(!state.accepts_text_input(window, cx));
            state.set_selected_text_range(0..4, window, cx);
            assert_eq!(state.edit.selections[0].head.column, 4);
        })
    });
}

#[gpui::test]
fn paint_does_not_emit_notifications(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "idle", 3, Edits::default());
    let count = Rc::new(Cell::new(0));
    let observed = count.clone();
    let _subscription =
        cx.update(|_, cx| cx.observe(&state, move |_, _| observed.set(observed.get() + 1)));
    for _ in 0..3 {
        cx.draw(
            point(px(30.0), px(50.0)),
            size(px(400.0), px(100.0)),
            |_, _| state.clone().into_any_element(),
        );
    }
    assert_eq!(count.get(), 0);
}

struct ControlledHarness {
    session: EditorSession,
    focus: FocusHandle,
}
impl Render for ControlledHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(400.0)).child(
            CodeEditor::new("controlled", self.session.buffer().clone())
                .selections(self.session.selections().to_vec())
                .focusable(self.focus.clone())
                .on_edit(cx.listener(|this, edit: &EditorEdit, _, cx| {
                    this.session.apply(edit.clone());
                    cx.notify();
                }))
                .on_command(cx.listener(|this, command, _, cx| {
                    match command {
                        EditorCommand::Undo => {
                            this.session.undo();
                        }
                        EditorCommand::Redo => {
                            this.session.redo();
                        }
                    }
                    cx.notify();
                })),
        )
    }
}

#[gpui::test]
fn controlled_public_component_retains_native_handler_across_renders(cx: &mut TestAppContext) {
    init(cx);
    let (view, cx) = cx.add_window_view(|_, cx| ControlledHarness {
        session: EditorSession::new(EditorBuffer::from_text("")),
        focus: cx.focus_handle().tab_stop(true),
    });
    cx.update(|window, cx| view.read(cx).focus.clone().focus(window, cx));
    cx.simulate_input("日本");
    cx.simulate_input("🙂");
    view.update(cx, |view, _| {
        assert_eq!(view.session.buffer().text(), "日本🙂")
    });
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.simulate_keystrokes(&format!("{modifier}-z"));
    view.update(cx, |view, _| {
        assert_eq!(view.session.buffer().text(), "日本")
    });
    cx.simulate_keystrokes(&format!("{modifier}-shift-z"));
    view.update(cx, |view, _| {
        assert_eq!(view.session.buffer().text(), "日本🙂")
    });
}

#[test]
fn insertion_normalizes_line_endings_and_combining_cursor() {
    let mut session = EditorSession::new(EditorBuffer::from_text("a"));
    session.set_selections(vec![EditorSelection::cursor(EditorPosition::new(0, 1))]);
    session.insert("\u{301}");
    assert_eq!(session.selections()[0].head, EditorPosition::new(0, 1));
    session.insert("\r\nb\rc");
    assert_eq!(session.buffer().text(), "a\u{301}\nb\nc");
    assert_eq!(session.selections()[0].head, EditorPosition::new(2, 1));
}

#[gpui::test]
fn focus_loss_commits_composition_once(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (state, cx) = surface(cx, "prefix", 3, edits.clone());
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    focus(&state, cx);
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.replace_and_mark_text_in_range(None, "日本", None, window, cx);
        })
    });
    cx.update(|window, _| window.blur());
    cx.run_until_parked();
    state.update(cx, |state, _| {
        assert!(state.composition.is_none());
        assert!(state.marked.is_none());
    });
    assert_eq!(edits.borrow().len(), 1);
    assert_eq!(edits.borrow()[0].buffer.text(), "prefix日本");
}

#[gpui::test]
fn buffer_shrink_clamps_scroll_and_geometry_updates_before_paint(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, &"long line\n".repeat(100), 3, Edits::default());
    focus(&state, cx);
    cx.simulate_keystrokes("left");
    state.update(cx, |state, _| assert!(state.scroll.y > px(0.0)));
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            let mut props = state.props.clone();
            props.buffer = EditorBuffer::from_text("a");
            props.selections = vec![EditorSelection::cursor(EditorPosition::new(0, 1))];
            state.configure(props, window, cx);
            cx.notify();
        })
    });
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            assert_eq!(state.scroll.y, px(0.0));
            let bounds = state.layout.as_ref().expect("layout").bounds;
            let before = state
                .bounds_for_range(1..1, bounds, window, cx)
                .expect("old geometry");
            state.replace_and_mark_text_in_range(None, "wide text", Some(9..9), window, cx);
            let after = state
                .bounds_for_range(10..10, bounds, window, cx)
                .expect("fresh geometry");
            assert!(after.left() > before.left());
            assert_eq!(after.top(), before.top());
        })
    });
}

#[gpui::test]
fn double_click_selects_word_and_triple_click_selects_line(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "one two\nnext", 3, Edits::default());
    let position = state.update(cx, |state, _| {
        let row = &state.layout.as_ref().expect("layout").rows[0];
        row.origin + point(row.x_for_column(5), px(10.0))
    });
    for (click_count, expected) in [(2, "two"), (3, "one two\n")] {
        cx.simulate_event(MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count,
            first_mouse: false,
        });
        cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
        state.update(cx, |state, _| {
            assert_eq!(
                state
                    .edit
                    .buffer
                    .text_in_range(state.edit.selections[0].ordered()),
                expected
            )
        });
    }
}

#[gpui::test]
fn cursor_callback_without_focus_and_diagnostic_activation_are_preserved(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "one two", 3, Edits::default());
    let cursor = Rc::new(Cell::new(EditorPosition::default()));
    let observed = cursor.clone();
    let position = state.update(cx, |state, cx| {
        state.props.focus_handle = None;
        state.props.on_cursor = Some(Rc::new(move |position, _, _| observed.set(*position)));
        state.props.diagnostics = vec![EditorDiagnostic::new(
            EditorPosition::new(0, 0)..EditorPosition::new(0, 1),
            DiagnosticSeverity::Warning,
            "Check this",
        )];
        let row = &state.layout.as_ref().expect("layout").rows[0];
        let point = row.origin + point(row.x_for_column(2), px(10.0));
        cx.notify();
        point
    });
    cx.simulate_click(position, Modifiers::none());
    assert_eq!(cursor.get(), EditorPosition::new(0, 2));
    let activations = Rc::new(Cell::new(0));
    let observed = activations.clone();
    let position = state.update(cx, |state, cx| {
        state.props.on_diagnostic = Some(Rc::new(move |_, _, _| observed.set(observed.get() + 1)));
        cx.notify();
        state.layout.as_ref().expect("layout").rows[0]
            .diagnostic
            .as_ref()
            .expect("diagnostic")
            .0
            .center()
    });
    cx.simulate_click(position, Modifiers::none());
    assert_eq!(activations.get(), 1);
    assert_eq!(cursor.get(), EditorPosition::new(0, 2));
}

#[gpui::test]
fn platform_bridge_preserves_composition_across_frames(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (state, cx) = surface(cx, "🙂", 3, edits.clone());
    focus(&state, cx);
    let mut handler = cx.update(|window, cx| {
        let bounds = state.read(cx).layout.as_ref().expect("layout").bounds;
        gpui::PlatformInputHandler::new(
            window.to_async(cx),
            Box::new(ElementInputHandler::new(bounds, state.clone())),
        )
    });
    assert!(handler.query_prefers_ime_for_printable_keys());
    for (text, selected) in [("n", 1..1), ("日本", 1..2)] {
        handler.replace_and_mark_text_in_range(None, text, Some(selected));
        cx.run_until_parked();
        assert!(edits.borrow().is_empty());
    }
    assert_eq!(handler.marked_text_range(), Some(2..4));
    handler.replace_text_in_range(None, "日本語");
    cx.run_until_parked();
    assert_eq!(edits.borrow().len(), 1);
    assert_eq!(edits.borrow()[0].buffer.text(), "🙂日本語");
}

#[gpui::test]
fn command_callback_remains_available_without_text_edit_callbacks(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "text", 3, Edits::default());
    let commands = Rc::new(Cell::new(0));
    let observed = commands.clone();
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.props.on_edit = None;
            state.props.on_command = Some(Rc::new(move |command, _, _| {
                assert_eq!(*command, EditorCommand::Undo);
                observed.set(observed.get() + 1);
            }));
            let shortcut = if cfg!(target_os = "macos") {
                "cmd-z"
            } else {
                "ctrl-z"
            };
            state.key_down(&key(shortcut), window, cx);
            assert_eq!(state.edit.buffer.text(), "text");
        })
    });
    assert_eq!(commands.get(), 1);
}

fn prepare_at(
    state: &mut EditorSurface,
    width: f32,
    height: f32,
    window: &Window,
    cx: &mut Context<EditorSurface>,
) -> Viewport {
    let viewport = state.prepare(
        Bounds::new(point(px(20.0), px(30.0)), size(px(width), px(height))),
        window,
        cx,
    );
    state.layout = Some(viewport.clone());
    viewport
}

#[gpui::test]
fn soft_wrap_maps_graphemes_tabs_selection_and_native_geometry(cx: &mut TestAppContext) {
    let text = "let value = \"日本🙂e\u{301}👩‍💻\";\t// long comment ".repeat(5);
    let (entity, cx) = surface(cx, &text, 8, Edits::default());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            let viewport = prepare_at(state, 240.0, 440.0, window, cx);
            assert!(viewport.rows.len() > 2);
            assert_eq!(state.edit.buffer.text(), text);
            let row = &viewport.rows[1];
            let column = row.columns.start + 1;
            let position = EditorPosition::new(0, column);
            let hit = row.origin + point(row.x_for_column(column), px(5.0));
            assert_eq!(state.hit_position(hit, window, cx), Some(position));
            let utf16 = state
                .edit
                .buffer
                .lines
                .byte_utf16(position_byte(&state.edit.buffer, position));
            let rect = state
                .bounds_for_range(utf16..utf16, viewport.bounds, window, cx)
                .expect("test fixture");
            assert_eq!(
                rect.origin,
                row.origin + point(row.x_for_column(column), px(0.0))
            );
            assert_eq!(
                state.character_index_for_point(hit, window, cx),
                Some(utf16)
            );
            for row in &viewport.rows {
                let (_, fragment) = state
                    .wrap
                    .as_ref()
                    .expect("test fixture")
                    .fragment(row.visual);
                assert!(text.is_char_boundary(fragment.bytes.start));
                assert_eq!(
                    text[..fragment.bytes.start].graphemes(true).count(),
                    fragment.columns.start
                );
                assert_eq!(
                    text[..fragment.bytes.end].graphemes(true).count(),
                    fragment.columns.end
                );
                assert!(row.line.width() <= viewport.body.size.width);
            }
            state.edit.selections = vec![EditorSelection::new(
                EditorPosition::new(0, 1),
                EditorPosition::new(0, viewport.rows[2].columns.end),
            )];
            let selected = prepare_at(state, 240.0, 440.0, window, cx);
            assert_eq!(selected.selections.len(), 3);
            assert!(selected.caret.is_none());
            assert!(
                !selected
                    .scrollbars
                    .iter()
                    .any(|b| b.axis == Axis::Horizontal)
            );
        })
    });
}

#[gpui::test]
fn wrapped_navigation_and_boundary_affinity_follow_visual_rows(cx: &mut TestAppContext) {
    let (entity, cx) = surface(cx, &"abcdefghij ".repeat(30), 5, Edits::default());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::new(0, 2))];
            let viewport = prepare_at(state, 230.0, 110.0, window, cx);
            let next_start = viewport.rows[1].columns.start;
            state.move_vertical("down", false, window, cx);
            assert_eq!(
                state.edit.selections[0].head,
                EditorPosition::new(0, next_start + 2)
            );
            state.key_down(&key("home"), window, cx);
            assert_eq!(state.edit.selections[0].head.column, next_start);
            state.key_down(&key("end"), window, cx);
            assert_eq!(
                state.edit.selections[0].head.column,
                viewport.rows[1].columns.end
            );
            let viewport = prepare_at(state, 230.0, 110.0, window, cx);
            assert_eq!(
                viewport.caret.expect("test fixture").top(),
                viewport.rows[1].origin.y
            );
            state.key_down(&key("right"), window, cx);
            let viewport = prepare_at(state, 230.0, 110.0, window, cx);
            assert_eq!(
                viewport.caret.expect("test fixture").top(),
                viewport.rows[2].origin.y
            );
            state.move_vertical("pagedown", true, window, cx);
            assert!(!state.edit.selections[0].is_empty());
            assert!(state.visual_row(state.edit.selections[0].head) >= 6);
        })
    });
}

#[gpui::test]
fn wrap_resize_first_line_and_prepaint_composition_use_current_mapping(cx: &mut TestAppContext) {
    let (entity, cx) = surface(
        cx,
        &format!("{}\ntail", "word ".repeat(100)),
        4,
        Edits::default(),
    );
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            let narrow = prepare_at(state, 180.0, 88.0, window, cx);
            let narrow_count = state.wrap.as_ref().expect("test fixture").row_count();
            let wide = prepare_at(state, 360.0, 88.0, window, cx);
            assert!(state.wrap.as_ref().expect("test fixture").row_count() < narrow_count);
            assert!(wide.rows[0].columns.end > narrow.rows[0].columns.end);
            let mut props = state.props.clone();
            props.options.first_line = 1;
            state.configure(props, window, cx);
            let viewport = prepare_at(state, 360.0, 88.0, window, cx);
            assert_eq!(viewport.rows.last().expect("test fixture").index, 1);
            assert!(state.scroll.y > px(0.0));
            state.replace_and_mark_text_in_range(Some(0..0), "日本🙂 ", Some(4..4), window, cx);
            // Geometry is requested before the next paint after reflow.
            state.scroll.y = px(0.0);
            let rect = state
                .bounds_for_range(4..4, viewport.bounds, window, cx)
                .expect("test fixture");
            assert_eq!(rect.top(), viewport.body.top());
            state.unmark_text(window, cx);
            assert!(state.edit.buffer.text().starts_with("日本🙂 "));
            state.props.options.soft_wrap = false;
            prepare_at(state, 360.0, 88.0, window, cx);
            assert_eq!(state.wrap.as_ref().expect("test fixture").row_count(), 2);
        })
    });
}

#[gpui::test]
fn large_document_reflows_only_changed_chunks_and_shapes_visible_fragments(
    cx: &mut TestAppContext,
) {
    let text = (0..100_000)
        .map(|i| format!("let value_{i} = 123; // example line"))
        .collect::<Vec<_>>()
        .join("\n");
    let (entity, cx) = surface(cx, &text, 6, Edits::default());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            let viewport = prepare_at(state, 240.0, 132.0, window, cx);
            state.reflow = None;
            state.wrap = Some(WrapMap::new(
                &state.edit.buffer,
                viewport.font.clone(),
                Some(viewport.body.size.width - px(PADDING)),
                window.text_system(),
            ));
            let initial = state.wrap.as_ref().expect("test fixture").measured_lines;
            assert_eq!(initial, 100_000);
            prepare_at(state, 240.0, 132.0, window, cx);
            assert_eq!(
                state.wrap.as_ref().expect("test fixture").measured_lines,
                initial
            );
            state
                .edit
                .buffer
                .insert(EditorPosition::new(99_999, 0), "x");
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::new(99_999, 1))];
            state.reveal = true;
            let viewport = prepare_at(state, 240.0, 132.0, window, cx);
            assert!(state.wrap.as_ref().expect("test fixture").measured_lines <= initial + 128);
            assert!(viewport.rows.len() <= 8);
            assert!(viewport.rows.last().expect("test fixture").index >= 99_998);
            assert!(viewport.caret.is_some());
            let total = state.text_length_utf16(window, cx).expect("test fixture");
            assert_eq!(total, text.len() + 1);
            let mut actual = None;
            assert_eq!(
                state
                    .text_for_range(total - 4..total, &mut actual, window, cx)
                    .expect("test fixture"),
                "line"
            );
            assert_eq!(actual, Some(total - 4..total));
        })
    });
}

#[gpui::test]
fn long_wrapped_line_shapes_only_visible_fragments(cx: &mut TestAppContext) {
    let (entity, cx) = surface(cx, "initial", 5, Edits::default());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            state.edit.buffer = EditorBuffer::from_text("word🙂 ".repeat(20_000));
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::new(0, 60_000))];
            state.reveal = true;
            let preview = prepare_at(state, 240.0, 110.0, window, cx);
            assert!(state.wrap.as_ref().expect("preview").is_provisional());
            assert!(preview.rows.iter().all(|row| row.columns.end <= 256));
            assert!(preview.caret.is_none());
            state.reflow = None;
            state.wrap = Some(WrapMap::new(
                &state.edit.buffer,
                preview.font.clone(),
                Some(preview.body.size.width - px(PADDING)),
                window.text_system(),
            ));
            state.reveal = true;
            let viewport = prepare_at(state, 240.0, 110.0, window, cx);
            assert!(state.wrap.as_ref().expect("test fixture").row_count() > 1_000);
            assert!(viewport.rows.len() <= 7);
            assert!(viewport.rows.iter().all(|row| row.line.len() < 200));
            assert!(viewport.caret.is_some());
        })
    });
}

#[gpui::test]
fn scrollbar_drag_and_track_click_do_not_change_selection(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (entity, cx) = surface(
        cx,
        &vec!["long line ".repeat(200); 100].join("\n"),
        5,
        edits.clone(),
    );
    let (vertical, horizontal, selection) = entity.update(cx, |state, _| {
        let viewport = state.layout.as_ref().expect("test fixture");
        (
            viewport
                .scrollbars
                .iter()
                .find(|b| b.axis == Axis::Vertical)
                .expect("test fixture")
                .clone(),
            viewport
                .scrollbars
                .iter()
                .find(|b| b.axis == Axis::Horizontal)
                .expect("test fixture")
                .clone(),
            state.edit.selections.clone(),
        )
    });
    let center = vertical.thumb.origin + point(px(3.0), vertical.thumb.size.height / 2.0);
    cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::none());
    // Drag beyond the viewport; capture must keep the scrollbar active.
    let outside = point(center.x + px(200.0), vertical.track.bottom() + px(100.0));
    cx.simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    entity.update(cx, |state, _| {
        assert!(state.scroll.y >= vertical.maximum - px(1.0));
        assert_eq!(state.edit.selections, selection);
        assert!(state.scrollbar_drag.is_none());
    });
    let track = point(
        horizontal.track.right() - px(2.0),
        horizontal.track.top() + px(4.0),
    );
    cx.simulate_click(track, Modifiers::none());
    entity.update(cx, |state, _| {
        assert!(state.scroll.x > px(0.0));
        assert_eq!(state.edit.selections, selection);
        assert!(!state.selecting);
    });
    assert!(edits.borrow().is_empty());
}

#[gpui::test]
fn scrollbar_geometry_handles_tiny_viewports_and_disabled_bars(cx: &mut TestAppContext) {
    let (entity, cx) = surface(cx, &"x\n".repeat(50), 2, Edits::default());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            let viewport = prepare_at(state, 50.0, 5.0, window, cx);
            for bar in &viewport.scrollbars {
                assert!(bar.track.contains(&bar.thumb.origin));
                assert!(
                    f32::from(bar.offset_for_drag(point(px(1000.0), px(1000.0)), 0.5)).is_finite()
                );
            }
            state.props.options.scrollbars = false;
            assert!(
                prepare_at(state, 50.0, 5.0, window, cx)
                    .scrollbars
                    .is_empty()
            );
            state.props.options.scrollbars = true;
            state.edit.buffer = EditorBuffer::from_text("");
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::default())];
            state.scroll = point(px(100.0), px(1000.0));
            let viewport = prepare_at(state, 300.0, 110.0, window, cx);
            assert!(viewport.scrollbars.is_empty());
            assert_eq!(state.scroll, point(px(0.0), px(0.0)));
        })
    });
}

#[gpui::test]
fn wrapped_home_after_native_newline_uses_current_document(cx: &mut TestAppContext) {
    let (entity, cx) = surface(cx, "prefix", 3, Edits::default());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            prepare_at(state, 200.0, 66.0, window, cx);
            state.replace_text_in_range(None, "\nnew line", window, cx);
            state.key_down(&key("home"), window, cx);
            assert_eq!(state.edit.selections[0].head, EditorPosition::new(1, 0));
        })
    });
}

#[gpui::test]
fn wrapped_diagnostics_reserve_horizontal_track_without_occluding_text(cx: &mut TestAppContext) {
    let (entity, cx) = surface(cx, "short", 3, Edits::default());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            state.props.diagnostics = vec![EditorDiagnostic::new(
                EditorPosition::new(0, 0)..EditorPosition::new(0, 1),
                DiagnosticSeverity::Warning,
                "A diagnostic with a deliberately long message for horizontal scrolling",
            )];
            let viewport = prepare_at(state, 240.0, 66.0, window, cx);
            let bar = viewport
                .scrollbars
                .iter()
                .find(|bar| bar.axis == Axis::Horizontal)
                .expect("horizontal diagnostic overflow");
            assert!(viewport.body.bottom() <= bar.track.top());
        })
    });
}

#[gpui::test]
fn large_initial_layout_accepts_input_and_scrolling_before_worker_completes(
    cx: &mut TestAppContext,
) {
    let edits = Edits::default();
    let (entity, cx) = surface(cx, "initial", 6, edits.clone());
    let buffer = EditorBuffer::from_text(
        "long example line with several words "
            .repeat(4)
            .to_string()
            + &"\nlong example line with several words ".repeat(99_999),
    );
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.edit = EditorEdit::new(
                buffer,
                vec![EditorSelection::cursor(EditorPosition::default())],
            );
            state.props.options.soft_wrap = true;
            let bounds = state.layout.as_ref().expect("layout").bounds;
            let preview = state.prepare(bounds, window, cx);
            state.layout = Some(preview.clone());
            assert!(state.wrap.as_ref().expect("preview map").is_provisional());
            assert_eq!(state.wrap.as_ref().expect("preview map").measured_lines, 0);
            assert!(preview.rows.len() <= 7);
            let cancelled = state
                .reflow
                .as_ref()
                .expect("background job")
                .cancelled
                .clone();
            state.replace_text_in_range(None, "typed ", window, cx);
            state.update_wrap(&preview.font, preview.body.size.width, window, cx);
            assert!(cancelled.load(std::sync::atomic::Ordering::Relaxed));
            assert!(
                state
                    .edit
                    .buffer
                    .line(0)
                    .expect("first line")
                    .starts_with("typed ")
            );
            state.wheel(
                &ScrollWheelEvent {
                    position: preview.body.center(),
                    delta: ScrollDelta::Lines(point(0.0, -50.0)),
                    modifiers: Modifiers::none(),
                    touch_phase: TouchPhase::Moved,
                },
                window,
                cx,
            );
            assert_eq!(state.scroll.y, px(50.0 * LINE_HEIGHT));
            assert!(state.reflow.is_some());
        })
    });
    cx.run_until_parked();
    entity.update(cx, |state, _| {
        assert!(state.reflow.is_none());
        let map = state.wrap.as_ref().expect("finished map");
        assert!(!map.is_provisional());
        assert!(map.same_source(&state.edit.buffer));
        let row = (f32::from(state.scroll.y) / LINE_HEIGHT).floor() as usize;
        assert_eq!(map.fragment(row).0, 50);
        assert_eq!(state.edit.selections[0].head.column, 6);
    });
    assert_eq!(edits.borrow().len(), 1);
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            let viewport = state.layout.clone().expect("finished layout");
            state.update_wrap(&viewport.font, viewport.body.size.width * 0.6, window, cx);
            assert!(state.reflow.is_some());
            assert!(
                !state
                    .wrap
                    .as_ref()
                    .expect("retained layout")
                    .is_provisional()
            );
            // Returning to the completed width cancels an unnecessary intermediate job.
            let cancelled = state.reflow.as_ref().expect("resize job").cancelled.clone();
            state.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
            assert!(cancelled.load(std::sync::atomic::Ordering::Relaxed));
            assert!(state.reflow.is_none());
        })
    });
}

#[gpui::test]
fn repeated_resize_retains_geometry_and_rejects_obsolete_results(cx: &mut TestAppContext) {
    let (entity, cx) = surface(
        cx,
        &"word repeated for wrapping "
            .repeat(4)
            .lines()
            .cycle()
            .take(3_000)
            .collect::<Vec<_>>()
            .join("\n"),
        5,
        Edits::default(),
    );
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            let viewport = state.layout.clone().expect("layout");
            let old_rows = state.wrap.as_ref().expect("map").row_count();
            state.update_wrap(&viewport.font, px(240.0), window, cx);
            assert_eq!(
                state.wrap.as_ref().expect("retained map").row_count(),
                old_rows
            );
            let generation = state.reflow_generation;
            let cancelled = state
                .reflow
                .as_ref()
                .expect("pending job")
                .cancelled
                .clone();
            let obsolete = WrapMap::new(
                &state.edit.buffer,
                viewport.font.clone(),
                Some(px(240.0 - PADDING)),
                window.text_system(),
            );
            for width in [300.0, 220.0, 500.0, 280.0] {
                state.update_wrap(&viewport.font, px(width), window, cx);
            }
            assert!(cancelled.load(std::sync::atomic::Ordering::Relaxed));
            state.finish_reflow(generation, obsolete, cx);
            assert_eq!(
                state.wrap.as_ref().expect("retained map").row_count(),
                old_rows
            );
            assert!(state.reflow.is_some());
            state.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
        })
    });
    cx.run_until_parked();
    entity.update(cx, |state, _| {
        let viewport = state.layout.as_ref().expect("layout");
        assert!(state.reflow.is_none());
        assert!(state.wrap.as_ref().expect("latest map").matches(
            &state.edit.buffer,
            &viewport.font,
            Some(viewport.body.size.width - px(PADDING))
        ));
    });
    cx.simulate_resize(size(px(320.0), px(600.0)));
    cx.run_until_parked();
    entity.update(cx, |state, _| {
        let viewport = state.layout.as_ref().expect("resized layout");
        assert!(state.reflow.is_none());
        assert!(state.wrap.as_ref().expect("resized map").matches(
            &state.edit.buffer,
            &viewport.font,
            Some(viewport.body.size.width - px(PADDING))
        ));
    });
}

#[gpui::test]
fn document_replacement_and_job_drop_cancel_old_reflow(cx: &mut TestAppContext) {
    let (entity, cx) = surface(cx, "initial", 5, Edits::default());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            let viewport = state.layout.clone().expect("layout");
            state.edit.buffer = EditorBuffer::from_text("old text\n".repeat(3_000));
            state.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
            let generation = state.reflow_generation;
            let cancelled = state.reflow.as_ref().expect("job").cancelled.clone();
            let obsolete = WrapMap::new(
                &state.edit.buffer,
                viewport.font.clone(),
                None,
                window.text_system(),
            );
            // Replacement can occur before another frame schedules a new job.
            state.edit.buffer = EditorBuffer::from_text("replacement");
            state.finish_reflow(generation, obsolete, cx);
            assert!(
                !state
                    .wrap
                    .as_ref()
                    .expect("preview")
                    .same_source(&state.edit.buffer)
            );
            state.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
            assert!(cancelled.load(std::sync::atomic::Ordering::Relaxed));
            assert!(state.reflow.is_none());
            assert_eq!(state.wrap.as_ref().expect("replacement map").row_count(), 1);
            state.edit.buffer = EditorBuffer::from_text("new text\n".repeat(3_000));
            state.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
            let cancelled = state.reflow.as_ref().expect("job").cancelled.clone();
            state.reflow = None;
            assert!(cancelled.load(std::sync::atomic::Ordering::Relaxed));
        })
    });
    cx.run_until_parked();
}

#[gpui::test]
fn deferred_caret_reveal_survives_long_line_reflow(cx: &mut TestAppContext) {
    let (entity, cx) = surface(cx, "initial", 4, Edits::default());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            state.edit.buffer = EditorBuffer::from_text("word🙂 ".repeat(20_000));
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::new(0, 60_000))];
            state.reveal = true;
            let bounds = state.layout.as_ref().expect("layout").bounds;
            let viewport = state.prepare(bounds, window, cx);
            assert!(viewport.caret.is_none());
            state.layout = Some(viewport.clone());
            assert!(state.reveal_after_reflow);
            assert!(
                state
                    .bounds_for_range(70_000..70_000, bounds, window, cx)
                    .is_none()
            );
        })
    });
    cx.run_until_parked();
    entity.update(cx, |state, _| {
        let viewport = state.layout.as_ref().expect("finished layout");
        assert!(state.reflow.is_none());
        assert!(viewport.caret.is_some());
        assert!(state.scroll.y > px(0.0));
        assert!(viewport.rows.iter().all(|row| row.line.len() < 1_000));
    });
}

#[gpui::test]
fn preview_work_is_bounded_even_for_a_single_enormous_grapheme(cx: &mut TestAppContext) {
    let (entity, cx) = surface(cx, "initial", 3, Edits::default());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.edit.buffer = EditorBuffer::from_text(format!("e{}", "\u{301}".repeat(20_000)));
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::default())];
            state.props.options.soft_wrap = true;
            let bounds = state.layout.as_ref().expect("layout").bounds;
            let preview = state.prepare(bounds, window, cx);
            assert!(state.wrap.as_ref().expect("map").is_provisional());
            assert_eq!(preview.rows[0].columns, 0..0);
            assert_eq!(preview.rows[0].line.text.as_ref(), "…");
            state.reflow = None;
        })
    });
}

#[gpui::test]
fn long_unwrapped_line_shapes_only_visible_segments_and_preserves_native_geometry(
    cx: &mut TestAppContext,
) {
    let text = format!(
        "let value = \"{}\"; // tail comment",
        "ab日本🙂e\u{301}\t ".repeat(20_000)
    );
    let (entity, cx) = surface(cx, &text, 4, Edits::default());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            let target = EditorPosition::new(0, 120_003);
            state.edit.selections = vec![EditorSelection::cursor(target)];
            state.reveal = true;
            let bounds = state.layout.as_ref().expect("layout").bounds;
            let viewport = state.prepare(bounds, window, cx);
            state.layout = Some(viewport.clone());
            assert!(state.scroll.x > px(100_000.0));
            assert!(
                viewport
                    .rows
                    .iter()
                    .all(|r| r.columns.len() <= crate::analysis::BLOCK_COLUMNS)
            );
            assert!(viewport.rows.len() <= 5);
            assert_eq!(viewport.rows.iter().filter(|r| r.number_visible).count(), 1);
            let caret = viewport.caret.expect("visible tail caret");
            assert!(viewport.body.contains(&caret.origin));
            let utf16 = state
                .edit
                .buffer
                .lines
                .byte_utf16(position_byte(&state.edit.buffer, target));
            assert_eq!(
                state
                    .bounds_for_range(utf16..utf16, bounds, window, cx)
                    .expect("IME bounds")
                    .origin,
                caret.origin
            );
            assert_eq!(
                state.hit_position(caret.origin + point(px(0.0), px(5.0)), window, cx),
                Some(target)
            );
            assert_eq!(
                state.character_index_for_point(caret.origin + point(px(0.0), px(5.0)), window, cx),
                Some(utf16)
            );
            let analysis = state
                .edit
                .buffer
                .lines
                .cached_analysis(0)
                .expect("cached syntax")
                .clone();
            assert!(
                analysis
                    .tokens
                    .iter()
                    .any(|t| t.kind == SyntaxTokenKind::String
                        && t.start < target.column
                        && t.end > target.column)
            );
            for _ in 0..20 {
                state.prepare(bounds, window, cx);
            }
            assert!(std::sync::Arc::ptr_eq(
                &analysis,
                state
                    .edit
                    .buffer
                    .lines
                    .cached_analysis(0)
                    .expect("shared analysis")
            ));
        })
    });
}

#[gpui::test]
fn selection_and_navigation_cross_horizontal_segment_boundaries(cx: &mut TestAppContext) {
    let text = format!("{}\n{}", "a🙂\t ".repeat(1_000), "b日本 ".repeat(1_000));
    let (entity, cx) = surface(cx, &text, 4, Edits::default());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            let start = EditorPosition::new(0, 2_553);
            let end = EditorPosition::new(0, 2_568);
            state.edit.selections = vec![EditorSelection::new(start, end)];
            state.reveal = true;
            let bounds = state.layout.as_ref().expect("layout").bounds;
            let viewport = state.prepare(bounds, window, cx);
            state.layout = Some(viewport.clone());
            assert!(viewport.selections.len() >= 2);
            let selected_width: Pixels = viewport.selections.iter().map(|r| r.size.width).sum();
            let left = state
                .shape_column(0, start.column, &viewport.font, window, cx)
                .x_for_column(start.column);
            let right = state
                .shape_column(0, end.column, &viewport.font, window, cx)
                .x_for_column(end.column);
            assert!((f32::from(selected_width - (right - left))).abs() < 0.01);
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::new(0, 2_560))];
            state.move_vertical("down", false, window, cx);
            assert_eq!(state.edit.selections[0].head.line, 1);
            assert!(state.edit.selections[0].head.column > 1_000);
            state.move_vertical("up", false, window, cx);
            assert_eq!(state.edit.selections[0].head, EditorPosition::new(0, 2_560));
        })
    });
}

#[gpui::test]
fn syntax_cache_is_shared_across_reflow_and_invalidated_by_edits(cx: &mut TestAppContext) {
    let text = format!(
        "let value = \"{}\"; // comment",
        "long string ".repeat(4_000)
    );
    let (entity, cx) = surface(cx, &text, 3, Edits::default());
    let old = entity.update(cx, |state, _| {
        state
            .edit
            .buffer
            .lines
            .cached_analysis(0)
            .expect("analysis")
            .clone()
    });
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            let viewport = state.layout.clone().expect("layout");
            state.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            assert!(std::sync::Arc::ptr_eq(
                &old,
                state
                    .edit
                    .buffer
                    .lines
                    .cached_analysis(0)
                    .expect("shared analysis")
            ));
            let quote = EditorPosition::new(0, 12);
            state
                .edit
                .buffer
                .replace_range(quote..EditorPosition::new(0, 13), "");
            assert!(state.edit.buffer.lines.cached_analysis(0).is_none());
            let viewport = state.layout.clone().expect("layout");
            state.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
            assert!(state.reflow.is_some());
        })
    });
    cx.run_until_parked();
    entity.update(cx, |state, _| {
        let next = state
            .edit
            .buffer
            .lines
            .cached_analysis(0)
            .expect("new analysis");
        assert!(!std::sync::Arc::ptr_eq(&old, next));
        assert_ne!(old.tokens, next.tokens);
    });
}

#[gpui::test]
fn large_native_paste_and_history_keep_latest_snapshot_during_reflow(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (entity, cx) = surface(cx, "seed", 5, edits.clone());
    let original = EditorBuffer::from_text("a🙂b ".repeat(30_000));
    // A completed layout normally supplies this index before editing.
    original
        .lines
        .analyze(0, &std::sync::atomic::AtomicBool::new(false));
    let mut session = EditorSession::new(original.clone());
    let column = original.line_len(0) - 2;
    session.set_selections(vec![EditorSelection::cursor(EditorPosition::new(
        0, column,
    ))]);
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            let mut props = state
                .props
                .clone()
                .selections(session.selections().to_vec());
            props.buffer = session.buffer().clone();
            state.configure(props, window, cx);
            let viewport = state.layout.clone().expect("viewport");
            state.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
            let cancelled = state
                .reflow
                .as_ref()
                .expect("initial job")
                .cancelled
                .clone();
            let paste = "x\r\n日本🙂\r\n".repeat(2_000);
            state.replace_text_in_range(None, &paste, window, cx);
            assert_eq!(edits.borrow().len(), 1);
            session.apply(edits.borrow()[0].clone());
            let pasted = session.buffer().clone();
            let mut props = state
                .props
                .clone()
                .selections(session.selections().to_vec());
            props.buffer = pasted.clone();
            state.configure(props, window, cx);
            let selected = state
                .selected_text_range(false, window, cx)
                .expect("native selection")
                .range;
            assert_eq!(selected.start, selected.end);
            assert_eq!(
                selected.start,
                original
                    .lines
                    .byte_utf16(original.lines.column_byte(0, column))
                    + normalize_newlines(&paste).encode_utf16().count()
            );
            state.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
            assert!(cancelled.load(std::sync::atomic::Ordering::Relaxed));
            for redo in [false, true, false, true] {
                assert!(if redo { session.redo() } else { session.undo() });
                let mut props = state
                    .props
                    .clone()
                    .selections(session.selections().to_vec());
                props.buffer = session.buffer().clone();
                state.configure(props, window, cx);
                state.update_wrap(&viewport.font, viewport.body.size.width, window, cx);
                assert!(
                    state
                        .edit
                        .buffer
                        .lines
                        .same_snapshot(&session.buffer().lines)
                );
            }
            assert!(state.edit.buffer.lines.same_snapshot(&pasted.lines));
            assert_eq!(edits.borrow().len(), 1);
        });
    });
    cx.run_until_parked();
    entity.update(cx, |state, _| {
        assert!(state.reflow.is_none());
        assert!(
            state
                .wrap
                .as_ref()
                .expect("completed map")
                .same_source(session.buffer())
        );
        assert_eq!(state.edit.selections, session.selections());
    });
}

#[gpui::test]
fn huge_unicode_paste_stages_ime_without_blocking_or_reordering(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (entity, cx) = surface(cx, "prefix suffix", 5, edits.clone());
    let paste = "日本語e\u{301}👩‍💻🇺🇸 ".repeat(40_000);
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::new(0, 7))];
            state.replace_text_in_range(None, &paste, window, cx);
            assert!(state.pending_edit.as_ref().expect("job").working);
            assert!(edits.borrow().is_empty());
            assert_eq!(state.edit.buffer.text(), "prefix suffix");
            assert_eq!(
                state
                    .selected_text_range(false, window, cx)
                    .expect("draft selection")
                    .range,
                0..0
            );
            state.replace_text_in_range(None, "A", window, cx);
            state.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx);
            assert_eq!(state.marked_text_range(window, cx), Some(0..2));
            state.replace_and_mark_text_in_range(Some(0..2), "日本🙂", Some(2..4), window, cx);
            assert_eq!(
                state
                    .selected_text_range(false, window, cx)
                    .expect("draft selection")
                    .range,
                2..4
            );
            assert_eq!(state.text_length_utf16(window, cx), Some(4));
            let mut actual = None;
            assert_eq!(
                state
                    .text_for_range(2..4, &mut actual, window, cx)
                    .as_deref(),
                Some("🙂")
            );
            assert_eq!(actual, Some(2..4));
            let bounds = state.layout.as_ref().expect("viewport").bounds;
            assert!(state.bounds_for_range(2..2, bounds, window, cx).is_some());
        });
    });
    cx.run_until_parked();
    assert_eq!(edits.borrow().len(), 2);
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            assert!(!state.pending_edit.as_ref().expect("active IME").working);
            assert_eq!(state.edit.buffer.text(), format!("prefix {paste}Asuffix"));
            assert_eq!(state.marked_text_range(window, cx), Some(0..4));
            state.replace_text_in_range(Some(0..4), "日本語", window, cx);
            assert!(state.pending_edit.is_some());
        });
    });
    cx.run_until_parked();
    assert_eq!(edits.borrow().len(), 3);
    let mut session = EditorSession::new(EditorBuffer::from_text("prefix suffix"));
    for edit in edits.borrow().iter() {
        session.apply(edit.clone());
    }
    assert_eq!(
        session.buffer().text(),
        format!("prefix {paste}A日本語suffix")
    );
    assert!(session.undo());
    assert_eq!(session.buffer().text(), format!("prefix {paste}Asuffix"));
    assert!(session.undo());
    assert_eq!(session.buffer().text(), format!("prefix {paste}suffix"));
    assert!(session.undo());
    assert_eq!(session.buffer().text(), "prefix suffix");
    for _ in 0..3 {
        assert!(session.redo());
    }
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            assert!(state.pending_edit.is_none());
            assert_eq!(state.edit.buffer, *session.buffer());
            let units = 7 + paste.encode_utf16().count() + 4;
            assert_eq!(
                state
                    .selected_text_range(false, window, cx)
                    .expect("document selection")
                    .range,
                units..units
            );
        })
    });
}

#[gpui::test]
fn async_paste_cancels_on_authoritative_changes_and_editability_loss(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (entity, cx) = surface(cx, "original", 5, edits.clone());
    let paste = "日本🙂".repeat(30_000);
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.replace_text_in_range(None, &paste, window, cx);
            let cancelled = state.pending_edit.as_ref().expect("job").cancelled.clone();
            state.replace_text_in_range(None, "queued", window, cx);
            let mut props = state.props.clone();
            props.buffer = EditorBuffer::from_text("replacement");
            state.configure(props, window, cx);
            assert!(cancelled.load(std::sync::atomic::Ordering::Relaxed));
            assert!(state.pending_edit.is_none());
            state.replace_text_in_range(None, &paste, window, cx);
            let mut props = state.props.clone();
            props.on_edit = None;
            state.configure(props, window, cx);
            assert!(state.pending_edit.is_none());
        })
    });
    cx.run_until_parked();
    assert!(edits.borrow().is_empty());
    assert_eq!(
        entity.read_with(cx, |state, _| state.edit.buffer.text()),
        "replacement"
    );
}

#[gpui::test]
fn async_paste_keeps_clipboard_order_and_allows_scrolling(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let text = "old line\n".repeat(300);
    let (entity, cx) = surface(cx, &text, 5, edits.clone());
    let paste = "日本🙂".repeat(20_000);
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.edit.selections = vec![
                EditorSelection::cursor(EditorPosition::new(0, 0)),
                EditorSelection::cursor(EditorPosition::new(2, 0)),
            ];
            let shortcut = if cfg!(target_os = "macos") {
                "cmd-v"
            } else {
                "ctrl-v"
            };
            cx.write_to_clipboard(ClipboardItem::new_string(paste.clone()));
            state.key_down(&key(shortcut), window, cx);
            cx.write_to_clipboard(ClipboardItem::new_string("second".into()));
            state.key_down(&key(shortcut), window, cx);
            cx.write_to_clipboard(ClipboardItem::new_string("changed later".into()));
            state.replace_text_in_range(None, "!", window, cx);
            let viewport = state.layout.as_ref().expect("viewport").clone();
            state.wheel(
                &ScrollWheelEvent {
                    position: viewport.body.center(),
                    delta: ScrollDelta::Lines(point(0.0, -10.0)),
                    modifiers: Modifiers::none(),
                    touch_phase: TouchPhase::Moved,
                },
                window,
                cx,
            );
            assert!(state.scroll.y > px(0.0));
            assert!(edits.borrow().is_empty());
        })
    });
    cx.run_until_parked();
    assert_eq!(edits.borrow().len(), 3);
    let expected = format!("{paste}second!old line");
    entity.update(cx, |state, _| {
        assert_eq!(state.edit.buffer.line(0).expect("first").as_ref(), expected);
        assert_eq!(
            state.edit.buffer.line(2).expect("second").as_ref(),
            expected
        );
        assert!(state.pending_edit.is_none());
    });
}

struct AsyncControlledHarness {
    session: EditorSession,
    editor: Entity<EditorSurface>,
    focus: FocusHandle,
}
impl Render for AsyncControlledHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let props = CodeEditor::new("async-controlled", self.session.buffer().clone())
            .selections(self.session.selections().to_vec())
            .focusable(self.focus.clone())
            .on_edit(cx.listener(|state, edit: &EditorEdit, _, cx| {
                state.session.apply(edit.clone());
                cx.notify();
            }))
            .on_command(cx.listener(|state, command, _, cx| {
                match command {
                    EditorCommand::Undo => {
                        state.session.undo();
                    }
                    EditorCommand::Redo => {
                        state.session.redo();
                    }
                }
                cx.notify();
            }));
        self.editor
            .update(cx, |state, cx| state.configure(props, window, cx));
        self.editor.clone()
    }
}

#[gpui::test]
fn queued_history_commands_wait_for_host_render_before_later_input(cx: &mut TestAppContext) {
    init(cx);
    let (host, cx) = cx.add_window_view(|_, cx| {
        let buffer = EditorBuffer::from_text("base");
        AsyncControlledHarness {
            session: EditorSession::new(buffer.clone()),
            focus: cx.focus_handle(),
            editor: cx.new(|_| EditorSurface::new(CodeEditor::new("async-controlled", buffer))),
        }
    });
    let entity = host.read_with(cx, |host, _| host.editor.clone());
    let paste = "日本🙂".repeat(20_000);
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.replace_text_in_range(None, &paste, window, cx);
            let modifier = if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            };
            state.key_down(&key(&format!("{modifier}-z")), window, cx);
            state.key_down(&key(&format!("{modifier}-shift-z")), window, cx);
            state.replace_text_in_range(None, "tail", window, cx);
            assert!(state.pending_edit.is_some());
        })
    });
    for _ in 0..8 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
            window.draw(cx).clear(cx);
        });
    }
    cx.run_until_parked();
    let expected = format!("{paste}tailbase");
    host.update(cx, |host, _| {
        assert_eq!(host.session.buffer().text(), expected)
    });
    entity.update(cx, |state, _| {
        assert!(state.pending_edit.is_none());
        assert_eq!(state.edit.buffer.text(), expected);
    });
    host.update(cx, |host, _| {
        assert!(host.session.undo());
        assert_eq!(host.session.buffer().text(), format!("{paste}base"));
        assert!(host.session.undo());
        assert_eq!(host.session.buffer().text(), "base");
    });
}

#[gpui::test]
fn queued_cut_copy_and_paste_use_logical_clipboard_order(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (entity, cx) = surface(cx, "base", 5, edits.clone());
    let paste = "日本🙂".repeat(20_000);
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::default())];
            state.replace_text_in_range(None, &paste, window, cx);
            let modifier = if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            };
            state.key_down(&key("shift-left"), window, cx);
            state.key_down(&key(&format!("{modifier}-c")), window, cx);
            state.key_down(&key(&format!("{modifier}-x")), window, cx);
            // Copying an empty selection must retain the preceding logical clipboard.
            state.key_down(&key(&format!("{modifier}-c")), window, cx);
            cx.write_to_clipboard(ClipboardItem::new_string("external old clipboard".into()));
            state.key_down(&key(&format!("{modifier}-v")), window, cx);
            state.replace_text_in_range(None, "!", window, cx);
        })
    });
    for _ in 0..4 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
            window.draw(cx).clear(cx);
        });
    }
    cx.run_until_parked();
    entity.update(cx, |state, cx| {
        assert!(state.pending_edit.is_none());
        assert_eq!(state.edit.buffer.text(), format!("{paste}!base"));
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some("🙂")
        );
    });
}

#[gpui::test]
fn async_native_replacement_commits_existing_ime_and_normalizes_ranges(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (entity, cx) = surface(cx, "🙂", 5, edits.clone());
    let paste = "日本\r\n🙂\r".repeat(12_000);
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx);
            state.replace_text_in_range(Some(2..4), &paste, window, cx);
            assert!(state.pending_edit.is_some());
            assert!(edits.borrow().is_empty());
        })
    });
    cx.run_until_parked();
    assert_eq!(edits.borrow().len(), 1);
    let mut session = EditorSession::new(EditorBuffer::from_text("🙂"));
    session.apply(edits.borrow()[0].clone());
    assert_eq!(
        session.buffer().text(),
        format!("🙂{}", normalize_newlines(&paste))
    );
    assert!(session.undo());
    assert_eq!(session.buffer().text(), "🙂");
    assert!(!session.undo());
}

#[gpui::test]
fn async_no_op_does_not_emit_edits_and_draft_cancel_does_not_create_history(
    cx: &mut TestAppContext,
) {
    let edits = Edits::default();
    let text = "日本🙂".repeat(20_000);
    let (entity, cx) = surface(cx, &text, 5, edits.clone());
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.replace_text_in_range(Some(0..text.encode_utf16().count()), &text, window, cx);
            state.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx);
            state.replace_and_mark_text_in_range(None, "", Some(0..0), window, cx);
            assert!(state.pending_edit.is_some());
        })
    });
    cx.run_until_parked();
    assert!(edits.borrow().is_empty());
    entity.update(cx, |state, _| {
        assert!(state.pending_edit.is_none());
        assert_eq!(state.edit.buffer.text(), text);
    });
}

#[gpui::test]
fn async_ime_unmark_keeps_one_commit_and_later_edit_keys_ordered(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (entity, cx) = surface(cx, "base", 5, edits.clone());
    let paste = "日本🙂".repeat(20_000);
    cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::default())];
            state.replace_text_in_range(None, &paste, window, cx);
            state.replace_and_mark_text_in_range(None, "日本", Some(2..2), window, cx);
            state.unmark_text(window, cx);
            state.key_down(&key("backspace"), window, cx);
            state.key_down(&key("enter"), window, cx);
            state.replace_text_in_range(None, "next", window, cx);
        })
    });
    cx.run_until_parked();
    assert_eq!(edits.borrow().len(), 5);
    entity.update(cx, |state, _| {
        assert_eq!(state.edit.buffer.text(), format!("{paste}日\nnextbase"))
    });
}

#[gpui::test]
fn closing_async_editor_cancels_pending_history_frames(cx: &mut TestAppContext) {
    init(cx);
    let (host, cx) = cx.add_window_view(|_, cx| {
        let buffer = EditorBuffer::from_text("base");
        AsyncControlledHarness {
            session: EditorSession::new(buffer.clone()),
            focus: cx.focus_handle(),
            editor: cx.new(|_| EditorSurface::new(CodeEditor::new("async-controlled", buffer))),
        }
    });
    let entity = host.read_with(cx, |host, _| host.editor.clone());
    let cancelled = cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.replace_text_in_range(None, &"日本🙂".repeat(20_000), window, cx);
            let shortcut = if cfg!(target_os = "macos") {
                "cmd-z"
            } else {
                "ctrl-z"
            };
            state.key_down(&key(shortcut), window, cx);
            state
                .pending_edit
                .as_ref()
                .expect("pending job")
                .cancelled
                .clone()
        })
    });
    cx.run_until_parked();
    assert!(entity.read_with(cx, |state, _| {
        state
            .pending_edit
            .as_ref()
            .expect("history barrier")
            .waiting_host
    }));
    drop(entity);
    drop(host);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
    assert!(cancelled.load(std::sync::atomic::Ordering::Relaxed));
}

#[derive(Default)]
struct ClipboardGate {
    value: Option<Option<String>>,
    waker: Option<std::task::Waker>,
}

fn delayed_clipboard(
    cx: &App,
) -> (
    gpui::Task<Option<String>>,
    std::sync::Arc<std::sync::Mutex<ClipboardGate>>,
) {
    let gate = std::sync::Arc::new(std::sync::Mutex::new(ClipboardGate::default()));
    let reader = gate.clone();
    let read = cx
        .background_executor()
        .spawn(std::future::poll_fn(move |cx| {
            let mut gate = reader.lock().expect("clipboard gate");
            if let Some(value) = gate.value.take() {
                std::task::Poll::Ready(value)
            } else {
                gate.waker = Some(cx.waker().clone());
                std::task::Poll::Pending
            }
        }));
    (read, gate)
}

fn finish_clipboard(gate: &std::sync::Mutex<ClipboardGate>, text: Option<&str>) {
    let mut gate = gate.lock().expect("clipboard gate");
    gate.value = Some(text.map(str::to_owned));
    if let Some(waker) = gate.waker.take() {
        waker.wake();
    }
}

#[gpui::test]
fn clipboard_transfer_wait_preserves_ime_order_and_cancellation(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (entity, cx) = surface(cx, "base", 3, edits.clone());
    let gate = cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::default())];
            let (read, gate) = delayed_clipboard(cx);
            state.enqueue_edit(Work::Paste { read, copied: None }, window, cx);
            state.replace_and_mark_text_in_range(None, "に", Some(1..1), window, cx);
            assert_eq!(state.marked_text_range(window, cx), Some(0..1));
            state.replace_text_in_range(None, "日本", window, cx);
            state.replace_text_in_range(None, "!", window, cx);
            gate
        })
    });
    cx.run_until_parked();
    assert!(edits.borrow().is_empty());
    finish_clipboard(&gate, Some("🙂\r\n"));
    cx.run_until_parked();
    assert_eq!(
        edits.borrow().last().expect("edit").buffer.text(),
        "🙂\n日本!base"
    );
    let mut session = EditorSession::new(EditorBuffer::from_text("base"));
    for edit in edits.borrow().iter() {
        session.apply(edit.clone());
    }
    for _ in 0..3 {
        assert!(session.undo());
    }
    assert_eq!(session.buffer().text(), "base");
    let gate = cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            let (read, gate) = delayed_clipboard(cx);
            state.enqueue_edit(Work::Paste { read, copied: None }, window, cx);
            let props = CodeEditor::new("test", EditorBuffer::from_text("replacement"))
                .focusable(state.props.focus_handle.clone().expect("focus"))
                .on_edit(|_, _, _| panic!("obsolete clipboard result"));
            state.configure(props, window, cx);
            gate
        })
    });
    finish_clipboard(&gate, Some("obsolete"));
    cx.run_until_parked();
    entity.update(cx, |state, _| {
        assert_eq!(state.edit.buffer.text(), "replacement")
    });
}

#[gpui::test]
fn empty_clipboard_keeps_selection_and_releases_following_input(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (entity, cx) = surface(cx, "hello", 3, edits.clone());
    let gate = cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.edit.selections = vec![EditorSelection::new(
                EditorPosition::new(0, 1),
                EditorPosition::new(0, 4),
            )];
            let (read, gate) = delayed_clipboard(cx);
            state.enqueue_edit(Work::Paste { read, copied: None }, window, cx);
            state.replace_text_in_range(None, "!", window, cx);
            gate
        })
    });
    cx.run_until_parked();
    finish_clipboard(&gate, None);
    cx.run_until_parked();
    assert_eq!(edits.borrow().len(), 1);
    assert_eq!(edits.borrow()[0].buffer.text(), "h!o");
}

#[gpui::test]
fn pointer_selection_is_visible_during_transfer_and_rebased_before_typing(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (entity, cx) = surface(cx, "one two three", 3, edits.clone());
    let gate = cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::default())];
            let (read, gate) = delayed_clipboard(cx);
            state.enqueue_edit(Work::Paste { read, copied: None }, window, cx);
            let row = &state.layout.as_ref().expect("viewport").rows[0];
            let position = row.origin + point(row.x_for_column(5), px(5.0));
            state.mouse_down(
                &MouseDownEvent {
                    position,
                    click_count: 2,
                    ..Default::default()
                },
                window,
                cx,
            );
            assert_eq!(
                state.pointer_selection(),
                EditorSelection::new(EditorPosition::new(0, 4), EditorPosition::new(0, 7))
            );
            let bounds = state.layout.as_ref().expect("viewport").bounds;
            let viewport = state.prepare(bounds, window, cx);
            assert!(!viewport.selections.is_empty());
            assert_eq!(state.edit.selections[0].head, EditorPosition::default());
            state.replace_text_in_range(None, "日本", window, cx);
            gate
        })
    });
    cx.run_until_parked();
    assert!(edits.borrow().is_empty());
    finish_clipboard(&gate, Some("🙂\n"));
    cx.run_until_parked();
    assert_eq!(
        edits.borrow().last().expect("edit").buffer.text(),
        "🙂\none 日本 three"
    );
    entity.update(cx, |state, _| assert!(state.pending_edit.is_none()));
}

#[gpui::test]
fn pending_drag_rebases_across_multiple_paste_ranges(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (entity, cx) = surface(cx, "abcdef", 3, edits.clone());
    let gate = cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.edit.selections = vec![
                EditorSelection::cursor(EditorPosition::new(0, 0)),
                EditorSelection::cursor(EditorPosition::new(0, 6)),
            ];
            let (read, gate) = delayed_clipboard(cx);
            state.enqueue_edit(Work::Paste { read, copied: None }, window, cx);
            let row = &state.layout.as_ref().expect("viewport").rows[0];
            let start = row.origin + point(row.x_for_column(1), px(5.0));
            let end = row.origin + point(row.x_for_column(4), px(5.0));
            state.mouse_down(
                &MouseDownEvent {
                    position: start,
                    click_count: 1,
                    ..Default::default()
                },
                window,
                cx,
            );
            state.mouse_move(
                &MouseMoveEvent {
                    position: end,
                    pressed_button: Some(MouseButton::Left),
                    ..Default::default()
                },
                window,
                cx,
            );
            state.mouse_up(&MouseUpEvent::default(), window, cx);
            assert_eq!(
                state.pointer_selection(),
                EditorSelection::new(EditorPosition::new(0, 1), EditorPosition::new(0, 4))
            );
            assert_eq!(state.pending_edit.as_ref().expect("pending").queue.len(), 1);
            state.replace_text_in_range(None, "X", window, cx);
            gate
        })
    });
    cx.run_until_parked();
    finish_clipboard(&gate, Some("🙂"));
    cx.run_until_parked();
    assert_eq!(
        edits.borrow().last().expect("edit").buffer.text(),
        "🙂aXef🙂"
    );
}

#[gpui::test]
fn clipboard_reads_complete_out_of_order_but_edits_keep_input_order(cx: &mut TestAppContext) {
    let edits = Edits::default();
    let (entity, cx) = surface(cx, "base", 3, edits.clone());
    let (first, second) = cx.update(|window, cx| {
        entity.update(cx, |state, cx| {
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::default())];
            let (read, first) = delayed_clipboard(cx);
            state.enqueue_edit(Work::Paste { read, copied: None }, window, cx);
            let (read, second) = delayed_clipboard(cx);
            state.enqueue_edit(Work::Paste { read, copied: None }, window, cx);
            state.replace_text_in_range(None, "!", window, cx);
            (first, second)
        })
    });
    finish_clipboard(&second, Some("B日"));
    cx.run_until_parked();
    assert!(edits.borrow().is_empty());
    finish_clipboard(&first, Some("A🙂"));
    cx.run_until_parked();
    let mut session = EditorSession::new(EditorBuffer::from_text("base"));
    for edit in edits.borrow().iter() {
        session.apply(edit.clone());
    }
    assert_eq!(session.buffer().text(), "A🙂B日!base");
    for _ in 0..3 {
        assert!(session.undo());
    }
    assert_eq!(session.buffer().text(), "base");
    for _ in 0..3 {
        assert!(session.redo());
    }
    assert_eq!(session.buffer().text(), "A🙂B日!base");
}

#[gpui::test]
fn pointer_after_queued_history_tracks_unicode_snapshot_positions(cx: &mut TestAppContext) {
    init(cx);
    for redo in [false, true] {
        let (host, cx) = cx.add_window_view(|_, cx| {
            let buffer = EditorBuffer::from_text("日本🙂tail");
            AsyncControlledHarness {
                session: EditorSession::new(buffer.clone()),
                focus: cx.focus_handle(),
                editor: cx.new(|_| EditorSurface::new(CodeEditor::new("async-controlled", buffer))),
            }
        });
        let entity = host.read_with(cx, |host, _| host.editor.clone());
        let gate = cx.update(|window, cx| {
            entity.update(cx, |state, cx| {
                let (read, gate) = delayed_clipboard(cx);
                state.enqueue_edit(Work::Paste { read, copied: None }, window, cx);
                let modifier = if cfg!(target_os = "macos") {
                    "cmd"
                } else {
                    "ctrl"
                };
                state.key_down(&key(&format!("{modifier}-z")), window, cx);
                if redo {
                    state.key_down(&key(&format!("{modifier}-shift-z")), window, cx);
                }
                state.select_pointer(
                    EditorSelection::cursor(EditorPosition::new(0, 2)),
                    window,
                    cx,
                );
                state.replace_text_in_range(None, "!", window, cx);
                gate
            })
        });
        finish_clipboard(&gate, Some("先🙂\n"));
        for _ in 0..12 {
            cx.run_until_parked();
            cx.update(|window, cx| {
                window.simulate_next_frame(cx);
                window.draw(cx).clear(cx);
            });
        }
        cx.run_until_parked();
        let expected = if redo {
            "先🙂\n日本!🙂tail"
        } else {
            "日本!🙂tail"
        };
        host.update(cx, |host, _| {
            assert_eq!(host.session.buffer().text(), expected)
        });
        entity.update(cx, |state, _| {
            assert!(state.pending_edit.is_none());
            assert_eq!(state.edit.buffer.text(), expected);
        });
    }
}

#[gpui::test]
fn stationary_drag_continues_scrolling_until_release(cx: &mut TestAppContext) {
    let text = (0..200).map(|n| format!("line {n}\n")).collect::<String>();
    let (state, cx) = surface(cx, &text, 4, Edits::default());
    let body = state.read_with(cx, |state, _| state.layout.as_ref().expect("layout").body);
    let start = body.origin + point(px(5.0), px(10.0));
    let outside = point(start.x, body.bottom() + px(30.0));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    let before = state.read_with(cx, |state, _| state.scroll.y);
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(32));
    cx.run_until_parked();
    let after = state.read_with(cx, |state, _| state.scroll.y);
    assert!(after > before);
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(64));
    cx.run_until_parked();
    state.read_with(cx, |state, _| {
        assert_eq!(state.scroll.y, after);
        assert!(state.drag_task.is_none());
    });
}

#[gpui::test]
fn cursor_blink_resets_for_input_and_stays_visible_during_composition(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "abc", 3, Edits::default());
    focus(&state, cx);
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    assert!(!state.read_with(cx, |state, _| state.caret_visible));
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.replace_and_mark_text_in_range(None, "n", Some(1..1), window, cx);
            assert!(state.caret_visible);
        })
    });
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    assert!(state.read_with(cx, |state, _| state.caret_visible));
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.replace_text_in_range(None, "new", window, cx);
            let mut props = state.props.clone();
            props.options = props.options.cursor_blink(false);
            state.configure(props, window, cx);
            assert!(state.caret_task.is_none());
        })
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    cx.run_until_parked();
    assert!(state.read_with(cx, |state, _| state.caret_visible));
}

#[gpui::test]
fn visible_secondary_cursors_are_drawn_and_blur_stops_blinking(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "abc\ndef", 3, Edits::default());
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    focus(&state, cx);
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.apply(
                EditorEdit::new(
                    state.edit.buffer.clone(),
                    vec![
                        EditorSelection::cursor(EditorPosition::new(0, 1)),
                        EditorSelection::cursor(EditorPosition::new(1, 2)),
                    ],
                ),
                window,
                cx,
            );
            let bounds = state.layout.as_ref().expect("layout").bounds;
            let viewport = state.prepare(bounds, window, cx);
            assert!(viewport.caret.is_some());
            assert_eq!(viewport.secondary_carets.len(), 1);
            assert!(
                viewport.secondary_carets[0].top() > viewport.caret.expect("primary cursor").top()
            );
        })
    });
    cx.update(|window, _| window.blur());
    cx.run_until_parked();
    state.read_with(cx, |state, _| {
        assert!(state.caret_task.is_none());
        assert!(state.caret_visible);
    });
}

fn rectangle_modifiers() -> Modifiers {
    Modifiers {
        alt: true,
        shift: true,
        ..Modifiers::none()
    }
}

fn row_point(state: &EditorSurface, visual: usize, column: usize) -> Point<Pixels> {
    let row = state
        .layout
        .as_ref()
        .expect("layout")
        .rows
        .iter()
        .find(|row| row.visual == visual && row.columns.contains(&column))
        .expect("visible column");
    row.origin + point(row.x_for_column(column), px(LINE_HEIGHT / 2.0))
}

#[gpui::test]
fn rectangle_selects_shaped_rows_and_preserves_short_row_width(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "abcdef\nx\n012345", 5, Edits::default());
    let (start, end, short) = state.read_with(cx, |state, _| {
        (
            row_point(state, 0, 4),
            row_point(state, 2, 1),
            row_point(state, 0, 4) + point(px(0.0), px(LINE_HEIGHT)),
        )
    });
    cx.simulate_mouse_down(start, MouseButton::Left, rectangle_modifiers());
    cx.simulate_mouse_move(end, MouseButton::Left, rectangle_modifiers());
    cx.simulate_mouse_up(end, MouseButton::Left, rectangle_modifiers());
    state.read_with(cx, |state, _| {
        assert_eq!(
            state.edit.selections,
            vec![
                EditorSelection::new(EditorPosition::new(0, 4), EditorPosition::new(0, 1)),
                EditorSelection::cursor(EditorPosition::new(1, 1)),
                EditorSelection::new(EditorPosition::new(2, 4), EditorPosition::new(2, 1)),
            ]
        );
        assert!(state.rectangle_anchor.is_none());
    });
    // Starting past a short row must not collapse the span on longer rows.
    cx.simulate_mouse_down(short, MouseButton::Left, rectangle_modifiers());
    cx.simulate_mouse_move(end, MouseButton::Left, rectangle_modifiers());
    cx.simulate_mouse_up(end, MouseButton::Left, rectangle_modifiers());
    state.read_with(cx, |state, _| {
        assert_eq!(state.edit.selections.len(), 2);
        assert_eq!(
            state.edit.selections[1].ordered(),
            EditorPosition::new(2, 1)..EditorPosition::new(2, 4)
        );
    });
}

#[gpui::test]
fn rectangle_replacement_copy_cut_and_history_preserve_all_ranges(cx: &mut TestAppContext) {
    let original = "abcdef\n012345\nuvwxyz";
    let edits = Edits::default();
    let (state, cx) = surface(cx, original, 5, edits.clone());
    let (start, end) = state.read_with(cx, |state, _| {
        (row_point(state, 0, 1), row_point(state, 2, 3))
    });
    cx.simulate_mouse_down(start, MouseButton::Left, rectangle_modifiers());
    cx.simulate_mouse_move(end, MouseButton::Left, rectangle_modifiers());
    cx.simulate_mouse_up(end, MouseButton::Left, rectangle_modifiers());
    let selected = state.read_with(cx, |state, _| state.edit.clone());
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.key_down(
                &key(if cfg!(target_os = "macos") {
                    "cmd-c"
                } else {
                    "ctrl-c"
                }),
                window,
                cx,
            );
            assert_eq!(
                cx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .as_deref(),
                Some("bc\n12\nvw")
            );
            state.replace_text_in_range(None, "日本", window, cx);
            assert_eq!(state.edit.buffer.text(), "a日本def\n0日本345\nu日本xyz");
            assert_eq!(state.edit.selections.len(), 3);
        })
    });
    let replaced = state.read_with(cx, |state, _| state.edit.clone());
    let mut session = EditorSession::new(EditorBuffer::from_text(original));
    session.apply(selected.clone());
    session.apply(replaced.clone());
    assert!(session.undo());
    assert_eq!(session.buffer().text(), original);
    assert_eq!(session.selections(), selected.selections);
    assert!(session.redo());
    assert_eq!(session.buffer().text(), replaced.buffer.text());
    assert_eq!(session.selections(), replaced.selections);
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.apply(selected, window, cx);
            state.key_down(
                &key(if cfg!(target_os = "macos") {
                    "cmd-x"
                } else {
                    "ctrl-x"
                }),
                window,
                cx,
            );
            assert_eq!(state.edit.buffer.text(), "adef\n0345\nuxyz");
        })
    });
}

#[gpui::test]
fn rectangle_selection_is_rebased_while_clipboard_read_is_pending(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, "abcdef\n012345\nuvwxyz", 5, Edits::default());
    let (start, end) = state.read_with(cx, |state, _| {
        (row_point(state, 0, 1), row_point(state, 2, 3))
    });
    let gate = cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.edit.selections = vec![EditorSelection::cursor(EditorPosition::default())];
            let (read, gate) = delayed_clipboard(cx);
            state.enqueue_edit(Work::Paste { read, copied: None }, window, cx);
            gate
        })
    });
    cx.simulate_mouse_down(start, MouseButton::Left, rectangle_modifiers());
    cx.simulate_mouse_move(end, MouseButton::Left, rectangle_modifiers());
    cx.simulate_mouse_up(end, MouseButton::Left, rectangle_modifiers());
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            assert_eq!(state.edit.selections.len(), 1);
            assert_eq!(state.pointer_selections().len(), 3);
            let bounds = state.layout.as_ref().expect("layout").bounds;
            assert_eq!(state.prepare(bounds, window, cx).selections.len(), 3);
            state.replace_text_in_range(None, "X", window, cx);
        })
    });
    finish_clipboard(&gate, Some("🙂\n"));
    cx.run_until_parked();
    state.read_with(cx, |state, _| {
        assert_eq!(state.edit.buffer.text(), "🙂\naXdef\n0X345\nuXxyz");
        assert_eq!(state.edit.selections.len(), 3);
    });
}

#[gpui::test]
fn rectangle_wraps_by_display_row_without_splitting_graphemes(cx: &mut TestAppContext) {
    let text = "a👩‍💻e\u{301}\t日本 z ".repeat(20);
    let (state, cx) = surface(cx, &text, 5, Edits::default());
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            state.pending_first_line = Some(0);
            let viewport = prepare_at(state, 240.0, 132.0, window, cx);
            let start = viewport.body.origin + point(px(12.0), px(10.0));
            let end = start + point(px(47.0), px(LINE_HEIGHT * 3.0));
            let expected: Vec<_> = (0..=3)
                .map(|visual| {
                    let row = state.shape_x(visual, px(12.0), &viewport.font, window, cx);
                    let right = state.shape_x(visual, px(59.0), &viewport.font, window, cx);
                    EditorSelection::new(
                        EditorPosition::new(row.index, row.column_for_x(px(12.0))),
                        EditorPosition::new(right.index, right.column_for_x(px(59.0))),
                    )
                })
                .collect();
            state.mouse_down(
                &MouseDownEvent {
                    position: start,
                    modifiers: rectangle_modifiers(),
                    ..Default::default()
                },
                window,
                cx,
            );
            state.mouse_move(
                &MouseMoveEvent {
                    position: end,
                    pressed_button: Some(MouseButton::Left),
                    modifiers: rectangle_modifiers(),
                },
                window,
                cx,
            );
            assert_eq!(state.edit.selections, expected);
            assert_eq!(state.edit.selections.len(), 4);
            for (visual, selection) in state.edit.selections.iter().enumerate() {
                let (_, fragment) = state.wrap.as_ref().expect("wrap").fragment(visual);
                assert!(selection.ordered().start.column >= fragment.columns.start);
                assert!(selection.ordered().end.column <= fragment.columns.end);
                assert_eq!(selection.anchor.line, 0);
                assert_eq!(selection.head.line, 0);
            }
            state.mouse_up(&MouseUpEvent::default(), window, cx);
            state.replace_text_in_range(None, "Z", window, cx);
            assert_eq!(state.edit.selections.len(), 4);
            assert_eq!(state.edit.buffer.text().matches('Z').count(), 4);
        })
    });
}

#[gpui::test]
fn zero_width_rectangle_creates_cursors_and_stops_scrolling_on_release(cx: &mut TestAppContext) {
    let text = "a👩‍💻e\u{301}z\n".repeat(200);
    let (state, cx) = surface(cx, &text, 4, Edits::default());
    let (start, outside) = state.read_with(cx, |state, _| {
        let start = row_point(state, 0, 2);
        (
            start,
            point(
                start.x,
                state.layout.as_ref().expect("layout").body.bottom() + px(30.0),
            ),
        )
    });
    cx.simulate_mouse_down(start, MouseButton::Left, rectangle_modifiers());
    cx.simulate_mouse_move(outside, MouseButton::Left, rectangle_modifiers());
    cx.run_until_parked();
    let before = state.read_with(cx, |state, _| state.edit.selections.len());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(32));
    cx.run_until_parked();
    let (after, scroll) = state.read_with(cx, |state, _| {
        assert!(
            state
                .edit
                .selections
                .iter()
                .all(|selection| selection.is_empty() && selection.head.column == 2)
        );
        (state.edit.selections.len(), state.scroll.y)
    });
    assert!(after > before);
    cx.simulate_mouse_up(outside, MouseButton::Left, rectangle_modifiers());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(64));
    cx.run_until_parked();
    state.read_with(cx, |state, _| {
        assert_eq!(state.scroll.y, scroll);
        assert!(state.rectangle_anchor.is_none());
        assert!(state.drag_task.is_none());
    });
}

#[gpui::test]
fn rectangle_ime_commits_primary_once_and_undo_restores_the_block(cx: &mut TestAppContext) {
    let original = "abcdef\n012345";
    let edits = Edits::default();
    let (state, cx) = surface(cx, original, 4, edits.clone());
    let (start, end) = state.read_with(cx, |state, _| {
        (row_point(state, 0, 1), row_point(state, 1, 3))
    });
    cx.simulate_mouse_down(start, MouseButton::Left, rectangle_modifiers());
    cx.simulate_mouse_move(end, MouseButton::Left, rectangle_modifiers());
    cx.simulate_mouse_up(end, MouseButton::Left, rectangle_modifiers());
    let selected = state.read_with(cx, |state, _| state.edit.clone());
    edits.borrow_mut().clear();
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.replace_and_mark_text_in_range(None, "ni", Some(2..2), window, cx);
            state.replace_and_mark_text_in_range(None, "日本", Some(2..2), window, cx);
            assert!(edits.borrow().is_empty());
            state.replace_text_in_range(None, "日本語", window, cx);
            assert_eq!(state.edit.buffer.text(), "a日本語def\n012345");
        })
    });
    assert_eq!(edits.borrow().len(), 1);
    let mut session = EditorSession::new(EditorBuffer::from_text(original));
    session.apply(selected.clone());
    session.apply(edits.borrow()[0].clone());
    assert!(session.undo());
    assert_eq!(session.buffer().text(), original);
    assert_eq!(session.selections(), selected.selections);
}

#[gpui::test]
fn rectangle_ranges_follow_queued_undo_and_redo(cx: &mut TestAppContext) {
    init(cx);
    for redo in [false, true] {
        let (host, cx) = cx.add_window_view(|_, cx| {
            let buffer = EditorBuffer::from_text("日本🙂tail\nabcdef");
            AsyncControlledHarness {
                session: EditorSession::new(buffer.clone()),
                focus: cx.focus_handle(),
                editor: cx.new(|_| EditorSurface::new(CodeEditor::new("async-controlled", buffer))),
            }
        });
        let entity = host.read_with(cx, |host, _| host.editor.clone());
        let gate = cx.update(|window, cx| {
            entity.update(cx, |state, cx| {
                let (read, gate) = delayed_clipboard(cx);
                state.enqueue_edit(Work::Paste { read, copied: None }, window, cx);
                let modifier = if cfg!(target_os = "macos") {
                    "cmd"
                } else {
                    "ctrl"
                };
                state.key_down(&key(&format!("{modifier}-z")), window, cx);
                if redo {
                    state.key_down(&key(&format!("{modifier}-shift-z")), window, cx);
                }
                state.select_pointers(
                    vec![
                        EditorSelection::new(EditorPosition::new(0, 1), EditorPosition::new(0, 3)),
                        EditorSelection::new(EditorPosition::new(1, 1), EditorPosition::new(1, 3)),
                    ],
                    window,
                    cx,
                );
                state.replace_text_in_range(None, "!", window, cx);
                gate
            })
        });
        finish_clipboard(&gate, Some("先🙂\n"));
        for _ in 0..12 {
            cx.run_until_parked();
            cx.update(|window, cx| {
                window.simulate_next_frame(cx);
                window.draw(cx).clear(cx);
            });
        }
        cx.run_until_parked();
        let expected = if redo {
            "先🙂\n日!tail\na!def"
        } else {
            "日!tail\na!def"
        };
        host.read_with(cx, |host, _| {
            assert_eq!(host.session.buffer().text(), expected);
            assert_eq!(host.session.selections().len(), 2);
        });
        entity.read_with(cx, |state, _| assert!(state.pending_edit.is_none()));
    }
}

#[gpui::test]
fn rectangle_cursors_at_wrap_ends_stay_on_the_selected_display_rows(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, &"word longerword ".repeat(60), 5, Edits::default());
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            state.pending_first_line = Some(0);
            let viewport = prepare_at(state, 240.0, 132.0, window, cx);
            let start = point(
                viewport.body.right() - px(1.0),
                viewport.body.top() + px(10.0),
            );
            let end = start + point(px(0.0), px(2.0 * LINE_HEIGHT));
            state.mouse_down(
                &MouseDownEvent {
                    position: start,
                    modifiers: rectangle_modifiers(),
                    ..Default::default()
                },
                window,
                cx,
            );
            state.mouse_move(
                &MouseMoveEvent {
                    position: end,
                    pressed_button: Some(MouseButton::Left),
                    modifiers: rectangle_modifiers(),
                },
                window,
                cx,
            );
            state.mouse_up(&MouseUpEvent::default(), window, cx);
            assert_eq!(state.edit.selections.len(), 3);
            assert!(state.edit.selections.iter().all(EditorSelection::is_empty));
            let viewport = prepare_at(state, 240.0, 132.0, window, cx);
            let caret = viewport.caret.expect("primary wrap-end cursor");
            assert_eq!(caret.top(), viewport.body.top());
            assert_eq!(viewport.secondary_carets.len(), 2);
            assert_eq!(
                viewport.secondary_carets[0].top(),
                viewport.body.top() + px(LINE_HEIGHT)
            );
            assert_eq!(
                viewport.secondary_carets[1].top(),
                viewport.body.top() + px(2.0 * LINE_HEIGHT)
            );
        })
    });
}

#[gpui::test]
fn rectangle_drag_ends_on_blur_and_host_replacement(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, &"abcdef\n".repeat(100), 4, Edits::default());
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let (start, end) = state.read_with(cx, |state, _| {
        (row_point(state, 0, 1), row_point(state, 2, 3))
    });
    cx.simulate_mouse_down(start, MouseButton::Left, rectangle_modifiers());
    cx.simulate_mouse_move(end, MouseButton::Left, rectangle_modifiers());
    cx.update(|window, _| window.blur());
    cx.run_until_parked();
    state.read_with(cx, |state, _| {
        assert!(!state.selecting);
        assert!(state.rectangle_anchor.is_none());
    });
    cx.simulate_mouse_down(start, MouseButton::Left, rectangle_modifiers());
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            let mut props = state.props.clone();
            props.buffer = EditorBuffer::from_text("new");
            state.configure(props, window, cx);
            assert!(!state.selecting);
            assert!(state.rectangle_anchor.is_none());
            assert!(state.rectangle_heads.is_empty());
        })
    });
}

#[gpui::test]
fn rectangle_remeasures_cached_rows_when_the_window_width_changes(cx: &mut TestAppContext) {
    let (state, cx) = surface(cx, &"word longerword ".repeat(80), 5, Edits::default());
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.props.options.soft_wrap = true;
            state.pending_first_line = Some(0);
            let viewport = prepare_at(state, 360.0, 132.0, window, cx);
            let start = viewport.body.origin + point(px(12.0), px(10.0));
            let end = start + point(px(70.0), px(2.0 * LINE_HEIGHT));
            state.mouse_down(
                &MouseDownEvent {
                    position: start,
                    modifiers: rectangle_modifiers(),
                    ..Default::default()
                },
                window,
                cx,
            );
            state.extend_rectangle(end, window, cx);
            let wide = state.edit.selections.clone();
            prepare_at(state, 220.0, 132.0, window, cx);
            state.extend_rectangle(end, window, cx);
            assert_eq!(state.edit.selections.len(), 3);
            assert_ne!(state.edit.selections[1], wide[1]);
            for (visual, selection) in state.edit.selections.iter().enumerate() {
                let (_, fragment) = state.wrap.as_ref().expect("wrap").fragment(visual);
                assert!(selection.ordered().start.column >= fragment.columns.start);
                assert!(selection.ordered().end.column <= fragment.columns.end);
            }
        })
    });
}

#[gpui::test]
fn rectangle_hits_long_lines_after_horizontal_scrolling(cx: &mut TestAppContext) {
    let text = (0..3)
        .map(|_| "abcdefghij".repeat(2000))
        .collect::<Vec<_>>()
        .join("\n");
    let (state, cx) = surface(cx, &text, 5, Edits::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.scroll.x = px(1200.0);
            state.reveal = false;
            let viewport = prepare_at(state, 300.0, 132.0, window, cx);
            assert!(state.scroll.x > px(0.0));
            let start = viewport.body.origin + point(px(12.0), px(10.0));
            let end = start + point(px(45.0), px(2.0 * LINE_HEIGHT));
            state.mouse_down(
                &MouseDownEvent {
                    position: start,
                    modifiers: rectangle_modifiers(),
                    ..Default::default()
                },
                window,
                cx,
            );
            state.extend_rectangle(end, window, cx);
            assert_eq!(state.edit.selections.len(), 3);
            let expected = state.edit.selections[0].ordered();
            assert!(expected.start.column > 100);
            assert!(expected.end.column > expected.start.column);
            for selection in &state.edit.selections {
                assert_eq!(selection.ordered().start.column, expected.start.column);
                assert_eq!(selection.ordered().end.column, expected.end.column);
            }
            state.mouse_up(&MouseUpEvent::default(), window, cx);
        })
    });
}

#[gpui::test]
fn large_rectangle_edits_use_the_background_queue_and_keep_input_order(cx: &mut TestAppContext) {
    let original = vec!["abc"; 256].join("\n");
    let edits = Edits::default();
    let (state, cx) = surface(cx, &original, 5, edits.clone());
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.select_pointers(
                (0..256)
                    .map(|line| {
                        EditorSelection::new(
                            EditorPosition::new(line, 0),
                            EditorPosition::new(line, 1),
                        )
                    })
                    .collect(),
                window,
                cx,
            );
            edits.borrow_mut().clear();
            state.replace_text_in_range(None, "X", window, cx);
            assert!(state.pending_edit.is_some());
            assert_eq!(state.edit.buffer.text(), original);
            state.key_down(&key("backspace"), window, cx);
            assert!(edits.borrow().is_empty());
        })
    });
    cx.run_until_parked();
    state.read_with(cx, |state, _| {
        assert_eq!(state.edit.buffer.text(), vec!["bc"; 256].join("\n"));
        assert_eq!(state.edit.selections.len(), 256);
        assert!(state.pending_edit.is_none());
    });
    assert_eq!(edits.borrow().len(), 2);
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.key_down(&key("delete"), window, cx);
            assert!(state.pending_edit.is_some());
            assert_eq!(state.edit.buffer.text(), vec!["bc"; 256].join("\n"));
            state.replace_text_in_range(None, "Q", window, cx);
        })
    });
    cx.run_until_parked();
    state.read_with(cx, |state, _| {
        assert_eq!(state.edit.buffer.text(), vec!["Qc"; 256].join("\n"));
        assert_eq!(state.edit.selections.len(), 256);
    });
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.select_pointers(
                (0..256)
                    .map(|line| {
                        EditorSelection::new(
                            EditorPosition::new(line, 0),
                            EditorPosition::new(line, 1),
                        )
                    })
                    .collect(),
                window,
                cx,
            );
            state.key_down(
                &key(if cfg!(target_os = "macos") {
                    "cmd-c"
                } else {
                    "ctrl-c"
                }),
                window,
                cx,
            );
            assert!(state.pending_edit.is_some());
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        state.update(cx, |state, cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some(vec!["Q"; 256].join("\n"))
            );
            assert_eq!(state.edit.buffer.text(), vec!["Qc"; 256].join("\n"));
            state.key_down(
                &key(if cfg!(target_os = "macos") {
                    "cmd-x"
                } else {
                    "ctrl-x"
                }),
                window,
                cx,
            );
            assert!(state.pending_edit.is_some());
            state.replace_text_in_range(None, "!", window, cx);
        })
    });
    cx.run_until_parked();
    state.read_with(cx, |state, _| {
        assert_eq!(state.edit.buffer.text(), vec!["!c"; 256].join("\n"));
        assert_eq!(state.edit.selections.len(), 256);
        assert!(state.pending_edit.is_none());
    });
}
