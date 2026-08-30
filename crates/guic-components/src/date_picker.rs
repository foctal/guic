use crate::{Button, ButtonVariant, ComponentSize, Label};
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

type DateHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type DateOpenHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// A controlled date picker trigger.
///
/// `DatePicker` renders the current date value and emits an open request. The
/// host owns calendar popover/dialog state and applies the selected date.
#[derive(gpui::IntoElement)]
pub struct DatePicker {
    id: SharedString,
    value: Option<SharedString>,
    placeholder: SharedString,
    open: bool,
    disabled: bool,
    on_request_open: Option<DateHandler>,
    on_open_change: Option<DateOpenHandler>,
    on_change: Option<DateHandler>,
}

impl DatePicker {
    /// Creates an empty date picker.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            value: None,
            placeholder: SharedString::from("Select date"),
            open: false,
            disabled: false,
            on_request_open: None,
            on_open_change: None,
            on_change: None,
        }
    }

    /// Sets the selected date text, typically `YYYY-MM-DD`.
    #[must_use]
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Sets placeholder text shown when no date is selected.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets whether the trigger is disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets whether the calendar surface is open.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Registers a handler for opening the host-owned date selection surface.
    #[must_use]
    pub fn on_request_open(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_request_open = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for calendar open-state changes.
    #[must_use]
    pub fn on_open_change(
        mut self,
        handler: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for selecting a date from the built-in calendar.
    #[must_use]
    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for DatePicker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let has_value = self.value.is_some();
        let value = self.value.unwrap_or_else(|| self.placeholder.clone());
        let mut trigger = div()
            .id(self.id.clone())
            .accessibility(
                AccessibilityProps::new(Role::Button)
                    .label(format!("Choose date, current value: {value}"))
                    .expanded(self.open)
                    .disabled(self.disabled),
            )
            .w_full()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .opacity(if self.disabled { 0.55 } else { 1.0 })
            .child(Label::new(value.clone()).muted(!has_value));

        let button = Button::new("Choose")
            .variant(ButtonVariant::Secondary)
            .size(ComponentSize::Small);
        if !self.disabled && (self.on_request_open.is_some() || self.on_open_change.is_some()) {
            let current = value.clone();
            let request_handler = self.on_request_open.clone();
            let open_handler = self.on_open_change.clone();
            let next_open = !self.open;
            trigger = trigger
                .tab_index(0)
                .key_context("GuicDatePicker")
                .cursor_pointer()
                .on_key_down({
                    let current = current.clone();
                    let request_handler = request_handler.clone();
                    let open_handler = open_handler.clone();
                    move |event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            if let Some(handler) = request_handler.as_ref() {
                                handler(&current, window, cx);
                            }
                            if let Some(handler) = open_handler.as_ref() {
                                handler(&next_open, window, cx);
                            }
                            cx.stop_propagation();
                        }
                    }
                })
                .on_click(move |event: &ClickEvent, window, cx| {
                    let _ = event;
                    if let Some(handler) = request_handler.as_ref() {
                        handler(&current, window, cx);
                    }
                    if let Some(handler) = open_handler.as_ref() {
                        handler(&next_open, window, cx);
                    }
                });
            trigger = trigger.child(button);
        } else {
            trigger = trigger.child(button.disabled(true));
        }

        let mut root = div()
            .relative()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(trigger);
        if self.open && !self.disabled {
            root = root.child(render_calendar(
                &self.id,
                &value,
                self.on_change.clone(),
                self.on_open_change.clone(),
                theme,
            ));
        }
        root
    }
}

fn render_calendar(
    id: &SharedString,
    value: &SharedString,
    on_change: Option<DateHandler>,
    on_open_change: Option<DateOpenHandler>,
    theme: &Theme,
) -> gpui::Div {
    let (year, month, selected_day) = parse_date(value.as_ref()).unwrap_or((2026, 6, 1));
    let days = days_in_month(year, month);
    let first_weekday = weekday_index(year, month, 1);
    let mut grid = div().grid().grid_cols(7).gap_1();
    for weekday in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
        grid = grid.child(
            div()
                .h(px(26.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(theme.typography.text_sm))
                .text_color(theme.muted_foreground())
                .child(weekday),
        );
    }
    for _ in 0..first_weekday {
        grid = grid.child(div().h(px(30.0)));
    }
    for day in 1..=days {
        let date = SharedString::from(format!("{year:04}-{month:02}-{day:02}"));
        let selected = day == selected_day;
        let change_handler = on_change.clone();
        let open_handler = on_open_change.clone();
        let mut button = div()
            .id(format!("{id}-day-{day}"))
            .accessibility(
                AccessibilityProps::new(Role::Button)
                    .label(date.clone())
                    .selected(selected),
            )
            .h(px(30.0))
            .rounded(px(theme.radius.sm))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(theme.typography.text_sm))
            .text_color(if selected {
                theme.background()
            } else {
                theme.foreground()
            })
            .bg(if selected {
                theme.primary()
            } else {
                theme.secondary().opacity(0.12)
            })
            .child(day.to_string());
        if change_handler.is_some() {
            button = button
                .tab_index(0)
                .key_context("GuicDatePickerDay")
                .cursor_pointer()
                .on_key_down({
                    let date = date.clone();
                    let change_handler = change_handler.clone();
                    let open_handler = open_handler.clone();
                    move |event: &KeyDownEvent, window, cx| {
                        let handled = if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            if let Some(handler) = change_handler.as_ref() {
                                handler(&date, window, cx);
                            }
                            if let Some(handler) = open_handler.as_ref() {
                                handler(&false, window, cx);
                            }
                            true
                        } else {
                            let moves = match event.keystroke.key.as_str() {
                                "left" => -(usize::from(day > 1) as isize),
                                "right" => usize::from(day < days) as isize,
                                "up" => -(day.saturating_sub(1).min(7) as isize),
                                "down" => days.saturating_sub(day).min(7) as isize,
                                "home" => -((day - 1) as isize),
                                "end" => (days - day) as isize,
                                _ => return,
                            };
                            crate::move_roving_focus(moves, window, cx);
                            true
                        };
                        if handled {
                            cx.stop_propagation();
                        }
                    }
                })
                .on_click(move |event, window, cx| {
                    let _ = event;
                    if let Some(handler) = change_handler.as_ref() {
                        handler(&date, window, cx);
                    }
                    if let Some(handler) = open_handler.as_ref() {
                        handler(&false, window, cx);
                    }
                });
        }
        grid = grid.child(button);
    }
    div()
        .w_full()
        .rounded(px(theme.radius.md))
        .border_1()
        .border_color(theme.border())
        .bg(theme.background())
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_color(theme.foreground())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(format!("{year:04}-{month:02}")),
        )
        .child(grid)
}

fn parse_date(value: &str) -> Option<(i32, u32, u32)> {
    let mut parts = value.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) {
        return None;
    }
    let max_day = days_in_month(year, month);
    (1..=max_day).contains(&day).then_some((year, month, day))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 30,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn weekday_index(year: i32, month: u32, day: u32) -> u32 {
    let (year, month) = if month < 3 {
        (year - 1, month + 12)
    } else {
        (year, month)
    };
    let k = year % 100;
    let j = year / 100;
    let h = (day as i32 + (13 * (month as i32 + 1)) / 5 + k + k / 4 + j / 4 + 5 * j) % 7;
    ((h + 5) % 7) as u32
}

#[cfg(test)]
mod tests {
    use super::{DatePicker, parse_date};

    #[test]
    fn date_picker_builder_tracks_value() {
        let picker = DatePicker::new("due")
            .value("2026-06-27")
            .placeholder("Due date")
            .disabled(true);
        assert_eq!(picker.value.as_deref(), Some("2026-06-27"));
        assert_eq!(picker.placeholder, "Due date");
        assert!(picker.disabled);
    }

    #[test]
    fn date_parser_rejects_invalid_or_trailing_fields() {
        assert_eq!(parse_date("2024-02-29"), Some((2024, 2, 29)));
        assert_eq!(parse_date("2023-02-29"), None);
        assert_eq!(parse_date("2026-06-00"), None);
        assert_eq!(parse_date("2026-06-27-extra"), None);
    }
}
