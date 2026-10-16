use gpui::{
    App, AppContext as _, Context, Global, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Window, div,
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
    router: Rc<RefCell<CommandRouter>>,
    checked: bool,
    command_count: usize,
    _activation: ActivationOwner,
    _quit: gpui::Subscription,
    tasks: Vec<gpui::Task<()>>,
}
impl Global for Owner {}
struct Shell {
    focus: gpui::FocusHandle,
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
            .track_focus(&self.focus)
            .on_key_down(|event, _, cx| {
                let command = cx.global::<Owner>().router.borrow()
                    .resolve(&event.keystroke, &[]).cloned();
                if let Some(command) = command {
                    run_command(&command, cx);
                    cx.stop_propagation();
                }
            })
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
                "Checked example: {}; command executions: {}. Ctrl+Shift+T toggles; Ctrl+Shift+D is disabled.",
                cx.global::<Owner>().checked, cx.global::<Owner>().command_count
            )))
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
        let view = cx.new(|cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            Shell {
                focus,
                state,
                menu,
                status,
                dirty,
                open: None,
            }
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
fn application_menu(checked: bool) -> Vec<NativeMenuItem> {
    vec![NativeMenuItem::Submenu {
        label: "Application".into(),
        items: vec![
            command("show", "Show window"),
            NativeMenuItem::Command(NativeCommand {
                id: "toggle".into(),
                label: "Checked example (Ctrl+Shift+T)".into(),
                enabled: true,
                checked: Some(checked),
            }),
            NativeMenuItem::Command(NativeCommand {
                id: "disabled".into(),
                label: "Disabled example (Ctrl+Shift+D)".into(),
                enabled: false,
                checked: None,
            }),
            NativeMenuItem::Separator,
            command("restart", "Restart"),
            command("quit", "Quit"),
        ],
    }]
}
fn run_command(command: &CommandSpec, cx: &mut App) {
    let lifecycle = cx.global::<Owner>().state.borrow().lifecycle.clone();
    match command.id.as_ref() {
        "show" => {
            if let Err(error) = open_main(cx) {
                eprintln!("Open failed: {error}");
            }
        }
        "quit" => lifecycle.request(ShutdownReason::Quit, cx),
        "restart" => lifecycle.request(ShutdownReason::Restart, cx),
        "toggle" => {
            let owner = cx.global_mut::<Owner>();
            owner.checked = !owner.checked;
            owner.command_count += 1;
            let items = application_menu(owner.checked);
            let menu = owner.menu.clone();
            cx.spawn(async move |_| {
                if let Err(error) = menu.set_menu(items).await {
                    eprintln!("Could not update menu: {error}");
                }
            })
            .detach();
            cx.refresh_windows();
        }
        "disabled" => {
            // A visible sentinel makes accidental dispatch observable during smoke tests.
            *cx.global::<Owner>().status.borrow_mut() = "ERROR: disabled command executed".into();
            cx.refresh_windows();
        }
        _ => {}
    }
}
fn main() -> anyhow::Result<()> {
    // GPUI 0.4's DirectComposition windows do not provide the GDI surface used
    // by native Win32 menubars. Select its supported swap-chain fallback before
    // initializing the platform, while respecting an explicit user override.
    #[cfg(target_os = "windows")]
    if std::env::var_os("GPUI_DISABLE_DIRECT_COMPOSITION").is_none() {
        // SAFETY: this runs at startup before this program creates any threads.
        unsafe { std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "1") };
    }
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
            let dirty = guard_dirty.clone();
            Box::pin(async move {
                Ok(if let Some(prompt) = prompt {
                    if prompt.await.unwrap_or(0) == 1 {
                        dirty.set(false);
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
            .set_menu(application_menu(false), cx)
            .expect("application menu");
        // This existing repository image is also used by the packaging example.
        if let Err(error) = menus.set_tray(TrayDefinition {
            id: "main".into(),
            label: "GUIC Desktop Shell".into(),
            icon: Some(include_bytes!("../../assets_demo/packaging/icon.png").to_vec()),
            menu: vec![command("show", "Show window"), command("quit", "Quit")],
        }) {
            eprintln!("Tray unavailable in this desktop session: {error}");
        } else {
            // Windows and Linux otherwise quit when the last window closes.
            // Keep the process alive only when a tray can reopen its window.
            cx.set_quit_mode(gpui::QuitMode::Explicit);
        }
        let menu = menus.handle();
        let tray = menu.clone();
        let tray_clicks = cx.spawn(async move |cx| {
            while let Ok(Some(click)) = tray.next_tray_click().await {
                if click.button == guic::native::TrayMouseButton::Left {
                    cx.update(|cx| {
                        if let Err(error) = open_main(cx) {
                            eprintln!("Open from tray failed: {error}");
                        }
                    });
                }
            }
        });
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
        router
            .register(
                CommandSpec::new("toggle", "Checked example")
                    .shortcut(gpui::Keystroke::parse("ctrl-shift-t").expect("toggle shortcut")),
            )
            .expect("register toggle");
        router
            .register(
                CommandSpec::new("disabled", "Disabled example")
                    .shortcut(gpui::Keystroke::parse("ctrl-shift-d").expect("disabled shortcut"))
                    .enabled(false),
            )
            .expect("register disabled");
        let router = Rc::new(RefCell::new(router));
        let dispatch = menu.dispatch(cx, router.clone(), |_| vec![], run_command);
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
            router,
            checked: false,
            command_count: 0,
            state,
            menu,
            status,
            dirty,
            _activation: activation,
            _quit: quit,
            tasks: vec![native_task, dispatch, delivery, tray_clicks],
        });
        open_main(cx).expect("main window");
    });
    Ok(())
}
