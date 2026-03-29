/// # gpui-editor
///
/// A multi-line textarea library built with [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui).
///
/// ## Features
///
/// - Multi-line text editing
/// - HTML textarea–like keyboard shortcuts
/// - Custom cursor color and width
/// - Selection background and text color
/// - Custom background and font colors
/// - Placeholder text support
/// - Read-only mode
/// - Mouse click-to-position and drag-to-select
/// - Double-click to select word, triple-click to select line
///
/// ## Usage
///
/// ```rust,no_run
/// use gpui_editor::textarea::Textarea;
/// use gpui_editor::style::TextareaStyle;
///
/// // In a gpui application:
/// // let textarea = cx.new(|cx| {
/// //     let mut ta = Textarea::new(cx);
/// //     ta.set_text("Hello, world!");
/// //     ta.set_placeholder("Type here...");
/// //     ta.set_style(TextareaStyle::dark());
/// //     ta
/// // });
/// ```
///
/// ## Modules
///
/// - [`buffer`] – Text buffer with line-based storage and editing operations
/// - [`cursor`] – Cursor position and selection management
/// - [`style`] – Theming and visual configuration
/// - [`textarea`] – The main textarea gpui component

/// Text buffer: stores multi-line text and provides editing primitives.
pub mod buffer;

/// Cursor and selection management.
pub mod cursor;

/// Styling / theming for the textarea.
pub mod style;

/// The main textarea component (gpui `Render` implementation).
pub mod textarea;

pub use buffer::{Position, TextBuffer, TextRange};
pub use cursor::Cursor;
pub use style::TextareaStyle;
pub use textarea::{Textarea, TextareaEvent};
