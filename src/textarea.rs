/// Multi-line textarea component built with gpui.
///
/// Implements a full-featured HTML-like textarea supporting:
/// - Multi-line text editing
/// - Keyboard shortcuts (Home, End, Ctrl+A, Ctrl+C/V/X, arrows, etc.)
/// - Mouse click to position cursor, click-drag to select
/// - Custom cursor, selection, background, and text colors
/// - Placeholder text
use gpui::{
    canvas, div, fill, point, px, size, App, Bounds, ClipboardItem, Context, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render,
    SharedString, Size, Styled, Window,
};

use crate::buffer::{Position, TextBuffer, TextRange};
use crate::cursor::Cursor;
use crate::style::TextareaStyle;

/// Events emitted by the textarea.
#[derive(Debug, Clone)]
pub enum TextareaEvent {
    /// Text content changed.
    Changed(String),
}

/// Multi-line textarea component.
pub struct Textarea {
    /// The text buffer.
    buffer: TextBuffer,
    /// Cursor / selection state.
    cursor: Cursor,
    /// Visual style.
    style: TextareaStyle,
    /// Focus handle for keyboard events.
    focus_handle: FocusHandle,
    /// Whether the mouse is currently pressed (for drag-to-select).
    is_dragging: bool,
    /// Placeholder text shown when buffer is empty.
    placeholder: String,
    /// Scroll offset (vertical, in pixels).
    scroll_offset_y: f32,
    /// Scroll offset (horizontal, in pixels).
    scroll_offset_x: f32,
    /// Cached content bounds from last render (for hit-testing).
    content_bounds: Option<Bounds<Pixels>>,
    /// Whether the textarea is read-only.
    read_only: bool,
}

impl EventEmitter<TextareaEvent> for Textarea {}

impl Textarea {
    /// Create a new textarea.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            buffer: TextBuffer::new(),
            cursor: Cursor::new(),
            style: TextareaStyle::default(),
            focus_handle: cx.focus_handle(),
            is_dragging: false,
            placeholder: String::new(),
            scroll_offset_y: 0.0,
            scroll_offset_x: 0.0,
            content_bounds: None,
            read_only: false,
        }
    }

    /// Set initial text content.
    pub fn set_text(&mut self, text: &str) {
        self.buffer = TextBuffer::from_text(text);
        self.cursor = Cursor::new();
        self.scroll_offset_y = 0.0;
        self.scroll_offset_x = 0.0;
    }

    /// Get current text content.
    pub fn text(&self) -> String {
        self.buffer.text()
    }

    /// Set the style.
    pub fn set_style(&mut self, style: TextareaStyle) {
        self.style = style;
    }

    /// Set placeholder text.
    pub fn set_placeholder(&mut self, text: &str) {
        self.placeholder = text.to_string();
    }

    /// Set read-only mode.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    /// Get a reference to the style.
    pub fn style(&self) -> &TextareaStyle {
        &self.style
    }

    /// Get mutale reference to the style.
    pub fn style_mut(&mut self) -> &mut TextareaStyle {
        &mut self.style
    }

    /// Get current cursor position.
    pub fn cursor_position(&self) -> Position {
        self.cursor.position
    }

    /// Get current selection range.
    pub fn selection(&self) -> Option<TextRange> {
        self.cursor.selection()
    }

    /// Get the text buffer.
    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    // ── Text editing ──────────────────────────────────

    /// Insert text at cursor, replacing selection if any.
    fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let new_pos = if let Some(sel) = self.cursor.selection() {
            self.buffer.replace(sel, text)
        } else {
            self.buffer.insert(self.cursor.position, text)
        };
        self.cursor.set_position(new_pos);
        cx.emit(TextareaEvent::Changed(self.buffer.text()));
        cx.notify();
    }

    /// Delete the character before the cursor (backspace).
    fn backspace(&mut self, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.cursor.has_selection() {
            let sel = self.cursor.selection().unwrap();
            let pos = self.buffer.delete(sel);
            self.cursor.set_position(pos);
        } else {
            let prev = self.buffer.move_left(self.cursor.position);
            if prev != self.cursor.position {
                let range = TextRange::new(prev, self.cursor.position);
                self.buffer.delete(range);
                self.cursor.set_position(prev);
            }
        }
        cx.emit(TextareaEvent::Changed(self.buffer.text()));
        cx.notify();
    }

    /// Delete the character after the cursor.
    fn delete_forward(&mut self, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.cursor.has_selection() {
            let sel = self.cursor.selection().unwrap();
            let pos = self.buffer.delete(sel);
            self.cursor.set_position(pos);
        } else {
            let next = self.buffer.move_right(self.cursor.position);
            if next != self.cursor.position {
                let range = TextRange::new(self.cursor.position, next);
                self.buffer.delete(range);
            }
        }
        cx.emit(TextareaEvent::Changed(self.buffer.text()));
        cx.notify();
    }

    /// Delete the word before the cursor.
    fn delete_word_left(&mut self, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.cursor.has_selection() {
            self.backspace(cx);
            return;
        }
        let word_start = self.buffer.move_word_left(self.cursor.position);
        if word_start != self.cursor.position {
            let range = TextRange::new(word_start, self.cursor.position);
            self.buffer.delete(range);
            self.cursor.set_position(word_start);
            cx.emit(TextareaEvent::Changed(self.buffer.text()));
            cx.notify();
        }
    }

    /// Get selected text.
    fn selected_text(&self) -> Option<String> {
        self.cursor
            .selection()
            .map(|sel| self.buffer.text_in_range(sel))
    }

    /// Cut selected text to clipboard.
    fn cut(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            let sel = self.cursor.selection().unwrap();
            let pos = self.buffer.delete(sel);
            self.cursor.set_position(pos);
            cx.emit(TextareaEvent::Changed(self.buffer.text()));
            cx.notify();
        }
    }

    /// Copy selected text to clipboard.
    fn copy(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    /// Paste from clipboard.
    fn paste(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                self.insert_text(&text, cx);
            }
        }
    }

    /// Select all text.
    fn select_all(&mut self, cx: &mut Context<Self>) {
        self.cursor.select_all(&self.buffer);
        cx.notify();
    }

    // ── Coordinate mapping ────────────────────────────

    /// Convert a pixel position (relative to content area origin) to a buffer position.
    fn pixel_to_position(&self, point: Point<Pixels>, content_origin: Point<Pixels>) -> Position {
        let x = f32::from(point.x - content_origin.x + px(self.scroll_offset_x));
        let y = f32::from(point.y - content_origin.y + px(self.scroll_offset_y));

        let line = ((y / self.style.line_height).floor() as usize)
            .min(self.buffer.line_count().saturating_sub(1));

        // Approximate character position from x coordinate.
        let char_width = self.style.font_size * 0.6; // Approximate monospace width.
        let col_approx = ((x / char_width).round() as usize).max(0);

        // Snap to actual grapheme boundary.
        let line_str = self.buffer.line(line);
        let mut byte_offset = 0;
        let mut grapheme_count = 0;
        use unicode_segmentation::UnicodeSegmentation;
        for grapheme in line_str.graphemes(true) {
            if grapheme_count >= col_approx {
                break;
            }
            byte_offset += grapheme.len();
            grapheme_count += 1;
        }

        self.buffer.clamp_position(Position::new(line, byte_offset))
    }

    /// Convert a buffer position to pixel coordinates (relative to content area origin).
    fn position_to_pixel(&self, pos: Position) -> Point<Pixels> {
        let char_width = self.style.font_size * 0.6;
        let line_str = self.buffer.line(pos.line);

        use unicode_segmentation::UnicodeSegmentation;
        let grapheme_count = line_str[..pos.col].graphemes(true).count();

        let x = grapheme_count as f32 * char_width - self.scroll_offset_x;
        let y = pos.line as f32 * self.style.line_height - self.scroll_offset_y;

        point(px(x), px(y))
    }

    /// Ensure cursor is visible by adjusting scroll offset.
    fn ensure_cursor_visible(&mut self, viewport_size: Size<Pixels>) {
        let cursor_px = self.position_to_pixel(self.cursor.position);
        let padding = self.style.padding;

        let view_width = f32::from(viewport_size.width) - padding * 2.0;
        let view_height = f32::from(viewport_size.height) - padding * 2.0;

        // Vertical scroll
        let cursor_y = f32::from(cursor_px.y) + self.scroll_offset_y;
        if cursor_y < self.scroll_offset_y {
            self.scroll_offset_y = cursor_y;
        } else if cursor_y + self.style.line_height > self.scroll_offset_y + view_height {
            self.scroll_offset_y = cursor_y + self.style.line_height - view_height;
        }

        // Horizontal scroll
        let cursor_x = f32::from(cursor_px.x) + self.scroll_offset_x;
        if cursor_x < self.scroll_offset_x {
            self.scroll_offset_x = cursor_x;
        } else if cursor_x + self.style.cursor_width > self.scroll_offset_x + view_width {
            self.scroll_offset_x = cursor_x + self.style.cursor_width - view_width;
        }

        self.scroll_offset_y = self.scroll_offset_y.max(0.0);
        self.scroll_offset_x = self.scroll_offset_x.max(0.0);
    }

    // ── Keyboard handling ─────────────────────────────

    /// Handle a key down event. Returns true if the event was handled.
    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let keystroke = &event.keystroke;
        let shift = keystroke.modifiers.shift;
        // On macOS, the "command" key is the platform modifier.
        let cmd = keystroke.modifiers.platform;
        let alt = keystroke.modifiers.alt;
        let ctrl = keystroke.modifiers.control;

        // Use cmd on macOS for standard shortcuts.
        let action_mod = cmd;

        match keystroke.key.as_str() {
            "left" => {
                if action_mod && shift {
                    self.cursor.move_home(&self.buffer, true);
                } else if action_mod {
                    self.cursor.move_home(&self.buffer, false);
                } else if alt {
                    self.cursor.move_word_left(&self.buffer, shift);
                } else {
                    self.cursor.move_left(&self.buffer, shift);
                }
                cx.notify();
                true
            }
            "right" => {
                if action_mod && shift {
                    self.cursor.move_end(&self.buffer, true);
                } else if action_mod {
                    self.cursor.move_end(&self.buffer, false);
                } else if alt {
                    self.cursor.move_word_right(&self.buffer, shift);
                } else {
                    self.cursor.move_right(&self.buffer, shift);
                }
                cx.notify();
                true
            }
            "up" => {
                if action_mod {
                    self.cursor.move_to_start(&self.buffer, shift);
                } else {
                    self.cursor.move_up(&self.buffer, shift);
                }
                cx.notify();
                true
            }
            "down" => {
                if action_mod {
                    self.cursor.move_to_end(&self.buffer, shift);
                } else {
                    self.cursor.move_down(&self.buffer, shift);
                }
                cx.notify();
                true
            }
            "home" => {
                if ctrl && shift {
                    self.cursor.move_to_start(&self.buffer, true);
                } else if ctrl {
                    self.cursor.move_to_start(&self.buffer, false);
                } else {
                    self.cursor.move_home(&self.buffer, shift);
                }
                cx.notify();
                true
            }
            "end" => {
                if ctrl && shift {
                    self.cursor.move_to_end(&self.buffer, true);
                } else if ctrl {
                    self.cursor.move_to_end(&self.buffer, false);
                } else {
                    self.cursor.move_end(&self.buffer, shift);
                }
                cx.notify();
                true
            }
            "backspace" => {
                if alt {
                    self.delete_word_left(cx);
                } else {
                    self.backspace(cx);
                }
                true
            }
            "delete" => {
                self.delete_forward(cx);
                true
            }
            "enter" => {
                self.insert_text("\n", cx);
                true
            }
            "tab" => {
                self.insert_text("    ", cx);
                true
            }
            "a" if action_mod => {
                self.select_all(cx);
                true
            }
            "c" if action_mod => {
                self.copy(window, cx);
                true
            }
            "x" if action_mod => {
                self.cut(window, cx);
                true
            }
            "v" if action_mod => {
                self.paste(window, cx);
                true
            }
            "z" if action_mod && shift => {
                // Redo - not implemented yet
                true
            }
            "z" if action_mod => {
                // Undo - not implemented yet
                true
            }
            _ => {
                // Insert the character if it's printable.
                if let Some(ref ch) = keystroke.key_char {
                    if !action_mod && !ctrl {
                        self.insert_text(ch, cx);
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Handle mouse down event.
    fn handle_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);

        if let Some(bounds) = self.content_bounds {
            let pos = self.pixel_to_position(event.position, bounds.origin);

            if event.click_count == 2 {
                // Double-click: select word.
                let word_start = self.buffer.move_word_left(pos);
                let word_end = self.buffer.move_word_right(pos);
                self.cursor.set_position(word_start);
                self.cursor.select_to(word_end);
            } else if event.click_count == 3 {
                // Triple-click: select line.
                let line_start = self.buffer.line_start(pos);
                let line_end = self.buffer.line_end(pos);
                self.cursor.set_position(line_start);
                self.cursor.select_to(line_end);
            } else if event.modifiers.shift {
                self.cursor.select_to(pos);
            } else {
                self.cursor.set_position(pos);
            }

            self.is_dragging = true;
            cx.notify();
        }
    }

    /// Handle mouse move event (for drag-to-select).
    fn handle_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_dragging {
            if let Some(bounds) = self.content_bounds {
                let pos = self.pixel_to_position(event.position, bounds.origin);
                self.cursor.select_to(pos);
                cx.notify();
            }
        }
    }

    /// Handle mouse up event.
    fn handle_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_dragging = false;
        cx.notify();
    }
}

impl Focusable for Textarea {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Textarea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let style = self.style.clone();
        let is_focused = self.focus_handle.is_focused(window);
        let border_color = if is_focused {
            style.focused_border_color
        } else {
            style.border_color
        };

        let lines: Vec<String> = self.buffer.lines().to_vec();
        let cursor_pos = self.cursor.position;
        let selection = self.cursor.selection();
        let line_height = style.line_height;
        let font_size = style.font_size;
        let padding = style.padding;
        let char_width = font_size * 0.6;
        let scroll_x = self.scroll_offset_x;
        let scroll_y = self.scroll_offset_y;
        let cursor_color = style.cursor_color;
        let cursor_width = style.cursor_width;
        let text_color = style.text_color;
        let selection_bg = style.selection_background;
        let _selection_text_col = style.selection_text_color;
        let bg_color = style.background;
        let placeholder = self.placeholder.clone();
        let placeholder_color = style.placeholder_color;
        let font_family_str = style.font_family.clone();

        let show_placeholder = lines.len() == 1 && lines[0].is_empty() && !placeholder.is_empty();

        let entity = cx.entity().clone();

        div()
            .track_focus(&self.focus_handle)
            .size_full()
            .border_color(border_color)
            .rounded(px(style.corner_radius))
            .overflow_hidden()
            .on_key_down(cx.listener(
                move |view: &mut Textarea, event: &KeyDownEvent, window, cx| {
                    view.handle_key_down(event, window, cx);
                    // Ensure cursor stays visible after key actions.
                    if let Some(bounds) = view.content_bounds {
                        view.ensure_cursor_visible(bounds.size);
                    }
                },
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(
                    move |view: &mut Textarea, event: &MouseDownEvent, window, cx| {
                        view.handle_mouse_down(event, window, cx);
                    },
                ),
            )
            .on_mouse_move(cx.listener(
                move |view: &mut Textarea, event: &MouseMoveEvent, window, cx| {
                    view.handle_mouse_move(event, window, cx);
                },
            ))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(
                    move |view: &mut Textarea, event: &MouseUpEvent, window, cx| {
                        view.handle_mouse_up(event, window, cx);
                    },
                ),
            )
            .child(
                canvas(
                    // Prepaint: store bounds.
                    {
                        let entity = entity.clone();
                        move |bounds, _window, cx| {
                            entity.update(cx, |view, _cx| {
                                view.content_bounds = Some(bounds);
                            });
                            bounds
                        }
                    },
                    // Paint: render text, cursor, selections.
                    move |bounds: Bounds<Pixels>, _prev: Bounds<Pixels>, window, cx| {
                        let origin = bounds.origin;
                        let _w = f32::from(bounds.size.width);
                        let h = f32::from(bounds.size.height);

                        // Background fill.
                        window.paint_quad(fill(bounds, bg_color));

                        // Draw selection highlights.
                        if let Some(sel) = selection {
                            for line_idx in sel.start.line..=sel.end.line {
                                if line_idx >= lines.len() {
                                    break;
                                }
                                let line_str = &lines[line_idx];
                                use unicode_segmentation::UnicodeSegmentation;

                                let sel_start_col = if line_idx == sel.start.line {
                                    line_str[..sel.start.col].graphemes(true).count()
                                } else {
                                    0
                                };
                                let sel_end_col = if line_idx == sel.end.line {
                                    line_str[..sel.end.col].graphemes(true).count()
                                } else {
                                    line_str.graphemes(true).count()
                                };

                                if sel_start_col == sel_end_col && line_idx != sel.end.line {
                                    // Full-width highlight for lines in the middle of selection
                                    // when the line is empty or fully selected.
                                }

                                let x1 = padding + sel_start_col as f32 * char_width - scroll_x;
                                let x2 = padding + sel_end_col as f32 * char_width - scroll_x;
                                let y = padding + line_idx as f32 * line_height - scroll_y;

                                if x2 > x1 {
                                    let sel_bounds = Bounds {
                                        origin: point(origin.x + px(x1), origin.y + px(y)),
                                        size: size(px(x2 - x1), px(line_height)),
                                    };
                                    window.paint_quad(fill(sel_bounds, selection_bg));
                                }
                            }
                        }

                        // Draw text lines.
                        let font_family: SharedString = font_family_str.clone().into();
                        for (line_idx, line_str) in lines.iter().enumerate() {
                            let y = padding + line_idx as f32 * line_height - scroll_y;
                            if y + line_height < 0.0 || y > h {
                                continue;
                            }

                            let display_text: SharedString = if show_placeholder && line_idx == 0 {
                                placeholder.clone().into()
                            } else {
                                line_str.clone().into()
                            };

                            if display_text.is_empty() {
                                continue;
                            }

                            let color = if show_placeholder && line_idx == 0 {
                                placeholder_color
                            } else {
                                text_color
                            };

                            let text_origin =
                                point(origin.x + px(padding - scroll_x), origin.y + px(y));

                            let run = gpui::TextRun {
                                len: display_text.len(),
                                font: gpui::Font {
                                    family: font_family.clone(),
                                    features: gpui::FontFeatures::default(),
                                    fallbacks: None,
                                    weight: gpui::FontWeight::NORMAL,
                                    style: gpui::FontStyle::Normal,
                                },
                                color,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            };

                            let shaped = window.text_system().shape_line(
                                display_text,
                                px(font_size),
                                &[run],
                                None,
                            );

                            shaped.paint(text_origin, px(line_height), window, cx).ok();
                        }

                        // Draw cursor (blinking caret).
                        if is_focused {
                            use unicode_segmentation::UnicodeSegmentation;
                            let cursor_line = &lines[cursor_pos.line.min(lines.len() - 1)];
                            let grapheme_count = cursor_line
                                [..cursor_pos.col.min(cursor_line.len())]
                                .graphemes(true)
                                .count();

                            let cx_pos = padding + grapheme_count as f32 * char_width - scroll_x;
                            let cy_pos = padding + cursor_pos.line as f32 * line_height - scroll_y;

                            let cursor_bounds = Bounds {
                                origin: point(origin.x + px(cx_pos), origin.y + px(cy_pos)),
                                size: size(px(cursor_width), px(line_height)),
                            };
                            window.paint_quad(fill(cursor_bounds, cursor_color));
                        }
                    },
                )
                .size_full(),
            )
    }
}
