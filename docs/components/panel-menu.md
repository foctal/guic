# PanelMenu

`PanelMenu` is a persistent, vertical navigation surface for sidebars and
settings panels. It is controlled: supply the selected action id and update it
from `on_select`.

```rust,ignore
PanelMenu::new("settings-navigation")
    .items(vec![
        MenuItem::header("Settings"),
        MenuItem::new("general", "General"),
        MenuItem::new("accounts", "Accounts"),
        MenuItem::separator(),
        MenuItem::new("billing", "Billing").disabled(true),
    ])
    .selected(Some(active_section.clone()))
    .on_select(cx.listener(|view, id, _, cx| {
        view.active_section = id.clone();
        cx.notify();
    }));
```

`MenuItem::header` and `MenuItem::separator` organize sections without being
interactive. Disabled actions remain visible and cannot be selected.
