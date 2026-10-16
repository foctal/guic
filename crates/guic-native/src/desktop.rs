use crate::{AppIdentity, AppPaths, Lifecycle, SidecarProcesses, WindowStateStore};
use anyhow::Result;
use gpui::{App, Subscription, Window};
use guic_core::JsonStore;
use guic_tokens::{ThemeController, ThemePreference};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferences {
    version: u32,
    theme: ThemePreference,
}

/// Application-owned native state and the standard initialization sequence.
///
/// Construct before entering `Application::run`. Install after `guic::init` and
/// before creating a `Root`. Retain the quit subscription returned by `install`.
/// Menus and activation remain separately owned so multi-instance applications
/// and applications without a tray do not acquire unused platform resources.
pub struct DesktopState {
    /// Stable runtime and packaging identity.
    pub identity: AppIdentity,
    /// Writable platform directories.
    pub paths: AppPaths,
    /// Recoverable window geometry.
    pub windows: WindowStateStore,
    /// Confirmation, persistence and shutdown coordinator.
    pub lifecycle: Lifecycle,
    /// Sidecars cleaned up after an approved application shutdown.
    pub processes: SidecarProcesses,
    preferences: JsonStore,
    preference: ThemePreference,
    monitor: std::cell::RefCell<Option<gpui::Task<()>>>,
}
impl DesktopState {
    /// Opens standard per-user native state directories.
    pub fn open(identity: AppIdentity) -> Result<Self> {
        let paths = AppPaths::discover(&identity)?;
        Self::with_paths(identity, paths)
    }
    /// Opens native state in explicit portable or test directories.
    pub fn with_paths(identity: AppIdentity, paths: AppPaths) -> Result<Self> {
        paths.create()?;
        let preferences = JsonStore::new(paths.config.join("native-preferences.json"));
        let saved = preferences
            .load_recovering::<Preferences>()?
            .map(|loaded| loaded.value)
            .unwrap_or(Preferences {
                version: 1,
                ..Default::default()
            });
        anyhow::ensure!(saved.version == 1, "unsupported native preference version");
        let windows = WindowStateStore::open(paths.config.join("windows.json"))?;
        let lifecycle = Lifecycle::new();
        let processes = SidecarProcesses::default();
        processes.bind(&lifecycle);
        Ok(Self {
            identity,
            paths,
            windows,
            lifecycle,
            processes,
            preferences,
            preference: saved.theme,
            monitor: Default::default(),
        })
    }
    /// Installs identity, theme preference and the final platform-exit cleanup hook.
    /// Call once and retain the returned subscription for the application lifetime.
    pub fn install(&self, cx: &mut App) -> Subscription {
        self.identity.install(cx);
        *self.monitor.borrow_mut() = Some(self.processes.monitor(cx));
        #[cfg(target_os = "linux")]
        if let Some(path) = std::env::var_os("APPIMAGE").map(std::path::PathBuf::from)
            && path.is_absolute()
            && path.is_file()
        {
            cx.set_restart_path(path);
        }
        ThemeController::default()
            .preference(self.preference)
            .install(cx);
        self.lifecycle.install(cx)
    }
    /// Persists and applies the user preference. A save failure leaves UI state unchanged.
    pub fn set_theme(
        &mut self,
        preference: ThemePreference,
        window: &Window,
        cx: &mut App,
    ) -> Result<()> {
        self.preferences.save(&Preferences {
            version: 1,
            theme: preference,
        })?;
        self.preference = preference;
        ThemeController::set_preference(preference, window, cx);
        Ok(())
    }
    /// Returns the saved preference independently from the resolved OS appearance.
    pub fn theme(&self) -> ThemePreference {
        self.preference
    }
    /// Connects close confirmation, final geometry persistence, and crash-recovery tracking.
    /// Retain the task for the window lifetime. Use `windows.get(id)?.options(cx)`
    /// before opening the window to restore its previous geometry.
    pub fn track_window(&self, id: impl Into<String>, window: &Window, cx: &App) -> gpui::Task<()> {
        let id = id.into();
        self.lifecycle.protect_window(window, cx);
        self.windows.bind(id.clone(), window, &self.lifecycle);
        self.windows.track(id, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[gpui::test]
    fn saved_preference_is_restored(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("state directory");
        let identity =
            AppIdentity::new("org.example.preferences", "Preferences", "1").expect("identity");
        let paths = AppPaths::under(dir.path(), &identity).expect("paths");
        let desktop = DesktopState::with_paths(identity.clone(), paths.clone()).expect("state");
        desktop
            .preferences
            .save(&Preferences {
                version: 1,
                theme: ThemePreference::Dark,
            })
            .expect("save");
        let restored = DesktopState::with_paths(identity, paths).expect("restore");
        let subscription = cx.update(|cx| restored.install(cx));
        assert_eq!(restored.theme(), ThemePreference::Dark);
        drop(subscription);
    }
}
