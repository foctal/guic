use crate::{FocusManager, GlobalState, OverlayManager, PlatformCapabilities};
use gpui::App;

/// Initializes the GUIC core runtime.
///
/// The initialization is idempotent and only installs globals that are not yet
/// present in the GPUI application context.
pub fn init(cx: &mut App) {
    if !cx.has_global::<GlobalState>() {
        cx.set_global(GlobalState::default());
    }

    if !cx.has_global::<FocusManager>() {
        cx.set_global(FocusManager::default());
    }

    if !cx.has_global::<OverlayManager>() {
        cx.set_global(OverlayManager::default());
    }

    if !cx.has_global::<PlatformCapabilities>() {
        cx.set_global(PlatformCapabilities::current());
    }
}
