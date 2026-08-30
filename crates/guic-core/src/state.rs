use gpui::{App, Global};

/// Shared GUIC global state.
#[derive(Default)]
pub struct GlobalState {
    mounted_roots: usize,
    active_theme_name: Option<String>,
}

impl Global for GlobalState {}

impl GlobalState {
    /// Returns the registered global state.
    #[must_use]
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// Returns the registered global state mutably.
    #[must_use]
    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Returns the number of mounted GUIC roots.
    #[must_use]
    pub fn mounted_roots(&self) -> usize {
        self.mounted_roots
    }

    /// Increments the mounted root count.
    pub fn mount_root(&mut self) {
        self.mounted_roots = self.mounted_roots.saturating_add(1);
    }

    /// Returns the active theme name, if known.
    #[must_use]
    pub fn active_theme_name(&self) -> Option<&str> {
        self.active_theme_name.as_deref()
    }

    /// Sets the active theme name.
    pub fn set_active_theme_name(&mut self, name: impl Into<String>) {
        self.active_theme_name = Some(name.into());
    }
}
