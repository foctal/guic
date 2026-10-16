use anyhow::{Result, ensure};
use gpui::{App, Bounds, Window, WindowBounds, WindowOptions, point, px, size};
use guic_core::JsonStore;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

/// Persistent window presentation mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SavedWindowMode {
    /// A normal resizable window.
    Windowed,
    /// A maximized window with separately retained restore bounds.
    Maximized,
    /// A fullscreen window with separately retained restore bounds.
    Fullscreen,
}

/// Native-handle-free window state in logical pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedWindow {
    /// Restore rectangle: x, y, width, height in logical pixels.
    pub rect: [f32; 4],
    /// Window presentation mode.
    pub mode: SavedWindowMode,
    /// Stable monitor identity, when provided by the platform.
    pub display: Option<String>,
}

impl SavedWindow {
    /// Captures restore bounds and mode without serializing platform handles.
    pub fn capture(window: &Window, cx: &App) -> Self {
        let bounds = window.window_bounds();
        let rect = bounds.get_bounds();
        Self {
            rect: [
                rect.origin.x.into(),
                rect.origin.y.into(),
                rect.size.width.into(),
                rect.size.height.into(),
            ],
            mode: match bounds {
                WindowBounds::Windowed(_) => SavedWindowMode::Windowed,
                WindowBounds::Maximized(_) => SavedWindowMode::Maximized,
                WindowBounds::Fullscreen(_) => SavedWindowMode::Fullscreen,
            },
            display: window
                .display(cx)
                .and_then(|d| d.uuid().ok())
                .map(|id| id.to_string()),
        }
    }

    /// Fits restore bounds inside a usable display area, including after DPI changes.
    pub fn fit(&self, display: [f32; 4]) -> Result<[f32; 4]> {
        ensure!(
            self.rect
                .iter()
                .chain(display.iter())
                .all(|v| v.is_finite())
                && self.rect[2] > 0.0
                && self.rect[3] > 0.0
                && display[2] > 0.0
                && display[3] > 0.0,
            "window geometry must be finite and positive"
        );
        let width = self.rect[2].min(display[2]);
        let height = self.rect[3].min(display[3]);
        let x = (self.rect[0] as f64).clamp(
            display[0] as f64,
            display[0] as f64 + (display[2] - width) as f64,
        ) as f32;
        let y = (self.rect[1] as f64).clamp(
            display[1] as f64,
            display[1] as f64 + (display[3] - height) as f64,
        ) as f32;
        Ok([x, y, width, height])
    }

    /// Restores on the saved display, or the display nearest the old window.
    pub fn options(&self, cx: &App) -> Result<WindowOptions> {
        let displays = cx.displays();
        let display = displays
            .iter()
            .find(|d| {
                d.uuid().ok().map(|id| id.to_string()) == self.display && self.display.is_some()
            })
            .or_else(|| {
                displays.iter().max_by(|a, b| {
                    overlap(self.rect, a.visible_bounds())
                        .total_cmp(&overlap(self.rect, b.visible_bounds()))
                        .then_with(|| {
                            distance(self.rect, b.visible_bounds())
                                .total_cmp(&distance(self.rect, a.visible_bounds()))
                        })
                })
            })
            .ok_or_else(|| anyhow::anyhow!("no display is available"))?;
        let area = display.visible_bounds();
        let [x, y, width, height] = self.fit([
            area.origin.x.into(),
            area.origin.y.into(),
            area.size.width.into(),
            area.size.height.into(),
        ])?;
        let bounds = Bounds::new(point(px(x), px(y)), size(px(width), px(height)));
        Ok(WindowOptions {
            display_id: Some(display.id()),
            window_bounds: Some(match self.mode {
                SavedWindowMode::Windowed => WindowBounds::Windowed(bounds),
                SavedWindowMode::Maximized => WindowBounds::Maximized(bounds),
                SavedWindowMode::Fullscreen => WindowBounds::Fullscreen(bounds),
            }),
            ..Default::default()
        })
    }
}

fn distance(rect: [f32; 4], display: Bounds<gpui::Pixels>) -> f64 {
    let x = rect[0] as f64 + rect[2] as f64 / 2.0
        - f32::from(display.origin.x) as f64
        - f32::from(display.size.width) as f64 / 2.0;
    let y = rect[1] as f64 + rect[3] as f64 / 2.0
        - f32::from(display.origin.y) as f64
        - f32::from(display.size.height) as f64 / 2.0;
    x * x + y * y
}

fn overlap(rect: [f32; 4], display: Bounds<gpui::Pixels>) -> f64 {
    let x = f32::from(display.origin.x) as f64;
    let y = f32::from(display.origin.y) as f64;
    let width = ((rect[0] as f64 + rect[2] as f64).min(x + f32::from(display.size.width) as f64)
        - (rect[0] as f64).max(x))
    .max(0.0);
    let height = ((rect[1] as f64 + rect[3] as f64).min(y + f32::from(display.size.height) as f64)
        - (rect[1] as f64).max(y))
    .max(0.0);
    width * height
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    version: u32,
    windows: BTreeMap<String, SavedWindow>,
    #[serde(skip)]
    revisions: BTreeMap<String, u64>,
}

/// Recoverable window-state storage with serialized writes across tracked windows.
#[derive(Clone)]
pub struct WindowStateStore {
    store: JsonStore,
    state: Arc<Mutex<Snapshot>>,
    next_revision: Arc<std::sync::atomic::AtomicU64>,
}

impl WindowStateStore {
    /// Loads the primary file or its backup. Unknown versions are rejected.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let store = JsonStore::new(path);
        let state = store
            .load_recovering::<Snapshot>()?
            .map(|s| s.value)
            .unwrap_or(Snapshot {
                version: 1,
                windows: BTreeMap::new(),
                revisions: BTreeMap::new(),
            });
        ensure!(state.version == 1, "unsupported window state version");
        Ok(Self {
            store,
            state: Arc::new(Mutex::new(state)),
            next_revision: Arc::default(),
        })
    }
    /// Returns the saved geometry for a stable application-defined window ID.
    pub fn get(&self, id: &str) -> Result<Option<SavedWindow>> {
        Ok(self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("window state lock poisoned"))?
            .windows
            .get(id)
            .cloned())
    }
    /// Writes a changed window state, preserving the previous snapshot as backup.
    pub fn save(&self, id: &str, window: SavedWindow) -> Result<()> {
        self.save_revision(id, window, self.revision())
    }
    fn revision(&self) -> u64 {
        self.next_revision
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1
    }
    fn save_revision(&self, id: &str, window: SavedWindow, revision: u64) -> Result<()> {
        ensure!(!id.is_empty(), "window identity cannot be empty");
        window.fit(window.rect)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("window state lock poisoned"))?;
        if state
            .revisions
            .get(id)
            .is_some_and(|saved| *saved >= revision)
        {
            return Ok(());
        }
        if state.windows.get(id) == Some(&window) {
            state.revisions.insert(id.into(), revision);
            return Ok(());
        }
        let previous = state.windows.insert(id.to_owned(), window);
        if let Err(error) = self.store.save(&*state) {
            if let Some(previous) = previous {
                state.windows.insert(id.to_owned(), previous);
            } else {
                state.windows.remove(id);
            }
            return Err(error.into());
        }
        state.revisions.insert(id.into(), revision);
        Ok(())
    }
    /// Saves exact final geometry before an approved close, quit, or restart.
    /// Combine with `track` for crash recovery while the window remains open.
    pub fn bind(&self, id: impl Into<String>, window: &Window, lifecycle: &crate::Lifecycle) {
        let id = id.into();
        let handle = window.window_handle();
        let store = self.clone();
        lifecycle.cleanup(move |reason, cx| {
            if matches!(reason, crate::ShutdownReason::CloseWindow(other) if other != handle) {
                return Box::pin(async { Ok(()) });
            }
            let state = handle.update(cx, |_, window, cx| SavedWindow::capture(window, cx));
            let store = store.clone();
            let id = id.clone();
            let revision = store.revision();
            Box::pin(cx.background_executor().spawn(async move {
                if let Ok(state) = state {
                    store.save_revision(&id, state, revision)?;
                }
                Ok(())
            }))
        });
    }

    /// Tracks a live window, saving changed geometry on a worker every 500 ms.
    /// Retain the task for the window lifetime and use `bind` for final close persistence.
    pub fn track(&self, id: impl Into<String>, window: &Window, cx: &App) -> gpui::Task<()> {
        let id = id.into();
        let store = self.clone();
        let handle = window.window_handle();
        cx.spawn(async move |cx| {
            loop {
                let state = handle.update(cx, |_, window, cx| SavedWindow::capture(window, cx));
                let Ok(state) = state else {
                    break;
                };
                let store = store.clone();
                let id = id.clone();
                let revision = store.revision();
                if let Err(error) = cx
                    .background_executor()
                    .spawn(async move { store.save_revision(&id, state, revision) })
                    .await
                {
                    tracing::error!("could not save window state: {error}");
                }
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_background_save_cannot_overwrite_final_geometry() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let store = WindowStateStore::open(dir.path().join("state.json"))?;
        let old = SavedWindow {
            rect: [0.0, 0.0, 100.0, 100.0],
            mode: SavedWindowMode::Windowed,
            display: None,
        };
        let final_state = SavedWindow {
            rect: [200.0, 50.0, 500.0, 300.0],
            ..old.clone()
        };
        let old_revision = store.revision();
        let final_revision = store.revision();
        store.save_revision("main", final_state.clone(), final_revision)?;
        store.save_revision("main", old, old_revision)?;
        assert_eq!(store.get("main")?, Some(final_state));
        Ok(())
    }
    #[test]
    fn removed_monitor_and_smaller_work_area_keep_windows_visible() -> Result<()> {
        let state = SavedWindow {
            rect: [4000.0, -900.0, 2000.0, 1000.0],
            mode: SavedWindowMode::Maximized,
            display: None,
        };
        assert_eq!(
            state.fit([0.0, 24.0, 1280.0, 696.0])?,
            [0.0, 24.0, 1280.0, 696.0]
        );
        assert!(
            SavedWindow {
                rect: [f32::NAN, 0.0, 10.0, 10.0],
                ..state.clone()
            }
            .fit([0.0, 0.0, 100.0, 100.0])
            .is_err()
        );
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("windows.json");
        let store = WindowStateStore::open(&path)?;
        store.save("main", state.clone())?;
        store.save("other", state.clone())?;
        std::fs::write(&path, "broken")?;
        assert_eq!(WindowStateStore::open(path)?.get("main")?, Some(state));
        Ok(())
    }
}
