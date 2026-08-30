use gpui::{App, FocusHandle, Global, Window};
use std::collections::{BTreeMap, BTreeSet};

/// Shared focus manager state.
#[derive(Default)]
pub struct FocusManager {
    next_scope_id: u64,
    scopes: BTreeSet<FocusScopeId>,
    active_scope: Option<FocusScopeId>,
    handles: BTreeMap<FocusScopeId, FocusHandle>,
}

impl Global for FocusManager {}

impl FocusManager {
    /// Returns the registered focus manager.
    #[must_use]
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// Returns the registered focus manager mutably.
    #[must_use]
    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Allocates a new focus scope identifier.
    #[must_use]
    pub fn allocate_scope(&mut self) -> FocusScopeId {
        let id = self.next_scope_id;
        self.next_scope_id = self.next_scope_id.saturating_add(1);
        FocusScopeId(id)
    }

    /// Registers a focus scope.
    pub fn register_scope(&mut self, scope: FocusScopeId) {
        self.scopes.insert(scope);
        self.active_scope.get_or_insert(scope);
    }

    /// Registers a focus scope together with its GPUI focus handle.
    pub fn register_handle(&mut self, scope: FocusScopeId, handle: FocusHandle) {
        self.register_scope(scope);
        self.handles.insert(scope, handle);
    }

    /// Unregisters a focus scope.
    pub fn unregister_scope(&mut self, scope: FocusScopeId) {
        self.scopes.remove(&scope);
        self.handles.remove(&scope);
        if self.active_scope == Some(scope) {
            self.active_scope = self.scopes.iter().next_back().copied();
        }
    }

    /// Returns the active focus scope.
    #[must_use]
    pub fn active_scope(&self) -> Option<FocusScopeId> {
        self.active_scope
    }

    /// Sets the active focus scope if it exists.
    pub fn set_active_scope(&mut self, scope: FocusScopeId) -> bool {
        if self.scopes.contains(&scope) {
            self.active_scope = Some(scope);
            true
        } else {
            false
        }
    }

    /// Moves logical focus within registered scopes.
    #[must_use]
    pub fn move_focus(&mut self, direction: FocusDirection) -> Option<FocusScopeId> {
        let scopes: Vec<_> = self.scopes.iter().copied().collect();
        let current_index = self
            .active_scope
            .and_then(|active| scopes.iter().position(|scope| *scope == active))
            .unwrap_or(0);

        let next_index = match direction {
            FocusDirection::Forward | FocusDirection::Down | FocusDirection::Right => {
                current_index.saturating_add(1)
            }
            FocusDirection::Backward | FocusDirection::Up | FocusDirection::Left => {
                current_index.saturating_sub(1)
            }
        };

        let next = scopes
            .get(next_index)
            .copied()
            .or_else(|| scopes.last().copied());
        if let Some(next) = next {
            self.active_scope = Some(next);
        }
        next
    }

    /// Activates a scope and forwards focus to its GPUI focus handle when known.
    pub fn focus_scope(&mut self, scope: FocusScopeId, window: &mut Window, cx: &mut App) -> bool {
        let Some(handle) = self.handles.get(&scope).cloned() else {
            return false;
        };

        if self.set_active_scope(scope) {
            handle.focus(window, cx);
            true
        } else {
            false
        }
    }

    /// Returns the GPUI focus handle for the given scope, if one is registered.
    #[must_use]
    pub fn handle_for_scope(&self, scope: FocusScopeId) -> Option<FocusHandle> {
        self.handles.get(&scope).cloned()
    }
}

/// A private-id style focus scope handle.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct FocusScopeId(u64);

/// Shared logical focus movement directions.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusDirection {
    /// Move focus to the next item.
    Forward,
    /// Move focus to the previous item.
    Backward,
    /// Move focus upward.
    Up,
    /// Move focus downward.
    Down,
    /// Move focus leftward.
    Left,
    /// Move focus rightward.
    Right,
}

#[cfg(test)]
mod tests {
    use super::{FocusDirection, FocusManager};

    #[test]
    fn tracks_registered_focus_scopes() {
        let mut manager = FocusManager::default();
        let first = manager.allocate_scope();
        let second = manager.allocate_scope();

        manager.register_scope(first);
        manager.register_scope(second);

        assert_eq!(manager.active_scope(), Some(first));
        assert_eq!(manager.move_focus(FocusDirection::Forward), Some(second));
        assert_eq!(manager.active_scope(), Some(second));

        manager.unregister_scope(second);
        assert_eq!(manager.active_scope(), Some(first));
    }
}
