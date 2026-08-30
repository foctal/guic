use gpui::{Keystroke, SharedString};
use std::collections::HashSet;

/// Common keyboard navigation metadata.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeyboardNavigation {
    /// Whether the target participates in tab traversal.
    pub tab_stop: bool,
}

/// Scope in which an application command is available.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CommandScope {
    /// A command available regardless of the focused surface.
    Global,
    /// A command available while a named application surface is active.
    Named(SharedString),
}

impl CommandScope {
    /// Creates a named command scope.
    #[must_use]
    pub fn named(name: impl Into<SharedString>) -> Self {
        Self::Named(name.into())
    }
}

/// Metadata for an application command and its optional keyboard shortcut.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    /// Stable command identifier.
    pub id: SharedString,
    /// User-facing command label.
    pub label: SharedString,
    /// Scope in which the command participates in routing.
    pub scope: CommandScope,
    /// Optional GPUI keyboard shortcut.
    pub shortcut: Option<Keystroke>,
    /// Whether the command can currently be resolved.
    pub enabled: bool,
}

impl CommandSpec {
    /// Creates an enabled global command without a shortcut.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            scope: CommandScope::Global,
            shortcut: None,
            enabled: true,
        }
    }

    /// Assigns the command to a scope.
    #[must_use]
    pub fn scope(mut self, scope: CommandScope) -> Self {
        self.scope = scope;
        self
    }

    /// Assigns a parsed GPUI shortcut.
    #[must_use]
    pub fn shortcut(mut self, shortcut: Keystroke) -> Self {
        self.shortcut = Some(normalize_keystroke(shortcut));
        self
    }

    /// Sets whether the command is enabled.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Registration failure reported by [`CommandRouter`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandRegistrationError {
    /// A command already uses the same stable identifier.
    DuplicateId(SharedString),
    /// Two commands in the same scope use the same shortcut.
    ShortcutConflict {
        /// Identifier of the existing command.
        existing_id: SharedString,
        /// Shortcut shared by both commands.
        shortcut: Keystroke,
        /// Scope containing the conflict.
        scope: CommandScope,
    },
}

impl std::fmt::Display for CommandRegistrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(formatter, "command id `{id}` is already registered"),
            Self::ShortcutConflict {
                existing_id,
                shortcut,
                scope,
            } => write!(
                formatter,
                "shortcut `{shortcut}` conflicts with command `{existing_id}` in scope {scope:?}"
            ),
        }
    }
}

impl std::error::Error for CommandRegistrationError {}

/// Deterministic application command and keyboard-shortcut registry.
///
/// Hosts pass active named scopes from most specific to least specific.
/// Routing checks those scopes in order and falls back to global commands.
#[derive(Clone, Debug, Default)]
pub struct CommandRouter {
    commands: Vec<CommandSpec>,
}

impl CommandRouter {
    /// Creates an empty command registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a command after validating identifier and shortcut conflicts.
    pub fn register(&mut self, command: CommandSpec) -> Result<(), CommandRegistrationError> {
        if self
            .commands
            .iter()
            .any(|existing| existing.id == command.id)
        {
            return Err(CommandRegistrationError::DuplicateId(command.id));
        }
        if let Some(shortcut) = command.shortcut.as_ref()
            && let Some(existing) = self.commands.iter().find(|existing| {
                existing.scope == command.scope
                    && existing
                        .shortcut
                        .as_ref()
                        .is_some_and(|candidate| keystrokes_match(candidate, shortcut))
            })
        {
            return Err(CommandRegistrationError::ShortcutConflict {
                existing_id: existing.id.clone(),
                shortcut: shortcut.clone(),
                scope: command.scope,
            });
        }
        self.commands.push(command);
        Ok(())
    }

    /// Removes a command by stable identifier.
    pub fn remove(&mut self, id: &str) -> Option<CommandSpec> {
        let index = self.commands.iter().position(|command| command.id == id)?;
        Some(self.commands.remove(index))
    }

    /// Updates the enabled state of a command.
    ///
    /// Returns `false` when no command has the requested identifier.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        let Some(command) = self.commands.iter_mut().find(|command| command.id == id) else {
            return false;
        };
        command.enabled = enabled;
        true
    }

    /// Returns commands in stable registration order.
    #[must_use]
    pub fn commands(&self) -> &[CommandSpec] {
        &self.commands
    }

    /// Resolves a keystroke against active scopes and then the global scope.
    #[must_use]
    pub fn resolve<'a>(
        &'a self,
        keystroke: &Keystroke,
        active_scopes: &[SharedString],
    ) -> Option<&'a CommandSpec> {
        let keystroke = normalize_keystroke(keystroke.clone());
        let mut visited = HashSet::with_capacity(active_scopes.len());
        for scope in active_scopes {
            if !visited.insert(scope) {
                continue;
            }
            if let Some(command) =
                self.find_enabled(&keystroke, &CommandScope::Named(scope.clone()))
            {
                return Some(command);
            }
        }
        self.find_enabled(&keystroke, &CommandScope::Global)
    }

    fn find_enabled(&self, keystroke: &Keystroke, scope: &CommandScope) -> Option<&CommandSpec> {
        self.commands.iter().find(|command| {
            command.enabled
                && &command.scope == scope
                && command
                    .shortcut
                    .as_ref()
                    .is_some_and(|shortcut| keystrokes_match(shortcut, keystroke))
        })
    }
}

fn normalize_keystroke(mut keystroke: Keystroke) -> Keystroke {
    keystroke.key.make_ascii_lowercase();
    keystroke
}

fn keystrokes_match(left: &Keystroke, right: &Keystroke) -> bool {
    left.modifiers == right.modifiers && left.key.eq_ignore_ascii_case(&right.key)
}

#[cfg(test)]
mod tests {
    use super::{CommandRegistrationError, CommandRouter, CommandScope, CommandSpec};
    use gpui::{Keystroke, SharedString};

    fn key(value: &str) -> Keystroke {
        Keystroke::parse(value).expect("test shortcut should parse")
    }

    #[test]
    fn scoped_commands_override_global_commands() {
        let mut router = CommandRouter::new();
        router
            .register(CommandSpec::new("global.save", "Save").shortcut(key("secondary-s")))
            .expect("global command should register");
        router
            .register(
                CommandSpec::new("editor.save", "Save editor")
                    .scope(CommandScope::named("editor"))
                    .shortcut(key("secondary-s")),
            )
            .expect("scoped command should register");

        assert_eq!(
            router
                .resolve(&key("secondary-s"), &[SharedString::from("editor")])
                .map(|command| command.id.as_ref()),
            Some("editor.save")
        );
        assert_eq!(
            router
                .resolve(&key("secondary-s"), &[])
                .map(|command| command.id.as_ref()),
            Some("global.save")
        );
    }

    #[test]
    fn disabled_scoped_commands_fall_back_to_global() {
        let mut router = CommandRouter::new();
        router
            .register(CommandSpec::new("global.find", "Find").shortcut(key("secondary-f")))
            .expect("global command should register");
        router
            .register(
                CommandSpec::new("terminal.find", "Find terminal")
                    .scope(CommandScope::named("terminal"))
                    .shortcut(key("secondary-f"))
                    .enabled(false),
            )
            .expect("scoped command should register");

        assert_eq!(
            router
                .resolve(&key("secondary-f"), &[SharedString::from("terminal")])
                .map(|command| command.id.as_ref()),
            Some("global.find")
        );
    }

    #[test]
    fn duplicate_ids_and_same_scope_shortcuts_are_rejected() {
        let mut router = CommandRouter::new();
        router
            .register(CommandSpec::new("app.open", "Open").shortcut(key("secondary-o")))
            .expect("first command should register");

        assert_eq!(
            router.register(CommandSpec::new("app.open", "Open again")),
            Err(CommandRegistrationError::DuplicateId("app.open".into()))
        );
        assert!(matches!(
            router.register(
                CommandSpec::new("app.other-open", "Other open").shortcut(key("secondary-o"))
            ),
            Err(CommandRegistrationError::ShortcutConflict { .. })
        ));
    }

    #[test]
    fn routing_is_case_insensitive_and_ignores_repeated_scopes() {
        let mut router = CommandRouter::new();
        router
            .register(
                CommandSpec::new("editor.palette", "Palette")
                    .scope(CommandScope::named("editor"))
                    .shortcut(key("secondary-shift-p")),
            )
            .expect("command should register");

        let mut uppercase = key("secondary-shift-p");
        uppercase.key = "P".to_string();
        assert_eq!(
            router
                .resolve(
                    &uppercase,
                    &[SharedString::from("editor"), SharedString::from("editor")]
                )
                .map(|command| command.id.as_ref()),
            Some("editor.palette")
        );
    }

    #[test]
    fn commands_can_be_disabled_and_removed() {
        let mut router = CommandRouter::new();
        router
            .register(CommandSpec::new("app.close", "Close").shortcut(key("secondary-w")))
            .expect("command should register");
        assert!(router.set_enabled("app.close", false));
        assert!(router.resolve(&key("secondary-w"), &[]).is_none());
        assert_eq!(
            router.remove("app.close").map(|command| command.id),
            Some("app.close".into())
        );
        assert!(!router.set_enabled("missing", true));
    }
}
