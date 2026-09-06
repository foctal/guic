//! Native terminal emulator primitives for GUIC.
//! Protocol, Unicode, and search improvements adapted from muxt-terminal (Apache-2.0).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use gpui::{
    App, Bounds, ClipboardItem, Context, ElementInputHandler, Entity, EntityInputHandler,
    FocusHandle, FontFeatures, FontStyle, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    KeyDownEvent, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    ParentElement as _, Pixels, Point, RenderOnce, ScrollDelta, ScrollWheelEvent, ShapedLine,
    SharedString, StatefulInteractiveElement as _, StrikethroughStyle, Styled as _, TextAlign,
    TextRun, UTF16Selection, UnderlineStyle, Window, canvas, div, fill, font, point, px, size,
};
use guic_tokens::Theme;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::VecDeque,
    io::{ErrorKind, Read, Write},
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, OnceLock, RwLock,
        mpsc::{Receiver, channel, sync_channel},
    },
    thread,
    time::{Duration, Instant},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use vte::{Params, Parser, Perform};

type InputHandler = std::rc::Rc<dyn Fn(&[u8], &mut Window, &mut App)>;
type SelectionHandler = std::rc::Rc<dyn Fn(&TerminalSelection, &mut Window, &mut App)>;
type ViewportScrollHandler = std::rc::Rc<dyn Fn(&isize, &mut Window, &mut App)>;
type ResizeHandler = std::rc::Rc<dyn Fn(&TerminalGridSize, &mut Window, &mut App)>;
type LinkHandler = std::rc::Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type LinkHoverHandler = std::rc::Rc<dyn Fn(&Option<SharedString>, &mut Window, &mut App)>;

/// Shared, mutable terminal state used by hosts that update a model independently
/// from GPUI's render tree. Keeping one stable model avoids copying scrollback into
/// event handlers on every frame.
pub type SharedTerminalModel = Rc<RefCell<TerminalModel>>;

/// Source retained by a terminal surface without copying screen or scrollback data.
#[derive(Clone)]
pub enum TerminalModelSource {
    /// Immutable snapshot, suitable for publishing from background model owners.
    Snapshot(Arc<TerminalModel>),
    /// Mutable model owned on the UI thread. Release mutable borrows before rendering.
    Shared(SharedTerminalModel),
}
impl From<Arc<TerminalModel>> for TerminalModelSource {
    fn from(model: Arc<TerminalModel>) -> Self {
        Self::Snapshot(model)
    }
}
impl From<SharedTerminalModel> for TerminalModelSource {
    fn from(model: SharedTerminalModel) -> Self {
        Self::Shared(model)
    }
}
enum TerminalModelRead<'a> {
    Snapshot(&'a TerminalModel),
    Shared(std::cell::Ref<'a, TerminalModel>),
}
impl std::ops::Deref for TerminalModelRead<'_> {
    type Target = TerminalModel;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Snapshot(model) => model,
            Self::Shared(model) => model,
        }
    }
}
impl TerminalModelSource {
    fn borrow(&self) -> TerminalModelRead<'_> {
        match self {
            Self::Snapshot(model) => TerminalModelRead::Snapshot(model),
            Self::Shared(model) => TerminalModelRead::Shared(model.borrow()),
        }
    }
}

const MAX_OSC_TITLE_BYTES: usize = 4 * 1024;
const MAX_OSC_HYPERLINK_BYTES: usize = 8 * 1024;
const MAX_CELL_GRAPHEME_CHARS: usize = 64;
const MAX_PROTOCOL_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_SEARCH_MATCHES: usize = 100_000;
const MAX_TERMINAL_PASTE_BYTES: usize = 1024 * 1024;

/// Terminal grid dimensions derived from the rendered pane size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalGridSize {
    /// Visible terminal columns.
    pub columns: usize,
    /// Visible terminal rows.
    pub rows: usize,
}

/// A terminal grid position.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerminalPosition {
    /// Zero-based row.
    pub row: usize,
    /// Zero-based column.
    pub column: usize,
}

/// Default base palette, in normal then bright ANSI color order.
pub const DEFAULT_ANSI_PALETTE: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 49, 49),
    (13, 188, 121),
    (229, 229, 16),
    (36, 114, 200),
    (188, 63, 188),
    (17, 168, 205),
    (229, 229, 229),
    (102, 102, 102),
    (241, 76, 76),
    (35, 209, 139),
    (245, 245, 67),
    (59, 142, 234),
    (214, 112, 214),
    (41, 184, 219),
    (255, 255, 255),
];

/// Accepted ANSI base palette formats.
pub struct TerminalPalette([[u8; 3]; 16]);
impl From<[(u8, u8, u8); 16]> for TerminalPalette {
    fn from(palette: [(u8, u8, u8); 16]) -> Self {
        Self(palette.map(|(r, g, b)| [r, g, b]))
    }
}
impl From<[[u8; 3]; 16]> for TerminalPalette {
    fn from(palette: [[u8; 3]; 16]) -> Self {
        Self(palette)
    }
}

/// ANSI terminal colors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalColor {
    /// Black.
    Black,
    /// Red.
    Red,
    /// Green.
    Green,
    /// Yellow.
    Yellow,
    /// Blue.
    Blue,
    /// Magenta.
    Magenta,
    /// Cyan.
    Cyan,
    /// White.
    White,
    /// Bright black.
    BrightBlack,
    /// Bright red.
    BrightRed,
    /// Bright green.
    BrightGreen,
    /// Bright yellow.
    BrightYellow,
    /// Bright blue.
    BrightBlue,
    /// Bright magenta.
    BrightMagenta,
    /// Bright cyan.
    BrightCyan,
    /// Bright white.
    BrightWhite,
    /// 256-color palette index.
    Indexed(u8),
    /// 24-bit RGB color.
    Rgb(u8, u8, u8),
}

/// Styling for a terminal cell.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerminalStyle {
    /// Whether bold text is active.
    pub bold: bool,
    /// Whether faint text is active.
    pub faint: bool,
    /// Whether italic text is active.
    pub italic: bool,
    /// Whether underlined text is active.
    pub underline: bool,
    /// Whether blinking text is requested.
    pub blink: bool,
    /// Whether foreground and background colors are reversed.
    pub inverse: bool,
    /// Whether text is hidden while preserving its occupied cells.
    pub hidden: bool,
    /// Whether struck-through text is active.
    pub strikethrough: bool,
    /// Optional foreground color.
    pub foreground: Option<TerminalColor>,
    /// Optional background color.
    pub background: Option<TerminalColor>,
}

/// Active terminal character set.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerminalCharset {
    /// ASCII/UTF-8 passthrough.
    #[default]
    Ascii,
    /// DEC special graphics character set.
    DecSpecialGraphics,
}

/// Terminal mode flags derived from DEC private modes and related sequences.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalModes {
    /// Whether X10 mouse reporting is enabled.
    pub x10_mouse_reporting: bool,
    /// Whether the cursor should be rendered.
    pub cursor_visible: bool,
    /// Whether cursor blink is requested.
    pub cursor_blink: bool,
    /// Whether bracketed paste mode is enabled.
    pub bracketed_paste: bool,
    /// Whether button-press mouse reporting is enabled.
    pub mouse_button_reporting: bool,
    /// Whether drag mouse reporting is enabled.
    pub mouse_drag_reporting: bool,
    /// Whether all-motion mouse reporting is enabled.
    pub mouse_all_motion_reporting: bool,
    /// Whether focus in/out reporting is enabled.
    pub focus_event_reporting: bool,
    /// Whether UTF-8 mouse coordinate encoding is enabled.
    pub utf8_mouse: bool,
    /// Whether SGR mouse coordinate encoding is enabled.
    pub sgr_mouse: bool,
    /// Whether URXVT mouse coordinate encoding is enabled.
    pub urxvt_mouse: bool,
    /// Whether alternate-scroll mode is enabled.
    pub alternate_scroll: bool,
    /// Whether the alternate screen buffer is active.
    pub alternate_screen: bool,
    /// Current cursor shape requested by the application.
    pub cursor_style: TerminalCursorStyle,
    /// Whether application cursor-key mode is enabled.
    pub application_cursor_keys: bool,
    /// Whether application keypad mode is enabled.
    pub application_keypad: bool,
    /// Whether 132-column mode is requested.
    pub column_mode_132: bool,
    /// Whether reverse video is requested.
    pub reverse_video: bool,
    /// Whether cursor positions are relative to the active scroll region.
    pub origin_mode: bool,
    /// Whether text auto-wraps at the right edge.
    pub auto_wrap: bool,
    /// Whether reverse wraparound is enabled.
    pub reverse_wraparound: bool,
    /// Whether auto-repeat is enabled.
    pub auto_repeat: bool,
    /// Whether left/right margin mode is enabled.
    pub left_right_margin_mode: bool,
    /// Whether meta/alt should be encoded as escape-prefixed input.
    pub meta_sends_escape: bool,
    /// Whether synchronized output mode is active.
    pub synchronized_output: bool,
}

impl Default for TerminalModes {
    fn default() -> Self {
        Self {
            x10_mouse_reporting: false,
            cursor_visible: true,
            cursor_blink: false,
            bracketed_paste: false,
            mouse_button_reporting: false,
            mouse_drag_reporting: false,
            mouse_all_motion_reporting: false,
            focus_event_reporting: false,
            utf8_mouse: false,
            sgr_mouse: false,
            urxvt_mouse: false,
            alternate_scroll: true,
            alternate_screen: false,
            cursor_style: TerminalCursorStyle::Block,
            application_cursor_keys: false,
            application_keypad: false,
            column_mode_132: false,
            reverse_video: false,
            origin_mode: false,
            auto_wrap: true,
            reverse_wraparound: false,
            auto_repeat: true,
            left_right_margin_mode: false,
            meta_sends_escape: true,
            synchronized_output: false,
        }
    }
}

/// Terminal cursor shape requested through DECSCUSR.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerminalCursorStyle {
    /// A block cursor.
    #[default]
    Block,
    /// An underline cursor.
    Underline,
    /// A vertical bar cursor.
    Bar,
}

/// A terminal text selection between two grid positions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSelection {
    /// Selection anchor.
    pub anchor: TerminalPosition,
    /// Selection head.
    pub head: TerminalPosition,
}

/// A text match addressed in the complete scrollback and live-screen history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSearchMatch {
    /// Zero-based row in `scrollback + live screen`.
    pub row: usize,
    /// Inclusive starting terminal column.
    pub start_column: usize,
    /// Inclusive ending terminal column.
    pub end_column: usize,
}

/// Options for bounded terminal-history search.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerminalSearchOptions {
    /// Match letter case exactly.
    pub case_sensitive: bool,
    /// Require word boundaries around every result.
    pub whole_word: bool,
    /// Interpret the query as a regular expression.
    pub regex: bool,
    /// Restrict results to the active terminal selection.
    pub selection_only: bool,
    /// Restrict results to output regions reported by OSC 133 shell integration.
    pub command_output_only: bool,
}

/// A completed command-output region reported by OSC 133 shell integration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalCommandOutput {
    /// Absolute row identity, including evicted scrollback history.
    pub start_row: u64,
    /// Inclusive starting column.
    pub start_column: usize,
    /// Inclusive absolute ending row identity.
    pub end_row: u64,
    /// Inclusive ending column.
    pub end_column: usize,
    /// Exit status reported by the shell, when present.
    pub exit_status: Option<i32>,
    /// Observed time between output start and completion.
    pub duration_ms: u64,
}

/// A bounded terminal notification reported through OSC 9 or OSC 777.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalNotification {
    /// Optional short notification title.
    pub title: SharedString,
    /// Notification body supplied by terminal output.
    pub body: SharedString,
}

impl TerminalSelection {
    /// Creates a selection from two terminal positions.
    #[must_use]
    pub fn new(anchor: TerminalPosition, head: TerminalPosition) -> Self {
        Self { anchor, head }
    }

    /// Returns the normalized inclusive selection bounds.
    #[must_use]
    pub fn bounds(&self) -> (TerminalPosition, TerminalPosition) {
        if (self.anchor.row, self.anchor.column) <= (self.head.row, self.head.column) {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    fn contains(&self, row: usize, column: usize) -> bool {
        let (start, end) = self.bounds();
        (row, column) >= (start.row, start.column) && (row, column) <= (end.row, end.column)
    }
}

/// Keyboard or mouse modifier state for terminal input encoding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerminalInputModifiers {
    /// Whether Shift is active.
    pub shift: bool,
    /// Whether Alt is active.
    pub alt: bool,
    /// Whether Control is active.
    pub control: bool,
}

/// Mouse button identity for terminal mouse reporting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalMouseButton {
    /// Left mouse button.
    Left,
    /// Middle mouse button.
    Middle,
    /// Right mouse button.
    Right,
    /// Wheel up.
    WheelUp,
    /// Wheel down.
    WheelDown,
    /// No button is currently pressed.
    None,
}

/// Mouse event kind for terminal mouse reporting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalMouseEventKind {
    /// Button press.
    Press,
    /// Button release.
    Release,
    /// Button drag.
    Drag,
    /// Pointer move without a button press.
    Move,
}

/// A mouse event encoded for terminal applications.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalMouseEvent {
    /// Zero-based grid row.
    pub row: usize,
    /// Zero-based grid column.
    pub column: usize,
    /// Mouse button.
    pub button: TerminalMouseButton,
    /// Mouse event kind.
    pub kind: TerminalMouseEventKind,
    /// Input modifiers.
    pub modifiers: TerminalInputModifiers,
}

/// Fixed terminal grid metrics used for pointer-to-cell conversion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalGridMetrics {
    /// Bounds of the rendered terminal grid in window coordinates.
    pub bounds: Bounds<Pixels>,
    /// Width of one terminal cell.
    pub cell_width: Pixels,
    /// Height of one terminal row.
    pub line_height: Pixels,
    /// Number of terminal columns.
    pub columns: usize,
    /// Number of rendered terminal rows.
    pub rows: usize,
}

/// Font-derived terminal cell metrics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalFontMetrics {
    /// Measured cell width.
    pub cell_width: Pixels,
    /// Measured line height.
    pub line_height: Pixels,
}

/// A snapshot of rendered terminal input geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalInputSnapshot {
    /// Cursor position in rendered rows, including visible scrollback rows.
    pub cursor: TerminalPosition,
    /// Rendered terminal grid metrics.
    pub metrics: TerminalGridMetrics,
    /// Current terminal modes used to encode committed IME text.
    pub modes: TerminalModes,
}

/// GPUI input-handler state for terminal IME composition.
///
/// Attach this state to [`Terminal::input_state`] when a terminal host needs
/// platform IME composition, marked-text display, and candidate-window
/// placement. The state keeps uncommitted marked text out of the PTY and sends
/// only committed text through the terminal's input callback.
pub struct TerminalInputState {
    snapshot: Option<TerminalInputSnapshot>,
    marked_text: String,
    selected_range: Range<usize>,
    marked_range: Option<Range<usize>>,
    on_input: Option<InputHandler>,
}

impl Default for TerminalInputState {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalInputState {
    /// Creates terminal IME input state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshot: None,
            marked_text: String::new(),
            selected_range: 0..0,
            marked_range: None,
            on_input: None,
        }
    }

    /// Returns the active marked text.
    #[must_use]
    pub fn marked_text(&self) -> &str {
        &self.marked_text
    }

    /// Returns whether platform IME composition is active.
    #[must_use]
    pub fn has_marked_text(&self) -> bool {
        !self.marked_text.is_empty()
    }

    /// Returns the most recent terminal input geometry snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Option<TerminalInputSnapshot> {
        self.snapshot
    }

    fn set_snapshot(&mut self, snapshot: TerminalInputSnapshot) {
        self.snapshot = Some(snapshot);
    }

    fn set_input_handler(&mut self, on_input: Option<InputHandler>) {
        self.on_input = on_input;
    }

    fn set_marked_text(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| byte_range_from_utf16(&self.marked_text, range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.marked_text =
            self.marked_text[..range.start].to_owned() + new_text + &self.marked_text[range.end..];
        if new_text.is_empty() {
            self.marked_range = None;
        } else {
            self.marked_range = Some(range.start..range.start + new_text.len());
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range| byte_range_from_utf16(new_text, range))
            .map(|range| {
                range.start + self.marked_range_start()..range.end + self.marked_range_start()
            })
            .unwrap_or_else(|| {
                let cursor = range.start + new_text.len();
                cursor..cursor
            });
    }

    fn commit_text(&mut self, range_utf16: Option<Range<usize>>, text: &str) -> Vec<u8> {
        let modes = self
            .snapshot
            .map(|snapshot| snapshot.modes)
            .unwrap_or_default();
        let bytes = terminal_text_input_bytes(text, modes);
        if let Some(range_utf16) = range_utf16 {
            let range = byte_range_from_utf16(&self.marked_text, &range_utf16);
            self.marked_text.replace_range(range, "");
        }
        self.marked_text.clear();
        self.selected_range = 0..0;
        self.marked_range = None;
        bytes
    }

    fn marked_range_start(&self) -> usize {
        self.marked_range
            .as_ref()
            .map(|range| range.start)
            .unwrap_or(0)
    }

    fn bounds_for_marked_range(
        &self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
    ) -> Option<Bounds<Pixels>> {
        let snapshot = self.snapshot?;
        let range = byte_range_from_utf16(&self.marked_text, &range_utf16);
        let prefix_width = text_display_width(&self.marked_text[..range.start]);
        let range_width = text_display_width(&self.marked_text[range]).max(1);
        let columns = snapshot.metrics.columns.max(1);
        let cursor_column = snapshot.cursor.column.saturating_add(prefix_width);
        let row = snapshot
            .cursor
            .row
            .saturating_add(cursor_column / columns)
            .min(snapshot.metrics.rows.saturating_sub(1));
        let column = cursor_column % columns;

        Some(Bounds::new(
            point(
                element_bounds.left() + snapshot.metrics.cell_width * column as f32,
                element_bounds.top() + snapshot.metrics.line_height * row as f32,
            ),
            size(
                snapshot.metrics.cell_width * range_width as f32,
                snapshot.metrics.line_height,
            ),
        ))
    }

    fn character_index_for_marked_point(
        &self,
        point: Point<Pixels>,
        element_bounds: Bounds<Pixels>,
    ) -> Option<usize> {
        let snapshot = self.snapshot?;
        let local_x = f32::from(point.x - element_bounds.left()).max(0.0);
        let cursor_x = f32::from(snapshot.metrics.cell_width * snapshot.cursor.column as f32);
        let cell_width = f32::from(snapshot.metrics.cell_width).max(1.0);
        let target_cells = ((local_x - cursor_x).max(0.0) / cell_width).round() as usize;
        let byte_index = byte_index_for_display_width(&self.marked_text, target_cells);
        Some(utf16_offset_for_byte_index(&self.marked_text, byte_index))
    }
}

impl EntityInputHandler for TerminalInputState {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = byte_range_from_utf16(&self.marked_text, &range_utf16);
        actual_range.replace(utf16_range_for_byte_range(&self.marked_text, &range));
        Some(self.marked_text[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: utf16_range_for_byte_range(&self.marked_text, &self.selected_range),
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| utf16_range_for_byte_range(&self.marked_text, range))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked_text.clear();
        self.selected_range = 0..0;
        self.marked_range = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bytes = self.commit_text(range_utf16, text);
        if !bytes.is_empty()
            && let Some(on_input) = self.on_input.clone()
        {
            on_input(&bytes, window, cx);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_marked_text(range_utf16, new_text, new_selected_range_utf16);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.bounds_for_marked_range(range_utf16, element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let snapshot = self.snapshot?;
        self.character_index_for_marked_point(point, snapshot.metrics.bounds)
    }
}

impl TerminalGridMetrics {
    /// Converts a window point into a clamped terminal grid position.
    #[must_use]
    pub fn position_for_point(&self, point: Point<Pixels>) -> TerminalPosition {
        let cell_width = f32::from(self.cell_width).max(1.0);
        let line_height = f32::from(self.line_height).max(1.0);
        let local_x = (f32::from(point.x) - f32::from(self.bounds.origin.x)).max(0.0);
        let local_y = (f32::from(point.y) - f32::from(self.bounds.origin.y)).max(0.0);
        TerminalPosition {
            row: ((local_y / line_height).floor() as usize).min(self.rows.saturating_sub(1)),
            column: ((local_x / cell_width).floor() as usize).min(self.columns.saturating_sub(1)),
        }
    }

    /// Returns the window-coordinate bounds for a clamped terminal cell.
    #[must_use]
    pub fn bounds_for_position(&self, position: TerminalPosition) -> Bounds<Pixels> {
        let row = position.row.min(self.rows.saturating_sub(1));
        let column = position.column.min(self.columns.saturating_sub(1));
        Bounds::new(
            point(
                self.bounds.origin.x + self.cell_width * column as f32,
                self.bounds.origin.y + self.line_height * row as f32,
            ),
            size(self.cell_width, self.line_height),
        )
    }
}

/// One cell in the terminal grid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalCell {
    /// Display text for the cell.
    pub text: SharedString,
    /// Cell style.
    pub style: TerminalStyle,
    /// Optional OSC 8 hyperlink URI attached to the cell.
    pub hyperlink: Option<SharedString>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SavedTerminalCursor {
    position: TerminalPosition,
    style: TerminalStyle,
    charset: TerminalCharset,
    origin_mode: bool,
    auto_wrap: bool,
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self {
            text: SharedString::from(" "),
            style: TerminalStyle::default(),
            hyperlink: None,
        }
    }
}

/// One terminal row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalLine {
    cells: Vec<TerminalCell>,
    wrapped: bool,
}

impl TerminalLine {
    /// Creates an empty line with a fixed column count.
    #[must_use]
    pub fn blank(columns: usize) -> Self {
        Self {
            cells: vec![TerminalCell::default(); columns],
            wrapped: false,
        }
    }

    /// Returns the cells in this line.
    #[must_use]
    pub fn cells(&self) -> &[TerminalCell] {
        &self.cells
    }

    /// Returns the line as plain text.
    #[must_use]
    pub fn text(&self) -> String {
        self.cells
            .iter()
            .map(|cell| cell.text.as_ref())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    /// Returns whether this row is visually wrapped into the next row.
    #[must_use]
    pub fn is_wrapped(&self) -> bool {
        self.wrapped
    }

    fn resize(&mut self, columns: usize) {
        self.cells.resize(columns, TerminalCell::default());
    }

    fn from_cells(columns: usize, cells: &[TerminalCell], wrapped: bool) -> Self {
        let mut line = Self::blank(columns);
        for (target, source) in line.cells.iter_mut().zip(cells.iter()) {
            *target = source.clone();
        }
        line.wrapped = wrapped;
        line
    }

    fn compact_for_scrollback(&mut self) {
        let default = TerminalCell::default();
        while self.cells.last() == Some(&default) {
            self.cells.pop();
        }
        self.cells.shrink_to_fit();
    }
}

/// Transport boundary for PTY-backed terminal integrations.
pub trait TerminalTransport {
    /// Writes bytes to the terminal process.
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()>;

    /// Resizes the terminal process.
    fn resize(&mut self, columns: usize, rows: usize) -> std::io::Result<()>;
}

/// Exit information for a local terminal process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalExitStatus {
    /// Platform process exit code, or `-1` if it could not be read.
    pub code: i32,
}

/// Current lifecycle state for a local PTY session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalProcessStatus {
    /// The process is still running.
    Running,
    /// The process has exited.
    Exited(TerminalExitStatus),
}

/// State of the background PTY output reader.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalReaderStatus {
    /// The reader is still receiving output.
    Running,
    /// The PTY stream reached end-of-file.
    Closed,
    /// Reading failed before the process lifecycle completed.
    Failed(String),
}

/// Host-managed terminal tab activity state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalTabStatus {
    /// Process lifecycle state for the tab.
    pub lifecycle: TerminalProcessStatus,
    /// Whether the tab has unread output since it was last focused.
    pub dirty: bool,
    /// Whether the host is waiting for output after recent user input.
    pub busy: bool,
}

impl TerminalTabStatus {
    /// Creates a running, clean, idle tab status.
    #[must_use]
    pub fn running() -> Self {
        Self {
            lifecycle: TerminalProcessStatus::Running,
            dirty: false,
            busy: false,
        }
    }

    /// Creates an exited tab status.
    #[must_use]
    pub fn exited(code: i32) -> Self {
        Self {
            lifecycle: TerminalProcessStatus::Exited(TerminalExitStatus { code }),
            dirty: false,
            busy: false,
        }
    }

    /// Returns whether the tab process is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        matches!(self.lifecycle, TerminalProcessStatus::Running)
    }
}

/// Close policy for a local terminal process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalCloseMode {
    /// Ask the shell to exit by sending its platform-appropriate exit input.
    Graceful,
    /// Forcefully terminate the child process.
    Force,
}

/// Timing policy for supervised terminal shutdown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalLifecyclePolicy {
    /// Time allowed for a shell to exit after a graceful close request.
    pub graceful_timeout: Duration,
}

impl Default for TerminalLifecyclePolicy {
    fn default() -> Self {
        Self {
            graceful_timeout: Duration::from_secs(2),
        }
    }
}

/// Next operation selected by a [`TerminalLifecycleSupervisor`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalLifecycleAction {
    /// No transport operation is currently required.
    Wait,
    /// Send the platform-appropriate graceful close input.
    RequestGracefulClose,
    /// Forcefully terminate the process after the grace period.
    ForceClose,
    /// The process has exited and shutdown is complete.
    Complete(TerminalExitStatus),
}

/// Host-side state machine for deterministic graceful terminal shutdown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalLifecycleSupervisor {
    policy: TerminalLifecyclePolicy,
    closing_for: Option<Duration>,
    graceful_requested: bool,
    force_requested: bool,
}

impl TerminalLifecycleSupervisor {
    /// Creates an idle supervisor using `policy`.
    #[must_use]
    pub fn new(policy: TerminalLifecyclePolicy) -> Self {
        Self {
            policy,
            closing_for: None,
            graceful_requested: false,
            force_requested: false,
        }
    }

    /// Starts supervised shutdown. Calling this repeatedly is idempotent.
    pub fn begin_close(&mut self) {
        self.closing_for.get_or_insert(Duration::ZERO);
    }

    /// Cancels an in-progress policy state, typically after restarting a pane.
    pub fn reset(&mut self) {
        self.closing_for = None;
        self.graceful_requested = false;
        self.force_requested = false;
    }

    /// Advances shutdown time and returns the next required host operation.
    #[must_use]
    pub fn advance(
        &mut self,
        elapsed: Duration,
        status: TerminalProcessStatus,
    ) -> TerminalLifecycleAction {
        if let TerminalProcessStatus::Exited(status) = status {
            return TerminalLifecycleAction::Complete(status);
        }
        let Some(closing_for) = &mut self.closing_for else {
            return TerminalLifecycleAction::Wait;
        };
        *closing_for = closing_for.saturating_add(elapsed);
        if !self.graceful_requested {
            self.graceful_requested = true;
            return TerminalLifecycleAction::RequestGracefulClose;
        }
        if *closing_for >= self.policy.graceful_timeout && !self.force_requested {
            self.force_requested = true;
            return TerminalLifecycleAction::ForceClose;
        }
        TerminalLifecycleAction::Wait
    }

    /// Returns whether shutdown supervision is active.
    #[must_use]
    pub fn is_closing(&self) -> bool {
        self.closing_for.is_some()
    }
}

/// A shell option that can be launched inside a local PTY.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalShellProfile {
    id: SharedString,
    label: SharedString,
    command: PathBuf,
    available: bool,
    is_default: bool,
}

impl TerminalShellProfile {
    /// Creates a shell profile.
    #[must_use]
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        command: impl Into<PathBuf>,
    ) -> Self {
        let command = command.into();
        let available = command_available(&command);
        Self {
            id: id.into(),
            label: label.into(),
            command,
            available,
            is_default: false,
        }
    }

    /// Marks whether this profile is the host default shell.
    #[must_use]
    pub fn default(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }

    /// Returns the stable profile id.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns the human-readable shell label.
    #[must_use]
    pub fn label(&self) -> &SharedString {
        &self.label
    }

    /// Returns the command used to launch the shell.
    #[must_use]
    pub fn command(&self) -> &std::path::Path {
        &self.command
    }

    /// Returns whether the shell command is available on this host.
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Returns whether this profile represents the host default shell.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.is_default
    }

    /// Returns a diagnostic suitable for host UI when the shell is unavailable.
    #[must_use]
    pub fn unavailable_message(&self) -> Option<String> {
        (!self.available).then(|| {
            format!(
                "Shell `{}` is not available on this host",
                self.command.display()
            )
        })
    }
}

/// Discovers common shell profiles for the current platform.
#[must_use]
pub fn discover_shell_profiles() -> Vec<TerminalShellProfile> {
    if cfg!(target_os = "windows") {
        return windows_shell_profiles();
    }

    unix_shell_profiles()
}

/// Returns the best available default shell command for the current platform.
#[must_use]
pub fn default_shell_command() -> PathBuf {
    discover_shell_profiles()
        .into_iter()
        .find(|profile| profile.is_default() && profile.is_available())
        .or_else(|| {
            discover_shell_profiles()
                .into_iter()
                .find(TerminalShellProfile::is_available)
        })
        .map(|profile| profile.command)
        .unwrap_or_else(|| {
            if cfg!(target_os = "windows") {
                PathBuf::from("cmd.exe")
            } else {
                PathBuf::from("/bin/sh")
            }
        })
}

type OutputNotifier = Arc<dyn Fn() + Send + Sync>;
type SharedOutputNotifier = Arc<RwLock<Option<OutputNotifier>>>;

fn notify_output(notifier: &SharedOutputNotifier) {
    let callback = notifier
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    if let Some(callback) = callback {
        callback();
    }
}

/// Structured PTY process configuration. Arguments and environment are passed directly.
#[derive(Clone, Debug)]
pub struct PtySpawnConfig {
    /// Executable path.
    pub program: PathBuf,
    /// Arguments, without shell parsing.
    pub arguments: Vec<std::ffi::OsString>,
    /// Environment overrides; the parent environment is inherited.
    pub environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    /// Initial working directory.
    pub working_directory: Option<PathBuf>,
}

impl PtySpawnConfig {
    /// Creates a configuration for an executable without implicit shell arguments.
    #[must_use]
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            arguments: Vec::new(),
            environment: Vec::new(),
            working_directory: None,
        }
    }
}

/// Final child-wait result, independent of PTY reader failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalSessionOutcome {
    /// The operating system reported an exit status.
    Exited {
        /// Reported exit code (zero indicates success).
        status: TerminalExitStatus,
        /// Whether the host successfully requested forceful termination.
        termination_requested: bool,
    },
    /// Waiting for the child failed; no exit code was obtained.
    WaitFailed {
        /// Diagnostic supplied by the process backend.
        message: String,
    },
}

/// A local PTY-backed terminal session.
pub struct LocalPtySession {
    command: PathBuf,
    cwd: Option<PathBuf>,
    columns: usize,
    rows: usize,
    notify: SharedOutputNotifier,
    spawn_config: Option<PtySpawnConfig>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    output: Receiver<Vec<u8>>,
    exit: Receiver<Result<i32, String>>,
    outcome: Option<TerminalSessionOutcome>,
    termination_requested: bool,
    reader_errors: Receiver<std::io::Error>,
    reader_state: Arc<RwLock<TerminalReaderStatus>>,
    cached_exit: Option<TerminalExitStatus>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

impl LocalPtySession {
    /// Spawns a program with explicit arguments, environment, and working directory.
    pub fn spawn(config: PtySpawnConfig, columns: usize, rows: usize) -> anyhow::Result<Self> {
        Self::spawn_config_impl(
            config.program.clone(),
            columns,
            rows,
            config.working_directory.clone(),
            None,
            Some(config),
        )
    }

    /// Rebinds output and exit notification, including after moving to another window.
    /// A callback already in flight may finish; subsequent events use the new callback.
    pub fn set_output_notifier(&mut self, notify: impl Fn() + Send + Sync + 'static) {
        *self
            .notify
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Some(Arc::new(notify));
    }

    /// Disconnects future output callbacks.
    pub fn clear_output_notifier(&mut self) {
        *self
            .notify
            .write()
            .unwrap_or_else(|error| error.into_inner()) = None;
    }

    /// Spawns the user's shell inside a local PTY.
    pub fn spawn_shell(columns: usize, rows: usize) -> anyhow::Result<Self> {
        Self::spawn_command(default_shell_command(), columns, rows)
    }

    /// Spawns the user's shell inside a local PTY with an initial working directory.
    pub fn spawn_shell_in_dir(
        columns: usize,
        rows: usize,
        cwd: impl Into<PathBuf>,
    ) -> anyhow::Result<Self> {
        Self::spawn_command_in_dir(default_shell_command(), columns, rows, cwd)
    }

    /// Spawns the user's shell and calls `notify` when output or exit status is available.
    pub fn spawn_shell_with_notifier(
        columns: usize,
        rows: usize,
        notify: impl Fn() + Send + Sync + 'static,
    ) -> anyhow::Result<Self> {
        Self::spawn_command_with_notifier(default_shell_command(), columns, rows, notify)
    }

    /// Spawns a discovered shell profile inside a local PTY.
    pub fn spawn_profile(
        profile: &TerminalShellProfile,
        columns: usize,
        rows: usize,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            profile.is_available(),
            "{}",
            profile
                .unavailable_message()
                .unwrap_or_else(|| "Shell profile is unavailable".to_string())
        );
        Self::spawn_command(profile.command().to_path_buf(), columns, rows)
    }

    /// Spawns a discovered shell profile inside a local PTY with an initial working directory.
    pub fn spawn_profile_in_dir(
        profile: &TerminalShellProfile,
        columns: usize,
        rows: usize,
        cwd: impl Into<PathBuf>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            profile.is_available(),
            "{}",
            profile
                .unavailable_message()
                .unwrap_or_else(|| "Shell profile is unavailable".to_string())
        );
        Self::spawn_command_in_dir(profile.command().to_path_buf(), columns, rows, cwd)
    }

    /// Spawns a command inside a local PTY.
    pub fn spawn_command(
        command: impl Into<PathBuf>,
        columns: usize,
        rows: usize,
    ) -> anyhow::Result<Self> {
        Self::spawn_command_impl(command, columns, rows, None, None)
    }

    /// Spawns a command inside a local PTY with an initial working directory.
    pub fn spawn_command_in_dir(
        command: impl Into<PathBuf>,
        columns: usize,
        rows: usize,
        cwd: impl Into<PathBuf>,
    ) -> anyhow::Result<Self> {
        Self::spawn_command_impl(command, columns, rows, Some(cwd.into()), None)
    }

    /// Spawns a command and calls `notify` when output or exit status is available.
    pub fn spawn_command_with_notifier(
        command: impl Into<PathBuf>,
        columns: usize,
        rows: usize,
        notify: impl Fn() + Send + Sync + 'static,
    ) -> anyhow::Result<Self> {
        Self::spawn_command_impl(command, columns, rows, None, Some(Arc::new(notify)))
    }

    fn spawn_command_impl(
        command: impl Into<PathBuf>,
        columns: usize,
        rows: usize,
        cwd: Option<PathBuf>,
        notify: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> anyhow::Result<Self> {
        Self::spawn_config_impl(command, columns, rows, cwd, notify, None)
    }

    fn spawn_config_impl(
        command: impl Into<PathBuf>,
        columns: usize,
        rows: usize,
        cwd: Option<PathBuf>,
        notify: Option<OutputNotifier>,
        spawn_config: Option<PtySpawnConfig>,
    ) -> anyhow::Result<Self> {
        let notify = Arc::new(RwLock::new(notify));
        let columns = normalize_pty_dimension(columns);
        let rows = normalize_pty_dimension(rows);
        if let Some(cwd) = &cwd {
            anyhow::ensure!(
                cwd.is_dir(),
                "PTY working directory does not exist: {}",
                cwd.display()
            );
        }
        let pty = native_pty_system();
        let pair = pty.openpty(PtySize {
            rows: rows as u16,
            cols: columns as u16,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let command = command.into();
        let mut command_builder = CommandBuilder::new(command.clone());
        if let Some(config) = &spawn_config {
            command_builder.args(&config.arguments);
            for (key, value) in &config.environment {
                command_builder.env(key, value);
            }
        } else {
            configure_shell_command(&mut command_builder, &command);
        }
        if let Some(cwd) = &cwd {
            command_builder.cwd(cwd.as_os_str());
        }
        let mut child = pair.slave.spawn_command(command_builder)?;
        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let killer = child.clone_killer();
        // Bound unread PTY data to about 1 MiB; the producer waits for the consumer.
        let (sender, output) = sync_channel(128);
        let (reader_error_sender, reader_errors) = channel();
        let reader_state = Arc::new(RwLock::new(TerminalReaderStatus::Running));
        let reader_state_sink = reader_state.clone();
        let output_notify = notify.clone();
        thread::spawn(move || {
            let mut buffer = [0_u8; 8192];
            let status = loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break TerminalReaderStatus::Closed,
                    Ok(read) => {
                        if sender.send(buffer[..read].to_vec()).is_err() {
                            break TerminalReaderStatus::Closed;
                        }
                        notify_output(&output_notify);
                    }
                    Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    // Unix PTY masters may report EIO after their slave closes.
                    Err(error) if cfg!(unix) && error.raw_os_error() == Some(5) => {
                        break TerminalReaderStatus::Closed;
                    }
                    Err(error) => {
                        let status = TerminalReaderStatus::Failed(error.to_string());
                        let _ = reader_error_sender.send(error);
                        break status;
                    }
                }
            };
            *reader_state_sink
                .write()
                .unwrap_or_else(|error| error.into_inner()) = status;
            notify_output(&output_notify);
        });
        let (exit_sender, exit) = channel();
        let exit_notify = notify.clone();
        thread::spawn(move || {
            let code = child
                .wait()
                .map(|status| status.exit_code() as i32)
                .map_err(|error| error.to_string());
            let _ = exit_sender.send(code);
            notify_output(&exit_notify);
        });
        Ok(Self {
            command,
            cwd,
            columns,
            rows,
            notify,
            spawn_config,
            master: pair.master,
            writer,
            output,
            exit,
            reader_errors,
            reader_state,
            outcome: None,
            termination_requested: false,
            cached_exit: None,
            killer,
        })
    }

    /// Returns the reader lifecycle independently from the child-wait result.
    #[must_use]
    pub fn reader_status(&self) -> TerminalReaderStatus {
        self.reader_state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// Returns a PTY reader failure independently from child process exit status.
    /// End-of-file is normal and does not produce an error here.
    pub fn take_reader_error(&mut self) -> Option<std::io::Error> {
        self.reader_errors.try_recv().ok()
    }

    /// Drains currently available PTY output.
    pub fn drain_output(&mut self) -> Vec<u8> {
        let mut bytes = Vec::new();
        while let Ok(chunk) = self.output.try_recv() {
            bytes.extend(chunk);
        }
        bytes
    }

    /// Returns the current process lifecycle status.
    #[must_use]
    pub fn process_status(&mut self) -> TerminalProcessStatus {
        match self.try_exit_status() {
            Some(status) => TerminalProcessStatus::Exited(status),
            None => TerminalProcessStatus::Running,
        }
    }

    /// Returns whether the child process is still running.
    #[must_use]
    pub fn is_running(&mut self) -> bool {
        matches!(self.process_status(), TerminalProcessStatus::Running)
    }

    /// Returns the process exit status if the child has exited.
    #[must_use]
    pub fn try_exit_status(&mut self) -> Option<TerminalExitStatus> {
        if self.cached_exit.is_none() {
            self.cached_exit = self.try_outcome().map(|outcome| match outcome {
                TerminalSessionOutcome::Exited { status, .. } => status,
                // Retain the legacy sentinel; new hosts should use try_outcome.
                TerminalSessionOutcome::WaitFailed { .. } => TerminalExitStatus { code: -1 },
            });
        }
        self.cached_exit
    }

    /// Returns the final wait result without turning wait failures into exit codes.
    /// Reader errors remain available separately through `take_reader_error`.
    #[must_use]
    pub fn try_outcome(&mut self) -> Option<TerminalSessionOutcome> {
        if self.outcome.is_none() {
            self.outcome = self.exit.try_recv().ok().map(|result| match result {
                Ok(code) => TerminalSessionOutcome::Exited {
                    status: TerminalExitStatus { code },
                    termination_requested: self.termination_requested,
                },
                Err(message) => TerminalSessionOutcome::WaitFailed { message },
            });
        }
        self.outcome.clone()
    }

    /// Returns the process exit code if the child has exited.
    #[must_use]
    pub fn try_exit_code(&mut self) -> Option<i32> {
        self.try_exit_status().map(|status| status.code)
    }

    /// Requests process shutdown according to `mode`.
    pub fn close(&mut self, mode: TerminalCloseMode) -> std::io::Result<()> {
        match mode {
            TerminalCloseMode::Graceful => {
                let bytes = graceful_close_bytes(&self.command);
                self.write(bytes)
            }
            TerminalCloseMode::Force => self.terminate(),
        }
    }

    /// Applies an operation selected by [`TerminalLifecycleSupervisor`].
    pub fn apply_lifecycle_action(
        &mut self,
        action: TerminalLifecycleAction,
    ) -> std::io::Result<()> {
        match action {
            TerminalLifecycleAction::RequestGracefulClose => {
                self.close(TerminalCloseMode::Graceful)
            }
            TerminalLifecycleAction::ForceClose => self.close(TerminalCloseMode::Force),
            TerminalLifecycleAction::Wait | TerminalLifecycleAction::Complete(_) => Ok(()),
        }
    }

    /// Sends the platform-appropriate shell exit input.
    pub fn request_graceful_close(&mut self) -> std::io::Result<()> {
        self.close(TerminalCloseMode::Graceful)
    }

    /// Forcefully terminates the child process.
    pub fn force_close(&mut self) -> std::io::Result<()> {
        self.close(TerminalCloseMode::Force)
    }

    /// Restarts the session with the original command and the current PTY size.
    pub fn restart(&mut self) -> anyhow::Result<()> {
        if self.is_running() {
            self.terminate()?;
        }
        let replacement = Self::spawn_config_impl(
            self.command.clone(),
            self.columns,
            self.rows,
            self.cwd.clone(),
            self.notify
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone(),
            self.spawn_config.clone(),
        )?;
        *self = replacement;
        Ok(())
    }

    /// Returns the initial working directory configured for this session.
    #[must_use]
    pub fn working_directory(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }

    /// Terminates the child process.
    pub fn terminate(&mut self) -> std::io::Result<()> {
        if matches!(
            self.try_outcome(),
            Some(TerminalSessionOutcome::Exited { .. })
        ) {
            return Ok(());
        }
        match self.killer.kill() {
            Ok(()) => {
                self.termination_requested = true;
                Ok(())
            }
            Err(error)
                if error.kind() == ErrorKind::InvalidInput
                    || error.kind() == ErrorKind::NotFound
                    || error.raw_os_error() == Some(6)
                    || (cfg!(target_os = "windows") && error.raw_os_error() == Some(0)) =>
            {
                Ok(())
            }
            result => result,
        }
    }
}

impl Drop for LocalPtySession {
    fn drop(&mut self) {
        if !matches!(
            self.try_outcome(),
            Some(TerminalSessionOutcome::Exited { .. })
        ) {
            let _ = self.killer.kill();
        }
    }
}

impl TerminalTransport for LocalPtySession {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }

    fn resize(&mut self, columns: usize, rows: usize) -> std::io::Result<()> {
        self.columns = normalize_pty_dimension(columns);
        self.rows = normalize_pty_dimension(rows);
        self.master
            .resize(PtySize {
                rows: self.rows as u16,
                cols: self.columns as u16,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(std::io::Error::other)
    }
}

fn normalize_pty_dimension(value: usize) -> usize {
    value.clamp(1, usize::from(u16::MAX))
}

/// A terminal screen model with scrollback and ANSI parsing.
pub struct TerminalModel {
    columns: usize,
    rows: usize,
    lines: Vec<TerminalLine>,
    scrollback: VecDeque<TerminalLine>,
    max_scrollback: usize,
    cursor: TerminalPosition,
    style: TerminalStyle,
    parser: Parser,
    saved_cursor: Option<SavedTerminalCursor>,
    wrap_next: bool,
    title: SharedString,
    current_directory: Option<SharedString>,
    viewport_offset: usize,
    modes: TerminalModes,
    alternate_lines: Option<(Vec<TerminalLine>, TerminalPosition)>,
    selection: Option<TerminalSelection>,
    scroll_region: Option<(usize, usize)>,
    tab_stops: Vec<bool>,
    charset: TerminalCharset,
    active_hyperlink: Option<SharedString>,
    saved_private_modes: Vec<(u16, bool)>,
    response_bytes: Vec<u8>,
    last_printed_char: Option<char>,
    bell_sequence: u64,
    history_origin: u64,
    search_reset_sequence: u64,
    ambiguous_width_is_wide: bool,
    command_outputs: VecDeque<TerminalCommandOutput>,
    active_command_output: Option<(u64, usize, Instant)>,
    notification: Option<TerminalNotification>,
    notification_sequence: u64,
}

impl Clone for TerminalModel {
    fn clone(&self) -> Self {
        Self {
            columns: self.columns,
            rows: self.rows,
            lines: self.lines.clone(),
            scrollback: self.scrollback.clone(),
            max_scrollback: self.max_scrollback,
            cursor: self.cursor,
            style: self.style,
            parser: Parser::new(),
            saved_cursor: self.saved_cursor,
            wrap_next: self.wrap_next,
            title: self.title.clone(),
            current_directory: self.current_directory.clone(),
            viewport_offset: self.viewport_offset,
            modes: self.modes,
            alternate_lines: self.alternate_lines.clone(),
            selection: self.selection,
            scroll_region: self.scroll_region,
            tab_stops: self.tab_stops.clone(),
            charset: self.charset,
            active_hyperlink: self.active_hyperlink.clone(),
            saved_private_modes: self.saved_private_modes.clone(),
            response_bytes: self.response_bytes.clone(),
            last_printed_char: self.last_printed_char,
            bell_sequence: self.bell_sequence,
            history_origin: self.history_origin,
            search_reset_sequence: self.search_reset_sequence,
            ambiguous_width_is_wide: self.ambiguous_width_is_wide,
            command_outputs: self.command_outputs.clone(),
            active_command_output: self.active_command_output,
            notification: self.notification.clone(),
            notification_sequence: self.notification_sequence,
        }
    }
}

impl std::fmt::Debug for TerminalModel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalModel")
            .field("columns", &self.columns)
            .field("rows", &self.rows)
            .field("cursor", &self.cursor)
            .field("scrollback_len", &self.scrollback.len())
            .field("viewport_offset", &self.viewport_offset)
            .field("title", &self.title)
            .field("current_directory", &self.current_directory)
            .field("modes", &self.modes)
            .field("selection", &self.selection)
            .field("scroll_region", &self.scroll_region)
            .field("charset", &self.charset)
            .field("active_hyperlink", &self.active_hyperlink)
            .field("saved_private_modes", &self.saved_private_modes)
            .field("response_bytes_len", &self.response_bytes.len())
            .field("last_printed_char", &self.last_printed_char)
            .field("bell_sequence", &self.bell_sequence)
            .field("history_origin", &self.history_origin)
            .field("search_reset_sequence", &self.search_reset_sequence)
            .field("ambiguous_width_is_wide", &self.ambiguous_width_is_wide)
            .finish()
    }
}

impl PartialEq for TerminalModel {
    fn eq(&self, other: &Self) -> bool {
        self.columns == other.columns
            && self.rows == other.rows
            && self.lines == other.lines
            && self.scrollback == other.scrollback
            && self.max_scrollback == other.max_scrollback
            && self.cursor == other.cursor
            && self.style == other.style
            && self.saved_cursor == other.saved_cursor
            && self.wrap_next == other.wrap_next
            && self.title == other.title
            && self.current_directory == other.current_directory
            && self.viewport_offset == other.viewport_offset
            && self.modes == other.modes
            && self.alternate_lines == other.alternate_lines
            && self.selection == other.selection
            && self.scroll_region == other.scroll_region
            && self.tab_stops == other.tab_stops
            && self.charset == other.charset
            && self.active_hyperlink == other.active_hyperlink
            && self.saved_private_modes == other.saved_private_modes
            && self.response_bytes == other.response_bytes
            && self.last_printed_char == other.last_printed_char
            && self.bell_sequence == other.bell_sequence
            && self.history_origin == other.history_origin
            && self.search_reset_sequence == other.search_reset_sequence
            && self.ambiguous_width_is_wide == other.ambiguous_width_is_wide
            && self.command_outputs == other.command_outputs
            && self.notification == other.notification
            && self.notification_sequence == other.notification_sequence
    }
}

impl Eq for TerminalModel {}

impl TerminalModel {
    /// Creates a terminal model.
    #[must_use]
    pub fn new(columns: usize, rows: usize) -> Self {
        let columns = columns.max(1);
        let rows = rows.max(1);
        Self {
            columns,
            rows,
            lines: vec![TerminalLine::blank(columns); rows],
            scrollback: VecDeque::new(),
            max_scrollback: 10_000,
            cursor: TerminalPosition::default(),
            style: TerminalStyle::default(),
            parser: Parser::new(),
            saved_cursor: None,
            wrap_next: false,
            title: SharedString::default(),
            current_directory: None,
            viewport_offset: 0,
            modes: TerminalModes::default(),
            alternate_lines: None,
            selection: None,
            scroll_region: None,
            tab_stops: default_tab_stops(columns),
            charset: TerminalCharset::default(),
            active_hyperlink: None,
            saved_private_modes: Vec::new(),
            response_bytes: Vec::new(),
            last_printed_char: None,
            bell_sequence: 0,
            history_origin: 0,
            search_reset_sequence: 0,
            ambiguous_width_is_wide: false,
            command_outputs: VecDeque::new(),
            active_command_output: None,
            notification: None,
            notification_sequence: 0,
        }
    }

    /// Sets the maximum scrollback line count.
    #[must_use]
    pub fn max_scrollback(mut self, max_scrollback: usize) -> Self {
        self.max_scrollback = max_scrollback;
        self
    }

    /// Chooses whether East Asian ambiguous-width characters occupy two cells.
    #[must_use]
    pub fn ambiguous_width(mut self, wide: bool) -> Self {
        self.ambiguous_width_is_wide = wide;
        self
    }

    /// Returns the visible lines.
    #[must_use]
    pub fn lines(&self) -> &[TerminalLine] {
        &self.lines
    }

    /// Returns the visible column count.
    #[must_use]
    pub fn columns(&self) -> usize {
        self.columns
    }

    /// Returns the visible row count.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Returns the scrollback buffer.
    #[must_use]
    pub fn scrollback(&self) -> &VecDeque<TerminalLine> {
        &self.scrollback
    }

    /// Returns a conservative estimate of heap bytes retained by grid lines,
    /// scrollback text, hyperlinks, tab stops, and protocol response buffers.
    ///
    /// Allocator bookkeeping and shared-string reference-count blocks are not
    /// included, so hosts should use this for trends and budgets rather than
    /// process-level accounting.
    #[must_use]
    pub fn estimated_heap_bytes(&self) -> usize {
        let line_bytes = |line: &TerminalLine| {
            line.cells.capacity() * std::mem::size_of::<TerminalCell>()
                + line
                    .cells
                    .iter()
                    .map(|cell| {
                        cell.text.len()
                            + cell
                                .hyperlink
                                .as_ref()
                                .map_or(0, |hyperlink| hyperlink.len())
                    })
                    .sum::<usize>()
        };
        self.lines.iter().map(line_bytes).sum::<usize>()
            + self.scrollback.iter().map(line_bytes).sum::<usize>()
            + self
                .alternate_lines
                .as_ref()
                .map_or(0, |(lines, _)| lines.iter().map(line_bytes).sum())
            + self.tab_stops.capacity() * std::mem::size_of::<bool>()
            + self.response_bytes.capacity()
            + self.title.len()
            + self
                .current_directory
                .as_ref()
                .map_or(0, |directory| directory.len())
            + self
                .active_hyperlink
                .as_ref()
                .map_or(0, |hyperlink| hyperlink.len())
    }

    /// Returns the cursor position.
    #[must_use]
    pub fn cursor(&self) -> TerminalPosition {
        self.cursor
    }

    /// Returns the terminal title reported through OSC sequences.
    #[must_use]
    pub fn title(&self) -> &SharedString {
        &self.title
    }

    /// Returns the current directory reported through OSC 7, when valid.
    #[must_use]
    pub fn current_directory(&self) -> Option<&SharedString> {
        self.current_directory.as_ref()
    }

    /// Returns bounded completed command-output metadata from OSC 133.
    pub fn command_outputs(&self) -> &VecDeque<TerminalCommandOutput> {
        &self.command_outputs
    }

    /// Returns the latest bounded terminal notification and its change sequence.
    pub fn latest_notification(&self) -> Option<(u64, &TerminalNotification)> {
        self.notification
            .as_ref()
            .map(|notification| (self.notification_sequence, notification))
    }

    /// Reveals and selects a completed OSC 133 command-output region.
    pub fn reveal_command_output(&mut self, index: usize) -> bool {
        let Some(output) = self.command_outputs.get(index).copied() else {
            return false;
        };
        let history_end = self
            .history_origin
            .saturating_add((self.scrollback.len() + self.lines.len()) as u64);
        if output.end_row < self.history_origin || output.start_row >= history_end {
            return false;
        }
        let start_row = output.start_row.saturating_sub(self.history_origin) as usize;
        let end_row = output
            .end_row
            .min(history_end.saturating_sub(1))
            .saturating_sub(self.history_origin) as usize;
        let live_start = self
            .scrollback
            .len()
            .saturating_add(self.lines.len())
            .saturating_sub(self.rows);
        let desired_start = end_row
            .saturating_add(1)
            .saturating_sub(self.rows)
            .min(start_row)
            .min(live_start);
        self.viewport_offset = live_start
            .saturating_sub(desired_start)
            .min(self.scrollback.len());
        self.selection = Some(TerminalSelection::new(
            TerminalPosition {
                row: start_row.saturating_sub(desired_start),
                column: output.start_column.min(self.columns.saturating_sub(1)),
            },
            TerminalPosition {
                row: end_row.saturating_sub(desired_start),
                column: output.end_column.min(self.columns.saturating_sub(1)),
            },
        ));
        true
    }

    /// Returns the current terminal mode flags.
    #[must_use]
    pub fn modes(&self) -> TerminalModes {
        self.modes
    }

    /// Returns whether the live grid or cursor currently requests animation.
    #[must_use]
    pub fn has_blinking_content(&self) -> bool {
        self.modes.cursor_blink
            || self
                .lines
                .iter()
                .any(|line| line.cells().iter().any(|cell| cell.style.blink))
    }

    /// Returns a monotonically wrapping sequence incremented for every BEL.
    #[must_use]
    pub fn bell_sequence(&self) -> u64 {
        self.bell_sequence
    }

    /// Returns the total number of history rows currently addressable by search.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.scrollback.len() + self.lines.len()
    }

    /// Returns the number of scrollback rows evicted since model creation.
    #[must_use]
    pub fn history_origin(&self) -> u64 {
        self.history_origin
    }

    /// Returns a sequence changed whenever cached search rows require a full rebuild.
    #[must_use]
    pub fn search_reset_sequence(&self) -> u64 {
        self.search_reset_sequence
    }

    /// Returns the active terminal character set.
    #[must_use]
    pub fn charset(&self) -> TerminalCharset {
        self.charset
    }

    /// Returns the active OSC 8 hyperlink URI, if any.
    #[must_use]
    pub fn active_hyperlink(&self) -> Option<&SharedString> {
        self.active_hyperlink.as_ref()
    }

    /// Returns the current selection, if any.
    #[must_use]
    pub fn selection(&self) -> Option<TerminalSelection> {
        self.selection
    }

    /// Sets the active text selection.
    pub fn set_selection(&mut self, selection: TerminalSelection) {
        self.selection = Some(selection);
    }

    /// Clears the active text selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Selects the complete live terminal grid.
    pub fn select_all(&mut self) {
        self.selection = Some(TerminalSelection::new(
            TerminalPosition { row: 0, column: 0 },
            TerminalPosition {
                row: self.rows.saturating_sub(1),
                column: self.columns.saturating_sub(1),
            },
        ));
    }

    /// Computes single-, double-, or triple-click selection for rendered rows.
    #[must_use]
    pub fn selection_for_click(
        &self,
        position: TerminalPosition,
        click_count: usize,
        visible_scrollback: usize,
    ) -> TerminalSelection {
        if click_count < 2 {
            return TerminalSelection::new(position, position);
        }
        let visible_scrollback = visible_scrollback.min(self.scrollback.len());
        let line = if self.viewport_offset > 0 && visible_scrollback == 0 {
            let total = self.scrollback.len() + self.lines.len();
            let end = total.saturating_sub(self.viewport_offset.min(self.scrollback.len()));
            let start = end.saturating_sub(self.rows);
            let index = start + position.row;
            if index < self.scrollback.len() {
                self.scrollback.get(index)
            } else {
                self.lines.get(index.saturating_sub(self.scrollback.len()))
            }
        } else if position.row < visible_scrollback {
            let start = self.scrollback.len().saturating_sub(visible_scrollback);
            self.scrollback.get(start + position.row)
        } else {
            self.lines.get(position.row - visible_scrollback)
        };
        if click_count >= 3 {
            return TerminalSelection::new(
                TerminalPosition {
                    row: position.row,
                    column: 0,
                },
                TerminalPosition {
                    row: position.row,
                    column: self.columns.saturating_sub(1),
                },
            );
        }
        let Some(line) = line else {
            return TerminalSelection::new(position, position);
        };
        let column = position.column.min(self.columns.saturating_sub(1));
        let class = |cell: &TerminalCell| {
            let text = cell.text.as_ref();
            if text.chars().all(char::is_whitespace) {
                0
            } else if text
                .chars()
                .all(|value| value.is_alphanumeric() || value == '_')
            {
                1
            } else {
                2
            }
        };
        let selected_class = line.cells.get(column).map_or(0, class);
        let mut start = column;
        while start > 0
            && line
                .cells
                .get(start - 1)
                .is_some_and(|cell| class(cell) == selected_class)
        {
            start -= 1;
        }
        let mut end = column;
        while end + 1 < self.columns
            && line
                .cells
                .get(end + 1)
                .is_some_and(|cell| class(cell) == selected_class)
        {
            end += 1;
        }
        TerminalSelection::new(
            TerminalPosition {
                row: position.row,
                column: start,
            },
            TerminalPosition {
                row: position.row,
                column: end,
            },
        )
    }

    /// Returns the OSC 8 hyperlink at a rendered grid position.
    #[must_use]
    pub fn hyperlink_at(
        &self,
        position: TerminalPosition,
        visible_scrollback: usize,
    ) -> Option<&SharedString> {
        let visible_scrollback = visible_scrollback.min(self.scrollback.len());
        let line = if self.viewport_offset > 0 && visible_scrollback == 0 {
            let total = self.scrollback.len() + self.lines.len();
            let end = total.saturating_sub(self.viewport_offset.min(self.scrollback.len()));
            let start = end.saturating_sub(self.rows);
            let index = start + position.row;
            if index < self.scrollback.len() {
                self.scrollback.get(index)
            } else {
                self.lines.get(index.saturating_sub(self.scrollback.len()))
            }
        } else if position.row < visible_scrollback {
            let start = self.scrollback.len().saturating_sub(visible_scrollback);
            self.scrollback.get(start + position.row)
        } else {
            self.lines
                .get(position.row.saturating_sub(visible_scrollback))
        }?;
        line.cells()
            .get(position.column.min(self.columns.saturating_sub(1)))?
            .hyperlink
            .as_ref()
    }

    /// Returns an OSC 8 link or a conservatively detected URL/file path.
    #[must_use]
    pub fn link_at(
        &self,
        position: TerminalPosition,
        visible_scrollback: usize,
    ) -> Option<SharedString> {
        if let Some(link) = self.hyperlink_at(position, visible_scrollback) {
            return Some(link.clone());
        }
        let visible_scrollback = visible_scrollback.min(self.scrollback.len());
        let line = if self.viewport_offset > 0 && visible_scrollback == 0 {
            let total = self.scrollback.len() + self.lines.len();
            let end = total.saturating_sub(self.viewport_offset.min(self.scrollback.len()));
            let start = end.saturating_sub(self.rows);
            let index = start + position.row;
            if index < self.scrollback.len() {
                self.scrollback.get(index)
            } else {
                self.lines.get(index.saturating_sub(self.scrollback.len()))
            }
        } else if position.row < visible_scrollback {
            let start = self.scrollback.len().saturating_sub(visible_scrollback);
            self.scrollback.get(start + position.row)
        } else {
            self.lines
                .get(position.row.saturating_sub(visible_scrollback))
        }?;
        detected_links_in_line(line)
            .into_iter()
            .find(|link| (link.start_column..=link.end_column).contains(&position.column))
            .map(|link| link.target)
    }

    /// Returns the selected text from the current viewport.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let selection = self.selection?;
        let viewport_lines = self.viewport_lines();
        selected_text_from_lines(selection, &viewport_lines, self.columns)
    }

    /// Returns selected text from rendered rows that include visible scrollback.
    #[must_use]
    pub fn selected_text_with_visible_scrollback(
        &self,
        visible_scrollback: usize,
    ) -> Option<String> {
        let selection = self.selection?;
        if self.viewport_offset > 0 {
            return self.selected_text();
        }
        let scrollback_start = self.scrollback.len().saturating_sub(visible_scrollback);
        let rendered_lines = self
            .scrollback
            .iter()
            .skip(scrollback_start)
            .chain(self.lines.iter())
            .cloned()
            .collect::<Vec<_>>();
        selected_text_from_lines(selection, &rendered_lines, self.columns)
    }

    /// Copies the current selection to the platform clipboard.
    pub fn copy_selection_to_clipboard(&self, cx: &mut App) -> bool {
        if let Some(text) = self.selected_text().filter(|text| !text.is_empty()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            true
        } else {
            false
        }
    }

    /// Copies the current rendered selection to the platform clipboard.
    pub fn copy_selection_to_clipboard_with_visible_scrollback(
        &self,
        visible_scrollback: usize,
        cx: &mut App,
    ) -> bool {
        if let Some(text) = self
            .selected_text_with_visible_scrollback(visible_scrollback)
            .filter(|text| !text.is_empty())
        {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            true
        } else {
            false
        }
    }

    /// Returns the current scrollback viewport offset from the bottom.
    #[must_use]
    pub fn viewport_offset(&self) -> usize {
        self.viewport_offset
    }

    /// Returns the maximum scrollback viewport offset.
    #[must_use]
    pub fn max_viewport_offset(&self) -> usize {
        self.scrollback.len()
    }

    /// Scrolls the viewport up into scrollback.
    pub fn scroll_up(&mut self, lines: usize) {
        self.viewport_offset = self
            .viewport_offset
            .saturating_add(lines)
            .min(self.max_viewport_offset());
    }

    /// Scrolls the viewport down toward the live screen.
    pub fn scroll_down(&mut self, lines: usize) {
        self.viewport_offset = self.viewport_offset.saturating_sub(lines);
    }

    /// Scrolls to the live bottom.
    pub fn scroll_to_bottom(&mut self) {
        self.viewport_offset = 0;
    }

    /// Returns visible rows including scrollback viewport state.
    #[must_use]
    pub fn viewport_lines(&self) -> Vec<TerminalLine> {
        if self.viewport_offset == 0 {
            return self.lines.clone();
        }
        let total = self.scrollback.len() + self.lines.len();
        let start = total
            .saturating_sub(self.rows)
            .saturating_sub(self.viewport_offset.min(self.scrollback.len()));
        (0..self.rows)
            .map(|offset| {
                let index = start + offset;
                if index < self.scrollback.len() {
                    self.scrollback[index].clone()
                } else if index < total {
                    self.lines[index - self.scrollback.len()].clone()
                } else {
                    TerminalLine::blank(self.columns)
                }
            })
            .collect()
    }

    /// Finds text across the complete scrollback and live screen.
    #[must_use]
    pub fn search(&self, query: &str, case_sensitive: bool) -> Vec<TerminalSearchMatch> {
        self.search_with_options(
            query,
            TerminalSearchOptions {
                case_sensitive,
                ..TerminalSearchOptions::default()
            },
        )
        .unwrap_or_default()
    }

    /// Finds text with case, whole-word, and regular-expression options.
    pub fn search_with_options(
        &self,
        query: &str,
        options: TerminalSearchOptions,
    ) -> Result<Vec<TerminalSearchMatch>, regex::Error> {
        self.search_from_with_options(query, options, 0)
    }

    /// Finds text at or after a complete-history row for incremental indexing.
    #[must_use]
    pub fn search_from(
        &self,
        query: &str,
        case_sensitive: bool,
        start_row: usize,
    ) -> Vec<TerminalSearchMatch> {
        self.search_from_with_options(
            query,
            TerminalSearchOptions {
                case_sensitive,
                ..TerminalSearchOptions::default()
            },
            start_row,
        )
        .unwrap_or_default()
    }

    /// Searches an immutable snapshot on the supplied background executor.
    ///
    /// Cloning the Arc does not copy scrollback. Hosts should discard stale query
    /// results and compare `history_origin` / `search_reset_sequence` before
    /// applying coordinates to a newer model. Keep at most one active query per
    /// view; synchronous search methods are intended for workers or small buffers.
    pub fn search_async(
        self: Arc<Self>,
        executor: &gpui::BackgroundExecutor,
        query: String,
        options: TerminalSearchOptions,
    ) -> gpui::Task<Result<Vec<TerminalSearchMatch>, regex::Error>> {
        executor.spawn(async move { self.search_with_options(&query, options) })
    }

    /// Finds configurable matches at or after a complete-history row.
    pub fn search_from_with_options(
        &self,
        query: &str,
        options: TerminalSearchOptions,
        start_row: usize,
    ) -> Result<Vec<TerminalSearchMatch>, regex::Error> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let regex = options.regex.then(|| {
            let mut builder = regex::RegexBuilder::new(query);
            builder
                .case_insensitive(!options.case_sensitive)
                .size_limit(1 << 20)
                .dfa_size_limit(1 << 20);
            builder.build()
        });
        let regex = regex.transpose()?;
        let selection_bounds = if options.selection_only {
            let selection = match self.selection {
                Some(selection) => selection,
                None => return Ok(Vec::new()),
            };
            let viewport_start = self
                .scrollback
                .len()
                .saturating_add(self.lines.len())
                .saturating_sub(self.rows)
                .saturating_sub(self.viewport_offset.min(self.scrollback.len()));
            let (start, end) = selection.bounds();
            Some((
                TerminalPosition {
                    row: viewport_start.saturating_add(start.row),
                    column: start.column,
                },
                TerminalPosition {
                    row: viewport_start.saturating_add(end.row),
                    column: end.column,
                },
            ))
        } else {
            None
        };
        let matches = self
            .scrollback
            .iter()
            .chain(self.lines.iter())
            .enumerate()
            .skip(start_row)
            .flat_map(|(row, line)| search_terminal_line(line, row, query, options, regex.as_ref()))
            .filter(|search_match| {
                selection_bounds.is_none_or(|(start, end)| {
                    let match_start = (search_match.row, search_match.start_column);
                    let match_end = (search_match.row, search_match.end_column);
                    match_start >= (start.row, start.column) && match_end <= (end.row, end.column)
                })
            })
            .filter(|search_match| {
                !options.command_output_only || self.match_is_command_output(*search_match)
            })
            .take(MAX_SEARCH_MATCHES)
            .collect::<Vec<_>>();
        Ok(matches)
    }

    fn match_is_command_output(&self, search_match: TerminalSearchMatch) -> bool {
        let start = (
            self.history_origin.saturating_add(search_match.row as u64),
            search_match.start_column,
        );
        let end = (start.0, search_match.end_column);
        self.command_outputs.iter().any(|output| {
            start >= (output.start_row, output.start_column)
                && end <= (output.end_row, output.end_column)
        }) || self
            .active_command_output
            .is_some_and(|(row, column, _)| start >= (row, column))
    }

    fn absolute_cursor_row(&self) -> u64 {
        self.history_origin
            .saturating_add(self.scrollback.len() as u64)
            .saturating_add(self.cursor.row as u64)
    }

    fn begin_command_output(&mut self) {
        self.active_command_output = Some((
            self.absolute_cursor_row(),
            self.cursor.column,
            Instant::now(),
        ));
    }

    fn finish_command_output(&mut self, exit_status: Option<i32>) {
        let Some((start_row, start_column, started_at)) = self.active_command_output.take() else {
            return;
        };
        const MAX_COMMAND_OUTPUTS: usize = 2048;
        if self.command_outputs.len() == MAX_COMMAND_OUTPUTS {
            self.command_outputs.pop_front();
        }
        let end = (self.absolute_cursor_row(), self.cursor.column).max((start_row, start_column));
        self.command_outputs.push_back(TerminalCommandOutput {
            start_row,
            start_column,
            end_row: end.0,
            end_column: end.1,
            exit_status,
            duration_ms: started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        });
    }

    /// Reveals and selects a search result in the current viewport.
    pub fn reveal_search_match(&mut self, search_match: TerminalSearchMatch) -> bool {
        let total_rows = self.scrollback.len() + self.lines.len();
        if search_match.row >= total_rows || search_match.start_column >= self.columns {
            return false;
        }
        let live_start = total_rows.saturating_sub(self.rows);
        self.viewport_offset = live_start
            .saturating_sub(search_match.row)
            .min(self.scrollback.len());
        let viewport_start = total_rows
            .saturating_sub(self.rows)
            .saturating_sub(self.viewport_offset);
        let viewport_row = search_match.row.saturating_sub(viewport_start);
        if viewport_row >= self.rows {
            return false;
        }
        self.selection = Some(TerminalSelection::new(
            TerminalPosition {
                row: viewport_row,
                column: search_match
                    .start_column
                    .min(self.columns.saturating_sub(1)),
            },
            TerminalPosition {
                row: viewport_row,
                column: search_match.end_column.min(self.columns.saturating_sub(1)),
            },
        ));
        true
    }

    /// Writes terminal output into the model.
    pub fn write(&mut self, text: &str) {
        let mut parser = std::mem::take(&mut self.parser);
        {
            let mut performer = TerminalPerformer { model: self };
            parser.advance(&mut performer, text.as_bytes());
        }
        self.parser = parser;
    }

    /// Encodes pasted text using the current bracketed paste mode.
    #[must_use]
    pub fn paste_bytes(&self, text: &str) -> Vec<u8> {
        terminal_paste_bytes(text, self.modes)
    }

    /// Reads clipboard text and encodes it for terminal input.
    #[must_use]
    pub fn clipboard_paste_bytes(&self, cx: &App) -> Option<Vec<u8>> {
        let text = cx.read_from_clipboard()?.text()?;
        Some(self.paste_bytes(&text))
    }

    /// Takes terminal response bytes generated by host-bound VT queries.
    ///
    /// PTY-backed hosts should write these bytes to the transport after feeding
    /// terminal output into [`TerminalModel::write`].
    #[must_use]
    pub fn take_response_bytes(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.response_bytes)
    }

    /// Resizes the visible grid.
    pub fn resize(&mut self, columns: usize, rows: usize) {
        let columns = columns.max(1);
        let rows = rows.max(1);
        if columns != self.columns || rows != self.rows {
            self.search_reset_sequence = self.search_reset_sequence.wrapping_add(1);
        }
        if columns != self.columns {
            self.reflow(columns, rows);
        } else {
            self.columns = columns;
            self.rows = rows;
            self.lines
                .resize_with(self.rows, || TerminalLine::blank(self.columns));
            for line in &mut self.lines {
                line.resize(self.columns);
            }
        }
        self.cursor.row = self.cursor.row.min(self.rows - 1);
        self.cursor.column = self.cursor.column.min(self.columns - 1);
        self.scroll_region = self.scroll_region.and_then(|(top, bottom)| {
            (top < self.rows && top < bottom).then_some((top, bottom.min(self.rows - 1)))
        });
        self.tab_stops.resize(self.columns, false);
        for column in (8..self.columns).step_by(8) {
            self.tab_stops[column] = true;
        }
        self.wrap_next = false;
    }

    fn reflow(&mut self, columns: usize, rows: usize) {
        // OSC 133 regions use physical history rows. Width changes invalidate
        // those addresses, so discard them instead of applying a stale filter.
        self.command_outputs.clear();
        self.active_command_output = None;
        self.notification = None;
        let total_rows = self.scrollback.len() + self.lines.len();
        let old_viewport_start = total_rows
            .saturating_sub(self.rows)
            .saturating_sub(self.viewport_offset.min(self.scrollback.len()));
        let selection_anchors = self.selection.map(|selection| {
            (
                reflow_anchor_for_position(
                    self.scrollback.iter().chain(self.lines.iter()),
                    old_viewport_start + selection.anchor.row,
                    selection.anchor.column,
                ),
                reflow_anchor_for_position(
                    self.scrollback.iter().chain(self.lines.iter()),
                    old_viewport_start + selection.head.row,
                    selection.head.column,
                ),
            )
        });
        let mut logical_lines: Vec<Vec<TerminalCell>> = Vec::new();
        let mut current = Vec::new();
        for line in self.scrollback.iter().chain(self.lines.iter()) {
            let cells = meaningful_cells(line);
            current.extend(cells);
            if !line.wrapped {
                logical_lines.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            logical_lines.push(current);
        }

        let logical_lengths = logical_lines.iter().map(Vec::len).collect::<Vec<_>>();

        let mut reflowed = Vec::new();
        for logical in logical_lines {
            if logical.is_empty() {
                reflowed.push(TerminalLine::blank(columns));
                continue;
            }
            let mut start = 0;
            while start < logical.len() {
                let end = start.saturating_add(columns).min(logical.len());
                reflowed.push(TerminalLine::from_cells(
                    columns,
                    &logical[start..end],
                    end < logical.len(),
                ));
                start = end;
            }
        }

        self.columns = columns;
        self.rows = rows;
        let split = reflowed.len().saturating_sub(rows);
        self.scrollback = reflowed
            .iter()
            .take(split)
            .cloned()
            .collect::<VecDeque<_>>();
        let mut evicted_rows = 0;
        while self.scrollback.len() > self.max_scrollback {
            self.scrollback.pop_front();
            self.history_origin = self.history_origin.wrapping_add(1);
            evicted_rows += 1;
        }
        self.lines = reflowed.into_iter().skip(split).collect();
        self.lines
            .resize_with(rows, || TerminalLine::blank(columns));
        self.viewport_offset = self.viewport_offset.min(self.max_viewport_offset());
        if let Some((Some(anchor), Some(head))) = selection_anchors {
            let anchor = position_for_reflow_anchor(anchor, &logical_lengths, columns)
                .map(|position| position_with_eviction(position, evicted_rows));
            let head = position_for_reflow_anchor(head, &logical_lengths, columns)
                .map(|position| position_with_eviction(position, evicted_rows));
            if let (Some(anchor), Some(head)) = (anchor.flatten(), head.flatten()) {
                let total_rows = self.scrollback.len() + self.lines.len();
                let selection_start = anchor.row.min(head.row);
                let selection_end = anchor.row.max(head.row);
                let live_start = total_rows.saturating_sub(rows);
                let desired_start = selection_end
                    .saturating_add(1)
                    .saturating_sub(rows)
                    .min(selection_start)
                    .min(live_start);
                self.viewport_offset = live_start
                    .saturating_sub(desired_start)
                    .min(self.scrollback.len());
                self.selection = Some(TerminalSelection::new(
                    TerminalPosition {
                        row: anchor.row.saturating_sub(desired_start),
                        column: anchor.column,
                    },
                    TerminalPosition {
                        row: head.row.saturating_sub(desired_start),
                        column: head.column,
                    },
                ));
            } else {
                self.selection = None;
            }
        }
    }

    fn write_char(&mut self, ch: char) {
        let ch = self.map_charset_char(ch);
        let width = if self.ambiguous_width_is_wide {
            UnicodeWidthChar::width_cjk(ch).unwrap_or(0)
        } else {
            UnicodeWidthChar::width(ch).unwrap_or(0)
        };
        if width == 0 {
            self.append_combining_char(ch);
            return;
        }
        if self.append_grapheme_extension(ch) {
            return;
        }
        self.last_printed_char = Some(ch);
        if self.wrap_next {
            self.mark_current_line_wrapped();
            self.newline();
            self.carriage_return();
            self.wrap_next = false;
        }
        if width == 2 && self.cursor.column + 1 >= self.columns {
            self.mark_current_line_wrapped();
            self.newline();
            self.carriage_return();
        }
        let row = self.cursor.row.min(self.rows - 1);
        let column = self.cursor.column.min(self.columns - 1);
        self.lines[row].cells[column] = TerminalCell {
            text: SharedString::from(ch.to_string()),
            style: self.style,
            hyperlink: self.active_hyperlink.clone(),
        };
        if width == 2 && column + 1 < self.columns {
            self.lines[row].cells[column + 1] = TerminalCell {
                text: SharedString::from(" "),
                style: self.style,
                hyperlink: self.active_hyperlink.clone(),
            }
        }
        if column + width >= self.columns {
            self.cursor.column = self.columns.saturating_sub(1);
            self.wrap_next = self.modes.auto_wrap;
        } else {
            self.cursor.column += width;
        }
        self.viewport_offset = 0;
    }

    fn append_combining_char(&mut self, ch: char) {
        let row = self.cursor.row.min(self.rows - 1);
        let mut column = if self.wrap_next {
            self.cursor.column
        } else if self.cursor.column > 0 {
            self.cursor.column - 1
        } else {
            return;
        };
        if column > 0
            && self.lines[row].cells[column].text.as_ref() == " "
            && self.lines[row].cells[column - 1]
                .text
                .chars()
                .next()
                .and_then(|character| {
                    if self.ambiguous_width_is_wide {
                        UnicodeWidthChar::width_cjk(character)
                    } else {
                        UnicodeWidthChar::width(character)
                    }
                })
                == Some(2)
        {
            column -= 1;
        }
        let cell = &mut self.lines[row].cells[column];
        if cell.text.chars().count() >= MAX_CELL_GRAPHEME_CHARS {
            return;
        }
        let mut text = cell.text.to_string();
        text.push(ch);
        cell.text = SharedString::from(text);
        self.viewport_offset = 0;
    }

    fn append_grapheme_extension(&mut self, ch: char) -> bool {
        let row = self.cursor.row.min(self.rows - 1);
        let mut column = if self.wrap_next {
            self.cursor.column
        } else if self.cursor.column > 0 {
            self.cursor.column - 1
        } else {
            return false;
        };
        if column > 0 && self.lines[row].cells[column].text.as_ref() == " " {
            column -= 1;
        }
        let Some(cell) = self.lines[row].cells.get_mut(column) else {
            return false;
        };
        if cell.text.chars().count() >= MAX_CELL_GRAPHEME_CHARS {
            return false;
        }
        let mut combined = cell.text.to_string();
        combined.push(ch);
        if UnicodeSegmentation::graphemes(combined.as_str(), true).count() != 1 {
            return false;
        }
        cell.text = SharedString::from(combined);
        self.viewport_offset = 0;
        true
    }

    fn mark_current_line_wrapped(&mut self) {
        if let Some(line) = self.lines.get_mut(self.cursor.row.min(self.rows - 1)) {
            line.wrapped = true;
        }
    }

    fn newline(&mut self) {
        self.wrap_next = false;
        let (top, bottom) = self.active_scroll_region();
        if self.cursor.row == bottom {
            self.scroll_region_up(top, bottom);
        } else if self.cursor.row + 1 >= self.rows {
            let mut line = self.lines.remove(0);
            line.compact_for_scrollback();
            self.scrollback.push_back(line);
            while self.scrollback.len() > self.max_scrollback {
                self.scrollback.pop_front();
                self.history_origin = self.history_origin.wrapping_add(1);
            }
            let erase_line = self.erase_line();
            self.lines.push(erase_line);
        } else {
            self.cursor.row += 1;
        }
    }

    fn reverse_index(&mut self) {
        self.wrap_next = false;
        let (top, bottom) = self.active_scroll_region();
        if self.cursor.row == top {
            self.scroll_region_down(top, bottom);
        } else {
            self.cursor.row = self.cursor.row.saturating_sub(1);
        }
    }

    fn carriage_return(&mut self) {
        self.cursor.column = 0;
        self.wrap_next = false;
    }

    fn backspace(&mut self) {
        if self.cursor.column == 0 && self.modes.reverse_wraparound && self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.column = self.columns.saturating_sub(1);
        } else {
            self.cursor.column = self.cursor.column.saturating_sub(1);
        }
        self.wrap_next = false;
    }

    fn tab(&mut self) {
        let next = self
            .tab_stops
            .iter()
            .enumerate()
            .skip(self.cursor.column.saturating_add(1))
            .find_map(|(column, enabled)| enabled.then_some(column))
            .unwrap_or_else(|| self.columns.saturating_sub(1));
        self.cursor.column = next.min(self.columns.saturating_sub(1));
        self.wrap_next = false;
    }

    fn back_tab(&mut self) {
        let previous = self
            .tab_stops
            .iter()
            .enumerate()
            .take(self.cursor.column)
            .rev()
            .find_map(|(column, enabled)| enabled.then_some(column))
            .unwrap_or(0);
        self.cursor.column = previous;
        self.wrap_next = false;
    }

    fn repeat_last_printed_char(&mut self, count: usize) {
        if let Some(ch) = self.last_printed_char {
            for _ in 0..count {
                self.write_char(ch);
            }
        }
    }

    fn set_tab_stop(&mut self) {
        if let Some(stop) = self.tab_stops.get_mut(self.cursor.column) {
            *stop = true;
        }
    }

    fn clear_tab_stop(&mut self, mode: u16) {
        match mode {
            3 => self.tab_stops.fill(false),
            _ => {
                if let Some(stop) = self.tab_stops.get_mut(self.cursor.column) {
                    *stop = false;
                }
            }
        }
    }

    fn clear_all(&mut self) {
        let erase_cell = self.erase_cell();
        for line in &mut self.lines {
            line.cells.fill(erase_cell.clone());
        }
        self.cursor = TerminalPosition::default();
        self.scroll_region = None;
        self.wrap_next = false;
    }

    fn erase_cell(&self) -> TerminalCell {
        TerminalCell {
            text: SharedString::from(" "),
            style: TerminalStyle {
                background: self.style.background,
                ..TerminalStyle::default()
            },
            hyperlink: None,
        }
    }

    fn erase_line(&self) -> TerminalLine {
        TerminalLine {
            cells: vec![self.erase_cell(); self.columns],
            wrapped: false,
        }
    }

    fn set_charset(&mut self, charset: TerminalCharset) {
        self.charset = charset;
    }

    fn save_cursor(&mut self) {
        self.saved_cursor = Some(SavedTerminalCursor {
            position: self.cursor,
            style: self.style,
            charset: self.charset,
            origin_mode: self.modes.origin_mode,
            auto_wrap: self.modes.auto_wrap,
        });
    }

    fn restore_cursor(&mut self) {
        let Some(saved) = self.saved_cursor else {
            return;
        };
        self.cursor = TerminalPosition {
            row: saved.position.row.min(self.rows - 1),
            column: saved.position.column.min(self.columns - 1),
        };
        self.style = saved.style;
        self.charset = saved.charset;
        self.modes.origin_mode = saved.origin_mode;
        self.modes.auto_wrap = saved.auto_wrap;
        self.wrap_next = false;
    }

    fn reset_terminal(&mut self) {
        self.lines = vec![TerminalLine::blank(self.columns); self.rows];
        self.scrollback.clear();
        self.search_reset_sequence = self.search_reset_sequence.wrapping_add(1);
        self.cursor = TerminalPosition::default();
        self.style = TerminalStyle::default();
        self.saved_cursor = None;
        self.wrap_next = false;
        self.title = SharedString::default();
        self.current_directory = None;
        self.viewport_offset = 0;
        self.modes = TerminalModes::default();
        self.alternate_lines = None;
        self.selection = None;
        self.scroll_region = None;
        self.tab_stops = default_tab_stops(self.columns);
        self.charset = TerminalCharset::default();
        self.active_hyperlink = None;
        self.saved_private_modes.clear();
        self.response_bytes.clear();
        self.last_printed_char = None;
        self.command_outputs.clear();
        self.active_command_output = None;
    }

    fn soft_reset_terminal(&mut self) {
        let alternate_screen = self.modes.alternate_screen;
        let reverse_video = self.modes.reverse_video;
        self.cursor = TerminalPosition::default();
        self.style = TerminalStyle::default();
        self.saved_cursor = None;
        self.wrap_next = false;
        self.viewport_offset = 0;
        self.modes = TerminalModes {
            alternate_screen,
            reverse_video,
            ..TerminalModes::default()
        };
        self.selection = None;
        self.scroll_region = None;
        self.charset = TerminalCharset::default();
        self.active_hyperlink = None;
        self.saved_private_modes.clear();
        self.last_printed_char = None;
    }

    fn set_active_hyperlink(&mut self, uri: Option<&str>) {
        self.active_hyperlink = uri
            .filter(|uri| !uri.is_empty())
            .map(|uri| SharedString::from(truncate_utf8(uri, MAX_OSC_HYPERLINK_BYTES).to_owned()));
    }

    fn queue_response(&mut self, bytes: impl AsRef<[u8]>) {
        let remaining = MAX_PROTOCOL_RESPONSE_BYTES.saturating_sub(self.response_bytes.len());
        let bytes = bytes.as_ref();
        self.response_bytes
            .extend_from_slice(&bytes[..bytes.len().min(remaining)]);
    }

    fn map_charset_char(&self, ch: char) -> char {
        if self.charset != TerminalCharset::DecSpecialGraphics {
            return ch;
        }
        match ch {
            '`' => '◆',
            'a' => '▒',
            'f' => '°',
            'g' => '±',
            'h' => '␤',
            'i' => '␋',
            'j' => '┘',
            'k' => '┐',
            'l' => '┌',
            'm' => '└',
            'n' => '┼',
            'o' => '⎺',
            'p' => '⎻',
            'q' => '─',
            'r' => '⎼',
            's' => '⎽',
            't' => '├',
            'u' => '┤',
            'v' => '┴',
            'w' => '┬',
            'x' => '│',
            'y' => '≤',
            'z' => '≥',
            '{' => 'π',
            '|' => '≠',
            '}' => '£',
            '~' => '·',
            _ => ch,
        }
    }

    fn clear_to_end_of_display(&mut self) {
        let row = self.cursor.row;
        let column = self.cursor.column;
        self.clear_line_range(row, column, self.columns);
        for row in row.saturating_add(1)..self.rows {
            self.clear_line_range(row, 0, self.columns);
        }
    }

    fn clear_from_start_of_display(&mut self) {
        let row = self.cursor.row;
        let column = self.cursor.column;
        for row in 0..row {
            self.clear_line_range(row, 0, self.columns);
        }
        self.clear_line_range(row, 0, column.saturating_add(1));
    }

    fn clear_line(&mut self, mode: u16) {
        match mode {
            1 => self.clear_line_range(self.cursor.row, 0, self.cursor.column.saturating_add(1)),
            2 => self.clear_line_range(self.cursor.row, 0, self.columns),
            _ => self.clear_line_range(self.cursor.row, self.cursor.column, self.columns),
        }
    }

    fn clear_line_range(&mut self, row: usize, start: usize, end: usize) {
        let erase_cell = self.erase_cell();
        if let Some(line) = self.lines.get_mut(row) {
            for cell in line.cells.iter_mut().take(end).skip(start) {
                *cell = erase_cell.clone();
            }
        }
    }

    fn delete_chars(&mut self, count: usize) {
        let row = self.cursor.row;
        let column = self.cursor.column;
        let erase_cell = self.erase_cell();
        if let Some(line) = self.lines.get_mut(row) {
            let len = line.cells.len();
            if column >= len {
                return;
            }
            let count = count.min(len.saturating_sub(column));
            for index in column..len.saturating_sub(count) {
                line.cells[index] = line.cells[index + count].clone();
            }
            for cell in line.cells.iter_mut().skip(len.saturating_sub(count)) {
                *cell = erase_cell.clone();
            }
        }
    }

    fn erase_chars(&mut self, count: usize) {
        self.clear_line_range(
            self.cursor.row,
            self.cursor.column,
            self.cursor.column.saturating_add(count).min(self.columns),
        );
    }

    fn insert_blank_chars(&mut self, count: usize) {
        let row = self.cursor.row;
        let column = self.cursor.column;
        let erase_cell = self.erase_cell();
        if let Some(line) = self.lines.get_mut(row) {
            let len = line.cells.len();
            if column >= len {
                return;
            }
            let count = count.min(len.saturating_sub(column));
            for index in (column + count..len).rev() {
                line.cells[index] = line.cells[index - count].clone();
            }
            for cell in line.cells.iter_mut().skip(column).take(count) {
                *cell = erase_cell.clone();
            }
        }
    }

    fn move_cursor(&mut self, row: usize, column: usize) {
        let row = if self.modes.origin_mode {
            let (top, bottom) = self.active_scroll_region();
            top.saturating_add(row).min(bottom)
        } else {
            row
        };
        self.cursor = TerminalPosition {
            row: row.min(self.rows - 1),
            column: column.min(self.columns - 1),
        };
        self.wrap_next = false;
    }

    fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        if top < bottom && bottom < self.rows {
            self.scroll_region = Some((top, bottom));
        } else {
            self.scroll_region = None;
        }
        self.move_cursor(0, 0);
    }

    fn active_scroll_region(&self) -> (usize, usize) {
        self.scroll_region
            .unwrap_or((0, self.rows.saturating_sub(1)))
    }

    fn scroll_region_up(&mut self, top: usize, bottom: usize) {
        let erase_line = self.erase_line();
        if top == 0 && bottom + 1 == self.rows {
            let mut line = self.lines.remove(0);
            line.compact_for_scrollback();
            self.scrollback.push_back(line);
            while self.scrollback.len() > self.max_scrollback {
                self.scrollback.pop_front();
                self.history_origin = self.history_origin.wrapping_add(1);
            }
            self.lines.push(erase_line);
            return;
        }
        for row in top..bottom {
            self.lines[row] = self.lines[row + 1].clone();
        }
        self.lines[bottom] = erase_line;
    }

    fn scroll_region_down(&mut self, top: usize, bottom: usize) {
        let erase_line = self.erase_line();
        for row in (top + 1..=bottom).rev() {
            self.lines[row] = self.lines[row - 1].clone();
        }
        self.lines[top] = erase_line;
    }

    fn insert_blank_lines(&mut self, count: usize) {
        let (top, bottom) = self.active_scroll_region();
        let row = self.cursor.row;
        if row < top || row > bottom {
            return;
        }
        let count = count.min(bottom - row + 1);
        let erase_line = self.erase_line();
        for target in (row + count..=bottom).rev() {
            self.lines[target] = self.lines[target - count].clone();
        }
        for target in row..row + count {
            self.lines[target] = erase_line.clone();
        }
    }

    fn delete_lines(&mut self, count: usize) {
        let (top, bottom) = self.active_scroll_region();
        let row = self.cursor.row;
        if row < top || row > bottom {
            return;
        }
        let count = count.min(bottom - row + 1);
        let erase_line = self.erase_line();
        for target in row..=bottom.saturating_sub(count) {
            self.lines[target] = self.lines[target + count].clone();
        }
        for target in bottom.saturating_sub(count).saturating_add(1)..=bottom {
            self.lines[target] = erase_line.clone();
        }
    }

    fn apply_sgr_codes(&mut self, codes: impl IntoIterator<Item = u16>) {
        for code in codes {
            match code {
                0 => self.style = TerminalStyle::default(),
                1 => self.style.bold = true,
                2 => self.style.faint = true,
                3 => self.style.italic = true,
                4 | 21 => self.style.underline = true,
                5 | 6 => self.style.blink = true,
                7 => self.style.inverse = true,
                8 => self.style.hidden = true,
                9 => self.style.strikethrough = true,
                22 => {
                    self.style.bold = false;
                    self.style.faint = false;
                }
                23 => self.style.italic = false,
                24 => self.style.underline = false,
                25 => self.style.blink = false,
                27 => self.style.inverse = false,
                28 => self.style.hidden = false,
                29 => self.style.strikethrough = false,
                30..=37 => self.style.foreground = terminal_color(code - 30, false),
                40..=47 => self.style.background = terminal_color(code - 40, false),
                90..=97 => self.style.foreground = terminal_color(code - 90, true),
                100..=107 => self.style.background = terminal_color(code - 100, true),
                39 => self.style.foreground = None,
                49 => self.style.background = None,
                _ => {}
            }
        }
    }

    fn apply_sgr_params(&mut self, params: &Params) {
        let params = params.iter().collect::<Vec<_>>();
        if params.is_empty() {
            self.style = TerminalStyle::default();
            return;
        }

        let mut index = 0;
        while index < params.len() {
            let Some(&code) = params[index].first() else {
                index += 1;
                continue;
            };
            if matches!(code, 38 | 48) {
                let target_foreground = code == 38;
                if params[index].len() > 1 {
                    if let Some(color) = Self::sgr_extended_color(&params[index][1..]) {
                        self.set_extended_color(target_foreground, color);
                    }
                    index += 1;
                    continue;
                }

                let mode = params
                    .get(index + 1)
                    .and_then(|param| param.first())
                    .copied();
                let (color, consumed) = match mode {
                    Some(5) => (
                        params
                            .get(index + 2)
                            .and_then(|param| param.first())
                            .and_then(|value| u8::try_from(*value).ok())
                            .map(TerminalColor::Indexed),
                        3,
                    ),
                    Some(2) => {
                        let rgb = params
                            .get(index + 2..index + 5)
                            .and_then(|params| match params {
                                [red, green, blue] => {
                                    Some((*red.first()?, *green.first()?, *blue.first()?))
                                }
                                _ => None,
                            })
                            .and_then(|(red, green, blue)| {
                                Some(TerminalColor::Rgb(
                                    u8::try_from(red).ok()?,
                                    u8::try_from(green).ok()?,
                                    u8::try_from(blue).ok()?,
                                ))
                            });
                        (rgb, 5)
                    }
                    _ => (None, 1),
                };
                if let Some(color) = color {
                    self.set_extended_color(target_foreground, color);
                }
                index += consumed;
                continue;
            }

            self.apply_sgr_codes([code]);
            index += 1;
        }
    }

    fn sgr_extended_color(params: &[u16]) -> Option<TerminalColor> {
        match params {
            [5, index, ..] => Some(TerminalColor::Indexed(u8::try_from(*index).ok()?)),
            [2, values @ ..] if values.len() >= 3 => {
                let rgb = &values[values.len() - 3..];
                Some(TerminalColor::Rgb(
                    u8::try_from(rgb[0]).ok()?,
                    u8::try_from(rgb[1]).ok()?,
                    u8::try_from(rgb[2]).ok()?,
                ))
            }
            _ => None,
        }
    }

    fn set_extended_color(&mut self, foreground: bool, color: TerminalColor) {
        if foreground {
            self.style.foreground = Some(color);
        } else {
            self.style.background = Some(color);
        }
    }

    fn set_private_mode(&mut self, mode: u16, enabled: bool) {
        match mode {
            3 => {
                self.modes.column_mode_132 = enabled;
                self.clear_all();
            }
            5 => self.modes.reverse_video = enabled,
            1 => self.modes.application_cursor_keys = enabled,
            6 => {
                self.modes.origin_mode = enabled;
                self.move_cursor(0, 0);
            }
            7 => self.modes.auto_wrap = enabled,
            8 => self.modes.auto_repeat = enabled,
            9 => self.modes.x10_mouse_reporting = enabled,
            12 => self.modes.cursor_blink = enabled,
            25 => self.modes.cursor_visible = enabled,
            40 => {}
            45 => self.modes.reverse_wraparound = enabled,
            66 => self.modes.application_keypad = enabled,
            69 => self.modes.left_right_margin_mode = enabled,
            1001 => {}
            1000 => self.modes.mouse_button_reporting = enabled,
            1002 => self.modes.mouse_drag_reporting = enabled,
            1003 => self.modes.mouse_all_motion_reporting = enabled,
            1004 => self.modes.focus_event_reporting = enabled,
            1005 => self.modes.utf8_mouse = enabled,
            1006 => self.modes.sgr_mouse = enabled,
            1007 => self.modes.alternate_scroll = enabled,
            1015 => self.modes.urxvt_mouse = enabled,
            1034 | 1036 | 1039 => self.modes.meta_sends_escape = enabled,
            1047 => self.set_alternate_screen(enabled),
            1049 => {
                if enabled {
                    self.save_cursor();
                    self.set_alternate_screen(true);
                } else {
                    self.set_alternate_screen(false);
                    self.restore_cursor();
                }
            }
            1048 => {
                if enabled {
                    self.save_cursor();
                } else {
                    self.restore_cursor();
                }
            }
            2004 => self.modes.bracketed_paste = enabled,
            2026 => self.modes.synchronized_output = enabled,
            _ => {}
        }
    }

    fn private_mode_enabled(&self, mode: u16) -> bool {
        match mode {
            1 => self.modes.application_cursor_keys,
            3 => self.modes.column_mode_132,
            5 => self.modes.reverse_video,
            6 => self.modes.origin_mode,
            7 => self.modes.auto_wrap,
            8 => self.modes.auto_repeat,
            9 => self.modes.x10_mouse_reporting,
            12 => self.modes.cursor_blink,
            25 => self.modes.cursor_visible,
            45 => self.modes.reverse_wraparound,
            66 => self.modes.application_keypad,
            69 => self.modes.left_right_margin_mode,
            1000 => self.modes.mouse_button_reporting,
            1002 => self.modes.mouse_drag_reporting,
            1003 => self.modes.mouse_all_motion_reporting,
            1004 => self.modes.focus_event_reporting,
            1005 => self.modes.utf8_mouse,
            1006 => self.modes.sgr_mouse,
            1007 => self.modes.alternate_scroll,
            1015 => self.modes.urxvt_mouse,
            1034 | 1036 | 1039 => self.modes.meta_sends_escape,
            1047 | 1049 => self.modes.alternate_screen,
            2004 => self.modes.bracketed_paste,
            2026 => self.modes.synchronized_output,
            _ => false,
        }
    }

    fn private_mode_status(&self, mode: u16) -> u8 {
        if matches!(
            mode,
            1 | 3
                | 5
                | 6
                | 7
                | 8
                | 9
                | 12
                | 25
                | 45
                | 66
                | 69
                | 1000
                | 1002
                | 1003
                | 1004
                | 1005
                | 1006
                | 1007
                | 1015
                | 1034
                | 1036
                | 1039
                | 1047
                | 1048
                | 1049
                | 2004
                | 2026
        ) {
            if self.private_mode_enabled(mode) {
                1
            } else {
                2
            }
        } else {
            0
        }
    }

    fn save_private_mode(&mut self, mode: u16) {
        let enabled = self.private_mode_enabled(mode);
        if let Some((_, saved)) = self
            .saved_private_modes
            .iter_mut()
            .find(|(saved_mode, _)| *saved_mode == mode)
        {
            *saved = enabled;
        } else {
            self.saved_private_modes.push((mode, enabled));
        }
    }

    fn restore_private_mode(&mut self, mode: u16) {
        if let Some((_, enabled)) = self
            .saved_private_modes
            .iter()
            .find(|(saved_mode, _)| *saved_mode == mode)
            .copied()
        {
            self.set_private_mode(mode, enabled);
        }
    }

    fn set_cursor_style(&mut self, style: u16) {
        self.modes.cursor_style = match style {
            3 | 4 => TerminalCursorStyle::Underline,
            5 | 6 => TerminalCursorStyle::Bar,
            _ => TerminalCursorStyle::Block,
        };
    }

    fn set_alternate_screen(&mut self, enabled: bool) {
        if enabled == self.modes.alternate_screen {
            return;
        }
        self.modes.alternate_screen = enabled;
        self.viewport_offset = 0;
        self.wrap_next = false;
        if enabled {
            let saved_lines = std::mem::replace(
                &mut self.lines,
                vec![TerminalLine::blank(self.columns); self.rows],
            );
            self.alternate_lines = Some((saved_lines, self.cursor));
            self.cursor = TerminalPosition::default();
        } else if let Some((saved_lines, saved_cursor)) = self.alternate_lines.take() {
            self.lines = saved_lines;
            self.cursor = saved_cursor;
        }
    }
}

fn search_terminal_line(
    line: &TerminalLine,
    row: usize,
    query: &str,
    options: TerminalSearchOptions,
    regex: Option<&regex::Regex>,
) -> Vec<TerminalSearchMatch> {
    let text = line.text();
    if let Some(regex) = regex {
        return regex
            .find_iter(&text)
            .filter(|matched| {
                matched.start() < matched.end()
                    && (!options.whole_word
                        || has_word_boundaries(&text, matched.start(), matched.end()))
            })
            .filter_map(|matched| {
                let start_column = terminal_column_for_byte(line, matched.start())?;
                let end_byte = matched.end().saturating_sub(1);
                let end_column = terminal_column_for_byte(line, end_byte)?;
                Some(TerminalSearchMatch {
                    row,
                    start_column,
                    end_column,
                })
            })
            .collect();
    }
    let (haystack, needle, byte_map) = if options.case_sensitive {
        (
            text.clone(),
            query.to_string(),
            (0..text.len()).collect::<Vec<_>>(),
        )
    } else {
        let (folded, byte_map) = folded_text_with_byte_map(&text);
        (folded, query.to_lowercase(), byte_map)
    };
    haystack
        .match_indices(&needle)
        .filter_map(|(start_byte, matched)| {
            let end_byte = start_byte + matched.len();
            let original_start = *byte_map.get(start_byte)?;
            let original_end = *byte_map.get(end_byte.saturating_sub(1))?;
            let original_end_exclusive = text[original_end..]
                .chars()
                .next()
                .map_or(original_end, |character| {
                    original_end + character.len_utf8()
                });
            if options.whole_word
                && !has_word_boundaries(&text, original_start, original_end_exclusive)
            {
                return None;
            }
            let start_column = terminal_column_for_byte(line, original_start)?;
            let end_column = terminal_column_for_byte(line, original_end)?;
            Some(TerminalSearchMatch {
                row,
                start_column,
                end_column,
            })
        })
        .collect()
}

fn has_word_boundaries(text: &str, start: usize, end: usize) -> bool {
    let is_word = |character: char| character.is_alphanumeric() || character == '_';
    let previous_is_word = text[..start].chars().next_back().is_some_and(is_word);
    let next_is_word = text[end..].chars().next().is_some_and(is_word);
    !previous_is_word && !next_is_word
}

fn folded_text_with_byte_map(text: &str) -> (String, Vec<usize>) {
    let mut folded = String::new();
    let mut byte_map = Vec::new();
    for (original_byte, character) in text.char_indices() {
        let lowercase = character.to_lowercase().to_string();
        byte_map.extend(std::iter::repeat_n(original_byte, lowercase.len()));
        folded.push_str(&lowercase);
    }
    (folded, byte_map)
}

fn terminal_column_for_byte(line: &TerminalLine, byte_offset: usize) -> Option<usize> {
    let mut consumed = 0;
    for (column, cell) in line.cells().iter().enumerate() {
        let next = consumed + cell.text.len();
        if byte_offset < next {
            return Some(column);
        }
        consumed = next;
    }
    None
}

struct TerminalPerformer<'a> {
    model: &'a mut TerminalModel,
}

impl TerminalPerformer<'_> {
    fn first_param(params: &Params) -> Option<u16> {
        params.iter().next().and_then(|sub| sub.first().copied())
    }

    fn second_param(params: &Params) -> Option<u16> {
        params.iter().nth(1).and_then(|sub| sub.first().copied())
    }

    fn count_param(params: &Params, default: u16) -> u16 {
        match Self::first_param(params).unwrap_or(default) {
            0 => default,
            value => value,
        }
    }

    fn position_param(value: Option<u16>) -> usize {
        usize::from(value.unwrap_or(1).max(1).saturating_sub(1))
    }
}

impl Perform for TerminalPerformer<'_> {
    fn print(&mut self, c: char) {
        self.model.write_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.model.newline(),
            b'\r' => self.model.carriage_return(),
            0x08 => self.model.backspace(),
            b'\t' => self.model.tab(),
            0x07 => self.model.bell_sequence = self.model.bell_sequence.wrapping_add(1),
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        if matches!(action, 'h' | 'l') && intermediates == b"?" {
            let enabled = action == 'h';
            for mode in params.iter().flat_map(|sub| sub.iter().copied()) {
                self.model.set_private_mode(mode, enabled);
            }
            return;
        }
        if matches!(action, 's' | 'r') && intermediates == b"?" {
            for mode in params.iter().flat_map(|sub| sub.iter().copied()) {
                if action == 's' {
                    self.model.save_private_mode(mode);
                } else {
                    self.model.restore_private_mode(mode);
                }
            }
            return;
        }

        match action {
            'c' if Self::first_param(params).unwrap_or(0) == 0 => {
                if intermediates == b">" {
                    self.model.queue_response(b"\x1b[>0;1;0c");
                } else if intermediates.is_empty() {
                    self.model.queue_response(b"\x1b[?1;2c");
                }
            }
            'A' => {
                let count = usize::from(Self::count_param(params, 1));
                let top = if self.model.modes.origin_mode {
                    self.model.active_scroll_region().0
                } else {
                    0
                };
                self.model.cursor.row = self.model.cursor.row.saturating_sub(count).max(top);
                self.model.wrap_next = false;
            }
            'B' => {
                let count = usize::from(Self::count_param(params, 1));
                let bottom = if self.model.modes.origin_mode {
                    self.model.active_scroll_region().1
                } else {
                    self.model.rows - 1
                };
                self.model.cursor.row = self.model.cursor.row.saturating_add(count).min(bottom);
                self.model.wrap_next = false;
            }
            'E' => {
                let count = usize::from(Self::count_param(params, 1));
                let bottom = if self.model.modes.origin_mode {
                    self.model.active_scroll_region().1
                } else {
                    self.model.rows - 1
                };
                self.model.cursor.row = self.model.cursor.row.saturating_add(count).min(bottom);
                self.model.cursor.column = 0;
                self.model.wrap_next = false;
            }
            'F' => {
                let count = usize::from(Self::count_param(params, 1));
                let top = if self.model.modes.origin_mode {
                    self.model.active_scroll_region().0
                } else {
                    0
                };
                self.model.cursor.row = self.model.cursor.row.saturating_sub(count).max(top);
                self.model.cursor.column = 0;
                self.model.wrap_next = false;
            }
            'C' => {
                let count = usize::from(Self::count_param(params, 1));
                self.model.cursor.column = self
                    .model
                    .cursor
                    .column
                    .saturating_add(count)
                    .min(self.model.columns - 1);
                self.model.wrap_next = false;
            }
            'D' => {
                let count = usize::from(Self::count_param(params, 1));
                self.model.cursor.column = self.model.cursor.column.saturating_sub(count);
                self.model.wrap_next = false;
            }
            'a' => {
                let count = usize::from(Self::count_param(params, 1));
                self.model.cursor.column = self
                    .model
                    .cursor
                    .column
                    .saturating_add(count)
                    .min(self.model.columns - 1);
                self.model.wrap_next = false;
            }
            'e' => {
                let count = usize::from(Self::count_param(params, 1));
                let bottom = if self.model.modes.origin_mode {
                    self.model.active_scroll_region().1
                } else {
                    self.model.rows - 1
                };
                self.model.cursor.row = self.model.cursor.row.saturating_add(count).min(bottom);
                self.model.wrap_next = false;
            }
            'G' => {
                let column = Self::position_param(Self::first_param(params));
                self.model.move_cursor(self.model.cursor.row, column);
            }
            'd' => {
                let row = Self::position_param(Self::first_param(params));
                self.model.move_cursor(row, self.model.cursor.column);
            }
            'H' | 'f' => {
                let row = Self::position_param(Self::first_param(params));
                let column = Self::position_param(Self::second_param(params));
                self.model.move_cursor(row, column);
            }
            'I' => {
                let count = usize::from(Self::count_param(params, 1));
                for _ in 0..count {
                    self.model.tab();
                }
            }
            'Z' => {
                let count = usize::from(Self::count_param(params, 1));
                for _ in 0..count {
                    self.model.back_tab();
                }
            }
            'g' => self
                .model
                .clear_tab_stop(Self::first_param(params).unwrap_or(0)),
            'J' => match Self::first_param(params).unwrap_or(0) {
                1 => self.model.clear_from_start_of_display(),
                2 | 3 => self.model.clear_all(),
                _ => self.model.clear_to_end_of_display(),
            },
            'K' => self
                .model
                .clear_line(Self::first_param(params).unwrap_or(0)),
            'P' => self
                .model
                .delete_chars(usize::from(Self::count_param(params, 1))),
            'S' => {
                let count = usize::from(Self::count_param(params, 1));
                let (top, bottom) = self.model.active_scroll_region();
                for _ in 0..count {
                    self.model.scroll_region_up(top, bottom);
                }
            }
            'T' => {
                let count = usize::from(Self::count_param(params, 1));
                let (top, bottom) = self.model.active_scroll_region();
                for _ in 0..count {
                    self.model.scroll_region_down(top, bottom);
                }
            }
            'X' => self
                .model
                .erase_chars(usize::from(Self::count_param(params, 1))),
            '@' => self
                .model
                .insert_blank_chars(usize::from(Self::count_param(params, 1))),
            'b' => self
                .model
                .repeat_last_printed_char(usize::from(Self::count_param(params, 1))),
            'L' => self
                .model
                .insert_blank_lines(usize::from(Self::count_param(params, 1))),
            'M' => self
                .model
                .delete_lines(usize::from(Self::count_param(params, 1))),
            'm' => self.model.apply_sgr_params(params),
            'n' => {
                let code = Self::first_param(params).unwrap_or(0);
                if intermediates == b"?" {
                    match code {
                        6 => {
                            let row = self.model.cursor.row.saturating_add(1);
                            let column = self.model.cursor.column.saturating_add(1);
                            self.model.queue_response(format!("\x1b[?{row};{column}R"));
                        }
                        15 => self.model.queue_response(b"\x1b[?13n"),
                        25 => self.model.queue_response(b"\x1b[?20n"),
                        26 => self.model.queue_response(b"\x1b[?27;1n"),
                        _ => {}
                    }
                } else {
                    match code {
                        5 => self.model.queue_response(b"\x1b[0n"),
                        6 => {
                            let row = self.model.cursor.row.saturating_add(1);
                            let column = self.model.cursor.column.saturating_add(1);
                            self.model.queue_response(format!("\x1b[{row};{column}R"));
                        }
                        _ => {}
                    }
                }
            }
            'r' => {
                let top = Self::position_param(Self::first_param(params));
                let bottom = Self::second_param(params)
                    .map(|value| Self::position_param(Some(value)))
                    .unwrap_or_else(|| self.model.rows.saturating_sub(1));
                self.model.set_scroll_region(top, bottom);
            }
            's' => self.model.save_cursor(),
            'u' => self.model.restore_cursor(),
            'q' if intermediates == b" " => {
                self.model
                    .set_cursor_style(Self::first_param(params).unwrap_or(0));
            }
            'p' if intermediates == b"!" => self.model.soft_reset_terminal(),
            'p' if intermediates == b"?$" => {
                for mode in params.iter().flat_map(|param| param.iter().copied()) {
                    let status = self.model.private_mode_status(mode);
                    self.model
                        .queue_response(format!("\x1b[?{mode};{status}$y"));
                }
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        match (intermediates, byte) {
            (b"(", b'0') => self.model.set_charset(TerminalCharset::DecSpecialGraphics),
            (b"(", b'B') => self.model.set_charset(TerminalCharset::Ascii),
            (_, b'H') => self.model.set_tab_stop(),
            (_, b'D') => self.model.newline(),
            (_, b'E') => {
                self.model.newline();
                self.model.carriage_return();
            }
            (_, b'M') => {
                self.model.reverse_index();
            }
            (_, b'Z') => self.model.queue_response(b"\x1b[?1;2c"),
            (_, b'=') => self.model.modes.application_keypad = true,
            (_, b'>') => self.model.modes.application_keypad = false,
            (_, b'7') => self.model.save_cursor(),
            (_, b'8') => self.model.restore_cursor(),
            (b"", b'c') => self.model.reset_terminal(),
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}

    fn put(&mut self, _byte: u8) {}

    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.len() >= 2
            && matches!(params[0], b"0" | b"1" | b"2")
            && let Some(title) = bounded_osc_text(&params[1..], MAX_OSC_TITLE_BYTES)
        {
            self.model.title = SharedString::from(title);
        } else if params.len() >= 2
            && params[0] == b"7"
            && let Some(uri) = bounded_osc_text(&params[1..], MAX_OSC_HYPERLINK_BYTES)
        {
            self.model.current_directory = file_uri_path(&uri).map(SharedString::from);
        } else if params.len() >= 3
            && params[0] == b"8"
            && let Some(uri) = bounded_osc_text(&params[2..], MAX_OSC_HYPERLINK_BYTES)
        {
            self.model.set_active_hyperlink(Some(&uri));
        } else if params.len() >= 2 && params[0] == b"133" {
            match params[1] {
                b"C" => self.model.begin_command_output(),
                b"D" => {
                    let exit_status = params
                        .get(2)
                        .and_then(|value| std::str::from_utf8(value).ok())
                        .and_then(|value| value.parse::<i32>().ok());
                    self.model.finish_command_output(exit_status);
                }
                _ => {}
            }
        } else if params.len() >= 2 && params[0] == b"9" && params[1] != b"4" {
            if let Some(body) = bounded_osc_text(&params[1..], MAX_OSC_TITLE_BYTES)
                && !body.is_empty()
            {
                self.model.notification = Some(TerminalNotification {
                    title: SharedString::default(),
                    body: SharedString::from(body),
                });
                self.model.notification_sequence = self.model.notification_sequence.wrapping_add(1);
            }
        } else if params.len() >= 4
            && params[0] == b"777"
            && params[1] == b"notify"
            && let (Some(title), Some(body)) = (
                bounded_osc_text(&params[2..3], MAX_OSC_TITLE_BYTES),
                bounded_osc_text(&params[3..], MAX_OSC_TITLE_BYTES),
            )
            && !body.is_empty()
        {
            self.model.notification = Some(TerminalNotification {
                title: SharedString::from(title),
                body: SharedString::from(body),
            });
            self.model.notification_sequence = self.model.notification_sequence.wrapping_add(1);
        }
    }
}

fn file_uri_path(uri: &str) -> Option<String> {
    let authority_and_path = uri.strip_prefix("file://")?;
    let path = if authority_and_path.starts_with('/') {
        authority_and_path
    } else {
        let (authority, path) = authority_and_path.split_once('/')?;
        if !authority.is_empty() && !authority.eq_ignore_ascii_case("localhost") {
            return None;
        }
        path.strip_prefix('/').map_or(path, |path| path)
    };
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    percent_decode_path(&path)
}

fn percent_decode_path(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes.get(index + 1)?;
            let low = *bytes.get(index + 2)?;
            let value = (hex_value(high)? << 4) | hex_value(low)?;
            if value == 0 {
                return None;
            }
            decoded.push(value);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded)
        .ok()
        .filter(|path| !path.chars().any(char::is_control))
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    &value[..end]
}

fn bounded_osc_text(params: &[&[u8]], max_bytes: usize) -> Option<String> {
    let mut output = String::with_capacity(max_bytes.min(256));
    for (index, param) in params.iter().enumerate() {
        let value = std::str::from_utf8(param).ok()?;
        if index > 0 && output.len() < max_bytes {
            output.push(';');
        }
        let remaining = max_bytes.saturating_sub(output.len());
        output.push_str(truncate_utf8(value, remaining));
        if output.len() == max_bytes {
            break;
        }
    }
    Some(output)
}

/// Rendering options for [`Terminal`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalOptions {
    visible_scrollback: usize,
    cell_width: u16,
    line_height: u16,
    font_family: Option<SharedString>,
    font_size: u16,
    font_weight: u16,
    line_height_percent: u16,
    ligatures: bool,
    cursor_style: Option<TerminalCursorStyle>,
    measure_font: bool,
    blink_visible: bool,
    automatic_blink: bool,
    ansi_palette: [[u8; 3]; 16],
    link_activation_modifier: TerminalLinkModifier,
}

/// Modifier required to activate detected terminal links.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TerminalLinkModifier {
    /// Command on macOS and Control on Linux/Windows.
    #[default]
    Secondary,
    /// The Alt/Option key.
    Alt,
    /// The Control key on every platform.
    Control,
}

impl TerminalLinkModifier {
    fn is_active(self, modifiers: &gpui::Modifiers) -> bool {
        match self {
            Self::Secondary => modifiers.secondary(),
            Self::Alt => modifiers.alt,
            Self::Control => modifiers.control,
        }
    }
}

impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            visible_scrollback: 0,
            cell_width: 8,
            line_height: 18,
            font_family: None,
            font_size: 13,
            font_weight: 400,
            line_height_percent: 135,
            ligatures: false,
            cursor_style: None,
            measure_font: false,
            blink_visible: true,
            automatic_blink: true,
            ansi_palette: default_ansi_palette(),
            link_activation_modifier: TerminalLinkModifier::Secondary,
        }
    }
}

impl TerminalOptions {
    /// Sets how many scrollback rows are rendered before the live grid.
    #[must_use]
    pub fn visible_scrollback(mut self, rows: usize) -> Self {
        self.visible_scrollback = rows;
        self
    }

    /// Sets fixed cell metrics used for rendering and pointer conversion.
    #[must_use]
    pub fn cell_size(mut self, width: u16, line_height: u16) -> Self {
        self.cell_width = width.max(1);
        self.line_height = line_height.max(1);
        self.measure_font = false;
        self
    }

    /// Sets the terminal font family used for rendering and optional measurement.
    #[must_use]
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// Sets the terminal font size.
    #[must_use]
    pub fn font_size(mut self, size: u16) -> Self {
        self.font_size = size.max(1);
        self
    }

    /// Sets the base font weight from 1 through 1000.
    #[must_use]
    pub fn font_weight(mut self, weight: u16) -> Self {
        self.font_weight = weight.clamp(1, 1000);
        self
    }

    /// Sets minimum line height in pixels for measured fonts, or fixed line height otherwise.
    #[must_use]
    pub fn line_height(mut self, height: u16) -> Self {
        self.line_height = height.max(1);
        self
    }

    /// Sets line height as a percentage of the font size.
    #[must_use]
    pub fn line_height_percent(mut self, percent: u16) -> Self {
        self.line_height_percent = percent.clamp(100, 250);
        self
    }

    /// Controls contextual terminal font ligatures. They are disabled by default
    /// so one grid cell cannot visually merge with its neighbors unexpectedly.
    #[must_use]
    pub fn ligatures(mut self, enabled: bool) -> Self {
        self.ligatures = enabled;
        self
    }

    /// Overrides the application-requested cursor style when configured.
    #[must_use]
    pub fn cursor_style(mut self, style: Option<TerminalCursorStyle>) -> Self {
        self.cursor_style = style;
        self
    }

    /// Overrides automatic blink scheduling with a host-controlled visible phase.
    #[must_use]
    pub fn blink_visible(mut self, visible: bool) -> Self {
        self.blink_visible = visible;
        self.automatic_blink = false;
        self
    }

    /// Replaces the normal and bright ANSI 16-color palette.
    #[must_use]
    pub fn ansi_palette(mut self, palette: impl Into<TerminalPalette>) -> Self {
        self.ansi_palette = palette.into().0;
        self
    }

    /// Selects the modifier required for pointer link activation.
    #[must_use]
    pub fn link_activation_modifier(mut self, modifier: TerminalLinkModifier) -> Self {
        self.link_activation_modifier = modifier;
        self
    }

    /// Enables font-derived cell measurement using GPUI's text system.
    #[must_use]
    pub fn measured_font(mut self) -> Self {
        self.measure_font = true;
        self
    }
}

/// A controlled native terminal emulator surface.
#[derive(gpui::IntoElement)]
pub struct Terminal {
    id: SharedString,
    model: TerminalModelSource,
    options: TerminalOptions,
    focus_handle: Option<FocusHandle>,
    input_state: Option<Entity<TerminalInputState>>,
    on_input: Option<InputHandler>,
    on_selection: Option<SelectionHandler>,
    on_viewport_scroll: Option<ViewportScrollHandler>,
    on_resize: Option<ResizeHandler>,
    on_link: Option<LinkHandler>,
    on_link_hover: Option<LinkHoverHandler>,
    hovered_link: Option<SharedString>,
    search_matches: Vec<TerminalSearchMatch>,
    active_search_match: Option<usize>,
}

impl Terminal {
    /// Creates a terminal surface from a model.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, model: TerminalModel) -> Self {
        Self {
            id: id.into(),
            model: TerminalModelSource::Snapshot(Arc::new(model)),
            options: TerminalOptions::default(),
            focus_handle: None,
            input_state: None,
            on_input: None,
            on_selection: None,
            on_viewport_scroll: None,
            on_resize: None,
            on_link: None,
            on_link_hover: None,
            hovered_link: None,
            search_matches: Vec::new(),
            active_search_match: None,
        }
    }

    /// Creates a terminal surface backed by a stable shared model.
    #[must_use]
    pub fn from_shared(id: impl Into<SharedString>, model: impl Into<TerminalModelSource>) -> Self {
        Self {
            id: id.into(),
            model: model.into(),
            options: TerminalOptions::default(),
            focus_handle: None,
            input_state: None,
            on_input: None,
            on_selection: None,
            on_viewport_scroll: None,
            on_resize: None,
            on_link: None,
            on_link_hover: None,
            hovered_link: None,
            search_matches: Vec::new(),
            active_search_match: None,
        }
    }

    /// Replaces rendering options.
    #[must_use]
    pub fn options(mut self, options: TerminalOptions) -> Self {
        self.options = options;
        self
    }

    /// Sets a focus handle so the terminal can receive keyboard input.
    #[must_use]
    pub fn focusable(mut self, handle: FocusHandle) -> Self {
        self.focus_handle = Some(handle);
        self
    }

    /// Sets platform IME input state for marked text and candidate-window placement.
    #[must_use]
    pub fn input_state(mut self, input_state: Entity<TerminalInputState>) -> Self {
        self.input_state = Some(input_state);
        self
    }

    /// Registers an input handler for PTY-backed hosts.
    #[must_use]
    pub fn on_input(mut self, handler: impl Fn(&[u8], &mut Window, &mut App) + 'static) -> Self {
        self.on_input = Some(std::rc::Rc::new(handler));
        self
    }

    /// Registers a selection-change handler for pointer selections.
    #[must_use]
    pub fn on_selection(
        mut self,
        handler: impl Fn(&TerminalSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection = Some(std::rc::Rc::new(handler));
        self
    }

    /// Registers a viewport scroll handler for host-managed scrollback.
    #[must_use]
    pub fn on_viewport_scroll(
        mut self,
        handler: impl Fn(&isize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_viewport_scroll = Some(std::rc::Rc::new(handler));
        self
    }

    /// Registers a resize handler for hosts that need to resize an attached PTY.
    ///
    /// The handler receives the grid dimensions calculated from the rendered
    /// terminal pane bounds and the active terminal font metrics.
    #[must_use]
    pub fn on_resize(
        mut self,
        handler: impl Fn(&TerminalGridSize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resize = Some(std::rc::Rc::new(handler));
        self
    }

    /// Registers a handler for modifier-click activation of an OSC 8 link.
    #[must_use]
    pub fn on_link(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_link = Some(std::rc::Rc::new(handler));
        self
    }

    /// Registers a handler that reports the OSC 8 link below the pointer.
    #[must_use]
    pub fn on_link_hover(
        mut self,
        handler: impl Fn(&Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_link_hover = Some(std::rc::Rc::new(handler));
        self
    }

    /// Marks an OSC 8 link as hovered for visible pointer feedback.
    #[must_use]
    pub fn hovered_link(mut self, uri: Option<SharedString>) -> Self {
        self.hovered_link = uri;
        self
    }

    /// Sets history-addressed matches to highlight and the active result index.
    #[must_use]
    pub fn search_matches(
        mut self,
        matches: Vec<TerminalSearchMatch>,
        active: Option<usize>,
    ) -> Self {
        self.search_matches = matches;
        self.active_search_match = active;
        self
    }
}

struct TerminalBlinkState {
    visible: bool,
    task: Option<gpui::Task<()>>,
}
impl TerminalBlinkState {
    fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if !active {
            self.task = None;
            self.visible = true;
        } else if self.task.is_none() {
            self.task = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(500))
                        .await;
                    if this
                        .update(cx, |state, cx| {
                            state.visible = !state.visible;
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
    }
}

impl RenderOnce for Terminal {
    fn render(mut self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let blink = _window.use_keyed_state(format!("{}-blink", self.id), cx, |_, _| {
            TerminalBlinkState {
                visible: true,
                task: None,
            }
        });
        let active = self.options.automatic_blink && self.model.borrow().has_blinking_content();
        let visible = blink.update(cx, |state, cx| {
            state.set_active(active, cx);
            state.visible
        });
        if self.options.automatic_blink {
            self.options.blink_visible = visible;
        }
        let theme = Theme::global(cx).clone();
        let font_family = terminal_font_family(&self.options, &theme, _window);
        let metrics = terminal_font_metrics(&self.options, &theme, &font_family, _window);
        let cell_width = metrics.cell_width;
        let line_height = metrics.line_height;
        let font_size = px(f32::from(self.options.font_size));
        let rendered_rows = rendered_row_count(&self.model.borrow(), &self.options);
        let rendered_cursor = rendered_cursor_position(&self.model.borrow(), &self.options);
        if let Some(input_state) = &self.input_state
            && let Some(cursor) = rendered_cursor
        {
            input_state.update(cx, |state, _| {
                state.set_snapshot(TerminalInputSnapshot {
                    cursor,
                    metrics: TerminalGridMetrics {
                        bounds: Bounds::new(
                            point(px(0.0), px(0.0)),
                            size(
                                cell_width * self.model.borrow().columns() as f32,
                                line_height * rendered_rows as f32,
                            ),
                        ),
                        cell_width,
                        line_height,
                        columns: self.model.borrow().columns(),
                        rows: rendered_rows,
                    },
                    modes: self.model.borrow().modes(),
                });
                state.set_input_handler(self.on_input.clone());
            });
        }
        let marked_text = self
            .input_state
            .as_ref()
            .map(|state| state.read(cx).marked_text().to_string())
            .filter(|text| !text.is_empty());
        let mut root = div()
            .id(self.id)
            .w_full()
            .h_full()
            .min_w_0()
            .min_h_0()
            .bg(theme.background())
            .flex()
            .flex_col()
            .overflow_hidden()
            .font_family(font_family.to_string())
            .text_size(font_size);
        if let Some(handle) = &self.focus_handle {
            root = root.key_context("GuicTerminal").track_focus(handle);
        }
        if let (Some(handle), Some(on_input)) = (self.focus_handle.clone(), self.on_input.clone()) {
            let terminal_model = self.model.clone();
            let terminal_options = self.options.clone();
            let has_input_state = self.input_state.is_some();
            root = root
                .cursor_text()
                .on_click(move |_, window, cx| window.focus(&handle, cx))
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.modifiers.secondary() && event.keystroke.key == "c" {
                        let copied = if terminal_model.borrow().viewport_offset() > 0
                            && terminal_options.visible_scrollback == 0
                        {
                            terminal_model.borrow().copy_selection_to_clipboard(cx)
                        } else {
                            terminal_model
                                .borrow()
                                .copy_selection_to_clipboard_with_visible_scrollback(
                                    terminal_options.visible_scrollback,
                                    cx,
                                )
                        };
                        if copied {
                            cx.stop_propagation();
                        }
                        return;
                    }
                    if event.keystroke.modifiers.secondary() && event.keystroke.key == "v" {
                        let bytes = { terminal_model.borrow().clipboard_paste_bytes(cx) };
                        if let Some(bytes) = bytes {
                            on_input(&bytes, window, cx);
                            cx.stop_propagation();
                        }
                        return;
                    }
                    if has_input_state && key_event_uses_text_input(event) {
                        return;
                    }
                    let bytes = {
                        let modes = terminal_model.borrow().modes();
                        terminal_key_down_event_bytes(event, modes)
                    };
                    if let Some(bytes) = bytes {
                        on_input(&bytes, window, cx);
                        cx.stop_propagation();
                    }
                });
        }

        let grid_bounds = std::rc::Rc::new(std::cell::Cell::new(None::<Bounds<Pixels>>));
        let bounds_sink = grid_bounds.clone();
        let resize_handler = self.on_resize.clone();
        let resize_metrics = metrics;
        let bounds_canvas = canvas(
            move |bounds, window, cx| {
                bounds_sink.set(Some(bounds));
                if let Some(on_resize) = &resize_handler {
                    on_resize(
                        &terminal_grid_size_for_bounds(bounds, resize_metrics),
                        window,
                        cx,
                    );
                }
            },
            |_bounds, _state, _window, _cx| {},
        )
        .absolute()
        .inset_0();
        let mut grid = div()
            .relative()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(bounds_canvas);
        if let (Some(handle), Some(input_state)) =
            (self.focus_handle.clone(), self.input_state.clone())
        {
            let input_canvas = canvas(
                |_bounds, _window, _cx| {},
                move |bounds, _state, window, cx| {
                    input_state.update(cx, |state, _| {
                        if let Some(mut snapshot) = state.snapshot() {
                            snapshot.metrics.bounds = bounds;
                            state.set_snapshot(snapshot);
                        }
                    });
                    window.handle_input(
                        &handle,
                        ElementInputHandler::new(bounds, input_state.clone()),
                        cx,
                    );
                },
            )
            .absolute()
            .inset_0();
            grid = grid.child(input_canvas);
        }
        if self.on_input.is_some()
            || self.on_selection.is_some()
            || self.on_link.is_some()
            || self.on_link_hover.is_some()
        {
            let mouse_model = self.model.clone();
            let mouse_metrics = metrics;
            let mouse_rows = rendered_row_count(&self.model.borrow(), &self.options);
            let mouse_visible_scrollback = self.options.visible_scrollback;
            let down_bounds = grid_bounds.clone();
            let move_bounds = grid_bounds.clone();
            let up_bounds = grid_bounds.clone();
            let selection_anchor = std::rc::Rc::new(std::cell::Cell::new(None::<TerminalPosition>));
            let down_anchor = selection_anchor.clone();
            let move_anchor = selection_anchor.clone();
            let up_anchor = selection_anchor.clone();
            let down_input = self.on_input.clone();
            let move_input = self.on_input.clone();
            let up_input = self.on_input.clone();
            let down_selection = self.on_selection.clone();
            let down_link = self.on_link.clone();
            let link_activation_modifier = self.options.link_activation_modifier;
            let move_link_hover = self.on_link_hover.clone();
            let exit_link_hover = self.on_link_hover.clone();
            let move_selection = self.on_selection.clone();
            let move_scroll = self.on_viewport_scroll.clone();
            let up_selection = self.on_selection.clone();
            let down_model = mouse_model.clone();
            let move_model = mouse_model.clone();
            let up_model = mouse_model;
            grid = grid
                .on_mouse_down(
                    MouseButton::Left,
                    move |event: &MouseDownEvent, window, cx| {
                        // Host callbacks may synchronously mutate this shared model.
                        // Materialize all borrowed values before invoking them.
                        let metrics = {
                            let model = down_model.borrow();
                            metrics_from_bounds(
                                down_bounds.get(),
                                &model,
                                mouse_metrics,
                                mouse_rows,
                            )
                        };
                        if let Some(metrics) = metrics {
                            let position = metrics.position_for_point(event.position);
                            let link = if link_activation_modifier.is_active(&event.modifiers) {
                                down_model
                                    .borrow()
                                    .link_at(position, mouse_visible_scrollback)
                            } else {
                                None
                            };
                            if let (Some(on_link), Some(uri)) = (&down_link, link) {
                                on_link(&uri, window, cx);
                                return;
                            }
                            let bytes = {
                                let modes = down_model.borrow().modes();
                                terminal_mouse_event_bytes(
                                    mouse_event_from_parts(
                                        position,
                                        TerminalMouseButton::Left,
                                        TerminalMouseEventKind::Press,
                                        event.modifiers,
                                    ),
                                    modes,
                                )
                            };
                            if let Some(bytes) = bytes {
                                if let Some(on_input) = &down_input {
                                    on_input(&bytes, window, cx);
                                }
                            } else if let Some(on_selection) = &down_selection {
                                let selection = {
                                    let model = down_model.borrow();
                                    if event.modifiers.shift {
                                        model
                                            .selection()
                                            .map(|selection| {
                                                TerminalSelection::new(
                                                    selection.bounds().0,
                                                    position,
                                                )
                                            })
                                            .unwrap_or_else(|| {
                                                TerminalSelection::new(position, position)
                                            })
                                    } else {
                                        model.selection_for_click(
                                            position,
                                            event.click_count,
                                            mouse_visible_scrollback,
                                        )
                                    }
                                };
                                down_anchor.set(Some(selection.bounds().0));
                                on_selection(&selection, window, cx);
                            }
                        }
                    },
                )
                .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                    let metrics = {
                        let model = move_model.borrow();
                        metrics_from_bounds(move_bounds.get(), &model, mouse_metrics, mouse_rows)
                    };
                    if let Some(metrics) = metrics {
                        if event.dragging()
                            && let Some(on_scroll) = &move_scroll
                        {
                            if event.position.y < metrics.bounds.top() {
                                on_scroll(&1, window, cx);
                            } else if event.position.y > metrics.bounds.bottom() {
                                on_scroll(&-1, window, cx);
                            }
                        }
                        let position = metrics.position_for_point(event.position);
                        if let Some(on_link_hover) = &move_link_hover {
                            let link = move_model
                                .borrow()
                                .link_at(position, mouse_visible_scrollback);
                            on_link_hover(&link, window, cx);
                        }
                        let mouse_bytes = event
                            .pressed_button
                            .and_then(terminal_mouse_button)
                            .and_then(|button| {
                                let modes = move_model.borrow().modes();
                                terminal_mouse_event_bytes(
                                    mouse_event_from_parts(
                                        position,
                                        button,
                                        if event.dragging() {
                                            TerminalMouseEventKind::Drag
                                        } else {
                                            TerminalMouseEventKind::Move
                                        },
                                        event.modifiers,
                                    ),
                                    modes,
                                )
                            });
                        if let Some(bytes) = mouse_bytes {
                            if let Some(on_input) = &move_input {
                                on_input(&bytes, window, cx);
                            }
                            return;
                        }
                        if event.dragging()
                            && let Some(anchor) = move_anchor.get()
                            && let Some(on_selection) = &move_selection
                        {
                            let selection = TerminalSelection::new(anchor, position);
                            on_selection(&selection, window, cx);
                        }
                    }
                })
                .on_mouse_exit(move |_, window, cx| {
                    if let Some(on_link_hover) = &exit_link_hover {
                        on_link_hover(&None, window, cx);
                    }
                })
                .on_mouse_up(
                    MouseButton::Left,
                    move |event: &MouseUpEvent, window, cx| {
                        let metrics = {
                            let model = up_model.borrow();
                            metrics_from_bounds(up_bounds.get(), &model, mouse_metrics, mouse_rows)
                        };
                        if let Some(metrics) = metrics {
                            let position = metrics.position_for_point(event.position);
                            let bytes = {
                                let modes = up_model.borrow().modes();
                                terminal_mouse_event_bytes(
                                    mouse_event_from_parts(
                                        position,
                                        TerminalMouseButton::Left,
                                        TerminalMouseEventKind::Release,
                                        event.modifiers,
                                    ),
                                    modes,
                                )
                            };
                            if let Some(bytes) = bytes {
                                if let Some(on_input) = &up_input {
                                    on_input(&bytes, window, cx);
                                }
                            } else if let Some(anchor) = up_anchor.get()
                                && let Some(on_selection) = &up_selection
                            {
                                let selection = TerminalSelection::new(anchor, position);
                                on_selection(&selection, window, cx);
                            }
                        }
                        up_anchor.set(None);
                    },
                );
        }
        if self.on_input.is_some() || self.on_viewport_scroll.is_some() {
            let wheel_model = self.model.clone();
            let wheel_metrics = metrics;
            let wheel_rows = rendered_row_count(&self.model.borrow(), &self.options);
            let wheel_bounds = grid_bounds.clone();
            let wheel_input = self.on_input.clone();
            let wheel_scroll = self.on_viewport_scroll.clone();
            grid = grid.on_scroll_wheel(move |event: &ScrollWheelEvent, window, cx| {
                let line_delta = terminal_scroll_line_delta(event.delta, wheel_metrics.line_height);
                if line_delta == 0 {
                    return;
                }
                let metrics = {
                    let model = wheel_model.borrow();
                    metrics_from_bounds(wheel_bounds.get(), &model, wheel_metrics, wheel_rows)
                };
                if let Some(metrics) = metrics {
                    let position = metrics.position_for_point(event.position);
                    let button = if line_delta > 0 {
                        TerminalMouseButton::WheelUp
                    } else {
                        TerminalMouseButton::WheelDown
                    };
                    let modes = wheel_model.borrow().modes();
                    let mouse_bytes = terminal_mouse_event_bytes(
                        mouse_event_from_parts(
                            position,
                            button,
                            TerminalMouseEventKind::Press,
                            event.modifiers,
                        ),
                        modes,
                    );
                    if let Some(bytes) = mouse_bytes {
                        if let Some(on_input) = &wheel_input {
                            for _ in 0..line_delta.unsigned_abs() {
                                on_input(&bytes, window, cx);
                            }
                        }
                        return;
                    }
                    let alternate_bytes = terminal_alternate_scroll_bytes(line_delta, modes);
                    if let Some(bytes) = alternate_bytes {
                        if let Some(on_input) = &wheel_input {
                            for _ in 0..line_delta.unsigned_abs() {
                                on_input(&bytes, window, cx);
                            }
                        }
                        return;
                    }
                }
                if let Some(on_viewport_scroll) = &wheel_scroll {
                    on_viewport_scroll(&line_delta, window, cx);
                }
            });
        }

        let model_for_paint = self.model.borrow();
        let render_rows = terminal_render_rows(&model_for_paint, &self.options);
        let search_matches = rendered_search_matches(
            &model_for_paint,
            &self.options,
            &self.search_matches,
            self.active_search_match,
        );
        let paint_plan = terminal_paint_plan(
            &render_rows,
            &theme,
            self.model.borrow().modes.reverse_video,
            self.model.borrow().modes.cursor_blink,
            self.options.blink_visible,
            &font_family,
            self.options.font_weight,
            self.options.ligatures,
            &self.options.ansi_palette,
            self.hovered_link.as_deref(),
            &search_matches,
        );
        grid = grid.child(
            canvas(
                move |bounds, window, _cx| {
                    paint_plan.layout(bounds, font_size, cell_width, line_height, window)
                },
                |_bounds, layout, window, cx| layout.paint(window, cx),
            )
            .absolute()
            .inset_0(),
        );
        if let (Some(marked_text), Some(cursor)) = (marked_text, rendered_cursor) {
            grid = grid.child(render_marked_text_overlay(
                &marked_text,
                cursor,
                &theme,
                cell_width,
                line_height,
            ));
        }
        root.child(grid)
    }
}

fn windows_shell_profiles() -> Vec<TerminalShellProfile> {
    let mut profiles = [
        ("pwsh", "PowerShell", "pwsh.exe"),
        ("powershell", "Windows PowerShell", "powershell.exe"),
        ("cmd", "Command Prompt", "cmd.exe"),
    ]
    .into_iter()
    .map(|(id, label, command)| TerminalShellProfile::new(id, label, command))
    .collect::<Vec<_>>();
    if let Some(profile) = profiles.iter_mut().find(|profile| profile.is_available()) {
        profile.is_default = true;
    }
    profiles
}

fn unix_shell_profiles() -> Vec<TerminalShellProfile> {
    let default = std::env::var_os("SHELL")
        .filter(|shell| !shell.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/bin/sh"));
    let default_label = default
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Default shell")
        .to_string();
    let mut profiles =
        vec![TerminalShellProfile::new("default", default_label, default.clone()).default(true)];
    for (id, label, command) in [
        ("zsh", "zsh", PathBuf::from("/bin/zsh")),
        ("bash", "bash", PathBuf::from("/bin/bash")),
        ("sh", "sh", PathBuf::from("/bin/sh")),
    ] {
        if !profiles.iter().any(|profile| profile.command == command) {
            profiles.push(TerminalShellProfile::new(id, label, command));
        }
    }
    profiles
}

fn command_available(command: &std::path::Path) -> bool {
    if command.is_absolute() {
        return command.exists();
    }

    if cfg!(target_os = "windows") {
        return std::process::Command::new("where")
            .arg(command)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
    }

    let shell_command = format!("command -v {}", shell_quote(command.as_os_str()));
    std::process::Command::new("/bin/sh")
        .arg("-lc")
        .arg(shell_command)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn shell_quote(value: &std::ffi::OsStr) -> String {
    let text = value.to_string_lossy();
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn configure_shell_command(builder: &mut CommandBuilder, command: &std::path::Path) {
    builder.env("TERM", "xterm-256color");
    builder.env("COLORTERM", "truecolor");
    builder.env("TERM_PROGRAM", "guic");

    if cfg!(target_os = "windows")
        && command
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| {
                name.eq_ignore_ascii_case("pwsh.exe") || name.eq_ignore_ascii_case("powershell.exe")
            })
            .unwrap_or(false)
    {
        builder.arg("-NoLogo");
        builder.arg("-NoProfile");
        builder.arg("-NoExit");
    }
}

fn graceful_close_bytes(command: &std::path::Path) -> &'static [u8] {
    if cfg!(target_os = "windows") {
        let name = command
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if name.eq_ignore_ascii_case("cmd.exe")
            || name.eq_ignore_ascii_case("pwsh.exe")
            || name.eq_ignore_ascii_case("powershell.exe")
        {
            return b"exit\r\n";
        }
    }

    b"\x04"
}

fn metrics_from_bounds(
    bounds: Option<Bounds<Pixels>>,
    model: &TerminalModel,
    metrics: TerminalFontMetrics,
    rows: usize,
) -> Option<TerminalGridMetrics> {
    Some(TerminalGridMetrics {
        bounds: bounds?,
        cell_width: metrics.cell_width,
        line_height: metrics.line_height,
        columns: model.columns(),
        rows,
    })
}

fn terminal_grid_size_for_bounds(
    bounds: Bounds<Pixels>,
    metrics: TerminalFontMetrics,
) -> TerminalGridSize {
    let width = f32::from(bounds.size.width).max(0.0);
    let height = f32::from(bounds.size.height).max(0.0);
    let cell_width = f32::from(metrics.cell_width).max(1.0);
    let line_height = f32::from(metrics.line_height).max(1.0);
    TerminalGridSize {
        columns: (width / cell_width).floor().max(1.0) as usize,
        rows: (height / line_height).floor().max(1.0) as usize,
    }
}

fn terminal_font_family(
    options: &TerminalOptions,
    theme: &Theme,
    window: &mut Window,
) -> SharedString {
    static AVAILABLE_FONT_NAMES: OnceLock<Vec<String>> = OnceLock::new();
    let preferred = options
        .font_family
        .clone()
        .unwrap_or_else(|| SharedString::from(theme.typography.mono_family.clone()));
    let available = AVAILABLE_FONT_NAMES.get_or_init(|| window.text_system().all_font_names());
    select_terminal_font_family(preferred.as_ref(), available)
}

fn select_terminal_font_family(preferred: &str, available: &[String]) -> SharedString {
    let find = |candidate: &str| {
        available
            .iter()
            .find(|name| name.eq_ignore_ascii_case(candidate))
            .cloned()
    };
    if let Some(family) = find(preferred) {
        return family.into();
    }

    #[cfg(target_os = "macos")]
    const FALLBACKS: &[&str] = &["SF Mono", "Menlo", "Monaco"];
    #[cfg(target_os = "windows")]
    const FALLBACKS: &[&str] = &["Cascadia Mono", "Consolas"];
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    const FALLBACKS: &[&str] = &[
        "DejaVu Sans Mono",
        "Liberation Mono",
        "Ubuntu Mono",
        "Courier New",
        "Courier",
        "monospace",
    ];

    FALLBACKS
        .iter()
        .find_map(|candidate| find(candidate))
        .unwrap_or_else(|| preferred.to_owned())
        .into()
}

fn terminal_font_metrics(
    options: &TerminalOptions,
    theme: &Theme,
    family: &SharedString,
    window: &mut Window,
) -> TerminalFontMetrics {
    if !options.measure_font {
        return TerminalFontMetrics {
            cell_width: px(f32::from(options.cell_width)),
            line_height: px(f32::from(options.line_height)),
        };
    }

    let font_size = px(f32::from(options.font_size));
    let mut metric_font = font(family.clone());
    metric_font.weight = FontWeight::from(f32::from(options.font_weight));
    if !options.ligatures {
        metric_font.features = FontFeatures::disable_ligatures();
    }
    let run = TextRun {
        len: 1,
        font: metric_font.clone(),
        color: theme.foreground(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let cell_width = window
        .text_system()
        .shape_text(SharedString::from("M"), font_size, &[run], None, None)
        .ok()
        .and_then(|lines| lines.first().map(|line| line.width()))
        .filter(|width| f32::from(*width).is_finite() && f32::from(*width) > 0.0)
        .unwrap_or_else(|| px(f32::from(options.cell_width)));
    let measured_line_height =
        px(f32::from(options.font_size) * f32::from(options.line_height_percent) / 100.0);
    let themed_line_height = px(theme
        .typography
        .line_height_sm
        .max(f32::from(options.font_size)));

    let font_id = window.text_system().resolve_font(&metric_font);
    let native_height = window.text_system().ascent(font_id, font_size)
        + window.text_system().descent(font_id, font_size);
    TerminalFontMetrics {
        cell_width,
        line_height: measured_line_height
            .max(themed_line_height)
            .max(native_height)
            .max(px(f32::from(options.line_height))),
    }
}

fn rendered_row_count(model: &TerminalModel, options: &TerminalOptions) -> usize {
    if model.viewport_offset() > 0 {
        model.rows()
    } else {
        model
            .rows()
            .saturating_add(options.visible_scrollback.min(model.scrollback().len()))
    }
}

fn rendered_cursor_position(
    model: &TerminalModel,
    options: &TerminalOptions,
) -> Option<TerminalPosition> {
    (model.viewport_offset() == 0).then_some(TerminalPosition {
        row: model
            .cursor()
            .row
            .saturating_add(options.visible_scrollback.min(model.scrollback().len())),
        column: model.cursor().column,
    })
}

fn key_event_uses_text_input(event: &KeyDownEvent) -> bool {
    if event.keystroke.modifiers.alt
        || event.keystroke.modifiers.control
        || event.keystroke.modifiers.platform
        || event.keystroke.modifiers.function
    {
        return false;
    }

    let text = event
        .keystroke
        .key_char
        .as_deref()
        .unwrap_or(&event.keystroke.key);
    text.chars().count() == 1 && text.chars().any(|character| !character.is_control())
}

fn text_display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn byte_index_for_display_width(text: &str, target_width: usize) -> usize {
    let mut width = 0;
    for (index, grapheme) in UnicodeSegmentation::grapheme_indices(text, true) {
        let next_width = width + UnicodeWidthStr::width(grapheme);
        if next_width > target_width {
            return index;
        }
        width = next_width;
    }
    text.len()
}

fn utf16_offset_for_byte_index(text: &str, byte_index: usize) -> usize {
    text[..byte_index.min(text.len())]
        .chars()
        .map(char::len_utf16)
        .sum()
}

fn byte_index_for_utf16_offset(text: &str, utf16_offset: usize) -> usize {
    if utf16_offset == 0 {
        return 0;
    }

    let mut units = 0;
    for (index, ch) in text.char_indices() {
        let next_units = units + ch.len_utf16();
        if next_units > utf16_offset {
            return index;
        }
        units = next_units;
    }
    text.len()
}

fn byte_range_from_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    let start = byte_index_for_utf16_offset(text, range.start);
    let end = byte_index_for_utf16_offset(text, range.end).max(start);
    start..end
}

fn utf16_range_for_byte_range(text: &str, range: &Range<usize>) -> Range<usize> {
    utf16_offset_for_byte_index(text, range.start)..utf16_offset_for_byte_index(text, range.end)
}

fn default_tab_stops(columns: usize) -> Vec<bool> {
    let mut stops = vec![false; columns];
    for column in (8..columns).step_by(8) {
        stops[column] = true;
    }
    stops
}

fn terminal_mouse_button(button: MouseButton) -> Option<TerminalMouseButton> {
    match button {
        MouseButton::Left => Some(TerminalMouseButton::Left),
        MouseButton::Middle => Some(TerminalMouseButton::Middle),
        MouseButton::Right => Some(TerminalMouseButton::Right),
        _ => None,
    }
}

fn mouse_event_from_parts(
    position: TerminalPosition,
    button: TerminalMouseButton,
    kind: TerminalMouseEventKind,
    modifiers: gpui::Modifiers,
) -> TerminalMouseEvent {
    TerminalMouseEvent {
        row: position.row,
        column: position.column,
        button,
        kind,
        modifiers: TerminalInputModifiers {
            shift: modifiers.shift,
            alt: modifiers.alt,
            control: modifiers.control,
        },
    }
}

fn terminal_scroll_line_delta(delta: ScrollDelta, line_height: Pixels) -> isize {
    match delta {
        ScrollDelta::Lines(point) => point.y.round() as isize,
        ScrollDelta::Pixels(point) => {
            let line_height = f32::from(line_height).max(1.0);
            let lines = f32::from(point.y) / line_height;
            if lines.abs() < 1.0 {
                lines.signum() as isize
            } else {
                lines.round() as isize
            }
        }
    }
}

fn selected_text_from_lines(
    selection: TerminalSelection,
    lines: &[TerminalLine],
    columns: usize,
) -> Option<String> {
    let (start, end) = selection.bounds();
    let mut text = String::new();
    for row in start.row..=end.row.min(lines.len().saturating_sub(1)) {
        let start_column = if row == start.row { start.column } else { 0 };
        let end_column = if row == end.row {
            end.column
        } else {
            columns.saturating_sub(1)
        };
        if let Some(line) = lines.get(row) {
            let line_text = line
                .cells()
                .iter()
                .skip(start_column)
                .take(end_column.saturating_sub(start_column).saturating_add(1))
                .map(|cell| cell.text.as_ref())
                .collect::<String>();
            let line_text = if !line.is_wrapped() && end_column == columns.saturating_sub(1) {
                line_text.trim_end().to_string()
            } else {
                line_text
            };
            if !text.is_empty()
                && row
                    .checked_sub(1)
                    .and_then(|previous| lines.get(previous))
                    .is_none_or(|line| !line.is_wrapped())
            {
                text.push('\n');
            }
            text.push_str(&line_text);
        }
    }
    Some(text)
}

fn meaningful_cells(line: &TerminalLine) -> Vec<TerminalCell> {
    if line.wrapped {
        return line.cells.clone();
    }
    let end = line
        .cells
        .iter()
        .rposition(|cell| cell.text.as_ref() != " ")
        .map(|index| index + 1)
        .unwrap_or(0);
    line.cells.iter().take(end).cloned().collect()
}

#[derive(Clone, Copy)]
struct ReflowAnchor {
    logical_line: usize,
    cell_offset: usize,
}

fn reflow_anchor_for_position<'a>(
    lines: impl Iterator<Item = &'a TerminalLine>,
    target_row: usize,
    target_column: usize,
) -> Option<ReflowAnchor> {
    let mut logical_line = 0;
    let mut cell_offset = 0;
    for (row, line) in lines.enumerate() {
        let meaningful_len = meaningful_cells(line).len();
        if row == target_row {
            return Some(ReflowAnchor {
                logical_line,
                cell_offset: cell_offset + target_column.min(meaningful_len.saturating_sub(1)),
            });
        }
        if line.is_wrapped() {
            cell_offset += meaningful_len;
        } else {
            logical_line += 1;
            cell_offset = 0;
        }
    }
    None
}

fn position_for_reflow_anchor(
    anchor: ReflowAnchor,
    logical_lengths: &[usize],
    columns: usize,
) -> Option<TerminalPosition> {
    let logical_len = *logical_lengths.get(anchor.logical_line)?;
    let preceding_rows = logical_lengths
        .iter()
        .take(anchor.logical_line)
        .map(|len| (*len).max(1).div_ceil(columns))
        .sum::<usize>();
    let offset = anchor.cell_offset.min(logical_len.saturating_sub(1));
    Some(TerminalPosition {
        row: preceding_rows + offset / columns,
        column: offset % columns,
    })
}

fn position_with_eviction(
    mut position: TerminalPosition,
    evicted_rows: usize,
) -> Option<TerminalPosition> {
    position.row = position.row.checked_sub(evicted_rows)?;
    Some(position)
}

/// Converts a GPUI keystroke into xterm-compatible input bytes.
#[must_use]
pub fn terminal_keystroke_bytes(keystroke: &Keystroke) -> Option<Vec<u8>> {
    terminal_keystroke_bytes_with_modes(keystroke, TerminalModes::default())
}

/// Converts a GPUI key-down event into terminal input bytes using terminal modes.
#[must_use]
pub fn terminal_key_down_event_bytes(
    event: &KeyDownEvent,
    modes: TerminalModes,
) -> Option<Vec<u8>> {
    if event.prefer_character_input
        && !event.keystroke.modifiers.control
        && !event.keystroke.modifiers.platform
        && !event.keystroke.modifiers.function
        && let Some(text) = &event.keystroke.key_char
    {
        return Some(terminal_text_input_bytes(text, modes));
    }

    terminal_keystroke_bytes_with_modes(&event.keystroke, modes)
}

/// Converts committed text input, including IME commit text, into terminal input bytes.
#[must_use]
pub fn terminal_text_input_bytes(text: &str, _modes: TerminalModes) -> Vec<u8> {
    text.as_bytes().to_vec()
}

/// Converts a GPUI keystroke into xterm-compatible input bytes using terminal modes.
#[must_use]
pub fn terminal_keystroke_bytes_with_modes(
    keystroke: &Keystroke,
    modes: TerminalModes,
) -> Option<Vec<u8>> {
    if keystroke.modifiers.platform || keystroke.modifiers.function {
        return None;
    }

    if keystroke.modifiers.control
        && !keystroke.modifiers.alt
        && !keystroke.modifiers.shift
        && let Some(byte) = control_byte(&keystroke.key)
    {
        return Some(vec![byte]);
    }

    let base = match keystroke.key.as_str() {
        "enter" => Some(b"\r".to_vec()),
        "backspace" => Some(vec![0x7f]),
        "tab" if keystroke.modifiers.shift => Some(b"\x1b[Z".to_vec()),
        "tab" => Some(b"\t".to_vec()),
        "escape" => Some(vec![0x1b]),
        "space" => Some(b" ".to_vec()),
        "up" => Some(cursor_key('A', keystroke, modes)),
        "down" => Some(cursor_key('B', keystroke, modes)),
        "right" => Some(cursor_key('C', keystroke, modes)),
        "left" => Some(cursor_key('D', keystroke, modes)),
        "home" => Some(cursor_key('H', keystroke, modes)),
        "end" => Some(cursor_key('F', keystroke, modes)),
        "pageup" => Some(modified_tilde(5, keystroke)),
        "pagedown" => Some(modified_tilde(6, keystroke)),
        "insert" => Some(modified_tilde(2, keystroke)),
        "delete" => Some(modified_tilde(3, keystroke)),
        "f1" => Some(modified_ss3('P', keystroke)),
        "f2" => Some(modified_ss3('Q', keystroke)),
        "f3" => Some(modified_ss3('R', keystroke)),
        "f4" => Some(modified_ss3('S', keystroke)),
        "f5" => Some(modified_tilde(15, keystroke)),
        "f6" => Some(modified_tilde(17, keystroke)),
        "f7" => Some(modified_tilde(18, keystroke)),
        "f8" => Some(modified_tilde(19, keystroke)),
        "f9" => Some(modified_tilde(20, keystroke)),
        "f10" => Some(modified_tilde(21, keystroke)),
        "f11" => Some(modified_tilde(23, keystroke)),
        "f12" => Some(modified_tilde(24, keystroke)),
        _ if !keystroke.modifiers.control && !keystroke.modifiers.shift => {
            keystroke.key_char.clone().map(String::into_bytes)
        }
        _ if !keystroke.modifiers.control => keystroke.key_char.clone().map(String::into_bytes),
        _ => None,
    }?;

    if keystroke.modifiers.alt && modes.meta_sends_escape {
        let mut bytes = Vec::with_capacity(base.len() + 1);
        bytes.push(0x1b);
        bytes.extend(base);
        Some(bytes)
    } else {
        Some(base)
    }
}

fn cursor_key(final_byte: char, keystroke: &Keystroke, modes: TerminalModes) -> Vec<u8> {
    if modes.application_cursor_keys && modifier_parameter(keystroke).is_none() {
        cursor_key_from_modes(final_byte, modes)
    } else {
        modified_csi(final_byte, keystroke)
    }
}

fn cursor_key_from_modes(final_byte: char, modes: TerminalModes) -> Vec<u8> {
    if modes.application_cursor_keys {
        format!("\u{1b}O{final_byte}").into_bytes()
    } else {
        format!("\u{1b}[{final_byte}").into_bytes()
    }
}

/// Encodes pasted text for a terminal session.
#[must_use]
pub fn terminal_paste_bytes(text: &str, modes: TerminalModes) -> Vec<u8> {
    let text = sanitized_terminal_paste(text);
    if modes.bracketed_paste {
        let mut bytes = Vec::with_capacity(text.len() + 12);
        bytes.extend_from_slice(b"\x1b[200~");
        bytes.extend_from_slice(text.as_bytes());
        bytes.extend_from_slice(b"\x1b[201~");
        bytes
    } else {
        text.into_bytes()
    }
}

fn sanitized_terminal_paste(text: &str) -> String {
    let mut output = String::with_capacity(text.len().min(MAX_TERMINAL_PASTE_BYTES));
    let mut skip_lf = false;
    for character in text.chars() {
        if skip_lf {
            skip_lf = false;
            if character == '\n' {
                continue;
            }
        }
        let character = if character == '\r' {
            skip_lf = true;
            '\n'
        } else {
            character
        };
        if character != '\n'
            && character != '\t'
            && (character.is_control() || character == '\u{7f}')
        {
            continue;
        }
        if output.len() + character.len_utf8() > MAX_TERMINAL_PASTE_BYTES {
            break;
        }
        output.push(character);
    }
    output
}

/// Encodes a mouse event according to the terminal's current mouse reporting modes.
#[must_use]
pub fn terminal_mouse_event_bytes(
    event: TerminalMouseEvent,
    modes: TerminalModes,
) -> Option<Vec<u8>> {
    let reporting_enabled = modes.mouse_button_reporting
        || modes.mouse_drag_reporting
        || modes.mouse_all_motion_reporting
        || modes.x10_mouse_reporting;
    if !reporting_enabled {
        return None;
    }
    if modes.x10_mouse_reporting && event.kind != TerminalMouseEventKind::Press {
        return None;
    }
    if event.kind == TerminalMouseEventKind::Drag
        && !(modes.mouse_drag_reporting || modes.mouse_all_motion_reporting)
    {
        return None;
    }
    if event.kind == TerminalMouseEventKind::Move && !modes.mouse_all_motion_reporting {
        return None;
    }

    let code = mouse_button_code(event)?;
    let x = event.column.saturating_add(1);
    let y = event.row.saturating_add(1);
    if modes.sgr_mouse {
        let suffix = if event.kind == TerminalMouseEventKind::Release {
            'm'
        } else {
            'M'
        };
        return Some(format!("\u{1b}[<{code};{x};{y}{suffix}").into_bytes());
    }
    if modes.urxvt_mouse {
        let suffix = if event.kind == TerminalMouseEventKind::Release {
            'm'
        } else {
            'M'
        };
        return Some(format!("\u{1b}[{code};{x};{y}{suffix}").into_bytes());
    }

    let x = u8::try_from(x.min(223)).ok()?;
    let y = u8::try_from(y.min(223)).ok()?;
    let code = u8::try_from(code.min(223)).ok()?;
    Some(vec![0x1b, b'[', b'M', code + 32, x + 32, y + 32])
}

/// Encodes focus-in/focus-out events for terminals that enabled focus reporting.
#[must_use]
pub fn terminal_focus_event_bytes(focused: bool, modes: TerminalModes) -> Option<Vec<u8>> {
    modes.focus_event_reporting.then(|| {
        if focused {
            b"\x1b[I".to_vec()
        } else {
            b"\x1b[O".to_vec()
        }
    })
}

/// Encodes alternate-screen wheel scrolling as cursor keys when enabled.
#[must_use]
pub fn terminal_alternate_scroll_bytes(delta: isize, modes: TerminalModes) -> Option<Vec<u8>> {
    (modes.alternate_screen && modes.alternate_scroll).then(|| {
        if delta > 0 {
            cursor_key_from_modes('A', modes)
        } else {
            cursor_key_from_modes('B', modes)
        }
    })
}

fn mouse_button_code(event: TerminalMouseEvent) -> Option<u16> {
    let base = match (event.kind, event.button) {
        (TerminalMouseEventKind::Release, _) => 3,
        (_, TerminalMouseButton::Left) => 0,
        (_, TerminalMouseButton::Middle) => 1,
        (_, TerminalMouseButton::Right) => 2,
        (_, TerminalMouseButton::WheelUp) => 64,
        (_, TerminalMouseButton::WheelDown) => 65,
        (_, TerminalMouseButton::None) if event.kind == TerminalMouseEventKind::Move => 35,
        (_, TerminalMouseButton::None) => return None,
    };
    let drag = if event.kind == TerminalMouseEventKind::Drag {
        32
    } else {
        0
    };
    let modifiers = u16::from(event.modifiers.shift) * 4
        + u16::from(event.modifiers.alt) * 8
        + u16::from(event.modifiers.control) * 16;
    Some(base + drag + modifiers)
}

fn control_byte(key: &str) -> Option<u8> {
    if key.chars().count() != 1 {
        return None;
    }
    let byte = key.as_bytes().first().copied()?;
    match byte.to_ascii_lowercase() {
        b'a'..=b'z' => Some(byte.to_ascii_lowercase() - b'a' + 1),
        b'[' => Some(0x1b),
        b'\\' => Some(0x1c),
        b']' => Some(0x1d),
        b'^' => Some(0x1e),
        b'_' => Some(0x1f),
        b'?' => Some(0x7f),
        _ => None,
    }
}

fn modifier_parameter(keystroke: &Keystroke) -> Option<u8> {
    let parameter = 1
        + u8::from(keystroke.modifiers.shift)
        + u8::from(keystroke.modifiers.alt) * 2
        + u8::from(keystroke.modifiers.control) * 4;
    (parameter > 1).then_some(parameter)
}

fn modified_csi(final_byte: char, keystroke: &Keystroke) -> Vec<u8> {
    if let Some(parameter) = modifier_parameter(keystroke) {
        format!("\u{1b}[1;{parameter}{final_byte}").into_bytes()
    } else {
        format!("\u{1b}[{final_byte}").into_bytes()
    }
}

fn modified_ss3(final_byte: char, keystroke: &Keystroke) -> Vec<u8> {
    if let Some(parameter) = modifier_parameter(keystroke) {
        format!("\u{1b}[1;{parameter}{final_byte}").into_bytes()
    } else {
        format!("\u{1b}O{final_byte}").into_bytes()
    }
}

fn modified_tilde(code: u8, keystroke: &Keystroke) -> Vec<u8> {
    if let Some(parameter) = modifier_parameter(keystroke) {
        format!("\u{1b}[{code};{parameter}~").into_bytes()
    } else {
        format!("\u{1b}[{code}~").into_bytes()
    }
}

struct TerminalRenderRow<'a> {
    line: Cow<'a, TerminalLine>,
    row: usize,
    cursor: Option<(usize, TerminalCursorStyle)>,
    selection: Option<TerminalSelection>,
    muted: bool,
}

struct DetectedTerminalLink {
    start_column: usize,
    end_column: usize,
    target: SharedString,
}

fn detected_links_in_line(line: &TerminalLine) -> Vec<DetectedTerminalLink> {
    let mut links = Vec::new();
    let mut start = 0;
    while start < line.cells.len() {
        while start < line.cells.len() && line.cells[start].text.chars().all(char::is_whitespace) {
            start += 1;
        }
        if start == line.cells.len() {
            break;
        }
        let mut end = start;
        while end + 1 < line.cells.len()
            && !line.cells[end + 1].text.chars().all(char::is_whitespace)
        {
            end += 1;
        }
        let raw = line.cells[start..=end]
            .iter()
            .map(|cell| cell.text.as_ref())
            .collect::<String>();
        let leading = raw
            .chars()
            .take_while(|character| "([{\"'".contains(*character))
            .count();
        let trailing = raw
            .chars()
            .rev()
            .take_while(|character| ".,;:!?)]}\"'".contains(*character))
            .count();
        let candidate = raw
            .chars()
            .skip(leading)
            .take(raw.chars().count().saturating_sub(leading + trailing))
            .collect::<String>();
        let candidate_lower = candidate.to_ascii_lowercase();
        let is_url = candidate_lower.starts_with("http://")
            || candidate_lower.starts_with("https://")
            || candidate_lower.starts_with("mailto:");
        let is_path = candidate.starts_with('/')
            || candidate.starts_with("./")
            || candidate.starts_with("../")
            || candidate.starts_with("~/");
        if candidate.len() <= MAX_OSC_HYPERLINK_BYTES
            && !candidate.chars().any(char::is_control)
            && (is_url || is_path)
        {
            links.push(DetectedTerminalLink {
                start_column: start.saturating_add(leading),
                end_column: end.saturating_sub(trailing),
                target: SharedString::from(candidate),
            });
        }
        start = end.saturating_add(1);
    }
    links
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TerminalPaintRect {
    row: usize,
    column: usize,
    cells: usize,
    color: Hsla,
}

#[derive(Clone)]
struct TerminalPaintTextBatch {
    row: usize,
    column: usize,
    end_column: usize,
    text: String,
    style: TextRun,
}

struct TerminalPaintPlan {
    backgrounds: Vec<TerminalPaintRect>,
    cursors: Vec<(usize, usize, TerminalCursorStyle, Hsla)>,
    text: Vec<TerminalPaintTextBatch>,
}

#[derive(Clone, Copy)]
struct RenderedSearchMatch {
    range: TerminalSearchMatch,
    active: bool,
}

struct TerminalCanvasLayout {
    quads: Vec<PaintQuad>,
    text: Vec<(Point<Pixels>, ShapedLine)>,
    line_height: Pixels,
}

impl TerminalPaintPlan {
    fn layout(
        self,
        bounds: Bounds<Pixels>,
        font_size: Pixels,
        cell_width: Pixels,
        line_height: Pixels,
        window: &mut Window,
    ) -> TerminalCanvasLayout {
        let mut quads = Vec::with_capacity(self.backgrounds.len() + self.cursors.len());
        for background in self.backgrounds {
            quads.push(fill(
                Bounds::new(
                    point(
                        bounds.left() + cell_width * background.column as f32,
                        bounds.top() + line_height * background.row as f32,
                    ),
                    size(cell_width * background.cells as f32, line_height),
                ),
                background.color,
            ));
        }
        for (row, column, style, color) in self.cursors {
            let origin = point(
                bounds.left() + cell_width * column as f32,
                bounds.top() + line_height * row as f32,
            );
            let cursor_bounds = match style {
                TerminalCursorStyle::Block => continue,
                TerminalCursorStyle::Underline => Bounds::new(
                    point(origin.x, origin.y + line_height - px(2.0)),
                    size(cell_width, px(2.0)),
                ),
                TerminalCursorStyle::Bar => Bounds::new(origin, size(px(2.0), line_height)),
            };
            quads.push(fill(cursor_bounds, color));
        }

        let text = self
            .text
            .into_iter()
            .map(|batch| {
                let position = point(
                    bounds.left() + cell_width * batch.column as f32,
                    bounds.top() + line_height * batch.row as f32,
                );
                let line = window.text_system().shape_line(
                    SharedString::from(batch.text),
                    font_size,
                    &[batch.style],
                    Some(cell_width),
                );
                (position, line)
            })
            .collect();
        TerminalCanvasLayout {
            quads,
            text,
            line_height,
        }
    }
}

impl TerminalCanvasLayout {
    fn paint(self, window: &mut Window, cx: &mut App) {
        for quad in self.quads {
            window.paint_quad(quad);
        }
        for (position, line) in self.text {
            let _ = line.paint(
                position,
                self.line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }
}

fn terminal_render_rows<'a>(
    model: &'a TerminalModel,
    options: &TerminalOptions,
) -> Vec<TerminalRenderRow<'a>> {
    let selection = model.selection;
    if model.viewport_offset > 0 && options.visible_scrollback == 0 {
        let total = model.scrollback.len() + model.lines.len();
        let end = total.saturating_sub(model.viewport_offset.min(model.scrollback.len()));
        let start = end.saturating_sub(model.rows());
        return (start..end)
            .enumerate()
            .map(|(row, index)| {
                let line = if index < model.scrollback.len() {
                    &model.scrollback[index]
                } else {
                    &model.lines[index - model.scrollback.len()]
                };
                TerminalRenderRow {
                    line: Cow::Borrowed(line),
                    row,
                    cursor: None,
                    selection,
                    muted: index < model.scrollback.len(),
                }
            })
            .collect();
    }
    if model.viewport_offset() > 0 {
        return model
            .viewport_lines()
            .into_iter()
            .enumerate()
            .map(|(row, line)| TerminalRenderRow {
                line: Cow::Owned(line),
                row,
                cursor: None,
                selection,
                muted: false,
            })
            .collect();
    }

    let scrollback_start = model
        .scrollback
        .len()
        .saturating_sub(options.visible_scrollback);
    let visible_scrollback = options.visible_scrollback.min(model.scrollback.len());
    let mut rows = Vec::with_capacity(visible_scrollback + model.rows());
    rows.extend(
        model
            .scrollback
            .iter()
            .skip(scrollback_start)
            .enumerate()
            .map(|(row, line)| TerminalRenderRow {
                line: Cow::Borrowed(line),
                row,
                cursor: None,
                selection: None,
                muted: true,
            }),
    );
    rows.extend(
        model
            .lines
            .iter()
            .enumerate()
            .map(|(row_index, line)| TerminalRenderRow {
                line: Cow::Borrowed(line),
                row: row_index + visible_scrollback,
                cursor: (model.modes.cursor_visible && row_index == model.cursor.row).then_some((
                    model.cursor.column,
                    options.cursor_style.unwrap_or(model.modes.cursor_style),
                )),
                selection,
                muted: false,
            }),
    );
    rows
}

fn rendered_search_matches(
    model: &TerminalModel,
    options: &TerminalOptions,
    matches: &[TerminalSearchMatch],
    active: Option<usize>,
) -> Vec<RenderedSearchMatch> {
    let total = model.history_len();
    let visible_scrollback = options.visible_scrollback.min(model.scrollback.len());
    let start = if model.viewport_offset > 0 && visible_scrollback == 0 {
        total
            .saturating_sub(model.rows)
            .saturating_sub(model.viewport_offset.min(model.scrollback.len()))
    } else {
        model.scrollback.len().saturating_sub(visible_scrollback)
    };
    let end = start.saturating_add(visible_scrollback + model.rows);
    matches
        .iter()
        .enumerate()
        .filter(|(_, search_match)| (start..end).contains(&search_match.row))
        .map(|(index, search_match)| RenderedSearchMatch {
            range: TerminalSearchMatch {
                row: search_match.row - start,
                ..*search_match
            },
            active: active == Some(index),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn terminal_paint_plan(
    rows: &[TerminalRenderRow<'_>],
    theme: &Theme,
    reverse_video: bool,
    cursor_blink: bool,
    blink_visible: bool,
    font_family: &SharedString,
    font_weight: u16,
    ligatures: bool,
    ansi_palette: &[[u8; 3]; 16],
    hovered_link: Option<&str>,
    search_matches: &[RenderedSearchMatch],
) -> TerminalPaintPlan {
    let mut backgrounds = Vec::new();
    let mut cursors = Vec::new();
    let mut text: Vec<TerminalPaintTextBatch> = Vec::new();

    for row in rows {
        let detected_links = detected_links_in_line(&row.line);
        for (column, cell) in row.line.cells.iter().enumerate() {
            let cursor_style = row.cursor.and_then(|(cursor_column, style)| {
                (cursor_column == column && (!cursor_blink || blink_visible)).then_some(style)
            });
            let (mut foreground, mut background) =
                terminal_cell_colors(cell.style, theme, reverse_video, ansi_palette);
            let detected_link = detected_links
                .iter()
                .find(|link| (link.start_column..=link.end_column).contains(&column));
            let has_link = cell.hyperlink.is_some() || detected_link.is_some();
            if has_link && cell.style.foreground.is_none() {
                foreground = theme.accent();
            }
            let link_hovered = cell
                .hyperlink
                .as_deref()
                .is_some_and(|link| Some(link) == hovered_link)
                || detected_link.is_some_and(|link| Some(link.target.as_ref()) == hovered_link);
            if row.muted {
                foreground = foreground.opacity(0.72);
            }
            if cell.style.faint {
                foreground = foreground.opacity(0.7);
            }
            if cell.style.hidden {
                foreground = foreground.opacity(0.0);
            }
            if cursor_style == Some(TerminalCursorStyle::Block) {
                std::mem::swap(&mut foreground, &mut background);
            }
            if row
                .selection
                .is_some_and(|selection| selection.contains(row.row, column))
            {
                background = theme.primary().opacity(0.28);
            } else if link_hovered {
                background = theme.accent().opacity(0.14);
            } else if let Some(search_match) = search_matches.iter().find(|search_match| {
                search_match.range.row == row.row
                    && (search_match.range.start_column..=search_match.range.end_column)
                        .contains(&column)
            }) {
                background = theme
                    .accent()
                    .opacity(if search_match.active { 0.32 } else { 0.14 });
            }
            if background != theme.background() {
                push_terminal_background(
                    &mut backgrounds,
                    TerminalPaintRect {
                        row: row.row,
                        column,
                        cells: 1,
                        color: background,
                    },
                );
            }
            if let Some(style) = cursor_style
                && style != TerminalCursorStyle::Block
            {
                cursors.push((row.row, column, style, theme.primary()));
            }
            if cell.text.as_ref() == " "
                || cell.style.hidden
                || (cell.style.blink && !blink_visible)
            {
                continue;
            }
            let mut cell_font = font(font_family.clone());
            cell_font.weight = FontWeight::from(f32::from(font_weight));
            if !ligatures {
                cell_font.features = FontFeatures::disable_ligatures();
            }
            if cell.style.bold {
                cell_font.weight =
                    FontWeight::from((f32::from(font_weight) + 300.0).clamp(100.0, 900.0));
            }
            if cell.style.italic {
                cell_font.style = FontStyle::Italic;
            }
            let underline = (has_link || cell.style.underline).then_some(UnderlineStyle {
                thickness: if link_hovered { px(2.0) } else { px(1.0) },
                color: Some(if has_link { theme.accent() } else { foreground }),
                wavy: false,
            });
            let strikethrough = cell.style.strikethrough.then_some(StrikethroughStyle {
                thickness: px(1.0),
                color: Some(foreground),
            });
            let style = TextRun {
                len: cell.text.len(),
                font: cell_font,
                color: foreground,
                background_color: None,
                underline,
                strikethrough,
            };
            let cell_span = text_display_width(cell.text.as_ref()).max(1);
            if let Some(batch) = text.last_mut()
                && batch.row == row.row
                && batch.end_column == column
                && terminal_text_runs_match(&batch.style, &style)
            {
                batch.text.push_str(cell.text.as_ref());
                batch.end_column = column + cell_span;
                batch.style.len += cell.text.len();
            } else {
                text.push(TerminalPaintTextBatch {
                    row: row.row,
                    column,
                    end_column: column + cell_span,
                    text: cell.text.to_string(),
                    style,
                });
            }
        }
    }

    TerminalPaintPlan {
        backgrounds,
        cursors,
        text,
    }
}

fn push_terminal_background(
    backgrounds: &mut Vec<TerminalPaintRect>,
    background: TerminalPaintRect,
) {
    if let Some(previous) = backgrounds.last_mut()
        && previous.row == background.row
        && previous.column + previous.cells == background.column
        && previous.color == background.color
    {
        previous.cells += background.cells;
    } else {
        backgrounds.push(background);
    }
}

fn terminal_text_runs_match(first: &TextRun, second: &TextRun) -> bool {
    first.font == second.font
        && first.color == second.color
        && first.underline == second.underline
        && first.strikethrough == second.strikethrough
}

fn render_marked_text_overlay(
    text: &str,
    cursor: TerminalPosition,
    theme: &Theme,
    cell_width: Pixels,
    line_height: Pixels,
) -> gpui::AnyElement {
    let width = text_display_width(text).max(1);
    div()
        .absolute()
        .left(cell_width * cursor.column as f32)
        .top(line_height * cursor.row as f32)
        .h(line_height)
        .min_w(cell_width)
        .w(cell_width * width as f32)
        .px(px(1.0))
        .overflow_hidden()
        .border_b_1()
        .border_color(theme.primary())
        .bg(theme.primary().opacity(0.16))
        .text_color(theme.foreground())
        .child(text.to_owned())
        .into_any_element()
}

fn terminal_cell_colors(
    style: TerminalStyle,
    theme: &Theme,
    reverse_video: bool,
    ansi_palette: &[[u8; 3]; 16],
) -> (gpui::Hsla, gpui::Hsla) {
    let foreground = terminal_hsla(style.foreground, theme, ansi_palette);
    let background = style.background.map_or(theme.background(), |color| {
        terminal_hsla(Some(color), theme, ansi_palette)
    });
    if style.inverse ^ reverse_video {
        (
            style.background.map_or(theme.background(), |color| {
                terminal_hsla(Some(color), theme, ansi_palette)
            }),
            style.foreground.map_or(theme.foreground(), |color| {
                terminal_hsla(Some(color), theme, ansi_palette)
            }),
        )
    } else {
        (foreground, background)
    }
}

fn terminal_color(code: u16, bright: bool) -> Option<TerminalColor> {
    Some(match (code, bright) {
        (0, false) => TerminalColor::Black,
        (1, false) => TerminalColor::Red,
        (2, false) => TerminalColor::Green,
        (3, false) => TerminalColor::Yellow,
        (4, false) => TerminalColor::Blue,
        (5, false) => TerminalColor::Magenta,
        (6, false) => TerminalColor::Cyan,
        (7, false) => TerminalColor::White,
        (0, true) => TerminalColor::BrightBlack,
        (1, true) => TerminalColor::BrightRed,
        (2, true) => TerminalColor::BrightGreen,
        (3, true) => TerminalColor::BrightYellow,
        (4, true) => TerminalColor::BrightBlue,
        (5, true) => TerminalColor::BrightMagenta,
        (6, true) => TerminalColor::BrightCyan,
        (7, true) => TerminalColor::BrightWhite,
        _ => return None,
    })
}

fn terminal_hsla(
    color: Option<TerminalColor>,
    theme: &Theme,
    ansi_palette: &[[u8; 3]; 16],
) -> gpui::Hsla {
    match color {
        Some(TerminalColor::Black) => indexed_hsla(0, ansi_palette),
        Some(TerminalColor::Red) => indexed_hsla(1, ansi_palette),
        Some(TerminalColor::Green) => indexed_hsla(2, ansi_palette),
        Some(TerminalColor::Yellow) => indexed_hsla(3, ansi_palette),
        Some(TerminalColor::Blue) => indexed_hsla(4, ansi_palette),
        Some(TerminalColor::Magenta) => indexed_hsla(5, ansi_palette),
        Some(TerminalColor::Cyan) => indexed_hsla(6, ansi_palette),
        Some(TerminalColor::White) => indexed_hsla(7, ansi_palette),
        Some(TerminalColor::BrightBlack) => indexed_hsla(8, ansi_palette),
        Some(TerminalColor::BrightRed) => indexed_hsla(9, ansi_palette),
        Some(TerminalColor::BrightGreen) => indexed_hsla(10, ansi_palette),
        Some(TerminalColor::BrightYellow) => indexed_hsla(11, ansi_palette),
        Some(TerminalColor::BrightBlue) => indexed_hsla(12, ansi_palette),
        Some(TerminalColor::BrightMagenta) => indexed_hsla(13, ansi_palette),
        Some(TerminalColor::BrightCyan) => indexed_hsla(14, ansi_palette),
        Some(TerminalColor::BrightWhite) => indexed_hsla(15, ansi_palette),
        None => theme.foreground(),
        Some(TerminalColor::Indexed(index)) => indexed_hsla(index, ansi_palette),
        Some(TerminalColor::Rgb(red, green, blue)) => rgb_hsla(red, green, blue),
    }
}

fn indexed_hsla(index: u8, ansi_palette: &[[u8; 3]; 16]) -> gpui::Hsla {
    if let Some([red, green, blue]) = ansi_palette.get(usize::from(index)).copied() {
        return rgb_hsla(red, green, blue);
    }
    if (16..=231).contains(&index) {
        let value = index - 16;
        let red = color_cube_component(value / 36);
        let green = color_cube_component((value / 6) % 6);
        let blue = color_cube_component(value % 6);
        return rgb_hsla(red, green, blue);
    }
    let gray = 8 + (index.saturating_sub(232) * 10);
    rgb_hsla(gray, gray, gray)
}

/// Returns the built-in normal and bright ANSI 16-color palette.
#[must_use]
pub const fn default_ansi_palette() -> [[u8; 3]; 16] {
    let mut palette = [[0; 3]; 16];
    let mut index = 0;
    while index < 16 {
        let (r, g, b) = DEFAULT_ANSI_PALETTE[index];
        palette[index] = [r, g, b];
        index += 1;
    }
    palette
}

fn color_cube_component(value: u8) -> u8 {
    if value == 0 { 0 } else { 55 + value * 40 }
}

fn rgb_hsla(red: u8, green: u8, blue: u8) -> gpui::Hsla {
    gpui::Rgba {
        r: f32::from(red) / 255.0,
        g: f32::from(green) / 255.0,
        b: f32::from(blue) / 255.0,
        a: 1.0,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::{
        Terminal, TerminalCharset, TerminalColor, TerminalCursorStyle, TerminalFontMetrics,
        TerminalGridMetrics, TerminalInputModifiers, TerminalInputSnapshot, TerminalInputState,
        TerminalLifecycleAction, TerminalLifecyclePolicy, TerminalLifecycleSupervisor,
        TerminalLine, TerminalModel, TerminalModes, TerminalMouseButton, TerminalMouseEvent,
        TerminalMouseEventKind, TerminalOptions, TerminalPosition, TerminalProcessStatus,
        TerminalSearchMatch, TerminalSearchOptions, TerminalSelection, TerminalStyle,
        default_ansi_palette, key_event_uses_text_input, normalize_pty_dimension,
        rendered_search_matches, select_terminal_font_family, terminal_alternate_scroll_bytes,
        terminal_focus_event_bytes, terminal_key_down_event_bytes, terminal_keystroke_bytes,
        terminal_keystroke_bytes_with_modes, terminal_mouse_event_bytes, terminal_paint_plan,
        terminal_paste_bytes, terminal_render_rows, terminal_text_input_bytes,
    };
    use gpui::{
        AppContext as _, Bounds, Context, Entity, FocusHandle, KeyDownEvent, Keystroke, Render,
        ScrollDelta, SharedString, TestAppContext, VisualContext as _, Window, point, px, size,
    };
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    #[test]
    fn base_palette_matches_indexed_colors_and_preserves_bright_variants() {
        let theme = super::Theme::dark();
        let options = TerminalOptions::default();
        assert_eq!(
            super::terminal_hsla(Some(TerminalColor::Red), &theme, &options.ansi_palette),
            super::terminal_hsla(
                Some(TerminalColor::Indexed(1)),
                &theme,
                &options.ansi_palette
            )
        );
        assert_ne!(
            super::terminal_hsla(Some(TerminalColor::Red), &theme, &options.ansi_palette),
            super::terminal_hsla(
                Some(TerminalColor::BrightRed),
                &theme,
                &options.ansi_palette
            )
        );
        let options = options.ansi_palette([(1, 2, 3); 16]);
        assert_eq!(
            super::terminal_hsla(
                Some(TerminalColor::BrightWhite),
                &theme,
                &options.ansi_palette
            ),
            super::rgb_hsla(1, 2, 3)
        );
    }

    #[test]
    fn shared_terminal_view_retains_model_allocation() {
        let model = Arc::new(TerminalModel::new(80, 24));
        let terminal = Terminal::from_shared("shared", model.clone());
        assert!(
            matches!(&terminal.model, super::TerminalModelSource::Snapshot(snapshot) if Arc::ptr_eq(snapshot, &model))
        );
    }

    #[test]
    fn output_notifier_can_be_rebound_without_retaining_old_callback() {
        let first = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let second = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let first_sink = first.clone();
        let notifier: super::SharedOutputNotifier =
            Arc::new(std::sync::RwLock::new(Some(Arc::new(move || {
                first_sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }))));
        super::notify_output(&notifier);
        let second_sink = second.clone();
        *notifier.write().expect("not poisoned") = Some(Arc::new(move || {
            second_sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));
        super::notify_output(&notifier);
        assert_eq!(first.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(second.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[cfg(unix)]
    #[test]
    fn structured_spawn_passes_literal_arguments() {
        let mut config = super::PtySpawnConfig::new("/usr/bin/printf");
        config.arguments = vec!["%s".into(), "literal $(not-a-command) ; end".into()];
        let mut session = super::LocalPtySession::spawn(config, 80, 24).expect("spawn printf");
        let mut bytes = Vec::new();
        for _ in 0..100 {
            bytes.extend(session.drain_output());
            if bytes.ends_with(b"; end") {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            String::from_utf8(bytes).expect("UTF-8"),
            "literal $(not-a-command) ; end"
        );
    }

    #[gpui::test]
    async fn background_search_matches_synchronous_results(cx: &mut TestAppContext) {
        let mut model = TerminalModel::new(80, 24);
        for _ in 0..1000 {
            model.write("alpha beta gamma\r\n");
        }
        let model = Arc::new(model);
        let options = super::TerminalSearchOptions {
            whole_word: true,
            ..Default::default()
        };
        let expected = model.search_with_options("beta", options).expect("query");
        let found = model
            .clone()
            .search_async(&cx.background_executor, "beta".into(), options)
            .await
            .expect("query");
        assert_eq!(found, expected);
    }

    struct TerminalFixture<'a> {
        name: &'a str,
        input: &'a str,
        columns: usize,
        rows: usize,
        expected_lines: &'a [&'a str],
        expected_cursor: TerminalPosition,
        expected_title: Option<&'a str>,
    }

    struct TerminalInputHarness {
        focus_handle: FocusHandle,
        input_state: Entity<TerminalInputState>,
        received: Arc<Mutex<Vec<u8>>>,
        focused: bool,
    }

    impl TerminalInputHarness {
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                focus_handle: cx.focus_handle(),
                input_state: cx.new(|_| TerminalInputState::new()),
                received: Arc::new(Mutex::new(Vec::new())),
                focused: false,
            }
        }
    }

    impl Render for TerminalInputHarness {
        fn render(
            &mut self,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            if !self.focused {
                window.focus(&self.focus_handle, cx);
                self.focused = true;
            }
            let received = self.received.clone();
            Terminal::new("input-routing-test", TerminalModel::new(80, 24))
                .focusable(self.focus_handle.clone())
                .input_state(self.input_state.clone())
                .on_input(move |bytes, _, _| {
                    received
                        .lock()
                        .expect("input capture lock should be available")
                        .extend_from_slice(bytes);
                })
        }
    }

    #[test]
    fn terminal_pty_dimensions_stay_within_transport_limits() {
        assert_eq!(normalize_pty_dimension(0), 1);
        assert_eq!(normalize_pty_dimension(80), 80);
        assert_eq!(normalize_pty_dimension(usize::MAX), usize::from(u16::MAX));
    }

    #[test]
    fn terminal_lifecycle_supervisor_escalates_and_resets() {
        let mut supervisor = TerminalLifecycleSupervisor::new(TerminalLifecyclePolicy {
            graceful_timeout: Duration::from_millis(500),
        });
        assert_eq!(
            supervisor.advance(Duration::from_secs(1), TerminalProcessStatus::Running),
            TerminalLifecycleAction::Wait
        );

        supervisor.begin_close();
        assert!(supervisor.is_closing());
        assert_eq!(
            supervisor.advance(Duration::ZERO, TerminalProcessStatus::Running),
            TerminalLifecycleAction::RequestGracefulClose
        );
        assert_eq!(
            supervisor.advance(Duration::from_millis(499), TerminalProcessStatus::Running),
            TerminalLifecycleAction::Wait
        );
        assert_eq!(
            supervisor.advance(Duration::from_millis(1), TerminalProcessStatus::Running),
            TerminalLifecycleAction::ForceClose
        );
        assert_eq!(
            supervisor.advance(
                Duration::ZERO,
                TerminalProcessStatus::Exited(super::TerminalExitStatus { code: 0 })
            ),
            TerminalLifecycleAction::Complete(super::TerminalExitStatus { code: 0 })
        );

        supervisor.reset();
        assert!(!supervisor.is_closing());
    }

    impl TerminalFixture<'_> {
        fn run(&self) {
            let mut model = TerminalModel::new(self.columns, self.rows);
            model.write(self.input);

            let lines = model
                .lines()
                .iter()
                .map(TerminalLine::text)
                .collect::<Vec<_>>();
            assert_eq!(lines, self.expected_lines, "fixture `{}` lines", self.name);
            assert_eq!(
                model.cursor(),
                self.expected_cursor,
                "fixture `{}` cursor",
                self.name
            );
            if let Some(title) = self.expected_title {
                assert_eq!(
                    model.title().as_ref(),
                    title,
                    "fixture `{}` title",
                    self.name
                );
            }
        }
    }

    #[test]
    fn terminal_conformance_fixtures_cover_core_csi_and_osc_sequences() {
        for fixture in [
            TerminalFixture {
                name: "linefeed and carriage return",
                input: "one\r\ntwo",
                columns: 8,
                rows: 2,
                expected_lines: &["one", "two"],
                expected_cursor: TerminalPosition { row: 1, column: 3 },
                expected_title: None,
            },
            TerminalFixture {
                name: "cursor address overwrite",
                input: "abcd\x1b[1;2HZ",
                columns: 8,
                rows: 2,
                expected_lines: &["aZcd", ""],
                expected_cursor: TerminalPosition { row: 0, column: 2 },
                expected_title: None,
            },
            TerminalFixture {
                name: "erase display",
                input: "one\r\ntwo\x1b[2J",
                columns: 8,
                rows: 2,
                expected_lines: &["", ""],
                expected_cursor: TerminalPosition { row: 0, column: 0 },
                expected_title: None,
            },
            TerminalFixture {
                name: "osc title",
                input: "\x1b]0;fixture title\x07ready",
                columns: 16,
                rows: 1,
                expected_lines: &["ready"],
                expected_cursor: TerminalPosition { row: 0, column: 5 },
                expected_title: Some("fixture title"),
            },
        ] {
            fixture.run();
        }
    }

    #[test]
    fn terminal_osc_metadata_preserves_semicolons_and_stays_bounded() {
        let mut model = TerminalModel::new(8, 1);
        model.write("\x1b]0;build;debug\x07");
        assert_eq!(model.title().as_ref(), "build;debug");

        model.write("\x1b]8;;https://example.test/a;b\x07link");
        assert_eq!(
            model.lines()[0].cells()[0]
                .hyperlink
                .as_ref()
                .map(AsRef::as_ref),
            Some("https://example.test/a;b")
        );
        model.write("\x1b]8;;\x07");
        assert_eq!(model.active_hyperlink(), None);

        let oversized_title = "é".repeat(super::MAX_OSC_TITLE_BYTES);
        model.write(&format!("\x1b]0;{oversized_title}\x07"));
        assert!(model.title().len() <= super::MAX_OSC_TITLE_BYTES);
        assert!(std::str::from_utf8(model.title().as_bytes()).is_ok());
    }

    #[test]
    fn terminal_bounds_combining_marks_per_cell() {
        let mut model = TerminalModel::new(2, 1);
        model.write("a");
        for _ in 0..(super::MAX_CELL_GRAPHEME_CHARS * 2) {
            model.write("\u{0301}");
        }

        assert_eq!(
            model.lines()[0].cells()[0].text.chars().count(),
            super::MAX_CELL_GRAPHEME_CHARS
        );
    }

    #[test]
    fn terminal_conformance_fixtures_cover_modes_buffers_and_queries() {
        let mut model = TerminalModel::new(12, 2);
        model.write("main\x1b[?1049halt");
        assert!(model.modes().alternate_screen);
        assert_eq!(model.lines()[0].text(), "alt");

        model.write("\x1b[?1049l");
        assert!(!model.modes().alternate_screen);
        assert_eq!(model.lines()[0].text(), "main");

        model.write("\x1b[?2004h");
        assert!(model.modes().bracketed_paste);
        model.write("\x1b[?2004l");
        assert!(!model.modes().bracketed_paste);

        model.write("\x1b[c");
        assert_eq!(model.take_response_bytes(), b"\x1b[?1;2c".to_vec());
    }

    #[test]
    fn terminal_conformance_fixtures_ignore_unsupported_control_payloads() {
        let mut model = TerminalModel::new(16, 1);
        model.write("\x1bPignored dcs payload\x1b\\ok");

        assert_eq!(model.lines()[0].text(), "ok");
        assert_eq!(model.cursor(), TerminalPosition { row: 0, column: 2 });

        model.write("\x1b[?9999h!");
        assert_eq!(model.lines()[0].text(), "ok!");
        assert_eq!(model.cursor(), TerminalPosition { row: 0, column: 3 });
    }

    #[test]
    fn terminal_conformance_fixtures_cover_wide_chars_and_scroll_regions() {
        let mut wide = TerminalModel::new(8, 1);
        wide.write("a語b");
        assert_eq!(wide.cursor(), TerminalPosition { row: 0, column: 4 });
        assert_eq!(wide.lines()[0].cells()[1].text.as_ref(), "語");
        assert_eq!(wide.lines()[0].cells()[3].text.as_ref(), "b");

        let mut region = TerminalModel::new(4, 5);
        region.write("\x1b[1;1HA\x1b[2;1HB\x1b[3;1HC\x1b[4;1HD\x1b[5;1HE");
        region.write("\x1b[2;4r\x1b[4;1H\n");
        assert_eq!(region.lines()[0].text(), "A");
        assert_eq!(region.lines()[1].text(), "C");
        assert_eq!(region.lines()[2].text(), "D");
        assert_eq!(region.lines()[3].text(), "");
        assert_eq!(region.lines()[4].text(), "E");
    }

    #[test]
    fn terminal_conformance_fixtures_cover_extended_cursor_and_tab_sequences() {
        let mut model = TerminalModel::new(12, 4);
        model.write("A\x1b[2Ebelow\x1b[1Fabove");
        assert_eq!(model.lines()[0].text(), "A");
        assert_eq!(model.lines()[1].text(), "above");
        assert_eq!(model.lines()[2].text(), "below");

        let mut model = TerminalModel::new(16, 2);
        model.write("x\x1b[2Iy\x1b[Zz");
        assert_eq!(model.lines()[0].cells()[0].text.as_ref(), "x");
        assert_eq!(model.lines()[0].cells()[8].text.as_ref(), "z");
        assert_eq!(model.lines()[0].cells()[15].text.as_ref(), "y");

        let mut model = TerminalModel::new(8, 3);
        model.write("\x1b[3;3HX\x1b[1dY\x1b[2aZ\x1b[1eQ");
        assert_eq!(model.lines()[0].cells()[3].text.as_ref(), "Y");
        assert_eq!(model.lines()[0].cells()[6].text.as_ref(), "Z");
        assert_eq!(model.lines()[1].cells()[7].text.as_ref(), "Q");
    }

    #[test]
    fn terminal_conformance_fixtures_cover_scroll_up_down_and_repeat() {
        let mut model = TerminalModel::new(4, 4);
        model.write("A\r\nB\r\nC\r\nD");
        model.write("\x1b[2S");
        assert_eq!(model.lines()[0].text(), "C");
        assert_eq!(model.lines()[1].text(), "D");
        assert_eq!(model.lines()[2].text(), "");
        assert_eq!(model.lines()[3].text(), "");

        model.write("\x1b[1T");
        assert_eq!(model.lines()[0].text(), "");
        assert_eq!(model.lines()[1].text(), "C");
        assert_eq!(model.lines()[2].text(), "D");

        let mut model = TerminalModel::new(8, 1);
        model.write("A\x1b[3b");
        assert_eq!(model.lines()[0].text(), "AAAA");
    }

    #[test]
    fn terminal_resize_reflows_soft_wrapped_lines_wider() {
        let mut model = TerminalModel::new(4, 3);
        model.write("abcdef");

        assert!(model.lines()[0].is_wrapped());
        assert_eq!(model.lines()[0].text(), "abcd");
        assert_eq!(model.lines()[1].text(), "ef");

        model.resize(6, 3);

        assert_eq!(model.lines()[0].text(), "abcdef");
        assert!(!model.lines()[0].is_wrapped());
        assert_eq!(model.lines()[1].text(), "");
    }

    #[test]
    fn terminal_resize_reflows_soft_wrapped_lines_narrower() {
        let mut model = TerminalModel::new(8, 3);
        model.write("abcdef");

        model.resize(3, 4);

        assert_eq!(model.lines()[0].text(), "abc");
        assert!(model.lines()[0].is_wrapped());
        assert_eq!(model.lines()[1].text(), "def");
        assert!(!model.lines()[1].is_wrapped());
    }

    #[test]
    fn terminal_resize_preserves_hard_line_breaks() {
        let mut model = TerminalModel::new(4, 3);
        model.write("abcd\r\nef");

        assert!(!model.lines()[0].is_wrapped());
        model.resize(8, 3);

        assert_eq!(model.lines()[0].text(), "abcd");
        assert_eq!(model.lines()[1].text(), "ef");
    }

    #[test]
    fn terminal_selection_omits_newline_between_soft_wrapped_rows() {
        let mut model = TerminalModel::new(4, 3);
        model.write("abcdef");
        model.set_selection(TerminalSelection::new(
            TerminalPosition { row: 0, column: 0 },
            TerminalPosition { row: 1, column: 1 },
        ));

        assert_eq!(model.selected_text().as_deref(), Some("abcdef"));
    }

    #[test]
    fn terminal_graceful_close_bytes_follow_platform_shell_conventions() {
        if cfg!(target_os = "windows") {
            assert_eq!(
                super::graceful_close_bytes(std::path::Path::new("cmd.exe")),
                b"exit\r\n"
            );
            assert_eq!(
                super::graceful_close_bytes(std::path::Path::new("pwsh.exe")),
                b"exit\r\n"
            );
        } else {
            assert_eq!(
                super::graceful_close_bytes(std::path::Path::new("/bin/zsh")),
                b"\x04"
            );
        }
    }

    #[test]
    fn terminal_writes_plain_text() {
        let mut model = TerminalModel::new(8, 2);
        model.write("hi");

        assert_eq!(model.lines()[0].text(), "hi");
        assert_eq!(model.cursor(), TerminalPosition { row: 0, column: 2 });
    }

    #[test]
    fn terminal_keeps_scrollback() {
        let mut model = TerminalModel::new(8, 2).max_scrollback(3);
        model.write("one\r\ntwo\r\nthree\r\nfour");

        assert_eq!(model.scrollback().len(), 2);
        assert_eq!(model.lines()[0].text(), "three");
    }

    #[test]
    fn terminal_parses_sgr_colors() {
        let mut model = TerminalModel::new(8, 1);
        model.write("\u{1b}[31mR");

        assert_eq!(
            model.lines()[0].cells()[0].style.foreground,
            Some(TerminalColor::Red)
        );
    }

    #[test]
    fn terminal_parses_extended_sgr_colors() {
        let mut model = TerminalModel::new(8, 1);
        model.write("\u{1b}[38;2;10;20;30mR\u{1b}[48;5;196mB");

        assert_eq!(
            model.lines()[0].cells()[0].style.foreground,
            Some(TerminalColor::Rgb(10, 20, 30))
        );
        assert_eq!(
            model.lines()[0].cells()[1].style.background,
            Some(TerminalColor::Indexed(196))
        );
    }

    #[test]
    fn terminal_parses_and_resets_sgr_text_attributes() {
        let mut model = TerminalModel::new(12, 1);
        model.write("\u{1b}[1;2;3;4;5;7;8;9mA");
        model.write("\u{1b}[22;23;24;25;27;28;29mB");

        let active = model.lines()[0].cells()[0].style;
        assert!(active.bold);
        assert!(active.faint);
        assert!(active.italic);
        assert!(active.underline);
        assert!(active.blink);
        assert!(active.inverse);
        assert!(active.hidden);
        assert!(active.strikethrough);
        assert_eq!(model.lines()[0].cells()[1].style, TerminalStyle::default());
    }

    #[test]
    fn terminal_parses_colon_delimited_sgr_colors() {
        let mut model = TerminalModel::new(8, 1);
        model.write("\u{1b}[38:2::10:20:30mR\u{1b}[48:5:196mB");

        assert_eq!(
            model.lines()[0].cells()[0].style.foreground,
            Some(TerminalColor::Rgb(10, 20, 30))
        );
        assert_eq!(
            model.lines()[0].cells()[1].style.background,
            Some(TerminalColor::Indexed(196))
        );
    }

    #[test]
    fn terminal_erase_operations_preserve_the_active_background_color() {
        let mut model = TerminalModel::new(5, 2);
        model.write("\u{1b}[44mA\u{1b}[K");

        for cell in &model.lines()[0].cells()[1..] {
            assert_eq!(cell.style.background, Some(TerminalColor::Blue));
            assert_eq!(cell.text.as_ref(), " ");
        }

        model.write("\u{1b}[2J");
        for line in model.lines() {
            for cell in line.cells() {
                assert_eq!(cell.style.background, Some(TerminalColor::Blue));
            }
        }
    }

    #[test]
    fn terminal_clears_line_remainders() {
        let mut model = TerminalModel::new(16, 1);
        model.write("prompt stale\r\u{1b}[Knext");

        assert_eq!(model.lines()[0].text(), "next");
    }

    #[test]
    fn terminal_tracks_unicode_cell_width() {
        let mut model = TerminalModel::new(8, 1);
        model.write("a語b");

        assert_eq!(model.cursor(), TerminalPosition { row: 0, column: 4 });
        assert_eq!(model.lines()[0].cells()[1].text.as_ref(), "語");
        assert_eq!(model.lines()[0].cells()[3].text.as_ref(), "b");
    }

    #[test]
    fn terminal_combining_marks_extend_the_previous_display_cell() {
        let mut model = TerminalModel::new(8, 1);
        model.write("e\u{301}語\u{20dd}");

        assert_eq!(model.lines()[0].cells()[0].text.as_ref(), "e\u{301}");
        assert_eq!(model.lines()[0].cells()[1].text.as_ref(), "語\u{20dd}");
        assert_eq!(model.cursor(), TerminalPosition { row: 0, column: 3 });
    }

    #[test]
    fn terminal_keeps_emoji_grapheme_clusters_in_one_display_cell() {
        let mut model = TerminalModel::new(12, 1);
        model.write("👩‍💻x");
        assert_eq!(model.lines()[0].cells()[0].text.as_ref(), "👩‍💻");
        assert_eq!(model.lines()[0].cells()[2].text.as_ref(), "x");
        assert_eq!(model.cursor(), TerminalPosition { row: 0, column: 3 });

        let mut variation = TerminalModel::new(12, 1);
        variation.write("✈️x");
        assert_eq!(variation.lines()[0].cells()[0].text.as_ref(), "✈️");
    }

    #[test]
    fn terminal_supports_wide_ambiguous_character_policy() {
        let mut narrow = TerminalModel::new(8, 1);
        narrow.write("·x");
        assert_eq!(narrow.cursor(), TerminalPosition { row: 0, column: 2 });

        let mut wide = TerminalModel::new(8, 1).ambiguous_width(true);
        wide.write("·x");
        assert_eq!(wide.cursor(), TerminalPosition { row: 0, column: 3 });
    }

    #[test]
    fn terminal_full_reset_clears_screen_history_and_modes() {
        let mut model = TerminalModel::new(8, 2);
        model.write("one\r\ntwo\r\nthree");
        model.write("\u{1b}[31;1m\u{1b}[?1h\u{1b}]0;changed\u{7}");
        assert!(!model.scrollback().is_empty());

        model.write("\u{1b}c");

        assert!(model.lines().iter().all(|line| line.text().is_empty()));
        assert!(model.scrollback().is_empty());
        assert_eq!(model.cursor(), TerminalPosition::default());
        assert_eq!(model.title().as_ref(), "");
        assert_eq!(model.modes(), TerminalModes::default());
    }

    #[test]
    fn terminal_soft_reset_preserves_screen_and_alternate_buffer() {
        let mut model = TerminalModel::new(8, 2);
        model.write("keep\u{1b}[31;1m\u{1b}[?1049hALT\u{1b}[?1h\u{1b}[2;2H");

        model.write("\u{1b}[!p");

        assert_eq!(model.lines()[0].text(), "ALT");
        assert!(model.modes().alternate_screen);
        assert!(!model.modes().application_cursor_keys);
        assert_eq!(model.cursor(), TerminalPosition::default());
        model.write("N");
        assert_eq!(model.lines()[0].cells()[0].style, TerminalStyle::default());
    }

    #[test]
    fn terminal_tracks_osc_title() {
        let mut model = TerminalModel::new(8, 1);
        model.write("\u{1b}]0;workspace\u{7}");

        assert_eq!(model.title().as_ref(), "workspace");
    }

    #[test]
    fn terminal_tracks_only_local_osc_7_file_paths() {
        let mut model = TerminalModel::new(20, 2);
        model.write("\x1b]7;file://localhost/Users/test/My%20Project\x07");
        assert_eq!(
            model.current_directory().map(AsRef::as_ref),
            Some("/Users/test/My Project")
        );

        model.write("\x1b]7;file://remote.example/tmp\x07");
        assert_eq!(model.current_directory(), None);
        model.write("\x1b]7;https://example.com/tmp\x07");
        assert_eq!(model.current_directory(), None);
        model.write("\x1b]7;file:///tmp/%00secret\x07");
        assert_eq!(model.current_directory(), None);
    }

    #[test]
    fn terminal_tracks_bell_events_without_rendering_control_text() {
        let mut model = TerminalModel::new(8, 2);
        model.write("one\x07two\x07");
        assert_eq!(model.bell_sequence(), 2);
        assert_eq!(model.lines()[0].text(), "onetwo");
    }

    #[test]
    fn terminal_queues_device_attribute_responses() {
        let mut model = TerminalModel::new(8, 1);

        model.write("\u{1b}[c");
        assert_eq!(model.take_response_bytes(), b"\x1b[?1;2c".to_vec());
        assert_eq!(model.take_response_bytes(), Vec::<u8>::new());

        model.write("\u{1b}Z");
        assert_eq!(model.take_response_bytes(), b"\x1b[?1;2c".to_vec());

        model.write("\u{1b}[>c");
        assert_eq!(model.take_response_bytes(), b"\x1b[>0;1;0c".to_vec());
    }

    #[test]
    fn terminal_queues_status_and_cursor_report_responses() {
        let mut model = TerminalModel::new(8, 4);

        model.write("\u{1b}[5n");
        assert_eq!(model.take_response_bytes(), b"\x1b[0n".to_vec());

        model.write("\u{1b}[2;3H\u{1b}[6n");
        assert_eq!(model.take_response_bytes(), b"\x1b[2;3R".to_vec());

        model.write("\u{1b}[?6n");
        assert_eq!(model.take_response_bytes(), b"\x1b[?2;3R".to_vec());
    }

    #[test]
    fn terminal_queues_dec_private_status_reports() {
        let mut model = TerminalModel::new(8, 1);

        model.write("\u{1b}[?15n\u{1b}[?25n\u{1b}[?26n");
        assert_eq!(
            model.take_response_bytes(),
            b"\x1b[?13n\x1b[?20n\x1b[?27;1n".to_vec()
        );
    }

    #[test]
    fn terminal_reports_dec_private_mode_state() {
        let mut model = TerminalModel::new(8, 1);
        model.write("\u{1b}[?25l\u{1b}[?25;2004;9999$p");

        assert_eq!(
            model.take_response_bytes(),
            b"\x1b[?25;2$y\x1b[?2004;2$y\x1b[?9999;0$y".to_vec()
        );

        model.write("\u{1b}[?2004h\u{1b}[?2004$p");
        assert_eq!(model.take_response_bytes(), b"\x1b[?2004;1$y".to_vec());
    }

    #[test]
    fn terminal_tracks_osc8_hyperlinks_on_cells() {
        let mut model = TerminalModel::new(12, 1);
        model.write("\u{1b}]8;;https://example.test\u{7}link\u{1b}]8;;\u{7} plain");

        assert_eq!(
            model.lines()[0].cells()[0].hyperlink.as_deref(),
            Some("https://example.test")
        );
        assert_eq!(
            model.lines()[0].cells()[3].hyperlink.as_deref(),
            Some("https://example.test")
        );
        assert_eq!(model.lines()[0].cells()[5].hyperlink, None);
        assert_eq!(model.active_hyperlink(), None);
        assert_eq!(
            model
                .hyperlink_at(TerminalPosition { row: 0, column: 2 }, 0)
                .map(AsRef::as_ref),
            Some("https://example.test")
        );
        assert_eq!(
            model.hyperlink_at(TerminalPosition { row: 0, column: 8 }, 0),
            None
        );
    }

    #[test]
    fn terminal_conservatively_detects_plain_urls_and_file_paths() {
        let mut model = TerminalModel::new(80, 2);
        model.write("See (https://example.com/docs), and ./src/main.rs");

        assert_eq!(
            model
                .link_at(TerminalPosition { row: 0, column: 8 }, 0)
                .as_deref(),
            Some("https://example.com/docs")
        );
        assert_eq!(
            model
                .link_at(TerminalPosition { row: 0, column: 42 }, 0)
                .as_deref(),
            Some("./src/main.rs")
        );
        assert_eq!(
            model.link_at(TerminalPosition { row: 0, column: 0 }, 0),
            None
        );
    }

    #[test]
    fn terminal_supports_dec_special_graphics_charset() {
        let mut model = TerminalModel::new(12, 1);
        model.write("\u{1b}(0lqk\u{1b}(Bq");

        assert_eq!(model.charset(), TerminalCharset::Ascii);
        assert_eq!(model.lines()[0].text(), "┌─┐q");
    }

    #[test]
    fn terminal_supports_configurable_tab_stops() {
        let mut model = TerminalModel::new(16, 1);
        model.write("a\tb");

        assert_eq!(model.lines()[0].cells()[0].text.as_ref(), "a");
        assert_eq!(model.lines()[0].cells()[8].text.as_ref(), "b");

        let mut model = TerminalModel::new(16, 1);
        model.write("\u{1b}[3gabc\u{1b}H\rx\ty");

        assert_eq!(model.lines()[0].cells()[0].text.as_ref(), "x");
        assert_eq!(model.lines()[0].cells()[3].text.as_ref(), "y");

        let mut model = TerminalModel::new(16, 1);
        model.write("\u{1b}[3gabc\u{1b}H\u{1b}[g\rx\ty");

        assert_eq!(model.lines()[0].cells()[0].text.as_ref(), "x");
        assert_eq!(model.lines()[0].cells()[15].text.as_ref(), "y");
    }

    #[test]
    fn terminal_supports_alternate_screen_and_private_modes() {
        let mut model = TerminalModel::new(8, 2);
        model.write("main\u{1b}[31;3m");
        model.write("\u{1b}[?1049h\u{1b}[32malt");

        assert!(model.modes().alternate_screen);
        assert_eq!(model.lines()[0].text(), "alt");

        model.write("\u{1b}[?25l\u{1b}[?2004h");
        assert!(!model.modes().cursor_visible);
        assert!(model.modes().bracketed_paste);

        model.write("\u{1b}[?1049l");
        assert!(!model.modes().alternate_screen);
        assert_eq!(model.lines()[0].text(), "main");
        model.write("X");
        assert_eq!(
            model.lines()[0].cells()[4].style.foreground,
            Some(TerminalColor::Red)
        );
        assert!(model.lines()[0].cells()[4].style.italic);
    }

    #[test]
    fn terminal_saved_cursor_restores_rendition_and_charset() {
        let mut model = TerminalModel::new(8, 2);
        model.write("\u{1b}[2;3H\u{1b}[31;3m\u{1b}(0\u{1b}7");
        model.write("\u{1b}[1;1H\u{1b}[0m\u{1b}(B\u{1b}8x");

        let cell = &model.lines()[1].cells()[2];
        assert_eq!(cell.text.as_ref(), "│");
        assert_eq!(cell.style.foreground, Some(TerminalColor::Red));
        assert!(cell.style.italic);
    }

    #[test]
    fn terminal_tracks_expanded_dec_private_modes() {
        let mut model = TerminalModel::new(8, 2);
        model.write("\u{1b}[?3;5;8;9;12;45;69;1004;1005;1007;1015;1039;2026h");

        assert!(model.modes().column_mode_132);
        assert!(model.modes().reverse_video);
        assert!(model.modes().auto_repeat);
        assert!(model.modes().x10_mouse_reporting);
        assert!(model.modes().cursor_blink);
        assert!(model.modes().reverse_wraparound);
        assert!(model.modes().left_right_margin_mode);
        assert!(model.modes().focus_event_reporting);
        assert!(model.modes().utf8_mouse);
        assert!(model.modes().alternate_scroll);
        assert!(model.modes().urxvt_mouse);
        assert!(model.modes().meta_sends_escape);
        assert!(model.modes().synchronized_output);

        model.write("\u{1b}[?5;9;12;45;69;1004;1005;1015;1039;2026l");

        assert!(!model.modes().reverse_video);
        assert!(!model.modes().x10_mouse_reporting);
        assert!(!model.modes().cursor_blink);
        assert!(!model.modes().reverse_wraparound);
        assert!(!model.modes().left_right_margin_mode);
        assert!(!model.modes().focus_event_reporting);
        assert!(!model.modes().utf8_mouse);
        assert!(!model.modes().urxvt_mouse);
        assert!(!model.modes().meta_sends_escape);
        assert!(!model.modes().synchronized_output);
    }

    #[test]
    fn terminal_saves_and_restores_dec_private_modes() {
        let mut model = TerminalModel::new(8, 1);
        model.write("\u{1b}[?7l\u{1b}[?7s\u{1b}[?7h");
        assert!(model.modes().auto_wrap);

        model.write("\u{1b}[?7r");
        assert!(!model.modes().auto_wrap);
    }

    #[test]
    fn terminal_scroll_region_scrolls_only_region() {
        let mut model = TerminalModel::new(8, 5);
        model.write("\u{1b}[1;1HA\u{1b}[2;1HB\u{1b}[3;1HC\u{1b}[4;1HD\u{1b}[5;1HE");
        model.write("\u{1b}[2;4r\u{1b}[4;1H\n");

        assert_eq!(model.lines()[0].text(), "A");
        assert_eq!(model.lines()[1].text(), "C");
        assert_eq!(model.lines()[2].text(), "D");
        assert_eq!(model.lines()[3].text(), "");
        assert_eq!(model.lines()[4].text(), "E");
    }

    #[test]
    fn terminal_origin_mode_positions_inside_scroll_region() {
        let mut model = TerminalModel::new(8, 5);
        model.write("\u{1b}[2;4r\u{1b}[?6h\u{1b}[1;1HX");

        assert_eq!(model.lines()[0].text(), "");
        assert_eq!(model.lines()[1].text(), "X");
    }

    #[test]
    fn terminal_auto_wrap_mode_can_be_disabled() {
        let mut model = TerminalModel::new(3, 2);
        model.write("\u{1b}[?7lABCD");

        assert_eq!(model.lines()[0].text(), "ABD");
        assert_eq!(model.lines()[1].text(), "");
    }

    #[test]
    fn terminal_tracks_cursor_style() {
        let mut model = TerminalModel::new(8, 1);

        model.write("\u{1b}[6 q");
        assert_eq!(model.modes().cursor_style, TerminalCursorStyle::Bar);

        model.write("\u{1b}[4 q");
        assert_eq!(model.modes().cursor_style, TerminalCursorStyle::Underline);

        model.write("\u{1b}[2 q");
        assert_eq!(model.modes().cursor_style, TerminalCursorStyle::Block);
    }

    #[test]
    fn terminal_selection_extracts_live_grid_text() {
        let mut model = TerminalModel::new(12, 2);
        model.write("alpha\r\nbeta");
        model.set_selection(TerminalSelection::new(
            TerminalPosition { row: 0, column: 1 },
            TerminalPosition { row: 1, column: 2 },
        ));

        assert_eq!(model.selected_text().as_deref(), Some("lpha\nbet"));

        model.clear_selection();
        assert_eq!(model.selected_text(), None);
    }

    #[test]
    fn terminal_selection_tracks_text_across_reflow() {
        let mut model = TerminalModel::new(12, 2);
        model.write("alpha beta");
        model.set_selection(TerminalSelection::new(
            TerminalPosition { row: 0, column: 2 },
            TerminalPosition { row: 0, column: 8 },
        ));

        model.resize(6, 2);

        assert_eq!(model.selected_text().as_deref(), Some("pha bet"));
        assert_eq!(
            model.selection(),
            Some(TerminalSelection::new(
                TerminalPosition { row: 0, column: 2 },
                TerminalPosition { row: 1, column: 2 },
            ))
        );
    }

    #[test]
    fn terminal_multi_click_selection_expands_to_word_and_line() {
        let mut model = TerminalModel::new(16, 2);
        model.write("alpha beta");
        let word = model.selection_for_click(TerminalPosition { row: 0, column: 7 }, 2, 0);
        assert_eq!(
            word.bounds(),
            (
                TerminalPosition { row: 0, column: 6 },
                TerminalPosition { row: 0, column: 9 },
            )
        );
        let line = model.selection_for_click(TerminalPosition { row: 0, column: 7 }, 3, 0);
        assert_eq!(line.bounds().0.column, 0);
        assert_eq!(line.bounds().1.column, 15);
    }

    #[test]
    fn terminal_render_rows_follow_scrollback_viewport() {
        let mut model = TerminalModel::new(8, 2).max_scrollback(8);
        model.write("one\r\ntwo\r\nthree");
        model.scroll_up(1);
        let rows = terminal_render_rows(&model, &TerminalOptions::default());
        assert_eq!(rows.len(), 2);
        assert!(rows[0].line.text().contains("one"));
        assert!(rows[1].line.text().contains("two"));
    }

    #[test]
    fn terminal_search_finds_and_reveals_scrollback_matches() {
        let mut model = TerminalModel::new(16, 2).max_scrollback(16);
        model.write("first needle\r\nsecond line\r\nthird NEEDLE\r\n");

        let matches = model.search("needle", false);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].start_column, 6);
        assert!(model.reveal_search_match(matches[0]));
        assert!(model.viewport_offset() > 0);
        assert_eq!(model.selected_text().as_deref(), Some("needle"));
        assert!(model.search("NEEDLE", true).len() == 1);
        assert!(model.search_from("needle", false, matches[1].row).len() == 1);

        let mut bounded = TerminalModel::new(8, 2).max_scrollback(2);
        bounded.write("one\r\ntwo\r\nthree\r\nfour\r\nfive");
        assert!(bounded.history_origin() > 0);
        assert_eq!(bounded.history_len(), 4);
    }

    #[test]
    fn terminal_search_supports_whole_word_case_and_bounded_regex() {
        let mut model = TerminalModel::new(32, 2);
        model.write("cat concatenate CAT 123\r\nerror-42 error-7");

        let whole_words = model
            .search_with_options(
                "cat",
                TerminalSearchOptions {
                    whole_word: true,
                    ..TerminalSearchOptions::default()
                },
            )
            .expect("literal search should compile");
        assert_eq!(whole_words.len(), 2);

        let exact_case = model
            .search_with_options(
                "cat",
                TerminalSearchOptions {
                    case_sensitive: true,
                    whole_word: true,
                    regex: false,
                    ..TerminalSearchOptions::default()
                },
            )
            .expect("literal search should compile");
        assert_eq!(exact_case.len(), 1);

        let regex = model
            .search_with_options(
                r"error-\d+",
                TerminalSearchOptions {
                    regex: true,
                    ..TerminalSearchOptions::default()
                },
            )
            .expect("regular expression should compile");
        assert_eq!(regex.len(), 2);
        assert!(
            model
                .search_with_options(
                    "(",
                    TerminalSearchOptions {
                        regex: true,
                        ..TerminalSearchOptions::default()
                    },
                )
                .is_err()
        );
    }

    #[test]
    fn visible_search_highlights_map_history_rows_to_the_viewport() {
        let mut model = TerminalModel::new(8, 2).max_scrollback(8);
        model.write("one\r\ntwo\r\nthree");
        let matches = vec![
            TerminalSearchMatch {
                row: 0,
                start_column: 0,
                end_column: 2,
            },
            TerminalSearchMatch {
                row: 1,
                start_column: 0,
                end_column: 2,
            },
            TerminalSearchMatch {
                row: 2,
                start_column: 0,
                end_column: 4,
            },
        ];

        let visible =
            rendered_search_matches(&model, &TerminalOptions::default(), &matches, Some(2));

        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].range.row, 0);
        assert_eq!(visible[1].range.row, 1);
        assert!(!visible[0].active);
        assert!(visible[1].active);
    }

    #[test]
    fn terminal_search_can_be_restricted_to_the_active_selection() {
        let mut model = TerminalModel::new(32, 2);
        model.write("cat outside cat\r\ncat second");
        model.set_selection(TerminalSelection::new(
            TerminalPosition { row: 0, column: 0 },
            TerminalPosition { row: 0, column: 2 },
        ));

        let matches = model
            .search_with_options(
                "cat",
                TerminalSearchOptions {
                    selection_only: true,
                    ..TerminalSearchOptions::default()
                },
            )
            .expect("selection search should compile");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].row, 0);
        assert_eq!(matches[0].start_column, 0);
    }

    #[test]
    fn osc_133_tracks_bounded_command_output_and_search_filtering() {
        let mut model = TerminalModel::new(40, 3);
        model.write("prompt\r\n\x1b]133;C\x07wanted output\x1b]133;D;7\x07 outside");

        assert_eq!(model.command_outputs().len(), 1);
        let output = model.command_outputs()[0];
        assert_eq!(output.exit_status, Some(7));
        assert!(output.end_column >= output.start_column);
        assert_eq!(
            model
                .search_with_options(
                    "wanted",
                    TerminalSearchOptions {
                        command_output_only: true,
                        ..TerminalSearchOptions::default()
                    },
                )
                .expect("command-output search should compile")
                .len(),
            1
        );
        assert!(
            model
                .search_with_options(
                    "outside",
                    TerminalSearchOptions {
                        command_output_only: true,
                        ..TerminalSearchOptions::default()
                    },
                )
                .expect("command-output search should compile")
                .is_empty()
        );
        assert!(model.reveal_command_output(0));
        assert!(
            model
                .selected_text()
                .is_some_and(|text| text.contains("wanted output"))
        );
    }

    #[test]
    fn terminal_notifications_are_bounded_and_sequence_changes() {
        let mut model = TerminalModel::new(20, 2);
        model.write("\x1b]777;notify;Build;Complete\x07");
        let (sequence, notification) = model
            .latest_notification()
            .expect("notification should be recorded");
        assert_eq!(sequence, 1);
        assert_eq!(notification.title.as_ref(), "Build");
        assert_eq!(notification.body.as_ref(), "Complete");

        model.write("\x1b]9;Second\x07");
        let (sequence, notification) = model
            .latest_notification()
            .expect("second notification should be recorded");
        assert_eq!(sequence, 2);
        assert_eq!(notification.body.as_ref(), "Second");
    }

    #[test]
    fn terminal_selection_extracts_scrollback_viewport_text() {
        let mut model = TerminalModel::new(12, 2);
        model.write("one\r\ntwo\r\nthree\r\nfour");
        model.scroll_up(2);
        model.set_selection(TerminalSelection::new(
            TerminalPosition { row: 0, column: 0 },
            TerminalPosition { row: 1, column: 2 },
        ));

        assert_eq!(model.selected_text().as_deref(), Some("one\ntwo"));
    }

    #[test]
    fn terminal_selection_extracts_visible_scrollback_text() {
        let mut model = TerminalModel::new(12, 2);
        model.write("one\r\ntwo\r\nthree");
        model.set_selection(TerminalSelection::new(
            TerminalPosition { row: 0, column: 0 },
            TerminalPosition { row: 1, column: 2 },
        ));

        assert_eq!(
            model.selected_text_with_visible_scrollback(1).as_deref(),
            Some("one\ntwo")
        );
    }

    #[test]
    fn terminal_grid_metrics_maps_points_to_clamped_cells() {
        let metrics = super::TerminalGridMetrics {
            bounds: Bounds::new(point(px(10.0), px(20.0)), size(px(80.0), px(36.0))),
            cell_width: px(8.0),
            line_height: px(18.0),
            columns: 10,
            rows: 2,
        };

        assert_eq!(
            metrics.position_for_point(point(px(34.0), px(38.0))),
            TerminalPosition { row: 1, column: 3 }
        );
        assert_eq!(
            metrics.position_for_point(point(px(-20.0), px(-20.0))),
            TerminalPosition { row: 0, column: 0 }
        );
        assert_eq!(
            metrics.position_for_point(point(px(500.0), px(500.0))),
            TerminalPosition { row: 1, column: 9 }
        );
        assert_eq!(
            metrics.bounds_for_position(TerminalPosition { row: 1, column: 3 }),
            Bounds::new(point(px(34.0), px(38.0)), size(px(8.0), px(18.0)))
        );
        assert_eq!(
            metrics.bounds_for_position(TerminalPosition {
                row: 100,
                column: 100
            }),
            Bounds::new(point(px(82.0), px(38.0)), size(px(8.0), px(18.0)))
        );
    }

    #[test]
    fn terminal_grid_size_uses_rendered_bounds_and_cell_metrics() {
        let metrics = TerminalFontMetrics {
            cell_width: px(8.0),
            line_height: px(18.0),
        };

        assert_eq!(
            super::terminal_grid_size_for_bounds(
                Bounds::new(point(px(0.0), px(0.0)), size(px(81.0), px(37.0))),
                metrics,
            ),
            super::TerminalGridSize {
                columns: 10,
                rows: 2
            }
        );
        assert_eq!(
            super::terminal_grid_size_for_bounds(
                Bounds::new(point(px(0.0), px(0.0)), size(px(0.0), px(0.0))),
                metrics,
            ),
            super::TerminalGridSize {
                columns: 1,
                rows: 1
            }
        );
    }

    #[test]
    fn terminal_scroll_delta_normalizes_to_lines() {
        assert_eq!(
            super::terminal_scroll_line_delta(ScrollDelta::Lines(point(0.0, 3.0)), px(18.0)),
            3
        );
        assert_eq!(
            super::terminal_scroll_line_delta(
                ScrollDelta::Pixels(point(px(0.0), px(-36.0))),
                px(18.0)
            ),
            -2
        );
        assert_eq!(
            super::terminal_scroll_line_delta(
                ScrollDelta::Pixels(point(px(0.0), px(3.0))),
                px(18.0)
            ),
            1
        );
    }

    #[test]
    fn terminal_paste_respects_bracketed_mode() {
        assert_eq!(
            terminal_paste_bytes("echo ok", TerminalModes::default()),
            b"echo ok".to_vec()
        );

        let modes = TerminalModes {
            bracketed_paste: true,
            ..TerminalModes::default()
        };
        assert_eq!(
            terminal_paste_bytes("echo ok", modes),
            b"\x1b[200~echo ok\x1b[201~".to_vec()
        );
        assert_eq!(
            terminal_paste_bytes("one\r\ntwo\rthree", TerminalModes::default()),
            b"one\ntwo\nthree".to_vec()
        );
        assert_eq!(
            terminal_paste_bytes("safe\0\x1b[201~tail\u{7f}", modes),
            b"\x1b[200~safe[201~tail\x1b[201~".to_vec()
        );

        let oversized = "é".repeat(super::MAX_TERMINAL_PASTE_BYTES);
        let bounded = terminal_paste_bytes(&oversized, TerminalModes::default());
        assert!(bounded.len() <= super::MAX_TERMINAL_PASTE_BYTES);
        assert!(std::str::from_utf8(&bounded).is_ok());
    }

    #[test]
    fn terminal_large_paste_and_scrollback_stay_bounded() {
        let mut model = TerminalModel::new(16, 4).max_scrollback(32);
        let payload = (0..200)
            .map(|index| format!("line-{index:03}\r\n"))
            .collect::<String>();

        model.write(&payload);

        assert_eq!(model.scrollback().len(), 32);
        assert_eq!(model.max_viewport_offset(), 32);
        model.scroll_up(500);
        assert_eq!(model.viewport_offset(), 32);
        model.scroll_down(500);
        assert_eq!(model.viewport_offset(), 0);
    }

    #[test]
    fn terminal_sustains_high_rate_output_with_bounded_history() {
        let mut model = TerminalModel::new(120, 40).max_scrollback(2_000);
        let payload = (0..20_000)
            .map(|index| format!("event={index:05} level=info message=terminal-stress\r\n"))
            .collect::<String>();

        model.write(&payload);

        assert_eq!(model.scrollback().len(), 2_000);
        assert_eq!(model.lines().len(), 40);
        assert!(
            model
                .lines()
                .last()
                .is_some_and(|line| line.text().is_empty())
        );
        assert!(model.estimated_heap_bytes() > 0);
        assert!(model.estimated_heap_bytes() < 64 * 1024 * 1024);
    }

    #[test]
    fn terminal_many_pane_models_keep_state_isolated() {
        let mut panes = (0..64)
            .map(|_| TerminalModel::new(80, 24).max_scrollback(128))
            .collect::<Vec<_>>();

        for (index, pane) in panes.iter_mut().enumerate() {
            pane.write(&format!("pane-{index}\r\n{}", "x".repeat(8_192)));
        }

        for (index, pane) in panes.iter().enumerate() {
            assert!(pane.scrollback().len() <= 128);
            assert!(pane.lines().iter().any(|line| line.text().contains('x')));
            if index != 0 {
                assert!(
                    !pane
                        .lines()
                        .iter()
                        .any(|line| line.text().contains("pane-0"))
                );
            }
        }
    }

    #[test]
    fn terminal_malformed_and_truncated_sequences_do_not_corrupt_following_text() {
        let mut model = TerminalModel::new(32, 3);
        model.write("before\x1b[999999999999999999999999;31mignored\x1b[?9999h");
        model.write("\x1b]8;;truncated");
        model.write("\x1b\\after");

        let rendered = model
            .lines()
            .iter()
            .map(TerminalLine::text)
            .collect::<String>();
        assert!(rendered.contains("before"));
        assert!(rendered.contains("after"));
    }

    #[test]
    fn terminal_mouse_reporting_encodes_sgr_and_legacy_events() {
        let mut modes = TerminalModes {
            mouse_button_reporting: true,
            sgr_mouse: true,
            ..TerminalModes::default()
        };
        let event = TerminalMouseEvent {
            row: 2,
            column: 4,
            button: TerminalMouseButton::Left,
            kind: TerminalMouseEventKind::Press,
            modifiers: TerminalInputModifiers {
                shift: true,
                alt: false,
                control: true,
            },
        };

        assert_eq!(
            terminal_mouse_event_bytes(event, modes),
            Some(b"\x1b[<20;5;3M".to_vec())
        );

        modes.sgr_mouse = false;
        assert_eq!(
            terminal_mouse_event_bytes(event, modes),
            Some(vec![0x1b, b'[', b'M', 52, 37, 35])
        );

        modes.urxvt_mouse = true;
        assert_eq!(
            terminal_mouse_event_bytes(event, modes),
            Some(b"\x1b[20;5;3M".to_vec())
        );

        modes.urxvt_mouse = false;
        modes.mouse_button_reporting = false;
        modes.x10_mouse_reporting = true;
        assert_eq!(
            terminal_mouse_event_bytes(event, modes),
            Some(vec![0x1b, b'[', b'M', 52, 37, 35])
        );
        assert_eq!(
            terminal_mouse_event_bytes(
                TerminalMouseEvent {
                    kind: TerminalMouseEventKind::Release,
                    ..event
                },
                modes
            ),
            None
        );
    }

    #[test]
    fn terminal_focus_and_alternate_scroll_helpers_follow_modes() {
        let mut modes = TerminalModes::default();
        assert_eq!(terminal_focus_event_bytes(true, modes), None);

        modes.focus_event_reporting = true;
        assert_eq!(
            terminal_focus_event_bytes(true, modes),
            Some(b"\x1b[I".to_vec())
        );
        assert_eq!(
            terminal_focus_event_bytes(false, modes),
            Some(b"\x1b[O".to_vec())
        );

        assert_eq!(terminal_alternate_scroll_bytes(1, modes), None);
        modes.alternate_screen = true;
        assert_eq!(
            terminal_alternate_scroll_bytes(1, modes),
            Some(b"\x1b[A".to_vec())
        );
        modes.application_cursor_keys = true;
        assert_eq!(
            terminal_alternate_scroll_bytes(-1, modes),
            Some(b"\x1bOB".to_vec())
        );
    }

    #[test]
    fn terminal_key_down_event_respects_prefer_character_input() {
        let event = KeyDownEvent {
            keystroke: parsed_keystroke("ctrl-alt-e->€"),
            is_held: false,
            prefer_character_input: true,
        };

        assert_eq!(
            terminal_key_down_event_bytes(&event, TerminalModes::default()),
            None
        );

        let event = KeyDownEvent {
            keystroke: parsed_keystroke("alt-e->é"),
            is_held: false,
            prefer_character_input: true,
        };
        assert_eq!(
            terminal_key_down_event_bytes(&event, TerminalModes::default()),
            Some("é".as_bytes().to_vec())
        );
        assert_eq!(
            terminal_text_input_bytes("日本語", TerminalModes::default()),
            "日本語".as_bytes().to_vec()
        );
    }

    #[test]
    fn terminal_printable_keys_use_only_the_text_input_path() {
        let printable = KeyDownEvent {
            keystroke: parsed_keystroke("l"),
            is_held: false,
            prefer_character_input: false,
        };
        assert!(key_event_uses_text_input(&printable));

        let shifted = KeyDownEvent {
            keystroke: parsed_keystroke("shift-l->L"),
            is_held: false,
            prefer_character_input: false,
        };
        assert!(key_event_uses_text_input(&shifted));

        for source in ["enter", "backspace", "ctrl-c", "alt-l"] {
            let event = KeyDownEvent {
                keystroke: parsed_keystroke(source),
                is_held: false,
                prefer_character_input: false,
            };
            assert!(!key_event_uses_text_input(&event), "{source}");
        }
    }

    #[gpui::test]
    fn terminal_printable_key_is_delivered_once(cx: &mut TestAppContext) {
        cx.update(guic_tokens::init);
        let (view, cx) = cx.add_window_view(|_, cx| TerminalInputHarness::new(cx));
        let window = cx.window_handle();
        cx.dispatch_keystroke(
            window,
            Keystroke::parse("l").expect("keystroke should parse"),
        );

        view.update(cx, |view, _| {
            assert_eq!(
                view.received
                    .lock()
                    .expect("input capture lock should be available")
                    .as_slice(),
                b"l"
            );
        });
    }

    #[gpui::test]
    fn terminal_gpu_plan_batches_dense_rows(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_tokens::init(cx);
            let theme = super::Theme::global(cx).clone();
            let mut model = TerminalModel::new(120, 30);
            model.write(&"x".repeat(120 * 30));
            let rows = terminal_render_rows(&model, &TerminalOptions::default());
            let plan = terminal_paint_plan(
                &rows,
                &theme,
                false,
                false,
                true,
                &SharedString::from("Menlo"),
                400,
                false,
                &default_ansi_palette(),
                None,
                &[],
            );

            assert_eq!(rows.len(), 30);
            assert!(
                plan.text.len() <= rows.len() + 1,
                "dense terminal text should batch by row, not by cell"
            );
            assert!(
                plan.backgrounds.len() <= 1,
                "default backgrounds should not create per-cell quads"
            );
        });
    }

    #[gpui::test]
    fn hovered_osc8_link_has_visible_paint_feedback(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_tokens::init(cx);
            let theme = super::Theme::global(cx).clone();
            let mut model = TerminalModel::new(16, 1);
            model.write("\u{1b}]8;;https://example.com\u{7}link\u{1b}]8;;\u{7}");
            let rows = terminal_render_rows(&model, &TerminalOptions::default());
            let plan = terminal_paint_plan(
                &rows,
                &theme,
                false,
                false,
                true,
                &SharedString::from("Menlo"),
                400,
                false,
                &default_ansi_palette(),
                Some("https://example.com"),
                &[],
            );

            assert!(
                plan.backgrounds
                    .iter()
                    .any(|background| background.cells == 4)
            );
            assert!(
                plan.text
                    .iter()
                    .all(|batch| batch.style.underline.is_some())
            );
        });
    }

    #[test]
    fn terminal_options_enable_font_measurement() {
        let options = TerminalOptions::default()
            .font_family("JetBrains Mono")
            .font_size(14)
            .font_weight(600)
            .line_height_percent(150)
            .ligatures(true)
            .measured_font();

        assert_eq!(options.font_family.as_deref(), Some("JetBrains Mono"));
        assert_eq!(options.font_size, 14);
        assert_eq!(options.font_weight, 600);
        assert_eq!(options.line_height_percent, 150);
        assert!(options.ligatures);
        assert!(options.measure_font);
    }

    #[test]
    fn terminal_font_selection_preserves_available_preference() {
        let available = vec!["Menlo".to_owned(), "JetBrains Mono".to_owned()];

        assert_eq!(
            select_terminal_font_family("jetbrains mono", &available),
            SharedString::from("JetBrains Mono")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn terminal_font_selection_uses_installed_platform_monospace_fallback() {
        let available = vec!["Helvetica".to_owned(), "Menlo".to_owned()];

        assert_eq!(
            select_terminal_font_family("Missing Mono", &available),
            SharedString::from("Menlo")
        );
    }

    #[test]
    fn terminal_input_state_tracks_marked_text_bounds() {
        let mut state = TerminalInputState::new();
        let bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(80.0), px(54.0)));
        state.set_snapshot(TerminalInputSnapshot {
            cursor: TerminalPosition { row: 1, column: 2 },
            metrics: TerminalGridMetrics {
                bounds,
                cell_width: px(8.0),
                line_height: px(18.0),
                columns: 10,
                rows: 3,
            },
            modes: TerminalModes::default(),
        });

        state.set_marked_text(None, "a語", Some(2..2));

        assert_eq!(state.marked_text(), "a語");
        assert!(state.has_marked_text());
        assert_eq!(
            state.bounds_for_marked_range(1..2, bounds),
            Some(Bounds::new(
                point(px(34.0), px(38.0)),
                size(px(16.0), px(18.0))
            ))
        );
        assert_eq!(
            state.character_index_for_marked_point(point(px(42.0), px(38.0)), bounds),
            Some(1)
        );
    }

    #[test]
    fn terminal_input_state_commits_and_clears_marked_text() {
        let mut state = TerminalInputState::new();
        state.set_marked_text(None, "ni", Some(2..2));

        assert_eq!(state.commit_text(None, "に"), "に".as_bytes().to_vec());
        assert_eq!(state.marked_text(), "");
        assert!(!state.has_marked_text());
        assert_eq!(state.marked_range, None);
    }

    #[test]
    fn terminal_keystrokes_emit_control_and_navigation_bytes() {
        let ctrl_c = parsed_keystroke("ctrl-c");
        assert_eq!(terminal_keystroke_bytes(&ctrl_c), Some(vec![0x03]));

        let shift_tab = parsed_keystroke("shift-tab");
        assert_eq!(
            terminal_keystroke_bytes(&shift_tab),
            Some(b"\x1b[Z".to_vec())
        );

        let ctrl_right = parsed_keystroke("ctrl-right");
        assert_eq!(
            terminal_keystroke_bytes(&ctrl_right),
            Some(b"\x1b[1;5C".to_vec())
        );

        let alt_f5 = parsed_keystroke("alt-f5");
        assert_eq!(
            terminal_keystroke_bytes(&alt_f5),
            Some(b"\x1b\x1b[15;3~".to_vec())
        );

        let modes = TerminalModes {
            application_cursor_keys: true,
            ..TerminalModes::default()
        };
        let up = parsed_keystroke("up");
        assert_eq!(
            terminal_keystroke_bytes_with_modes(&up, modes),
            Some(b"\x1bOA".to_vec())
        );
    }

    fn parsed_keystroke(source: &str) -> Keystroke {
        match Keystroke::parse(source) {
            Ok(keystroke) => keystroke,
            Err(error) => panic!("{error}"),
        }
    }
    #[gpui::test]
    fn blink_timer_stops_when_content_is_static(cx: &mut TestAppContext) {
        let state = cx.new(|_| super::TerminalBlinkState {
            visible: true,
            task: None,
        });
        state.update(cx, |state, cx| state.set_active(true, cx));
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(500));
        cx.run_until_parked();
        state.update(cx, |state, cx| {
            assert!(!state.visible);
            state.set_active(false, cx);
            assert!(state.visible);
            assert!(state.task.is_none());
        });
        cx.executor().advance_clock(Duration::from_secs(2));
        cx.run_until_parked();
        state.update(cx, |state, _| assert!(state.visible));
    }

    #[cfg(unix)]
    #[test]
    fn session_outcome_distinguishes_exit_from_wait_failure() {
        let mut config = super::PtySpawnConfig::new("/bin/sh");
        config.arguments = vec!["-c".into(), "exit 7".into()];
        let mut session = super::LocalPtySession::spawn(config, 80, 24).expect("spawn");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let outcome = loop {
            if let Some(outcome) = session.try_outcome() {
                break outcome;
            }
            assert!(std::time::Instant::now() < deadline, "process exit timeout");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(
            outcome,
            super::TerminalSessionOutcome::Exited {
                status: super::TerminalExitStatus { code: 7 },
                termination_requested: false
            }
        );
        let (sender, receiver) = std::sync::mpsc::channel();
        sender
            .send(Err("wait backend failed".to_owned()))
            .expect("send");
        session.exit = receiver;
        session.outcome = None;
        assert_eq!(
            session.try_outcome(),
            Some(super::TerminalSessionOutcome::WaitFailed {
                message: "wait backend failed".into()
            })
        );
    }

    #[test]
    fn malformed_osc_and_progress_do_not_create_notifications() {
        let mut model = TerminalModel::new(80, 24);
        model.write("\x1b]7;file:///tmp/%0Aevil\x07\x1b]9;4;1;50\x07");
        assert!(model.current_directory().is_none());
        assert_eq!(model.notification_sequence, 0);
    }
    struct SharedLinkHarness {
        model: super::SharedTerminalModel,
        activations: std::rc::Rc<std::cell::Cell<usize>>,
    }
    impl Render for SharedLinkHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
            use gpui::{InteractiveElement as _, ParentElement as _, Styled as _};
            let model = self.model.clone();
            let activations = self.activations.clone();
            gpui::div()
                .size_full()
                .debug_selector(|| "shared-link-grid".into())
                .child(
                    Terminal::from_shared("shared-links", self.model.clone())
                        .options(
                            TerminalOptions::default()
                                .cell_size(8, 18)
                                .link_activation_modifier(super::TerminalLinkModifier::Control),
                        )
                        .on_link(move |_, _, _| {
                            model.borrow_mut().write("activated");
                            activations.set(activations.get() + 1);
                        }),
                )
        }
    }
    #[gpui::test]
    fn shared_model_link_callback_can_mutate_model(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
        });
        let mut model = TerminalModel::new(80, 24);
        model.write("\x1b]8;;https://example.com\x07Link\x1b]8;;\x07");
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let captured = calls.clone();
        let (_, cx) = cx.add_window_view(|_, _| SharedLinkHarness {
            model: std::rc::Rc::new(std::cell::RefCell::new(model)),
            activations: captured,
        });
        let bounds = cx.debug_bounds("shared-link-grid").expect("grid");
        cx.simulate_click(
            bounds.origin + point(px(4.), px(9.)),
            gpui::Modifiers {
                control: true,
                ..Default::default()
            },
        );
        assert_eq!(calls.get(), 1);
    }
}
