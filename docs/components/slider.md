# Slider

## Purpose

A draggable, keyboard-operable value slider. Unlike most components, `Slider` is
a stateful entity (like `TextInput`): it owns its value and reports changes.

## Import

```rust
use guic::prelude::Slider;
```

## Basic Usage

```rust
let slider = cx.new(|cx| {
    Slider::new("volume", cx)
        .range(0.0, 100.0)
        .step(5.0)
        .value(40.0)
        .on_change(|value, _window, _cx| { /* react */ })
});
```

Store the returned `Entity<Slider>` and render `self.slider.clone()`. Read the
current value with `slider.read(cx).current_value()`.

## Interaction

- Pointer: click or drag along the track to set the value.
- Keyboard (when focused): `Left`/`Down` decrement, `Right`/`Up` increment,
  `Home`/`End` jump to the bounds. A step of `0.0` allows continuous values.
- Keyboard focus is indicated with the active theme's focus-ring color.

Non-finite range, step, and value inputs are ignored so invalid application data
cannot corrupt slider layout or interaction.
