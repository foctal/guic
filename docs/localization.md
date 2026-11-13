# Localization

GUIC exposes an application-owned translation catalog through `guic::core`.
Initialize GUIC, register the application's catalogs, then select the locale:

```rust
use guic::core::{Localization, set_locale, translate};

fn configure(cx: &mut guic::gpui::App) {
    guic::init(cx);
    cx.global_mut::<Localization>().register("en-GB", [
        ("date.placeholder", "Choose a date"),
        ("table.empty", "No records"),
    ]);
    set_locale("en-GB", cx);
    let message = translate(cx, "table.empty", "No rows available");
}
```

Locale identifiers are normalized to lowercase with hyphens. Lookup tries the
full locale, then its language, then the English fallback supplied by the caller.
Calling `set_locale` refreshes all windows. Application-provided labels and date
values remain under application control.

The initial built-in keys are `date.placeholder`, `date.choose`,
`date.previous_month`, `date.next_month`, `date.mon` through `date.sun`,
`number.increment`, `number.decrement`, `table.empty`, `editor.label`, and
`input.pasting`.
Applications can register their own keys and use the same lookup function.

This is a translation catalog, not a locale-aware number/date parser or a plural
rules engine. Date values retain the documented ISO format; numeric input uses
Rust's decimal syntax. Other component copy remains application-configurable
through its existing label and placeholder builders.
