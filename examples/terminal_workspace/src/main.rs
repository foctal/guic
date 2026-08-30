use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, WindowBounds, WindowOptions, div, px,
    size,
};
use guic::prelude::{
    Button, ComponentSize, Dialog, Dock, DockCommand, DockLayout, DockNode, DockPlacement,
    DockStackSelection, DockTab, DockTabSelection, DockTabs, Label, Root, Terminal,
    TerminalExitStatus, TerminalGridSize, TerminalInputState, TerminalModel, TerminalOptions,
    TerminalProcessStatus, TerminalSelection, TerminalShellProfile, TerminalTabStatus,
    discover_shell_profiles,
};
use guic::terminal::{LocalPtySession, TerminalTransport as _};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

const TERMINAL_COLUMNS: usize = 96;
const TERMINAL_ROWS: usize = 24;
const WORKSPACE_SNAPSHOT_VERSION: u32 = 1;
const WORKSPACE_SNAPSHOT_FILE: &str = "guic-terminal-workspace.json";

type PaneInputRouter = Rc<dyn Fn(&PaneInput, &mut Window, &mut App)>;
type PaneSelectionRouter = Rc<dyn Fn(&PaneSelection, &mut Window, &mut App)>;
type PaneScrollRouter = Rc<dyn Fn(&PaneScroll, &mut Window, &mut App)>;
type PaneResizeRouter = Rc<dyn Fn(&PaneResize, &mut Window, &mut App)>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct TerminalWorkspaceSnapshot {
    version: u32,
    layout: DockLayout,
    panes: Vec<TerminalPaneSnapshot>,
    active_stack_id: String,
    active_pane_id: String,
    next_pane: usize,
    next_stack: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct TerminalPaneSnapshot {
    id: String,
    title: String,
    shell: String,
    working_directory: Option<PathBuf>,
    columns: usize,
    rows: usize,
    lifecycle: TerminalPaneLifecycleSnapshot,
    dirty: bool,
    busy: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum TerminalPaneLifecycleSnapshot {
    Running,
    Exited { code: i32 },
}

#[derive(Clone)]
struct PaneInput {
    pane_id: SharedString,
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct PaneSelection {
    pane_id: SharedString,
    selection: TerminalSelection,
}

#[derive(Clone)]
struct PaneScroll {
    pane_id: SharedString,
    delta: isize,
}

#[derive(Clone)]
struct PaneResize {
    pane_id: SharedString,
    size: TerminalGridSize,
}

#[derive(Clone)]
struct PaneRenderState {
    id: SharedString,
    model: TerminalModel,
    input_state: Entity<TerminalInputState>,
    focus: FocusHandle,
}

struct TerminalPane {
    id: SharedString,
    title: SharedString,
    shell: SharedString,
    working_directory: Option<PathBuf>,
    model: TerminalModel,
    session: Option<LocalPtySession>,
    status: TerminalTabStatus,
    input_state: Entity<TerminalInputState>,
    focus: FocusHandle,
}

struct TerminalWorkspace {
    layout: DockLayout,
    panes: Vec<TerminalPane>,
    active_stack_id: SharedString,
    active_pane_id: SharedString,
    dock_focus: FocusHandle,
    tab_scroll_handles: HashMap<SharedString, ScrollHandle>,
    shell_profiles: Vec<TerminalShellProfile>,
    next_pane: usize,
    next_stack: usize,
    dialog_open: bool,
    status: SharedString,
}

impl TerminalWorkspace {
    fn new(cx: &mut Context<Self>) -> Self {
        let shell_profiles = discover_shell_profiles();
        let first_id = SharedString::from("terminal-1");
        let first_stack = SharedString::from("main");
        let pane = Self::create_pane(
            first_id.clone(),
            "Default",
            "default",
            current_working_directory(),
            &shell_profiles,
            cx,
        );
        let workspace = Self {
            layout: DockLayout::new(DockNode::Tabs(DockTabs::new(
                first_stack.clone(),
                vec![DockTab::new(
                    first_id.clone(),
                    pane.title.clone(),
                    pane.shell.clone(),
                )],
            ))),
            panes: vec![pane],
            active_stack_id: first_stack,
            active_pane_id: first_id,
            dock_focus: cx.focus_handle(),
            tab_scroll_handles: HashMap::from([(SharedString::from("main"), ScrollHandle::new())]),
            shell_profiles,
            next_pane: 2,
            next_stack: 2,
            dialog_open: false,
            status: SharedString::from("Workspace ready"),
        };
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                if this.update(cx, |this, cx| this.refresh_output(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        workspace
    }

    fn create_pane(
        id: SharedString,
        title: impl Into<SharedString>,
        shell: impl Into<SharedString>,
        working_directory: Option<PathBuf>,
        profiles: &[TerminalShellProfile],
        cx: &mut Context<Self>,
    ) -> TerminalPane {
        let title = title.into();
        let shell = shell.into();
        let mut model = TerminalModel::new(TERMINAL_COLUMNS, TERMINAL_ROWS);
        let session = spawn_shell(
            shell.as_ref(),
            profiles,
            TERMINAL_COLUMNS,
            TERMINAL_ROWS,
            working_directory.as_ref(),
        );
        let status = if session.is_ok() {
            TerminalTabStatus::running()
        } else {
            TerminalTabStatus::exited(-1)
        };
        if let Err(error) = &session {
            model.write(&format!("failed to start shell `{shell}`: {error}\r\n"));
        }
        TerminalPane {
            id,
            title,
            shell,
            working_directory,
            model,
            session: session.ok(),
            status,
            input_state: cx.new(|_| TerminalInputState::new()),
            focus: cx.focus_handle(),
        }
    }

    fn render_states(&self) -> Vec<PaneRenderState> {
        self.panes
            .iter()
            .map(|pane| PaneRenderState {
                id: pane.id.clone(),
                model: pane.model.clone(),
                input_state: pane.input_state.clone(),
                focus: pane.focus.clone(),
            })
            .collect()
    }

    fn snapshot(&self) -> TerminalWorkspaceSnapshot {
        TerminalWorkspaceSnapshot {
            version: WORKSPACE_SNAPSHOT_VERSION,
            layout: self.layout.clone(),
            panes: self
                .panes
                .iter()
                .map(|pane| TerminalPaneSnapshot {
                    id: pane.id.to_string(),
                    title: pane.title.to_string(),
                    shell: pane.shell.to_string(),
                    working_directory: pane.working_directory.clone(),
                    columns: pane.model.columns(),
                    rows: pane.model.rows(),
                    lifecycle: pane_lifecycle_snapshot(pane.status),
                    dirty: pane.status.dirty,
                    busy: pane.status.busy,
                })
                .collect(),
            active_stack_id: self.active_stack_id.to_string(),
            active_pane_id: self.active_pane_id.to_string(),
            next_pane: self.next_pane,
            next_stack: self.next_stack,
        }
    }

    fn restore_snapshot(
        &mut self,
        snapshot: TerminalWorkspaceSnapshot,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        validate_workspace_snapshot(&snapshot)?;

        self.terminate_all_sessions();

        let panes = snapshot
            .panes
            .iter()
            .map(|pane| {
                let mut restored = Self::create_pane(
                    SharedString::from(pane.id.clone()),
                    pane.title.clone(),
                    pane.shell.clone(),
                    pane.working_directory
                        .clone()
                        .or_else(current_working_directory),
                    &self.shell_profiles,
                    cx,
                );
                restored.model.resize(pane.columns.max(1), pane.rows.max(1));
                if let Some(session) = &mut restored.session {
                    let _ = session.resize(pane.columns.max(1), pane.rows.max(1));
                }
                restored.status.dirty = pane.dirty;
                restored.status.busy = pane.busy && restored.status.is_running();
                restored.model.write(&format!(
                    "restored workspace metadata; previous process state: {}\r\n",
                    lifecycle_snapshot_label(pane.lifecycle)
                ));
                restored
            })
            .collect::<Vec<_>>();

        self.layout = snapshot.layout;
        self.tab_scroll_handles = self
            .layout
            .stack_ids()
            .into_iter()
            .map(|stack_id| (stack_id, ScrollHandle::new()))
            .collect();
        self.panes = panes;
        self.active_stack_id = valid_stack_id(&self.layout, &snapshot.active_stack_id)
            .unwrap_or_else(|| {
                self.layout
                    .stack_ids()
                    .first()
                    .cloned()
                    .unwrap_or_else(|| SharedString::from("main"))
            });
        let _ = self.layout.focus_stack(&self.active_stack_id);
        self.active_pane_id = self
            .panes
            .iter()
            .find(|pane| pane.id.as_ref() == snapshot.active_pane_id)
            .or_else(|| self.panes.first())
            .map(|pane| pane.id.clone())
            .unwrap_or_else(|| SharedString::from("terminal-1"));
        self.next_pane = snapshot
            .next_pane
            .max(next_number_after_prefix(&self.panes, "terminal-"));
        self.next_stack = snapshot.next_stack.max(next_stack_number(&self.layout));
        self.dialog_open = false;
        self.status = SharedString::from("Restored workspace snapshot");
        cx.notify();
        Ok(())
    }

    fn save_workspace(&mut self, cx: &mut Context<Self>) {
        match write_workspace_snapshot(&self.snapshot()) {
            Ok(path) => {
                self.status = SharedString::from(format!("Saved workspace to {}", path.display()));
            }
            Err(error) => {
                self.status = SharedString::from(format!("Workspace save failed: {error}"));
            }
        }
        cx.notify();
    }

    fn restore_saved_workspace(&mut self, cx: &mut Context<Self>) {
        match read_workspace_snapshot().and_then(|snapshot| self.restore_snapshot(snapshot, cx)) {
            Ok(()) => {}
            Err(error) => {
                self.status = SharedString::from(format!("Workspace restore failed: {error}"));
                cx.notify();
            }
        }
    }

    fn reset_workspace(&mut self, cx: &mut Context<Self>) {
        self.terminate_all_sessions();
        *self = Self::new(cx);
        self.status = SharedString::from("Reset workspace");
        cx.notify();
    }

    fn terminate_all_sessions(&mut self) {
        for pane in &mut self.panes {
            if let Some(session) = &mut pane.session {
                let _ = session.terminate();
            }
        }
    }

    fn add_terminal(&mut self, shell: &str, cx: &mut Context<Self>) {
        let id = SharedString::from(format!("terminal-{}", self.next_pane));
        self.next_pane += 1;
        let title = terminal_title(shell, self.next_pane - 1);
        let pane = Self::create_pane(
            id.clone(),
            title.clone(),
            shell,
            current_working_directory(),
            &self.shell_profiles,
            cx,
        );
        self.layout.insert_tab(
            self.active_stack_id.as_ref(),
            DockTab::new(id.clone(), title, shell),
        );
        self.active_pane_id = id;
        self.panes.push(pane);
        self.status = SharedString::from(format!("Added {shell} terminal"));
        cx.notify();
    }

    fn split_active(&mut self, placement: DockPlacement, cx: &mut Context<Self>) {
        let shell = self
            .active_pane()
            .map(|pane| pane.shell.to_string())
            .unwrap_or_else(|| "default".to_string());
        let pane_id = SharedString::from(format!("terminal-{}", self.next_pane));
        self.next_pane += 1;
        let stack_id = SharedString::from(format!("split-{}", self.next_stack));
        self.next_stack += 1;
        let title = terminal_title(&shell, self.next_pane - 1);
        let pane = Self::create_pane(
            pane_id.clone(),
            title.clone(),
            shell.clone(),
            self.active_pane()
                .and_then(|pane| pane.working_directory.clone())
                .or_else(current_working_directory),
            &self.shell_profiles,
            cx,
        );
        if self.layout.split_stack_with_tab(
            self.active_stack_id.as_ref(),
            placement,
            stack_id.clone(),
            DockTab::new(pane_id.clone(), title, shell),
        ) {
            self.active_stack_id = stack_id;
            self.active_pane_id = pane_id;
            self.panes.push(pane);
            self.status = SharedString::from("Split terminal pane");
        }
        cx.notify();
    }

    fn terminate_active(&mut self, cx: &mut Context<Self>) {
        self.close_active_session(false, cx);
    }

    fn force_close_active(&mut self, cx: &mut Context<Self>) {
        self.close_active_session(true, cx);
    }

    fn close_active_session(&mut self, force: bool, cx: &mut Context<Self>) {
        if let Some(pane) = self
            .panes
            .iter_mut()
            .find(|pane| pane.id == self.active_pane_id)
        {
            if let Some(session) = &mut pane.session {
                let result = if force {
                    session.force_close()
                } else {
                    session.request_graceful_close()
                };
                match result {
                    Ok(()) => {
                        let verb = if force {
                            "Force closed"
                        } else {
                            "Close requested for"
                        };
                        pane.status.busy = true;
                        self.status = SharedString::from(format!("{verb} {}", pane.title));
                    }
                    Err(error) => {
                        let verb = if force {
                            "Force close"
                        } else {
                            "Graceful close"
                        };
                        self.status = SharedString::from(format!(
                            "{verb} failed for {}: {error}",
                            pane.title
                        ));
                    }
                }
            } else {
                self.status = SharedString::from(format!("{} has no running session", pane.title));
            }
            cx.notify();
        }
    }

    fn restart_active(&mut self, cx: &mut Context<Self>) {
        let Some(pane) = self
            .panes
            .iter_mut()
            .find(|pane| pane.id == self.active_pane_id)
        else {
            return;
        };
        pane.model.write("\r\nrestarting session...\r\n");
        let restart_result = if let Some(session) = &mut pane.session {
            session.restart()
        } else {
            spawn_shell(
                pane.shell.as_ref(),
                &self.shell_profiles,
                pane.model.columns(),
                pane.model.rows(),
                pane.working_directory.as_ref(),
            )
            .map(|session| {
                pane.session = Some(session);
            })
        };
        match restart_result {
            Ok(()) => {
                pane.status = TerminalTabStatus::running();
                self.status = SharedString::from(format!("Restarted {}", pane.title));
            }
            Err(error) => {
                pane.session = None;
                pane.status = TerminalTabStatus::exited(-1);
                pane.model.write(&format!(
                    "failed to restart shell `{}`: {error}\r\n",
                    pane.shell
                ));
                self.status = SharedString::from(format!("Restart failed: {error}"));
            }
        }
        cx.notify();
    }

    fn close_pane(&mut self, selection: &DockTabSelection, cx: &mut Context<Self>) {
        if !self
            .layout
            .close_tab(selection.stack_id().as_ref(), selection.tab_id().as_ref())
        {
            return;
        }
        let pane_id = selection.tab_id().clone();
        if let Some(index) = self.panes.iter().position(|pane| pane.id == pane_id) {
            if let Some(session) = &mut self.panes[index].session {
                let _ = session.terminate();
            }
            self.panes.remove(index);
        }
        self.sync_active_dock_selection();
        self.status = SharedString::from("Closed terminal pane");
        cx.notify();
    }

    fn close_stack(&mut self, selection: &DockStackSelection, cx: &mut Context<Self>) {
        let stack_id = selection.stack_id().clone();
        let pane_ids = self
            .panes
            .iter()
            .filter(|pane| {
                stack_contains_tab(self.layout.root(), stack_id.as_ref(), pane.id.as_ref())
            })
            .map(|pane| pane.id.clone())
            .collect::<Vec<_>>();
        if !self.layout.close_stack(stack_id.as_ref()) {
            self.status = SharedString::from("Pinned terminal stack cannot be closed");
            cx.notify();
            return;
        }
        for pane_id in pane_ids {
            if let Some(index) = self.panes.iter().position(|pane| pane.id == pane_id) {
                if let Some(session) = &mut self.panes[index].session {
                    let _ = session.terminate();
                }
                self.panes.remove(index);
            }
        }
        self.sync_active_dock_selection();
        self.status = SharedString::from("Closed terminal stack");
        cx.notify();
    }

    fn select_tab(&mut self, selection: &DockTabSelection, cx: &mut Context<Self>) {
        self.layout
            .select_tab(selection.stack_id().as_ref(), selection.tab_id().as_ref());
        self.active_stack_id = selection.stack_id().clone();
        self.active_pane_id = selection.tab_id().clone();
        if let Some(pane) = self
            .panes
            .iter_mut()
            .find(|pane| pane.id == self.active_pane_id)
        {
            pane.status.dirty = false;
        }
        cx.notify();
    }

    fn apply_dock_command(&mut self, command: &DockCommand, cx: &mut Context<Self>) {
        match command {
            DockCommand::SelectTab(selection) => self.select_tab(selection, cx),
            DockCommand::CloseTab(selection) => self.close_pane(selection, cx),
            DockCommand::CloseStack(selection) => self.close_stack(selection, cx),
            _ => {
                if self.layout.apply(command) {
                    self.sync_active_dock_selection();
                    self.tab_scroll_handles
                        .retain(|stack_id, _| self.layout.stack_ids().contains(stack_id));
                    for stack_id in self.layout.stack_ids() {
                        self.tab_scroll_handles.entry(stack_id).or_default();
                    }
                    self.status = SharedString::from("Updated terminal dock layout");
                    cx.notify();
                }
            }
        }
    }

    fn sync_active_dock_selection(&mut self) {
        if let Some(stack_id) = self.layout.focused_stack_id().cloned()
            && let Some(tab_id) = active_tab_id(self.layout.root(), &stack_id)
        {
            self.active_stack_id = stack_id;
            self.active_pane_id = tab_id;
            return;
        }
        if let Some(pane_id) = self
            .panes
            .iter()
            .find(|pane| pane.id == self.active_pane_id)
            .or_else(|| self.panes.first())
            .map(|pane| pane.id.clone())
            && let Some(stack_id) = find_tab_stack(self.layout.root(), &pane_id)
        {
            let _ = self.layout.focus_stack(&stack_id);
            self.active_stack_id = stack_id;
            self.active_pane_id = pane_id;
        }
    }

    fn active_pane(&self) -> Option<&TerminalPane> {
        self.panes
            .iter()
            .find(|pane| pane.id == self.active_pane_id)
    }

    fn refresh_output(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        for pane in &mut self.panes {
            if let Some(mut session) = pane.session.take() {
                let output = session.drain_output();
                if !output.is_empty() {
                    pane.model.write(&String::from_utf8_lossy(&output));
                    flush_terminal_responses(&mut pane.model, &mut session);
                    pane.status.busy = false;
                    if pane.id != self.active_pane_id {
                        pane.status.dirty = true;
                    }
                    changed = true;
                }
                if let Some(code) = session.try_exit_code() {
                    pane.status.lifecycle =
                        TerminalProcessStatus::Exited(TerminalExitStatus { code });
                    pane.status.busy = false;
                    if pane.id != self.active_pane_id {
                        pane.status.dirty = true;
                    }
                    pane.model
                        .write(&format!("\r\nprocess exited with status {code}\r\n"));
                    changed = true;
                } else {
                    pane.session = Some(session);
                }
            }
        }
        if changed {
            cx.notify();
        }
    }

    fn write_input(&mut self, input: &PaneInput, cx: &mut Context<Self>) {
        if let Some(pane) = self.panes.iter_mut().find(|pane| pane.id == input.pane_id)
            && let Some(session) = &mut pane.session
        {
            if !pane.status.is_running() {
                return;
            }
            pane.status.busy = true;
            if let Err(error) = session.write(&input.bytes) {
                self.status = SharedString::from(format!("PTY write failed: {error}"));
                pane.status.busy = false;
            }
            let output = session.drain_output();
            if !output.is_empty() {
                pane.model.write(&String::from_utf8_lossy(&output));
                flush_terminal_responses(&mut pane.model, session);
                pane.status.busy = false;
            }
            cx.notify();
        }
    }

    fn update_selection(&mut self, payload: &PaneSelection, cx: &mut Context<Self>) {
        if let Some(pane) = self
            .panes
            .iter_mut()
            .find(|pane| pane.id == payload.pane_id)
        {
            pane.model.set_selection(payload.selection);
            cx.notify();
        }
    }

    fn scroll_viewport(&mut self, payload: &PaneScroll, cx: &mut Context<Self>) {
        if let Some(pane) = self
            .panes
            .iter_mut()
            .find(|pane| pane.id == payload.pane_id)
        {
            if payload.delta > 0 {
                pane.model.scroll_up(payload.delta.unsigned_abs());
            } else {
                pane.model.scroll_down(payload.delta.unsigned_abs());
            }
            cx.notify();
        }
    }

    fn resize_pane(&mut self, payload: &PaneResize, cx: &mut Context<Self>) {
        if let Some(pane) = self
            .panes
            .iter_mut()
            .find(|pane| pane.id == payload.pane_id)
        {
            let columns = payload.size.columns.max(1);
            let rows = payload.size.rows.max(1);
            if pane.model.columns() == columns && pane.model.rows() == rows {
                return;
            }
            pane.model.resize(columns, rows);
            if let Some(session) = &mut pane.session
                && let Err(error) = session.resize(columns, rows)
            {
                self.status = SharedString::from(format!("PTY resize failed: {error}"));
            }
            cx.notify();
        }
    }
}

impl Render for TerminalWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = guic::tokens::Theme::global(cx);
        let panes = self.render_states();
        let input_router = Rc::new(cx.listener(|this, input: &PaneInput, _, cx| {
            this.write_input(input, cx);
        }));
        let selection_router = Rc::new(cx.listener(|this, payload: &PaneSelection, _, cx| {
            this.update_selection(payload, cx);
        }));
        let scroll_router = Rc::new(cx.listener(|this, payload: &PaneScroll, _, cx| {
            this.scroll_viewport(payload, cx);
        }));
        let resize_router = Rc::new(cx.listener(|this, payload: &PaneResize, _, cx| {
            this.resize_pane(payload, cx);
        }));
        let dock_focus = self.dock_focus.clone();
        let active_stack_id = self.active_stack_id.clone();
        let tab_scroll_handles = self.tab_scroll_handles.clone();

        div()
            .size_full()
            .bg(theme.background())
            .flex()
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(self.render_header(cx))
                    .child(
                        div().flex_1().min_h_0().p_4().child(
                            tab_scroll_handles
                                .into_iter()
                                .fold(
                                    Dock::new("terminal-workspace-dock", self.layout.clone())
                                        .focusable(dock_focus)
                                        .keyboard_stack(active_stack_id),
                                    |dock, (stack_id, handle)| {
                                        dock.track_tab_scroll(stack_id, handle)
                                    },
                                )
                                .on_command(cx.listener(|this, command: &DockCommand, _, cx| {
                                    this.apply_dock_command(command, cx);
                                }))
                                .render_tab_body(move |selection, _tab| {
                                    render_terminal_body(
                                        selection.tab_id(),
                                        &panes,
                                        input_router.clone(),
                                        selection_router.clone(),
                                        scroll_router.clone(),
                                        resize_router.clone(),
                                    )
                                }),
                        ),
                    )
                    .child(self.render_dialog(cx)),
            )
    }
}

impl TerminalWorkspace {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = guic::tokens::Theme::global(cx);
        let sidebar = div()
            .w(px(220.0))
            .h_full()
            .p_4()
            .border_r_1()
            .border_color(theme.border())
            .bg(theme.secondary().opacity(0.08))
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new("muxt").secondary("GUIC workspace"))
            .child(Label::new("Sessions").muted(true))
            .child(Label::new(format!("{} panes", self.panes.len())))
            .child(Label::new(format!("Active: {}", self.active_pane_id)).muted(true));

        self.panes
            .iter()
            .fold(sidebar, |sidebar, pane| {
                sidebar.child(Label::new(pane.title.clone()).secondary(format!(
                    "{} - {}",
                    pane.shell,
                    tab_status_label(pane.status)
                )))
            })
            .into_any_element()
    }

    fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = guic::tokens::Theme::global(cx);
        let header = div()
            .w_full()
            .min_w_0()
            .p_3()
            .border_b_1()
            .border_color(theme.border())
            .flex()
            .items_center()
            .gap_2()
            .child(Label::new("Terminal Workspace").secondary(self.status.clone()));
        let mut controls = div()
            .id("terminal-workspace-controls")
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap_2()
            .overflow_x_scroll();

        for profile in &self.shell_profiles {
            let shell_id = profile.id().clone();
            let label = if profile.is_default() {
                "New".to_string()
            } else {
                profile.label().to_string()
            };
            controls = controls.child(
                Button::new(label)
                    .secondary()
                    .size(ComponentSize::Small)
                    .disabled(!profile.is_available())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.add_terminal(shell_id.as_ref(), cx);
                    })),
            );
        }

        controls = controls
            .child(header_button(
                "Split Right",
                |this, cx| this.split_active(DockPlacement::Right, cx),
                cx,
            ))
            .child(header_button(
                "Split Down",
                |this, cx| this.split_active(DockPlacement::Bottom, cx),
                cx,
            ))
            .child(header_button(
                "Restart",
                |this, cx| this.restart_active(cx),
                cx,
            ))
            .child(header_button(
                "Terminate",
                |this, cx| this.terminate_active(cx),
                cx,
            ))
            .child(header_button(
                "Force Close",
                |this, cx| this.force_close_active(cx),
                cx,
            ))
            .child(header_button(
                "Save Layout",
                |this, cx| this.save_workspace(cx),
                cx,
            ))
            .child(header_button(
                "Restore Layout",
                |this, cx| this.restore_saved_workspace(cx),
                cx,
            ))
            .child(header_button(
                "Reset",
                |this, cx| this.reset_workspace(cx),
                cx,
            ))
            .child(
                Button::new("About")
                    .secondary()
                    .size(ComponentSize::Small)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dialog_open = true;
                        cx.notify();
                    })),
            );

        header.child(controls).into_any_element()
    }

    fn render_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        Dialog::new("terminal-workspace-about")
            .open(self.dialog_open)
            .title("Terminal workspace")
            .description("This example exercises GUIC layout, dock tabs, splits, dialogs, and guic-terminal PTY panes.")
            .content(Label::new("Use the header actions to create shell tabs and split panes. Shell availability depends on the host OS."))
            .secondary_label("Close")
            .on_cancel(cx.listener(|this, _, _, cx| {
                this.dialog_open = false;
                cx.notify();
            }))
            .into_any_element()
    }
}

fn header_button(
    label: &'static str,
    action: fn(&mut TerminalWorkspace, &mut Context<TerminalWorkspace>),
    cx: &mut Context<TerminalWorkspace>,
) -> Button {
    Button::new(label)
        .secondary()
        .size(ComponentSize::Small)
        .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
}

fn render_terminal_body(
    pane_id: &SharedString,
    panes: &[PaneRenderState],
    input_router: PaneInputRouter,
    selection_router: PaneSelectionRouter,
    scroll_router: PaneScrollRouter,
    resize_router: PaneResizeRouter,
) -> AnyElement {
    let Some(pane) = panes.iter().find(|pane| pane.id == *pane_id) else {
        return Label::new("Missing terminal pane")
            .muted(true)
            .into_any_element();
    };
    let input_id = pane.id.clone();
    let selection_id = pane.id.clone();
    let scroll_id = pane.id.clone();
    let resize_id = pane.id.clone();
    div()
        .size_full()
        .min_w_0()
        .min_h_0()
        .overflow_hidden()
        .child(
            Terminal::new(format!("workspace-terminal-{pane_id}"), pane.model.clone())
                .focusable(pane.focus.clone())
                .input_state(pane.input_state.clone())
                .options(
                    TerminalOptions::default()
                        .visible_scrollback(0)
                        .measured_font(),
                )
                .on_input(move |bytes, window, cx| {
                    input_router(
                        &PaneInput {
                            pane_id: input_id.clone(),
                            bytes: bytes.to_vec(),
                        },
                        window,
                        cx,
                    );
                })
                .on_selection(move |selection, window, cx| {
                    selection_router(
                        &PaneSelection {
                            pane_id: selection_id.clone(),
                            selection: *selection,
                        },
                        window,
                        cx,
                    );
                })
                .on_viewport_scroll(move |delta, window, cx| {
                    scroll_router(
                        &PaneScroll {
                            pane_id: scroll_id.clone(),
                            delta: *delta,
                        },
                        window,
                        cx,
                    );
                })
                .on_resize(move |size, window, cx| {
                    resize_router(
                        &PaneResize {
                            pane_id: resize_id.clone(),
                            size: *size,
                        },
                        window,
                        cx,
                    );
                }),
        )
        .into_any_element()
}

fn flush_terminal_responses(model: &mut TerminalModel, session: &mut LocalPtySession) {
    let responses = model.take_response_bytes();
    if !responses.is_empty() {
        let _ = session.write(&responses);
    }
}

fn workspace_snapshot_path() -> anyhow::Result<PathBuf> {
    Ok(std::env::current_dir()?.join(WORKSPACE_SNAPSHOT_FILE))
}

fn write_workspace_snapshot(snapshot: &TerminalWorkspaceSnapshot) -> anyhow::Result<PathBuf> {
    let path = workspace_snapshot_path()?;
    write_workspace_snapshot_to(&path, snapshot)?;
    Ok(path)
}

fn read_workspace_snapshot() -> anyhow::Result<TerminalWorkspaceSnapshot> {
    let path = workspace_snapshot_path()?;
    read_workspace_snapshot_from(&path)
}

fn write_workspace_snapshot_to(
    path: &Path,
    snapshot: &TerminalWorkspaceSnapshot,
) -> anyhow::Result<()> {
    validate_workspace_snapshot(snapshot)?;
    let temporary = path.with_extension("json.tmp");
    let backup = path.with_extension("json.backup");
    let bytes = serde_json::to_vec_pretty(snapshot)?;

    let _ = fs::remove_file(&temporary);
    let mut file = fs::File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;

    let had_existing = path.exists();
    if had_existing {
        let _ = fs::remove_file(&backup);
        fs::rename(path, &backup)?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if had_existing {
            let _ = fs::rename(&backup, path);
        }
        return Err(error.into());
    }
    if had_existing {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn read_workspace_snapshot_from(path: &Path) -> anyhow::Result<TerminalWorkspaceSnapshot> {
    let backup = path.with_extension("json.backup");
    match decode_workspace_snapshot(path) {
        Ok(snapshot) => Ok(snapshot),
        Err(primary_error) if backup.exists() => decode_workspace_snapshot(&backup).map_err(
            |backup_error| {
                anyhow::anyhow!(
                    "workspace snapshot and backup are invalid: primary: {primary_error}; backup: {backup_error}"
                )
            },
        ),
        Err(error) => Err(error),
    }
}

fn decode_workspace_snapshot(path: &Path) -> anyhow::Result<TerminalWorkspaceSnapshot> {
    let snapshot: TerminalWorkspaceSnapshot = serde_json::from_slice(&fs::read(path)?)?;
    validate_workspace_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn validate_workspace_snapshot(snapshot: &TerminalWorkspaceSnapshot) -> anyhow::Result<()> {
    anyhow::ensure!(
        snapshot.version == WORKSPACE_SNAPSHOT_VERSION,
        "unsupported workspace snapshot version {}",
        snapshot.version
    );
    anyhow::ensure!(
        !snapshot.panes.is_empty(),
        "workspace snapshot does not contain panes"
    );

    let mut layout_tabs = Vec::new();
    collect_layout_tab_ids(snapshot.layout.root(), &mut layout_tabs);
    let layout_tab_set = layout_tabs
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    anyhow::ensure!(
        layout_tab_set.len() == layout_tabs.len(),
        "workspace snapshot layout contains duplicate tab identifiers"
    );

    let pane_ids = snapshot
        .panes
        .iter()
        .map(|pane| pane.id.as_str())
        .collect::<HashSet<_>>();
    anyhow::ensure!(
        pane_ids.len() == snapshot.panes.len(),
        "workspace snapshot contains duplicate pane identifiers"
    );
    anyhow::ensure!(
        pane_ids == layout_tab_set,
        "workspace snapshot pane identifiers do not match layout tab identifiers"
    );
    anyhow::ensure!(
        snapshot.panes.iter().all(|pane| !pane.id.trim().is_empty()
            && !pane.title.trim().is_empty()
            && !pane.shell.trim().is_empty()
            && pane.columns > 0
            && pane.rows > 0),
        "workspace snapshot contains invalid pane metadata"
    );

    let stack_ids = snapshot.layout.stack_ids();
    let unique_stack_ids = stack_ids.iter().map(AsRef::as_ref).collect::<HashSet<_>>();
    anyhow::ensure!(
        unique_stack_ids.len() == stack_ids.len()
            && unique_stack_ids.iter().all(|id| !id.trim().is_empty()),
        "workspace snapshot contains invalid or duplicate stack identifiers"
    );
    anyhow::ensure!(
        unique_stack_ids.contains(snapshot.active_stack_id.as_str()),
        "workspace snapshot active stack does not exist"
    );
    anyhow::ensure!(
        pane_ids.contains(snapshot.active_pane_id.as_str()),
        "workspace snapshot active pane does not exist"
    );
    anyhow::ensure!(
        stack_contains_tab(
            snapshot.layout.root(),
            &snapshot.active_stack_id,
            &snapshot.active_pane_id,
        ),
        "workspace snapshot active pane is not in the active stack"
    );
    Ok(())
}

fn collect_layout_tab_ids(node: &DockNode, ids: &mut Vec<String>) {
    match node {
        DockNode::Split { first, second, .. } => {
            collect_layout_tab_ids(first, ids);
            collect_layout_tab_ids(second, ids);
        }
        DockNode::Tabs(tabs) => {
            ids.extend(tabs.tabs().iter().map(|tab| tab.id().to_string()));
        }
    }
}

fn current_working_directory() -> Option<PathBuf> {
    std::env::current_dir().ok()
}

fn pane_lifecycle_snapshot(status: TerminalTabStatus) -> TerminalPaneLifecycleSnapshot {
    match status.lifecycle {
        TerminalProcessStatus::Running => TerminalPaneLifecycleSnapshot::Running,
        TerminalProcessStatus::Exited(exit) => {
            TerminalPaneLifecycleSnapshot::Exited { code: exit.code }
        }
    }
}

fn lifecycle_snapshot_label(lifecycle: TerminalPaneLifecycleSnapshot) -> String {
    match lifecycle {
        TerminalPaneLifecycleSnapshot::Running => "running".to_string(),
        TerminalPaneLifecycleSnapshot::Exited { code } => format!("exited ({code})"),
    }
}

fn valid_stack_id(layout: &DockLayout, stack_id: &str) -> Option<SharedString> {
    layout
        .stack_ids()
        .into_iter()
        .find(|id| id.as_ref() == stack_id)
}

fn next_number_after_prefix(panes: &[TerminalPane], prefix: &str) -> usize {
    panes
        .iter()
        .filter_map(|pane| pane.id.as_ref().strip_prefix(prefix)?.parse::<usize>().ok())
        .max()
        .map_or(1, |number| number + 1)
}

fn next_stack_number(layout: &DockLayout) -> usize {
    layout
        .stack_ids()
        .iter()
        .filter_map(|id| id.as_ref().strip_prefix("split-")?.parse::<usize>().ok())
        .max()
        .map_or(2, |number| number + 1)
}

fn spawn_shell(
    shell: &str,
    profiles: &[TerminalShellProfile],
    columns: usize,
    rows: usize,
    working_directory: Option<&PathBuf>,
) -> anyhow::Result<LocalPtySession> {
    if shell == "default" {
        if let Some(working_directory) = working_directory {
            LocalPtySession::spawn_shell_in_dir(columns, rows, working_directory)
        } else {
            LocalPtySession::spawn_shell(columns, rows)
        }
    } else if let Some(profile) = profiles
        .iter()
        .find(|profile| profile.id().as_ref() == shell)
    {
        if let Some(working_directory) = working_directory {
            LocalPtySession::spawn_profile_in_dir(profile, columns, rows, working_directory)
        } else {
            LocalPtySession::spawn_profile(profile, columns, rows)
        }
    } else if let Some(working_directory) = working_directory {
        LocalPtySession::spawn_command_in_dir(shell, columns, rows, working_directory)
    } else {
        LocalPtySession::spawn_command(shell, columns, rows)
    }
}

fn terminal_title(shell: &str, index: usize) -> String {
    if shell == "default" {
        format!("Terminal {index}")
    } else {
        format!("{shell} {index}")
    }
}

fn tab_status_label(status: TerminalTabStatus) -> String {
    let lifecycle = match status.lifecycle {
        TerminalProcessStatus::Running => "running".to_string(),
        TerminalProcessStatus::Exited(exit) => format!("exited ({})", exit.code),
    };
    match (status.dirty, status.busy) {
        (true, true) => format!("{lifecycle}, unread, busy"),
        (true, false) => format!("{lifecycle}, unread"),
        (false, true) => format!("{lifecycle}, busy"),
        (false, false) => lifecycle,
    }
}

fn stack_contains_tab(node: &DockNode, stack_id: &str, tab_id: &str) -> bool {
    match node {
        DockNode::Split { first, second, .. } => {
            stack_contains_tab(first, stack_id, tab_id)
                || stack_contains_tab(second, stack_id, tab_id)
        }
        DockNode::Tabs(tabs) => {
            tabs.id().as_ref() == stack_id
                && tabs.tabs().iter().any(|tab| tab.id().as_ref() == tab_id)
        }
    }
}

fn active_tab_id(node: &DockNode, stack_id: &str) -> Option<SharedString> {
    match node {
        DockNode::Split { first, second, .. } => {
            active_tab_id(first, stack_id).or_else(|| active_tab_id(second, stack_id))
        }
        DockNode::Tabs(tabs) if tabs.id().as_ref() == stack_id => {
            tabs.active().map(|tab| tab.id().clone())
        }
        DockNode::Tabs(_) => None,
    }
}

fn find_tab_stack(node: &DockNode, tab_id: &str) -> Option<SharedString> {
    match node {
        DockNode::Split { first, second, .. } => {
            find_tab_stack(first, tab_id).or_else(|| find_tab_stack(second, tab_id))
        }
        DockNode::Tabs(tabs) if tabs.tabs().iter().any(|tab| tab.id().as_ref() == tab_id) => {
            Some(tabs.id().clone())
        }
        DockNode::Tabs(_) => None,
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut gpui::App| {
        guic::init(cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1280.), px(760.)),
                cx,
            ))),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("GUIC Terminal Workspace".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let app = cx.new(TerminalWorkspace::new);
            cx.new(|cx| Root::new(app, window, cx))
        })
        .expect("failed to open terminal workspace window");
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_snapshot() -> TerminalWorkspaceSnapshot {
        TerminalWorkspaceSnapshot {
            version: WORKSPACE_SNAPSHOT_VERSION,
            layout: DockLayout::new(DockNode::Tabs(DockTabs::new(
                "main",
                vec![DockTab::new("terminal-1", "Terminal 1", "default")],
            ))),
            panes: vec![TerminalPaneSnapshot {
                id: "terminal-1".to_string(),
                title: "Terminal 1".to_string(),
                shell: "default".to_string(),
                working_directory: Some(PathBuf::from("/tmp")),
                columns: 120,
                rows: 32,
                lifecycle: TerminalPaneLifecycleSnapshot::Running,
                dirty: true,
                busy: false,
            }],
            active_stack_id: "main".to_string(),
            active_pane_id: "terminal-1".to_string(),
            next_pane: 2,
            next_stack: 2,
        }
    }

    #[test]
    fn workspace_snapshot_roundtrips_layout_and_pane_metadata() {
        let snapshot = sample_snapshot();
        validate_workspace_snapshot(&snapshot).expect("snapshot should be valid");

        let json =
            serde_json::to_string(&snapshot).expect("workspace snapshot should serialize to JSON");
        let restored: TerminalWorkspaceSnapshot =
            serde_json::from_str(&json).expect("workspace snapshot should deserialize from JSON");

        assert_eq!(restored, snapshot);
        assert_eq!(restored.layout.tab_count(), 1);
        assert_eq!(
            restored.panes[0].working_directory,
            Some(PathBuf::from("/tmp"))
        );
    }

    #[test]
    fn workspace_snapshot_rejects_corrupt_identity_and_geometry() {
        let mut mismatched = sample_snapshot();
        mismatched.panes[0].id = "missing-from-layout".to_string();
        assert!(validate_workspace_snapshot(&mismatched).is_err());

        let mut invalid_size = sample_snapshot();
        invalid_size.panes[0].columns = 0;
        assert!(validate_workspace_snapshot(&invalid_size).is_err());

        let mut invalid_active = sample_snapshot();
        invalid_active.active_stack_id = "missing-stack".to_string();
        assert!(validate_workspace_snapshot(&invalid_active).is_err());
    }

    #[test]
    fn workspace_snapshot_atomic_write_and_backup_recovery() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "guic-terminal-workspace-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let path = directory.join("workspace.json");
        let backup = path.with_extension("json.backup");

        let snapshot = sample_snapshot();
        write_workspace_snapshot_to(&path, &snapshot).expect("snapshot should write atomically");
        assert_eq!(
            read_workspace_snapshot_from(&path).expect("snapshot should read"),
            snapshot
        );
        assert!(!path.with_extension("json.tmp").exists());

        fs::copy(&path, &backup).expect("backup should be created");
        fs::write(&path, b"not valid JSON").expect("primary should be corrupted for the test");
        assert_eq!(
            read_workspace_snapshot_from(&path).expect("backup should recover"),
            snapshot
        );

        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }
}
