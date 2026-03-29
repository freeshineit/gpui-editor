/// # gpui-editor
///
/// A multi-line textarea component built with [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui).
///
/// ## Features
///
/// - Multi-line text editing with IME support
/// - Keyboard shortcuts (Backspace, Delete, Arrow keys, Cmd+A/C/V/X, Home/End)
/// - Mouse click to position cursor, drag to select, Shift+click to extend
/// - Customizable colors (background, cursor, text, selection)
/// - Placeholder text
/// - Word wrap via gpui `WrappedLine`
///
/// ## Usage
///
/// ```rust,ignore
/// use gpui_editor::textarea::{TextInput, Textarea, init, render_textarea};
///
/// // In your gpui App setup:
/// init(cx); // register key bindings
///
/// let textarea = cx.new(|cx| {
///     TextInput::new(cx)
///         .placeholder("Type here...")
///         .cursor_color(hsla(210.0/360.0, 1.0, 0.5, 1.0))
/// });
/// ```

/// The main textarea component.
pub mod textarea;
