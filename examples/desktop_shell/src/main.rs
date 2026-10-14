use gpui::{
    App, AppContext as _, Context, Global, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};
use guic::{
    native::{
        ActivationOwner, ActivationRequest, DesktopMenuHandle, DesktopMenus, DesktopState,
        Instance, ShutdownDecision, ShutdownReason,
    },
    prelude::*,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

type State = Rc<RefCell<DesktopState>>;
struct Owner {
    state: State,
    menu: DesktopMenuHandle,
    status: Rc<RefCell<String>>,
    dirty: Rc<Cell<bool>>,
    _activation: ActivationOwner,
    _quit: gpui::Subscription,
    tasks: Vec<gpui::Task<()>>,
}
impl Global for Owner {}
struct Shell {
    state: State,
    menu: DesktopMenuHandle,
    status: Rc<RefCell<String>>,
    dirty: Rc<Cell<bool>>,
    open: Option<usize>,
}
impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.status.borrow().clone();
        let dirty = self.dirty.clone();
        let state = self.state.clone();
        let lifecycle = state.borrow().lifecycle.clone();
        let menu = self.menu.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .p_6()
            .gap_3()
            .bg(guic::tokens::Theme::global(cx).background())
            .child(
                Menubar::new("desktop-menu")
                    .native(&self.menu.menu())
                    .open(self.open)
                    .on_open(cx.listener(|this, open, _, cx| {
                        this.open = *open;
                        cx.notify();
                    }))
                    .on_activate(cx.listener(
                        move |this, event: &guic::components::MenubarActivation, _, cx| {
                            this.open = None;
                            if let Err(error) = menu.invoke(&event.item) {
                                *this.status.borrow_mut() = error.to_string();
                            }
                            cx.notify();
                        },
                    )),
            )
            .child(Label::new("GUIC desktop integration"))
            .child(Label::new(status))
            .child(Label::new(format!(
                "Lifecycle: {:?}; unsaved changes: {}",
                lifecycle.state(),
                self.dirty.get()
            )))
            .child(
                Button::new("Toggle unsaved changes").on_click(move |_, _, cx| {
                    dirty.set(!dirty.get());
                    cx.refresh_windows();
                }),
            )
            .child(
                Button::new("Cycle System / Light / Dark").on_click(move |_, window, cx| {
                    let next = match state.borrow().theme() {
                        ThemePreference::System => ThemePreference::Light,
                        ThemePreference::Light => ThemePreference::Dark,
                        ThemePreference::Dark => ThemePreference::System,
                    };
                    if let Err(error) = state.borrow_mut().set_theme(next, window, cx) {
                        eprintln!("Could not save theme: {error}");
                    }
                }),
            )
            .child(
                Button::new("Quit through confirmation")
                    .on_click(move |_, _, cx| lifecycle.request(ShutdownReason::Quit, cx)),
            )
    }
}
fn open_main(cx: &mut App) -> anyhow::Result<()> {
    if !cx.windows().is_empty() {
        guic::native::activate_window(cx);
        return Ok(());
    }
    let owner = cx.global::<Owner>();
    let state = owner.state.clone();
    let menu = owner.menu.clone();
    let status = owner.status.clone();
    let dirty = owner.dirty.clone();
    let options = state
        .borrow()
        .windows
        .get("main")?
        .map(|saved| saved.options(cx))
        .transpose()?
        .unwrap_or_default();
    let handle = cx.open_window(options, |window, cx| {
        let tracking = state.borrow().track_window("main", window, cx);
        cx.global_mut::<Owner>().tasks.push(tracking);
        let view = cx.new(|_| Shell {
            state,
            menu,
            status,
            dirty,
            open: None,
        });
        cx.new(|cx| Root::new(view, window, cx))
    })?;
    let attach = cx.global::<Owner>().menu.attach_window(handle.into());
    cx.spawn(async move |_| {
        if let Err(error) = attach.await {
            eprintln!("Could not attach menu: {error}");
        }
    })
    .detach();
    Ok(())
}
fn command(id: &str, label: &str) -> NativeMenuItem {
    NativeMenuItem::Command(NativeCommand {
        id: id.into(),
        label: label.into(),
        enabled: true,
        checked: None,
    })
}
fn main() -> anyhow::Result<()> {
    let state = DesktopState::open(guic::native::application_identity!()?)?;
    let Instance::Primary(activation) =
        Instance::acquire(&state.paths, ActivationRequest::current()?)?
    else {
        return Ok(());
    };
    let application = gpui_platform::application();
    activation.bind(&application);
    application.run(move |cx| {
        guic::init(cx);
        let quit = state.install(cx);
        let state = Rc::new(RefCell::new(state));
        let status = Rc::new(RefCell::new(
            "Ready. Launch again with arguments to test forwarding.".into(),
        ));
        let dirty = Rc::new(Cell::new(false));
        let guard_dirty = dirty.clone();
        state.borrow().lifecycle.guard(move |reason, cx| {
            if !guard_dirty.get() {
                return Box::pin(async { Ok(ShutdownDecision::Proceed) });
            }
            let handle = match reason {
                ShutdownReason::CloseWindow(handle) => Some(handle),
                _ => cx.active_window().or_else(|| cx.windows().first().copied()),
            };
            let prompt = handle.and_then(|handle| {
                handle
                    .update(cx, |_, window, cx| {
                        window.prompt(
                            gpui::PromptLevel::Warning,
                            "Discard unsaved example changes?",
                            None,
                            &["Keep working", "Discard"],
                            cx,
                        )
                    })
                    .ok()
            });
            Box::pin(async move {
                Ok(if let Some(prompt) = prompt {
                    if prompt.await.unwrap_or(0) == 1 {
                        ShutdownDecision::Proceed
                    } else {
                        ShutdownDecision::Cancel
                    }
                } else {
                    ShutdownDecision::Cancel
                })
            })
        });
        let mut menus = DesktopMenus::new().expect("desktop menu owner");
        menus
            .set_menu(
                vec![NativeMenuItem::Submenu {
                    label: "Application".into(),
                    items: vec![
                        command("show", "Show window"),
                        command("restart", "Restart"),
                        command("quit", "Quit"),
                    ],
                }],
                cx,
            )
            .expect("application menu");
        // This existing repository image is also used by the packaging example.
        if let Err(error) = menus.set_tray(TrayDefinition {
            id: "main".into(),
            label: "GUIC Desktop Shell".into(),
            icon: Some(include_bytes!("../../assets_demo/packaging/icon.png").to_vec()),
            menu: vec![command("show", "Show window"), command("quit", "Quit")],
        }) {
            eprintln!("Tray unavailable in this desktop session: {error}");
        }
        let menu = menus.handle();
        let mut router = CommandRouter::new();
        for (id, label) in [
            ("show", "Show window"),
            ("quit", "Quit"),
            ("restart", "Restart"),
        ] {
            router
                .register(CommandSpec::new(id, label))
                .expect("register command");
        }
        let lifecycle = state.borrow().lifecycle.clone();
        let dispatch = menu.dispatch(
            cx,
            Rc::new(RefCell::new(router)),
            |_| vec![],
            move |command, cx| match command.id.as_ref() {
                "show" => {
                    if let Err(error) = open_main(cx) {
                        eprintln!("Open failed: {error}");
                    }
                }
                "quit" => lifecycle.request(ShutdownReason::Quit, cx),
                "restart" => lifecycle.request(ShutdownReason::Restart, cx),
                _ => {}
            },
        );
        let activation_status = status.clone();
        let delivery = activation.dispatch(cx, move |request, cx| {
            *activation_status.borrow_mut() = format!(
                "Activation: {:?}; URLs: {:?}; cwd: {}",
                request.arguments,
                request.urls,
                request.working_directory.display()
            );
            if let Err(error) = open_main(cx) {
                eprintln!("Open failed: {error}");
            }
            cx.refresh_windows();
        });
        let native_task = menus.run(cx);
        cx.set_global(Owner {
            state,
            menu,
            status,
            dirty,
            _activation: activation,
            _quit: quit,
            tasks: vec![native_task, dispatch, delivery],
        });
        open_main(cx).expect("main window");
    });
    Ok(())
}
