use anyhow::{Result, ensure};
use async_channel::{Receiver, Sender};
use gpui::{App, Task};
use guic_core::{
    ApplicationMenuService, CommandRouter, NativeMenuItem, ServiceError, ServiceErrorKind,
    ServiceFuture, TrayDefinition, TrayService,
};
use std::{
    collections::HashMap,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
};

static INSTALLED: AtomicBool = AtomicBool::new(false);
type Reply = Sender<Result<(), ServiceError>>;
enum Request {
    Menu(Vec<NativeMenuItem>, Reply),
    Tray(TrayDefinition, Reply),
    Remove(String, Reply),
    Attach(gpui::AnyWindowHandle, Reply),
}

/// Thread-safe menu and tray service proxy. Retain its [`DesktopMenus`] owner.
#[derive(Clone)]
pub struct DesktopMenuHandle {
    requests: Sender<Request>,
    menu_commands: Receiver<String>,
    tray_commands: Receiver<String>,
    model: Arc<RwLock<Vec<NativeMenuItem>>>,
    invoke_sender: Sender<String>,
    tray_clicks: Receiver<TrayClick>,
}
/// Mouse button reported by the platform tray backend.
pub use tray_icon::MouseButton as TrayMouseButton;

/// An actionable tray click, using the application's stable tray identifier.
#[derive(Clone, Debug)]
pub struct TrayClick {
    /// Stable identifier from `TrayDefinition`.
    pub id: String,
    /// Mouse button released over the tray icon.
    pub button: TrayMouseButton,
    /// Whether this is a Windows double-click event.
    pub double_click: bool,
}

impl DesktopMenuHandle {
    /// Current application menu model, also used by the in-window Linux menubar.
    pub fn menu(&self) -> Vec<NativeMenuItem> {
        self.model.read().expect("menu lock poisoned").clone()
    }
    /// Enqueues an enabled in-window menu command in the same queue as native menus.
    pub fn invoke(&self, id: &str) -> Result<()> {
        fn enabled(items: &[NativeMenuItem], id: &str) -> bool {
            items.iter().any(|item| match item {
                NativeMenuItem::Command(command) => command.id == id && command.enabled,
                NativeMenuItem::Submenu { items, .. } => enabled(items, id),
                NativeMenuItem::Separator => false,
            })
        }
        ensure!(
            enabled(&self.menu(), id),
            "menu command is missing or disabled"
        );
        self.invoke_sender.try_send(id.into()).map_err(Into::into)
    }
    /// Attaches the current and future menus to a Windows window through the UI task.
    pub fn attach_window(&self, window: gpui::AnyWindowHandle) -> ServiceFuture<()> {
        self.submit(|reply| Request::Attach(window, reply))
    }
    /// Dispatches menu and tray commands through current router enablement and scopes.
    /// Retain this task, and use one consumer for these queues. Router mutations
    /// take effect before the next invocation, including while a menu remains open.
    pub fn dispatch(
        &self,
        cx: &mut App,
        router: std::rc::Rc<std::cell::RefCell<CommandRouter>>,
        scopes: impl Fn(&App) -> Vec<gpui::SharedString> + 'static,
        mut handler: impl FnMut(&guic_core::CommandSpec, &mut App) + 'static,
    ) -> Task<()> {
        let menu = self.menu_commands.clone();
        let tray = self.tray_commands.clone();
        cx.spawn(async move |cx| {
            loop {
                let menu_next = Box::pin(menu.recv());
                let tray_next = Box::pin(tray.recv());
                let result = match futures::future::select(menu_next, tray_next).await {
                    futures::future::Either::Left((result, _))
                    | futures::future::Either::Right((result, _)) => result,
                };
                let Ok(id) = result else {
                    break;
                };
                cx.update(|cx| {
                    let command = router.borrow().resolve_id(&id, &scopes(cx)).cloned();
                    if let Some(command) = command {
                        handler(&command, cx);
                    }
                });
            }
        })
    }
    /// Receives tray clicks (release or double-click), or `None` when the owner stops.
    /// Desktop support varies; tray menu commands remain the portable interaction.
    pub fn next_tray_click(&self) -> ServiceFuture<Option<TrayClick>> {
        let clicks = self.tray_clicks.clone();
        Box::pin(async move { Ok(clicks.recv().await.ok()) })
    }
    fn submit(&self, make: impl FnOnce(Reply) -> Request) -> ServiceFuture<()> {
        let (tx, rx) = async_channel::bounded(1);
        let request = make(tx);
        let sender = self.requests.clone();
        Box::pin(async move {
            sender.send(request).await.map_err(|_| unavailable())?;
            rx.recv().await.map_err(|_| unavailable())?
        })
    }
}
fn unavailable() -> ServiceError {
    ServiceError::new(
        ServiceErrorKind::Unavailable,
        "native menu owner has stopped",
    )
}
impl ApplicationMenuService for DesktopMenuHandle {
    fn set_menu(&self, menu: Vec<NativeMenuItem>) -> ServiceFuture<()> {
        self.submit(|tx| Request::Menu(menu, tx))
    }
    fn next_command(&self) -> ServiceFuture<Option<String>> {
        let rx = self.menu_commands.clone();
        Box::pin(async move { Ok(rx.recv().await.ok()) })
    }
}
impl TrayService for DesktopMenuHandle {
    fn set_tray(&self, tray: TrayDefinition) -> ServiceFuture<()> {
        self.submit(|tx| Request::Tray(tray, tx))
    }
    fn remove_tray(&self, id: String) -> ServiceFuture<()> {
        self.submit(|tx| Request::Remove(id, tx))
    }
    fn next_command(&self) -> ServiceFuture<Option<String>> {
        let rx = self.tray_commands.clone();
        Box::pin(async move { Ok(rx.recv().await.ok()) })
    }
}

struct InstalledMenu {
    menu: Menu,
    commands: HashMap<String, String>,
}
struct InstalledTray {
    _tray: TrayIcon,
    menu: InstalledMenu,
}

/// Main-thread owner of native menus and tray icons.
///
/// Install only one owner per process. macOS uses its system menubar; Windows
/// attaches the menu to windows with `attach_window`; Linux applications render
/// the shared model with GUIC's `Menubar::native` because GPUI does not use GTK.
/// Tray icons use StatusNotifierItem on Linux and native status areas elsewhere.
/// Do not consume muda's global event receiver separately while this owner runs.
pub struct DesktopMenus {
    handle: DesktopMenuHandle,
    requests: Receiver<Request>,
    menu_commands: Sender<String>,
    tray_commands: Sender<String>,
    tray_clicks: Sender<TrayClick>,
    menu: Option<InstalledMenu>,
    trays: HashMap<String, InstalledTray>,
    #[cfg(target_os = "windows")]
    windows: Vec<gpui::AnyWindowHandle>,
    #[cfg(target_os = "windows")]
    attachments: HashMap<gpui::WindowId, windows_menu::Attachment>,
}
impl DesktopMenus {
    /// Acquires the process-wide menu backend on the GPUI thread.
    pub fn new() -> Result<Self> {
        ensure!(
            !INSTALLED.swap(true, Ordering::AcqRel),
            "desktop menus already installed"
        );
        let (tx, rx) = async_channel::bounded(64);
        let (menu_tx, menu_rx) = async_channel::unbounded();
        let (tray_tx, tray_rx) = async_channel::unbounded();
        let (click_tx, click_rx) = async_channel::unbounded();
        Ok(Self {
            handle: DesktopMenuHandle {
                requests: tx,
                menu_commands: menu_rx,
                tray_commands: tray_rx,
                model: Arc::default(),
                invoke_sender: menu_tx.clone(),
                tray_clicks: click_rx,
            },
            requests: rx,
            menu_commands: menu_tx,
            tray_commands: tray_tx,
            tray_clicks: click_tx,
            menu: None,
            trays: HashMap::new(),
            #[cfg(target_os = "windows")]
            windows: Vec::new(),
            #[cfg(target_os = "windows")]
            attachments: HashMap::new(),
        })
    }
    /// Returns the service proxy for background tasks or dependency injection.
    pub fn handle(&self) -> DesktopMenuHandle {
        self.handle.clone()
    }
    /// Replaces the menu synchronously on the GPUI thread.
    pub fn set_menu(&mut self, items: Vec<NativeMenuItem>, cx: &mut App) -> Result<()> {
        validate(&items, true)?;
        let menu = build_menu(&items)?;
        #[cfg(target_os = "windows")]
        {
            self.attachments.clear();
            self.windows.retain(|handle| cx.windows().contains(handle));
            for handle in &self.windows {
                let attachment = handle.update(cx, |_, window, _| attach(&menu.menu, window))??;
                self.attachments.insert(handle.window_id(), attachment);
            }
        }
        #[cfg(target_os = "macos")]
        menu.menu.init_for_nsapp();
        self.menu = Some(menu);
        *self.handle.model.write().expect("menu lock poisoned") = items;
        cx.refresh_windows();
        Ok(())
    }
    /// Creates or atomically replaces a tray icon. Encoded icons accept PNG or ICO.
    pub fn set_tray(&mut self, definition: TrayDefinition) -> Result<()> {
        ensure!(!definition.id.is_empty(), "tray id cannot be empty");
        validate(&definition.menu, false)?;
        ensure!(
            definition.icon.is_some() || cfg!(target_os = "macos"),
            "tray icons require a PNG or ICO image on this platform"
        );
        let menu = build_menu(&definition.menu)?;
        let mut builder = TrayIconBuilder::new()
            .with_id(format!("guic-{}", uuid::Uuid::new_v4()))
            .with_tooltip(&definition.label)
            .with_menu(Box::new(menu.menu.clone()));
        #[cfg(target_os = "macos")]
        if definition.icon.is_none() {
            builder = builder.with_title(&definition.label);
        }
        if let Some(bytes) = definition.icon {
            let image = image::load_from_memory(&bytes)?.into_rgba8();
            let (width, height) = image.dimensions();
            builder =
                builder.with_icon(tray_icon::Icon::from_rgba(image.into_raw(), width, height)?);
        }
        let tray = builder.build()?;
        self.trays
            .insert(definition.id, InstalledTray { _tray: tray, menu });
        Ok(())
    }
    /// Removes an icon. Removing an absent icon succeeds.
    pub fn remove_tray(&mut self, id: &str) {
        self.trays.remove(id);
    }
    /// Attaches the menu to a Windows window. Other platforms need no per-window attachment.
    pub fn attach_window(&mut self, window: &gpui::Window, cx: &mut App) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            self.windows.retain(|handle| cx.windows().contains(handle));
            self.attachments
                .retain(|id, _| self.windows.iter().any(|handle| handle.window_id() == *id));
            if let Some(menu) = &self.menu {
                let id = window.window_handle().window_id();
                self.attachments.remove(&id);
                self.attachments.insert(id, attach(&menu.menu, window)?);
            }
            if !self.windows.contains(&window.window_handle()) {
                self.windows.push(window.window_handle());
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = (window, cx);
        Ok(())
    }
    /// Runs service requests and menu event delivery. Retain the returned task;
    /// dropping it removes the icons and closes pending service requests.
    pub fn run(mut self, cx: &mut App) -> Task<()> {
        cx.spawn(async move |cx| {
            loop {
                // Bound each iteration so a producer cannot starve the UI thread.
                for _ in 0..64 {
                    let Ok(request) = self.requests.try_recv() else {
                        break;
                    };
                    let (result, reply) = cx.update(|cx| match request {
                        Request::Menu(items, reply) => (self.set_menu(items, cx), reply),
                        Request::Tray(tray, reply) => (self.set_tray(tray), reply),
                        Request::Remove(id, reply) => {
                            self.remove_tray(&id);
                            (Ok(()), reply)
                        }
                        Request::Attach(handle, reply) => (
                            handle
                                .update(cx, |_, window, cx| self.attach_window(window, cx))
                                .and_then(|result| result),
                            reply,
                        ),
                    });
                    let _ =
                        reply.try_send(result.map_err(|e| {
                            ServiceError::new(ServiceErrorKind::Platform, e.to_string())
                        }));
                }
                for _ in 0..64 {
                    let Ok(event) = MenuEvent::receiver().try_recv() else {
                        break;
                    };
                    if let Some(command) = self
                        .menu
                        .as_ref()
                        .and_then(|menu| menu.commands.get(event.id.as_ref()))
                    {
                        let _ = self.menu_commands.try_send(command.clone());
                    }
                    for tray in self.trays.values() {
                        if let Some(command) = tray.menu.commands.get(event.id.as_ref()) {
                            let _ = self.tray_commands.try_send(command.clone());
                            break;
                        }
                    }
                }
                for _ in 0..256 {
                    let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() else {
                        break;
                    };
                    let click = match &event {
                        tray_icon::TrayIconEvent::Click {
                            button,
                            button_state: tray_icon::MouseButtonState::Up,
                            ..
                        } => Some((*button, false)),
                        tray_icon::TrayIconEvent::DoubleClick { button, .. } => {
                            Some((*button, true))
                        }
                        _ => None,
                    };
                    if let Some((button, double_click)) = click
                        && let Some((id, _)) = self
                            .trays
                            .iter()
                            .find(|(_, tray)| tray._tray.id() == event.id())
                    {
                        let _ = self.tray_clicks.try_send(TrayClick {
                            id: id.clone(),
                            button,
                            double_click,
                        });
                    }
                }
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
            }
        })
    }
}
impl Drop for DesktopMenus {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        self.attachments.clear();
        #[cfg(target_os = "macos")]
        if let Some(menu) = &self.menu {
            menu.menu.remove_for_nsapp();
        }
        self.requests.close();
        self.menu_commands.close();
        self.tray_commands.close();
        self.tray_clicks.close();
        INSTALLED.store(false, Ordering::Release);
    }
}

/// Resolves a native command through the same enabled state and scopes as shortcuts.
/// Call this immediately before executing commands from either service queue or a menubar.
pub fn route_native_command<'a>(
    router: &'a CommandRouter,
    id: &str,
    scopes: &[gpui::SharedString],
) -> Option<&'a guic_core::CommandSpec> {
    router.resolve_id(id, scopes)
}

fn validate(items: &[NativeMenuItem], top_level: bool) -> Result<()> {
    fn walk(items: &[NativeMenuItem], depth: usize, count: &mut usize) -> Result<()> {
        ensure!(depth <= 16, "menu nesting exceeds 16 levels");
        for item in items {
            *count += 1;
            ensure!(*count <= 4096, "menu exceeds 4096 entries");
            match item {
                NativeMenuItem::Command(command) => ensure!(
                    !command.id.is_empty() && !command.label.contains('\0'),
                    "invalid menu command"
                ),
                NativeMenuItem::Submenu { label, items } => {
                    ensure!(!label.contains('\0'), "invalid submenu label");
                    walk(items, depth + 1, count)?;
                }
                NativeMenuItem::Separator => {}
            }
        }
        Ok(())
    }
    if top_level {
        ensure!(
            items
                .iter()
                .all(|item| matches!(item, NativeMenuItem::Submenu { .. })),
            "application menu roots must be submenus"
        );
    }
    walk(items, 0, &mut 0)
}
fn build_menu(items: &[NativeMenuItem]) -> Result<InstalledMenu> {
    fn build(
        items: &[NativeMenuItem],
        commands: &mut HashMap<String, String>,
    ) -> Result<Vec<Box<dyn IsMenuItem>>> {
        let mut result: Vec<Box<dyn IsMenuItem>> = Vec::new();
        for item in items {
            result.push(match item {
                NativeMenuItem::Separator => Box::new(PredefinedMenuItem::separator()),
                NativeMenuItem::Submenu { label, items } => {
                    let menu = Submenu::new(label, true);
                    for item in build(items, commands)? {
                        menu.append(item.as_ref())?;
                    }
                    Box::new(menu)
                }
                NativeMenuItem::Command(command) => {
                    let id = format!("guic-{}", uuid::Uuid::new_v4());
                    if command.enabled {
                        commands.insert(id.clone(), command.id.clone());
                    }
                    match command.checked {
                        Some(checked) => Box::new(CheckMenuItem::with_id(
                            id,
                            &command.label,
                            command.enabled,
                            checked,
                            None,
                        )),
                        None => {
                            Box::new(MenuItem::with_id(id, &command.label, command.enabled, None))
                        }
                    }
                }
            });
        }
        Ok(result)
    }
    let menu = Menu::new();
    let mut commands = HashMap::new();
    for item in build(items, &mut commands)? {
        menu.append(item.as_ref())?;
    }
    Ok(InstalledMenu { menu, commands })
}

#[cfg(target_os = "windows")]
fn attach(menu: &Menu, window: &gpui::Window) -> Result<windows_menu::Attachment> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(window)?.as_raw() else {
        anyhow::bail!("expected a Win32 window")
    };
    // SAFETY: GPUI owns a live HWND, and this operation runs on its UI thread.
    unsafe { windows_menu::Attachment::new(menu.clone(), handle.hwnd.get() as _) }
}
#[cfg(target_os = "windows")]
mod windows_menu;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_routing_rechecks_enabled_state_and_scope() {
        let mut router = CommandRouter::new();
        router
            .register(
                guic_core::CommandSpec::new("save", "Save")
                    .scope(guic_core::CommandScope::named("editor")),
            )
            .expect("register");
        assert!(route_native_command(&router, "save", &[]).is_none());
        assert!(route_native_command(&router, "save", &["editor".into()]).is_some());
        router.set_enabled("save", false);
        assert!(route_native_command(&router, "save", &["editor".into()]).is_none());
        assert!(route_native_command(&router, "missing", &["editor".into()]).is_none());
    }
    #[test]
    fn malformed_or_deep_menus_are_rejected_before_native_calls() {
        let command = NativeMenuItem::Command(guic_core::NativeCommand {
            id: "quit".into(),
            label: "Quit".into(),
            enabled: true,
            checked: None,
        });
        assert!(validate(std::slice::from_ref(&command), true).is_err());
        assert!(validate(std::slice::from_ref(&command), false).is_ok());
        let mut items = vec![command];
        for _ in 0..18 {
            items = vec![NativeMenuItem::Submenu {
                label: "Nested".into(),
                items,
            }];
        }
        assert!(validate(&items, true).is_err());
    }
    #[test]
    fn dropped_owner_closes_service_requests() {
        // No native API is called until a model or tray is installed.
        let menus = DesktopMenus::new().expect("menu owner");
        let handle = menus.handle();
        drop(menus);
        assert!(futures::executor::block_on(handle.set_menu(vec![])).is_err());
        assert_eq!(
            futures::executor::block_on(ApplicationMenuService::next_command(&handle))
                .expect("closed stream"),
            None
        );
    }
}
