use crate::{Button, ButtonVariant, ComponentSize, Label};
use chrono::Datelike as _;
pub use chrono::{NaiveDate as CalendarDate, Weekday as CalendarWeekday};
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

type DateHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type RangeHandler = Rc<dyn Fn(&DateRangeSelection, &mut Window, &mut App)>;
type DateOpenHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// The controlled selection of an inclusive calendar range.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DateRangeSelection {
    /// Neither endpoint has been chosen.
    #[default]
    Empty,
    /// The next selection completes a range with this anchor.
    Start(CalendarDate),
    /// Both endpoints have been chosen.
    Complete {
        /// First included date.
        start: CalendarDate,
        /// Last included date.
        end: CalendarDate,
    },
}

impl DateRangeSelection {
    /// Returns an inclusive range in chronological order.
    #[must_use]
    pub fn between(a: CalendarDate, b: CalendarDate) -> Self {
        Self::Complete {
            start: a.min(b),
            end: a.max(b),
        }
    }

    fn pick(self, date: CalendarDate) -> Self {
        match self {
            Self::Start(anchor) => Self::between(anchor, date),
            _ => Self::Start(date),
        }
    }

    fn contains(self, date: CalendarDate) -> bool {
        match self {
            Self::Empty => false,
            Self::Start(anchor) => date == anchor,
            Self::Complete { start, end } => (start..=end).contains(&date),
        }
    }

    fn start(self) -> Option<CalendarDate> {
        match self {
            Self::Empty => None,
            Self::Start(start) | Self::Complete { start, .. } => Some(start),
        }
    }
}

#[derive(Clone)]
struct CalendarRules {
    first_weekday: CalendarWeekday,
    enabled: Option<Rc<dyn Fn(CalendarDate) -> bool>>,
    range: Option<DateRangeSelection>,
}

impl CalendarRules {
    fn allows(&self, date: CalendarDate) -> bool {
        (1..=9999).contains(&date.year())
            && self.enabled.as_ref().is_none_or(|enabled| enabled(date))
    }

    fn target(&self, current: CalendarDate, key: &str, shift: bool) -> Option<CalendarDate> {
        let (target, step) = match key {
            "pageup" | "pagedown" => {
                let direction = if key == "pageup" { -1 } else { 1 };
                let (year, month) = shift_month(
                    (current.year(), current.month()),
                    direction * if shift { 12 } else { 1 },
                );
                (
                    CalendarDate::from_ymd_opt(
                        year,
                        month,
                        current.day().min(days_in_month(year, month)),
                    )?,
                    direction,
                )
            }
            "home" => (current.with_day(1)?, 1),
            "end" => (
                current.with_day(days_in_month(current.year(), current.month()))?,
                -1,
            ),
            "left" | "right" | "up" | "down" => {
                let days = match key {
                    "left" => -1,
                    "right" => 1,
                    "up" => -7,
                    _ => 7,
                };
                (
                    current.checked_add_signed(chrono::Duration::days(days))?,
                    days.signum() as i32,
                )
            }
            _ => return None,
        };
        // Bound predicate calls even when a host disables an entire month.
        let mut candidate = target;
        let cross_months = matches!(key, "left" | "right" | "up" | "down");
        for _ in 0..31 {
            if !cross_months && candidate.month() != target.month() {
                break;
            }
            if self.allows(candidate) {
                return Some(candidate);
            }
            candidate = candidate.checked_add_signed(chrono::Duration::days(i64::from(step)))?;
        }
        None
    }
}

/// A controlled date picker with an optional calendar grid.
///
/// The application supplies the date and open state, then applies changes from
/// the selection and open-state callbacks.
#[derive(gpui::IntoElement)]
pub struct DatePicker {
    id: SharedString,
    value: Option<SharedString>,
    placeholder: SharedString,
    open: bool,
    disabled: bool,
    rules: CalendarRules,
    on_range_change: Option<RangeHandler>,
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
            rules: CalendarRules {
                first_weekday: CalendarWeekday::Mon,
                enabled: None,
                range: None,
            },
            on_range_change: None,
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

    /// Shows an inclusive date range. The first activation sets an anchor and
    /// keeps the calendar open. The second completes the range in either order.
    #[must_use]
    pub fn range(mut self, selection: DateRangeSelection) -> Self {
        let supported = |date: CalendarDate| (1..=9999).contains(&date.year());
        self.rules.range = Some(match selection {
            DateRangeSelection::Complete { start, end } if !supported(start) || !supported(end) => {
                DateRangeSelection::Empty
            }
            DateRangeSelection::Start(date) if !supported(date) => DateRangeSelection::Empty,
            DateRangeSelection::Complete { start, end } => DateRangeSelection::between(start, end),
            other => other,
        });
        self
    }

    /// Registers a controlled range selection callback for range mode.
    #[must_use]
    pub fn on_range_change(
        mut self,
        handler: impl Fn(&DateRangeSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_range_change = Some(Rc::new(handler));
        self
    }

    /// Sets the first day shown in each calendar week. Defaults to Monday.
    #[must_use]
    pub fn first_weekday(mut self, weekday: CalendarWeekday) -> Self {
        self.rules.first_weekday = weekday;
        self
    }

    /// Allows only dates accepted by the predicate. Applies to pointer and keyboard selection.
    #[must_use]
    pub fn date_enabled(mut self, enabled: impl Fn(CalendarDate) -> bool + 'static) -> Self {
        self.rules.enabled = Some(Rc::new(enabled));
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

struct CalendarState {
    selected: Option<(i32, u32, u32)>,
    month: (i32, u32),
    trigger: gpui::FocusHandle,
    days: Vec<gpui::FocusHandle>,
}

fn shift_month((year, month): (i32, u32), delta: i32) -> (i32, u32) {
    let index = ((year - 1) * 12 + month as i32 - 1 + delta).clamp(0, 9999 * 12 - 1);
    (index / 12 + 1, (index % 12 + 1) as u32)
}

impl RenderOnce for DatePicker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let selected = match self.rules.range {
            Some(range) => range.start().map(|d| (d.year(), d.month(), d.day())),
            None => self.value.as_deref().and_then(parse_date),
        };
        let calendar =
            _window.use_keyed_state(format!("{}-calendar-state", self.id), cx, |_, cx| {
                let today = chrono::Local::now().date_naive();
                CalendarState {
                    trigger: cx.focus_handle().tab_stop(true),
                    days: (0..31).map(|_| cx.focus_handle().tab_stop(true)).collect(),
                    selected,
                    month: selected
                        .map(|(y, m, _)| (y, m))
                        .unwrap_or((today.year(), today.month())),
                }
            });
        calendar.update(cx, |state, _| {
            if state.selected != selected {
                state.selected = selected;
                if let Some((y, m, _)) = selected {
                    state.month = (y, m);
                }
            }
        });
        let trigger_focus = calendar.read(cx).trigger.clone();
        let theme = Theme::global(cx);
        let display_value = match self.rules.range {
            Some(DateRangeSelection::Empty) => None,
            Some(DateRangeSelection::Start(date)) => Some(format!("{date} – …").into()),
            Some(DateRangeSelection::Complete { start, end }) => {
                Some(format!("{start} – {end}").into())
            }
            None => self.value,
        };
        let has_value = display_value.is_some();
        let value = display_value.unwrap_or_else(|| {
            if self.placeholder != "Select date" {
                return self.placeholder.clone();
            }
            if self.rules.range.is_some() {
                guic_core::translate(cx, "date.range_placeholder", "Select date range")
            } else {
                guic_core::translate(cx, "date.placeholder", "Select date")
            }
        });
        let mut trigger = div()
            .id(self.id.clone())
            .accessible(
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

        let button = Button::new(guic_core::translate(cx, "date.choose", "Choose"))
            .variant(ButtonVariant::Secondary)
            .size(ComponentSize::Small);
        if !self.disabled && (self.on_request_open.is_some() || self.on_open_change.is_some()) {
            let current = value.clone();
            let request_handler = self.on_request_open.clone();
            let open_handler = self.on_open_change.clone();
            let next_open = !self.open;
            trigger = trigger
                .track_focus(&trigger_focus)
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
            let select = if let Some(range) = self.rules.range {
                self.on_range_change.map(|handler| {
                    Rc::new(
                        move |date: CalendarDate, window: &mut Window, cx: &mut App| {
                            let next = range.pick(date);
                            handler(&next, window, cx);
                            matches!(next, DateRangeSelection::Complete { .. })
                        },
                    ) as CalendarSelectionHandler
                })
            } else {
                self.on_change.map(|handler| {
                    Rc::new(
                        move |date: CalendarDate, window: &mut Window, cx: &mut App| {
                            handler(&date.format("%Y-%m-%d").to_string().into(), window, cx);
                            true
                        },
                    ) as CalendarSelectionHandler
                })
            };
            root = root.child(render_calendar(
                &self.id,
                calendar,
                select,
                self.on_open_change.clone(),
                &self.rules,
                theme,
                cx,
            ));
        }
        root
    }
}

type CalendarSelectionHandler = Rc<dyn Fn(CalendarDate, &mut Window, &mut App) -> bool>;

fn render_calendar(
    id: &SharedString,
    calendar: gpui::Entity<CalendarState>,
    on_change: Option<CalendarSelectionHandler>,
    on_open_change: Option<DateOpenHandler>,
    rules: &CalendarRules,
    theme: &Theme,
    cx: &App,
) -> gpui::Div {
    let (year, month) = calendar.read(cx).month;
    let selected = calendar.read(cx).selected;
    let days = days_in_month(year, month);
    let start = rules.first_weekday.num_days_from_monday();
    let first_weekday = (weekday_index(year, month, 1) + 7 - start) % 7;
    let mut grid = div().grid().grid_cols(7).gap_1();
    let weekdays = [
        ("date.mon", "Mon"),
        ("date.tue", "Tue"),
        ("date.wed", "Wed"),
        ("date.thu", "Thu"),
        ("date.fri", "Fri"),
        ("date.sat", "Sat"),
        ("date.sun", "Sun"),
    ];
    for i in 0..7 {
        let (key, weekday) = weekdays[(i + start as usize) % 7];
        grid = grid.child(
            div()
                .h(px(26.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(theme.typography.text_sm))
                .text_color(theme.muted_foreground())
                .child(guic_core::translate(cx, key, weekday)),
        );
    }
    for _ in 0..first_weekday {
        grid = grid.child(div().h(px(30.0)));
    }
    for day in 1..=days {
        let date = SharedString::from(format!("{year:04}-{month:02}-{day:02}"));
        let current = CalendarDate::from_ymd_opt(year, month, day).expect("valid calendar day");
        let selected = rules
            .range
            .map_or(selected == Some((year, month, day)), |range| {
                range.contains(current)
            });
        let enabled = rules.allows(current);
        let change_handler = on_change.clone();
        let open_handler = on_open_change.clone();
        let mut button = div()
            .id(format!("{id}-day-{day}"))
            .debug_selector(move || format!("guic-date-{year:04}-{month:02}-{day:02}"))
            .opacity(if enabled { 1. } else { 0.45 })
            .accessible(
                AccessibilityProps::new(Role::Button)
                    .label(date.clone())
                    .disabled(!enabled)
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
        if enabled && change_handler.is_some() {
            let rules = rules.clone();
            let trigger = calendar.read(cx).trigger.clone();
            let day_focus = calendar.read(cx).days[(day - 1) as usize].clone();
            button = button
                .track_focus(&day_focus)
                .key_context("GuicDatePickerDay")
                .cursor_pointer()
                .on_key_down({
                    let calendar = calendar.clone();
                    let trigger = trigger.clone();
                    let change_handler = change_handler.clone();
                    let open_handler = open_handler.clone();
                    move |event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" {
                            trigger.focus(window, cx);
                            if let Some(handler) = &open_handler {
                                handler(&false, window, cx);
                            }
                            cx.stop_propagation();
                            return;
                        }
                        let key = event.keystroke.key.as_str();
                        let handled = if matches!(key, "enter" | "space") {
                            if change_handler
                                .as_ref()
                                .is_some_and(|select| select(current, window, cx))
                            {
                                trigger.focus(window, cx);
                                if let Some(handler) = open_handler.as_ref() {
                                    handler(&false, window, cx);
                                }
                            }
                            true
                        } else if matches!(
                            key,
                            "pageup"
                                | "pagedown"
                                | "home"
                                | "end"
                                | "left"
                                | "right"
                                | "up"
                                | "down"
                        ) {
                            if let Some(next) =
                                rules.target(current, key, event.keystroke.modifiers.shift)
                            {
                                let focus = calendar.update(cx, |state, cx| {
                                    let month = (next.year(), next.month());
                                    if state.month != month {
                                        state.month = month;
                                        cx.notify();
                                    }
                                    state.days[(next.day() - 1) as usize].clone()
                                });
                                focus.focus(window, cx);
                            }
                            true
                        } else {
                            false
                        };
                        if handled {
                            cx.stop_propagation();
                        }
                    }
                })
                .on_click(move |_, window, cx| {
                    if change_handler
                        .as_ref()
                        .is_some_and(|select| select(current, window, cx))
                    {
                        trigger.focus(window, cx);
                        if let Some(handler) = open_handler.as_ref() {
                            handler(&false, window, cx);
                        }
                    } else {
                        day_focus.focus(window, cx);
                    }
                });
        }
        grid = grid.child(button);
    }
    let previous = calendar.clone();
    let next = calendar;
    let heading = div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            Button::new("<")
                .id(format!("{id}-previous-month"))
                .accessible_label(guic_core::translate(
                    cx,
                    "date.previous_month",
                    "Previous month",
                ))
                .disabled((year, month) == (1, 1))
                .on_click(move |_, _, cx| {
                    previous.update(cx, |state, cx| {
                        state.month = shift_month(state.month, -1);
                        cx.notify();
                    });
                }),
        )
        .child(Label::new(format!("{year:04}-{month:02}")))
        .child(
            Button::new(">")
                .id(format!("{id}-next-month"))
                .accessible_label(guic_core::translate(cx, "date.next_month", "Next month"))
                .disabled((year, month) == (9999, 12))
                .on_click(move |_, _, cx| {
                    next.update(cx, |state, cx| {
                        state.month = shift_month(state.month, 1);
                        cx.notify();
                    });
                }),
        );
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
        .child(heading)
        .child(grid)
}

fn parse_date(value: &str) -> Option<(i32, u32, u32)> {
    let mut parts = value.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
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
    use super::{DatePicker, parse_date, shift_month, weekday_index};

    #[test]
    fn keyboard_targets_cross_months_and_skip_disabled_dates() {
        use super::{CalendarDate, CalendarRules, CalendarWeekday};
        use chrono::Datelike as _;
        let picker = DatePicker::new("d")
            .first_weekday(CalendarWeekday::Sun)
            .date_enabled(|d| d.weekday().num_days_from_monday() < 5);
        let date = |y, m, d| CalendarDate::from_ymd_opt(y, m, d).expect("valid test fixture");
        assert_eq!(
            picker.rules.target(date(2024, 5, 31), "right", false),
            Some(date(2024, 6, 3))
        );
        assert_eq!(
            picker.rules.target(date(2024, 6, 3), "up", false),
            Some(date(2024, 5, 27))
        );
        assert_eq!(
            picker.rules.target(date(2024, 3, 29), "pageup", false),
            Some(date(2024, 2, 29))
        );
        let blocked = CalendarRules {
            first_weekday: CalendarWeekday::Mon,
            enabled: Some(std::rc::Rc::new(|_| false)),
            range: None,
        };
        assert!(blocked.target(date(2024, 2, 1), "end", false).is_none());
    }

    #[gpui::test]
    fn range_picker_keeps_the_anchor_open_and_completes_with_keyboard(
        cx: &mut gpui::TestAppContext,
    ) {
        use super::{CalendarDate, DateRangeSelection};
        use gpui::{Context, IntoElement, Modifiers, Render, Window};
        struct Booking {
            selection: DateRangeSelection,
            open: bool,
        }
        impl Render for Booking {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                DatePicker::new("booking")
                    .range(self.selection)
                    .open(self.open)
                    .date_enabled(|date| chrono::Datelike::day(&date) != 18)
                    .on_range_change(cx.listener(|booking, selection, _, cx| {
                        booking.selection = *selection;
                        cx.notify();
                    }))
                    .on_open_change(cx.listener(|booking, open, _, cx| {
                        booking.open = *open;
                        cx.notify();
                    }))
            }
        }
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
        });
        let day = |d| CalendarDate::from_ymd_opt(2024, 5, d).expect("valid booking date");
        let (view, cx) = cx.add_window_view(|_, _| Booking {
            selection: DateRangeSelection::between(day(2), day(7)),
            open: true,
        });
        let b = cx
            .debug_bounds("guic-date-2024-05-19")
            .expect("date visible");
        cx.simulate_click(b.center(), Modifiers::none());
        view.read_with(cx, |v, _| {
            assert!(v.open);
            assert_eq!(v.selection, DateRangeSelection::Start(day(19)));
        });
        let disabled = cx
            .debug_bounds("guic-date-2024-05-18")
            .expect("disabled date visible");
        cx.simulate_click(disabled.center(), Modifiers::none());
        view.read_with(cx, |v, _| {
            assert_eq!(v.selection, DateRangeSelection::Start(day(19)))
        });
        cx.simulate_click(b.center(), Modifiers::none());
        view.update(cx, |v, cx| {
            v.open = true;
            cx.notify();
        });
        cx.simulate_click(b.center(), Modifiers::none());
        cx.simulate_keystrokes("left enter");
        view.read_with(cx, |v, _| {
            assert!(!v.open);
            assert_eq!(v.selection, DateRangeSelection::between(day(17), day(19)));
        });
    }

    #[test]
    fn calendar_navigation_and_boundaries() {
        assert_eq!(shift_month((2024, 1), -1), (2023, 12));
        assert_eq!(shift_month((2024, 12), 1), (2025, 1));
        assert_eq!(shift_month((2024, 2), 12), (2025, 2));
        assert_eq!(shift_month((1, 1), -1), (1, 1));
        assert_eq!(shift_month((9999, 12), 1), (9999, 12));
        assert_eq!(parse_date("2147483647-01-01"), None);
        assert_eq!(parse_date("0000-01-01"), None);
        assert_eq!(weekday_index(2024, 1, 1), 0);
    }

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
