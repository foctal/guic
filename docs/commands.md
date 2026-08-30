# Application Commands

`CommandRouter` provides a deterministic registry for application commands and
keyboard shortcuts. It keeps menus, command palettes, Dock, editors, terminals,
and dialogs on one command model.

```rust
use gpui::Keystroke;
use guic::prelude::{CommandRouter, CommandScope, CommandSpec};

let mut commands = CommandRouter::new();
commands.register(
    CommandSpec::new("workspace.save", "Save workspace")
        .shortcut(Keystroke::parse("secondary-s")?),
)?;
commands.register(
    CommandSpec::new("terminal.clear", "Clear terminal")
        .scope(CommandScope::named("terminal"))
        .shortcut(Keystroke::parse("secondary-k")?),
)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Pass active scopes from most specific to least specific when resolving an
incoming keystroke. Named scopes may reuse a global shortcut. The most specific
active named scope wins, followed by less-specific scopes and then the global
command. Duplicate identifiers and conflicting shortcuts within one scope are
rejected during registration. Disabled scoped commands do not shadow an
enabled fallback.

Use GPUI's `secondary-` shortcut modifier for Command on macOS and Control on
Windows and Linux. Keep command execution in application state; the router owns
metadata and resolution, not side effects.
