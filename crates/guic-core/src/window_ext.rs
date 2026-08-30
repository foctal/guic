//! Window helpers that bridge GUIC runtime state with GPUI windows.

use crate::{FocusManager, FocusScopeId};
use gpui::{App, Window};

/// Convenience methods for GUIC-aware windows.
pub trait WindowExt {
    /// Attempts to focus the given GUIC focus scope.
    fn focus_scope(&mut self, scope: FocusScopeId, cx: &mut App) -> bool;
}

impl WindowExt for Window {
    fn focus_scope(&mut self, scope: FocusScopeId, cx: &mut App) -> bool {
        let handle = {
            let manager = FocusManager::global_mut(cx);
            if !manager.set_active_scope(scope) {
                return false;
            }
            manager.handle_for_scope(scope)
        };

        if let Some(handle) = handle {
            handle.focus(self, cx);
            true
        } else {
            false
        }
    }
}
