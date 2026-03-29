/// Styling configuration for the textarea.
///
/// Allows customization of colors, fonts, and appearance.
use gpui::Hsla;

/// Styling options for the textarea.
#[derive(Debug, Clone)]
pub struct TextareaStyle {
    /// Background color of the textarea.
    pub background: Hsla,
    /// Default text color.
    pub text_color: Hsla,
    /// Cursor (caret) color.
    pub cursor_color: Hsla,
    /// Selection background color.
    pub selection_background: Hsla,
    /// Selected text color (if set, overrides default text color for selected text).
    pub selection_text_color: Option<Hsla>,
    /// Font family name.
    pub font_family: String,
    /// Font size in pixels.
    pub font_size: f32,
    /// Line height in pixels.
    pub line_height: f32,
    /// Padding inside the textarea (pixels).
    pub padding: f32,
    /// Border color.
    pub border_color: Hsla,
    /// Border width in pixels.
    pub border_width: f32,
    /// Corner radius in pixels.
    pub corner_radius: f32,
    /// Cursor width in pixels.
    pub cursor_width: f32,
    /// Focused border color.
    pub focused_border_color: Hsla,
    /// Placeholder text color.
    pub placeholder_color: Hsla,
}

impl Default for TextareaStyle {
    fn default() -> Self {
        Self {
            background: gpui::hsla(0.0, 0.0, 1.0, 1.0), // white
            text_color: gpui::hsla(0.0, 0.0, 0.1, 1.0), // near-black
            cursor_color: gpui::hsla(0.0, 0.0, 0.1, 1.0),
            selection_background: gpui::hsla(0.58, 0.8, 0.75, 0.4), // blue highlight
            selection_text_color: None,
            font_family: "Monaco".to_string(),
            font_size: 14.0,
            line_height: 20.0,
            padding: 8.0,
            border_color: gpui::hsla(0.0, 0.0, 0.75, 1.0), // light gray
            border_width: 1.0,
            corner_radius: 4.0,
            cursor_width: 2.0,
            focused_border_color: gpui::hsla(0.58, 0.8, 0.55, 1.0), // blue
            placeholder_color: gpui::hsla(0.0, 0.0, 0.6, 1.0),
        }
    }
}

/// Dark theme preset.
impl TextareaStyle {
    /// Create a dark-themed style.
    pub fn dark() -> Self {
        Self {
            background: gpui::hsla(0.0, 0.0, 0.12, 1.0),
            text_color: gpui::hsla(0.0, 0.0, 0.9, 1.0),
            cursor_color: gpui::hsla(0.0, 0.0, 0.95, 1.0),
            selection_background: gpui::hsla(0.58, 0.6, 0.4, 0.5),
            selection_text_color: None,
            font_family: "Monaco".to_string(),
            font_size: 14.0,
            line_height: 20.0,
            padding: 8.0,
            border_color: gpui::hsla(0.0, 0.0, 0.3, 1.0),
            border_width: 1.0,
            corner_radius: 4.0,
            cursor_width: 2.0,
            focused_border_color: gpui::hsla(0.58, 0.8, 0.55, 1.0),
            placeholder_color: gpui::hsla(0.0, 0.0, 0.4, 1.0),
        }
    }
}
