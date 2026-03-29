/// # gpui-editor
///
/// A multi-line textarea component built with [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui).
///
/// ## Features
///
/// - Multi-line text editing with word wrap
/// - IME support (Chinese, Japanese, Korean input methods)
/// - Keyboard shortcuts (Arrow keys, Up/Down, Backspace/Delete, Home/End, Cmd+A/C/V/X)
/// - Mouse click to position cursor, drag to select, Shift+click to extend
/// - Cursor blinking animation (500ms interval)
/// - Content auto-height with optional max height and scroll
/// - Input character limit (by Unicode grapheme count)
/// - Enter key mode switching (Enter submits or inserts newline)
/// - Customizable colors (background, cursor, text, selection)
/// - Placeholder text
///
/// ## Usage
///
/// ```rust,ignore
/// use gpui_editor::textarea::{TextInput, Textarea, EnterMode, init, render_textarea};
///
/// // In your gpui App setup:
/// init(cx); // register key bindings
///
/// let textarea = cx.new(|cx| {
///     TextInput::new(cx)
///         .placeholder("Type here...")
///         .max_length(500)
///         .max_height(px(300.0))
///         .enter_mode(EnterMode::EnterNewline)
///         .cursor_color(hsla(210.0/360.0, 1.0, 0.5, 1.0))
/// });
/// ```

/// The main textarea component.
pub mod textarea;
