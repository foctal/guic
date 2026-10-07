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

Results use a scrollable viewport capped at 360 logical pixels and reduced for
short windows. Rows keep their content height, including wrapped descriptions;
arrow navigation reveals the active row and changing the query returns to the
first match. `max_results` limits the number of matches, not the viewport size.

The search input is the sole owner of the palette's focus handle. Do not also
register that handle on a parent with `track_focus`: two focused accessibility
nodes violate GPUI's tree invariant. The wrapper retains its dialog role and
key context without a second focus registration.
