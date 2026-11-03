# Select

`Select` is a controlled dropdown. Supply the selected index and open state,
then update them from `on_select` and `on_toggle`.

```rust
use guic::prelude::{ComponentSize, Select, SelectItem};

Select::new("mode")
    .items(vec![
        SelectItem::new("light", "Light"),
        SelectItem::new("dark", "Dark"),
    ])
    .selected(Some(0))
    .expanded(is_open)
    .size(ComponentSize::Small)
    .on_toggle(|_expanded, _window, _cx| { /* update open state */ })
    .on_select(|_index, _window, _cx| { /* update selected index */ })
```

`size` sets trigger and popup typography. Small options use compact padding.
Long labels truncate in the trigger and wrap in the menu. `empty_message`
sets the text shown for an empty option list.

Only the trigger participates in normal layout. The menu flips upward when
needed, stays within the viewport, and scrolls to the selected option.
`width` sets the trigger width; `menu_width` accepts `MatchTrigger`, `Content`,
`Fixed(Pixels)`, or `MinTriggerWidth`. `max_menu_height` caps the popup height.

Selects share a window-scoped exclusive group by default. Opening one requests
closure of the previous member through its callback. Use `exclusive_group(None)`
to opt out. Outside clicks and selection also request closure through `on_toggle`;
the application must apply these state changes.

## Keyboard

- Enter or Space toggles the menu; Escape closes it.
- Up/Down selects the previous/next enabled option.
- Home/End selects the first/last enabled option.
- Typing a prefix selects the next matching enabled option, wrapping once.

Interactive triggers participate in tab navigation. Use `focusable(handle)`
for programmatic focus and restoration after mouse selection. Set
`accessible_label(...)` when the control needs a label beyond its visible value.
Disabled controls and options cannot be activated.
