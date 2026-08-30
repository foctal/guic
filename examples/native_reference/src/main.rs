use gpui::{
    AppContext as _, Bounds, Context, FocusHandle, IntoElement, ParentElement as _, Render,
    Styled as _, Window, WindowBounds, WindowOptions, div, px, size,
};
use guic::prelude::{
    Alert, Button, CancellationToken, ChartDataset, ChartInteractionState, ChartOptions,
    ChartPoint, ChartSeries, CodeEditor, CodeEditorOptions, Dialog, Dock, DockCommand, DockLayout,
    DockNode, DockTab, DockTabs, EditorCommand, EditorEdit, EditorSession, FilePicker, Form,
    FormField, JsonStore, Label, LineChart, LoadSource, PathPromptOptions, Progress, Root,
    Terminal, TerminalModel, TerminalOptions, ValidationSeverity,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, rc::Rc, time::Duration};

#[derive(Deserialize, Serialize)]
struct ReferenceSnapshot {
    version: u32,
    dock: String,
    progress: f32,
}

struct InspectorWindow;

impl Render for InspectorWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = guic::tokens::Theme::global(cx);
        div()
            .size_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_3()
            .bg(theme.background())
            .child(Label::new("Native secondary window"))
            .child(Label::new(
                "This window has independent GPUI and GUIC lifecycle state.",
            ))
    }
}

struct NativeReferenceApp {
    dock: DockLayout,
    dock_focus: FocusHandle,
    editor_focus: FocusHandle,
    editor: EditorSession,
    chart: ChartInteractionState,
    terminal: TerminalModel,
    dialog_open: bool,
    progress: f32,
    cancellation: CancellationToken,
    error: Option<String>,
    status: String,
    store: JsonStore,
    light_theme: bool,
}

impl NativeReferenceApp {
    fn new(cx: &mut Context<Self>) -> Self {
        let dock = DockLayout::new(DockNode::horizontal(
            DockNode::Tabs(DockTabs::new(
                "work",
                vec![
                    DockTab::new("dashboard", "Dashboard", "Dashboard"),
                    DockTab::new("settings", "Settings", "Settings"),
                ],
            )),
            DockNode::Tabs(DockTabs::new(
                "tools",
                vec![
                    DockTab::new("editor", "Editor", "Editor"),
                    DockTab::new("terminal", "Terminal", "Terminal"),
                ],
            )),
            560,
        ));
        let mut terminal = TerminalModel::new(72, 12).max_scrollback(2_000);
        terminal.write("\x1b[32mNative reference workspace ready\x1b[0m\r\n");
        let store = JsonStore::new(reference_state_path().join("workspace.json"));
        let mut app = Self {
            dock,
            dock_focus: cx.focus_handle(),
            editor_focus: cx.focus_handle(),
            editor: EditorSession::new(guic::editor::EditorBuffer::from_text(
                "fn main() {\n    println!(\"native GUIC\");\n}",
            )),
            chart: ChartInteractionState::new(),
            terminal,
            dialog_open: false,
            progress: 35.0,
            cancellation: CancellationToken::new(),
            error: None,
            status: "Background work is running".into(),
            store,
            light_theme: false,
        };
        match app.store.load_recovering::<ReferenceSnapshot>() {
            Ok(Some(recovered)) if recovered.value.version == 1 => {
                if let Ok(dock) = DockLayout::from_json(&recovered.value.dock) {
                    app.dock = dock;
                    app.progress = recovered.value.progress.clamp(0.0, 100.0);
                    app.status = match recovered.source {
                        LoadSource::Primary => "Restored primary workspace state".into(),
                        LoadSource::Backup => "Recovered workspace state from backup".into(),
                    };
                }
            }
            Ok(_) => {}
            Err(error) => app.error = Some(format!("State recovery failed: {error}")),
        }
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if !this.cancellation.is_cancelled() && this.progress < 100.0 {
                            this.progress = (this.progress + 0.5).min(100.0);
                            this.status = if this.progress >= 100.0 {
                                "Background work completed".into()
                            } else {
                                "Background work is running".into()
                            };
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        app
    }

    fn snapshot(&self) -> Result<ReferenceSnapshot, String> {
        Ok(ReferenceSnapshot {
            version: 1,
            dock: self.dock.to_json().map_err(|error| error.to_string())?,
            progress: self.progress,
        })
    }
}

fn reference_state_path() -> PathBuf {
    std::env::temp_dir().join("guic-native-reference")
}

fn reference_chart_series() -> ChartSeries {
    LineChart::new("reference-export")
        .options(ChartOptions::default().title("Native workload"))
        .datasets(vec![ChartDataset::new("jobs", "Jobs").points(vec![
            ChartPoint::category("Mon", 42.0),
            ChartPoint::category("Tue", 61.0),
            ChartPoint::category("Wed", 55.0),
            ChartPoint::category("Thu", 78.0),
            ChartPoint::category("Fri", 69.0),
        ])])
        .series()
        .clone()
}

impl Render for NativeReferenceApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = guic::tokens::Theme::global(cx);
        let editor_buffer = self.editor.buffer().clone();
        let editor_selections = self.editor.selections().to_vec();
        let editor_focus = self.editor_focus.clone();
        let terminal = self.terminal.clone();
        let domain = self.chart.domain();
        let status_text = self.status.clone();
        let asset_count = guic::assets::AssetManifest::global(cx).len();
        let editor_edit = Rc::new(cx.listener(|this, edit: &EditorEdit, _, cx| {
            this.editor.apply(edit.clone());
            cx.notify();
        }));
        let editor_command = Rc::new(cx.listener(|this, command: &EditorCommand, _, cx| {
            match command {
                EditorCommand::Undo => {
                    this.editor.undo();
                }
                EditorCommand::Redo => {
                    this.editor.redo();
                }
            }
            cx.notify();
        }));

        let dock = Dock::new("reference-workspace", self.dock.clone())
            .title("Native application workspace")
            .focusable(self.dock_focus.clone())
            .on_command(cx.listener(|this, command: &DockCommand, _, cx| {
                if this.dock.apply(command) {
                    cx.notify();
                }
            }))
            .render_tab_body(move |_selection, tab| match tab.id().as_ref() {
                "dashboard" => div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Label::new("Operational dashboard"))
                    .child(
                        LineChart::new("reference-chart")
                            .options({
                                let options = ChartOptions::default()
                                    .title("Native workload")
                                    .height(220.0)
                                    .values(true);
                                if let Some(domain) = domain {
                                    options.domain(domain)
                                } else {
                                    options
                                }
                            })
                            .datasets(vec![ChartDataset::new("jobs", "Jobs").points(vec![
                                ChartPoint::category("Mon", 42.0),
                                ChartPoint::category("Tue", 61.0),
                                ChartPoint::category("Wed", 55.0),
                                ChartPoint::category("Thu", 78.0),
                                ChartPoint::category("Fri", 69.0),
                            ])]),
                    )
                    .into_any_element(),
                "settings" => Form::new("reference-settings")
                    .label("Application settings")
                    .child(
                        FormField::new(
                            "workspace-name",
                            "Workspace name",
                            Label::new("Native workspace"),
                        )
                        .required(true)
                        .validation(ValidationSeverity::Success, "Valid workspace name"),
                    )
                    .child(
                        FilePicker::new("reference-import")
                            .label("Import data")
                            .on_request_pick(|_, cx| {
                                let _selection = cx.prompt_for_paths(PathPromptOptions {
                                    files: true,
                                    directories: false,
                                    multiple: true,
                                    prompt: Some("Import".into()),
                                });
                            }),
                    )
                    .into_any_element(),
                "editor" => CodeEditor::new("reference-editor", editor_buffer.clone())
                    .selections(editor_selections.clone())
                    .focusable(editor_focus.clone())
                    .options(CodeEditorOptions::default().visible_lines(18))
                    .on_edit({
                        let editor_edit = editor_edit.clone();
                        move |edit, window, cx| editor_edit(edit, window, cx)
                    })
                    .on_command({
                        let editor_command = editor_command.clone();
                        move |command, window, cx| editor_command(command, window, cx)
                    })
                    .into_any_element(),
                "terminal" => Terminal::new("reference-terminal", terminal.clone())
                    .options(TerminalOptions::default().visible_scrollback(4))
                    .into_any_element(),
                _ => Label::new("Unknown workspace surface").into_any_element(),
            });

        let mut status = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new(status_text))
            .child(Progress::new(self.progress).id("reference-progress"));
        if let Some(error) = &self.error {
            status = status.child(Alert::new(error.clone()).title("Operation failed").danger());
        }

        div()
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(theme.background())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(Label::new("GUIC Native Reference").secondary(
                        format!(
                            "Dashboard, forms, native workflows, Dock, editor, terminal, and {asset_count} registered asset",
                        ),
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(Button::new("Cancel work").on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.cancellation.cancel();
                                    this.error = Some("Background work was cancelled safely".into());
                                    cx.notify();
                                },
                            )))
                            .child(Button::new("Retry").on_click(cx.listener(|this, _, _, cx| {
                                this.cancellation = CancellationToken::new();
                                this.error = None;
                                this.progress = 0.0;
                                this.status = "Background work restarted".into();
                                cx.notify();
                            })))
                            .child(Button::new("Save state").on_click(cx.listener(
                                |this, _, _, cx| {
                                    let result = this
                                        .snapshot()
                                        .and_then(|snapshot| {
                                            this.store.save(&snapshot).map_err(|error| error.to_string())
                                        });
                                    match result {
                                        Ok(()) => {
                                            this.error = None;
                                            this.status = format!(
                                                "Saved crash-resistant state to {}",
                                                this.store.path().display()
                                            );
                                        }
                                        Err(error) => this.error = Some(error),
                                    }
                                    cx.notify();
                                },
                            )))
                            .child(Button::new("Export SVG").on_click(cx.listener(
                                |this, _, _, cx| {
                                    let path = reference_state_path().join("workload.svg");
                                    let result = std::fs::create_dir_all(reference_state_path())
                                        .and_then(|()| {
                                            std::fs::write(&path, reference_chart_series().to_svg(960, 540))
                                        });
                                    match result {
                                        Ok(()) => {
                                            this.error = None;
                                            this.status = format!("Exported chart to {}", path.display());
                                        }
                                        Err(error) => this.error = Some(format!("Export failed: {error}")),
                                    }
                                    cx.notify();
                                },
                            )))
                            .child(Button::new("New window").on_click(cx.listener(
                                |_this, _, _, cx| {
                                    let _window = cx.open_window(WindowOptions::default(), |window, cx| {
                                        let inspector = cx.new(|_| InspectorWindow);
                                        cx.new(|cx| Root::new(inspector, window, cx))
                                    });
                                },
                            )))
                            .child(Button::new("Toggle theme").on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.light_theme = !this.light_theme;
                                    cx.set_global(if this.light_theme {
                                        guic::tokens::Theme::light()
                                    } else {
                                        guic::tokens::Theme::dark()
                                    });
                                    cx.refresh_windows();
                                },
                            )))
                            .child(Button::new("Open dialog").primary().on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.dialog_open = true;
                                    cx.notify();
                                },
                            ))),
                    ),
            )
            .child(status)
            .child(div().flex_1().min_h_0().child(dock))
            .child(
                Dialog::new("reference-dialog")
                    .open(self.dialog_open)
                    .title("Native confirmation")
                    .description("This dialog uses the shared GUIC overlay and focus system.")
                    .secondary_label("Close")
                    .on_cancel(cx.listener(|this, _, _, cx| {
                        this.dialog_open = false;
                        cx.notify();
                    })),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut gpui::App| {
        guic::init(cx);
        guic::assets::AssetManifest::global_mut(cx).register(guic::assets::AssetSpec::new(
            "native-reference/chart-export",
            guic::assets::AssetKind::Vector,
            reference_state_path()
                .join("workload.svg")
                .display()
                .to_string(),
        ));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1280.0), px(820.0)),
                cx,
            ))),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("GUIC Native Reference".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            let app = cx.new(NativeReferenceApp::new);
            cx.new(|cx| Root::new(app, window, cx))
        })
        .expect("failed to open native reference application");
    });
}
