# Command Palette

`CommandPalette` is a stateful, searchable command launcher with prefix/keyword
ranking, a bounded result list, disabled-command filtering, accessible dialog
and listbox semantics, and keyboard operation.

```rust,ignore
let palette = cx.new(|cx| {
    CommandPalette::new("workspace-commands", cx)
        .items(vec![
            CommandPaletteItem::new("file.open", "Open file")
                .shortcut("CmdOrCtrl+O")
                .keywords(["load", "document"]),
            CommandPaletteItem::new("settings.open", "Open settings"),
        ])
        .on_activate(cx.listener(|view, command, window, cx| {
            view.run_command(command.id(), window, cx);
        }))
        .on_dismiss(cx.listener(|view, _, cx| {
            view.palette_open = false;
            cx.notify();
        }))
});
```

Focus the handle returned by `focus_handle()` when the overlay opens. `Up` and
`Down` wrap through visible commands, `Enter` activates the highlighted command,
and `Escape` requests dismissal. The application should register the same
stable identifiers with `guic_core::CommandRouter` so menus, shortcuts, and the
palette share one command implementation.
