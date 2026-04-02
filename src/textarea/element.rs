//! 自定义 Element 渲染实现：文本排版、光标/选区绘制、滚动裁剪。

use std::cmp::{max, min};

use gpui::{
    App, Bounds, Context, CursorStyle, ElementId, ElementInputHandler, Entity, FocusHandle,
    Focusable, GlobalElementId, LayoutId, MouseButton, PaintQuad, Pixels, Style, TextAlign,
    TextRun, UnderlineStyle, Window, div, fill, hsla, point, prelude::*, px, relative, size,
};

use super::TextInput;

pub(crate) struct TextElement {
    pub(crate) input: Entity<TextInput>,
}

pub(crate) struct PrepaintState {
    lines: Vec<gpui::WrappedLine>,
    line_starts: Vec<usize>,
    line_offsets: Vec<Pixels>,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
    cursor_visible: bool,
    scroll_offset: Pixels,
    content_height: Pixels,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let cursor_visible = input.cursor_visible;
        let scroll_offset = input.scroll_offset;
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let wrap_width = Some(bounds.size.width);

        let (display_text, text_color) = if content.is_empty() {
            let gpui::Hsla { h, s, l, a } = input.text_color;
            (input.placeholder.clone(), hsla(h, s, l, a * 0.5))
        } else {
            (content.clone(), input.text_color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let runs = if !content.is_empty() {
            if let Some(marked_range) = input.marked_range.as_ref() {
                vec![
                    TextRun {
                        len: marked_range.start,
                        ..run.clone()
                    },
                    TextRun {
                        len: marked_range.end - marked_range.start,
                        underline: Some(UnderlineStyle {
                            color: Some(run.color),
                            thickness: px(1.0),
                            wavy: false,
                        }),
                        ..run.clone()
                    },
                    TextRun {
                        len: display_text.len() - marked_range.end,
                        ..run
                    },
                ]
                .into_iter()
                .filter(|segment| segment.len > 0)
                .collect::<Vec<_>>()
            } else {
                vec![run]
            }
        } else {
            vec![run]
        };

        let mut lines = window
            .text_system()
            .shape_text(display_text, font_size, &runs, wrap_width, None)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();

        if lines.is_empty() {
            lines.push(
                window
                    .text_system()
                    .shape_text(" ".into(), font_size, &[], wrap_width, None)
                    .unwrap_or_default()
                    .into_iter()
                    .next()
                    .unwrap(),
            );
        }

        let mut line_starts = Vec::new();

        if content.is_empty() {
            line_starts.push(0);
        } else {
            let line_texts: Vec<&str> = content.split('\n').collect();
            let mut offset = 0;

            for (idx, line_text) in line_texts.iter().enumerate() {
                line_starts.push(offset);
                let line_len = line_text.len();

                offset += line_len;
                if idx + 1 < line_texts.len() {
                    offset += 1;
                }
            }
        }

        if line_starts.len() != lines.len() {
            line_starts.resize(lines.len(), content.len());
        }

        let line_height = window.line_height();
        let mut line_offsets = Vec::with_capacity(lines.len());
        let mut accumulated_y = px(0.0);
        for line in &lines {
            line_offsets.push(accumulated_y);
            accumulated_y += line_height * TextInput::visual_line_count(line) as f32;
        }

        let content_height = accumulated_y;

        let mut selections = Vec::new();

        let cursor = if selected_range.is_empty() {
            let line_index = TextInput::line_index_for_offset(&line_starts, cursor);
            let line_start = line_starts[line_index];
            let cursor_pos = lines[line_index]
                .position_for_index(cursor.saturating_sub(line_start), line_height)
                .unwrap_or(point(px(0.0), px(0.0)));
            let top = bounds.top() + line_offsets[line_index] + cursor_pos.y - scroll_offset;

            Some(fill(
                Bounds::new(
                    point(bounds.left() + cursor_pos.x, top),
                    size(px(2.), line_height),
                ),
                input.cursor_color,
            ))
        } else {
            let start_line = TextInput::line_index_for_offset(&line_starts, selected_range.start);
            let end_line = TextInput::line_index_for_offset(&line_starts, selected_range.end);

            for line_index in start_line..=end_line {
                let line_start = line_starts[line_index];
                let line_end = line_start + lines[line_index].len();

                let segment_start = max(selected_range.start, line_start);
                let segment_end = min(selected_range.end, line_end);
                if segment_start >= segment_end {
                    continue;
                }

                let local_start = segment_start - line_start;
                let local_end = segment_end - line_start;
                let start_pos = lines[line_index]
                    .position_for_index(local_start, line_height)
                    .unwrap_or(point(px(0.0), px(0.0)));
                let end_pos = lines[line_index]
                    .position_for_index(local_end, line_height)
                    .unwrap_or(point(px(0.0), start_pos.y));

                let top_base = bounds.top() + line_offsets[line_index] - scroll_offset;
                if (end_pos.y - start_pos.y).abs() < px(0.5) {
                    selections.push(fill(
                        Bounds::from_corners(
                            point(bounds.left() + start_pos.x, top_base + start_pos.y),
                            point(
                                bounds.left() + end_pos.x,
                                top_base + end_pos.y + line_height,
                            ),
                        ),
                        input.selection_color,
                    ));
                } else {
                    selections.push(fill(
                        Bounds::from_corners(
                            point(bounds.left() + start_pos.x, top_base + start_pos.y),
                            point(bounds.right(), top_base + start_pos.y + line_height),
                        ),
                        input.selection_color,
                    ));

                    let mut y = start_pos.y + line_height;
                    while y < end_pos.y {
                        selections.push(fill(
                            Bounds::from_corners(
                                point(bounds.left(), top_base + y),
                                point(bounds.right(), top_base + y + line_height),
                            ),
                            input.selection_color,
                        ));
                        y += line_height;
                    }

                    if end_pos.x > px(0.0) {
                        selections.push(fill(
                            Bounds::from_corners(
                                point(bounds.left(), top_base + end_pos.y),
                                point(
                                    bounds.left() + end_pos.x,
                                    top_base + end_pos.y + line_height,
                                ),
                            ),
                            input.selection_color,
                        ));
                    }
                }
            }

            None
        };

        PrepaintState {
            lines,
            line_starts,
            line_offsets,
            cursor,
            selections,
            cursor_visible,
            scroll_offset,
            content_height,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        for selection in prepaint.selections.drain(..) {
            window.paint_quad(selection);
        }

        let line_height = window.line_height();
        let scroll_offset = prepaint.scroll_offset;
        for (line_index, line) in prepaint.lines.iter().enumerate() {
            let origin = point(
                bounds.left(),
                bounds.top() + prepaint.line_offsets[line_index] - scroll_offset,
            );
            let line_bounds = Bounds::new(
                origin,
                size(
                    bounds.size.width,
                    line_height * TextInput::visual_line_count(line) as f32,
                ),
            );
            line.paint(
                origin,
                line_height,
                TextAlign::Left,
                Some(line_bounds),
                window,
                cx,
            )
            .unwrap();
        }

        if focus_handle.is_focused(window) && prepaint.cursor_visible {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        let lines = std::mem::take(&mut prepaint.lines);
        let line_starts = std::mem::take(&mut prepaint.line_starts);
        let line_offsets = std::mem::take(&mut prepaint.line_offsets);
        let content_height = prepaint.content_height;

        self.input.update(cx, |input, cx| {
            let height_changed = input.last_content_height != Some(content_height);
            input.last_layout = Some(lines);
            input.last_line_starts = Some(line_starts);
            input.last_line_offsets = Some(line_offsets);
            input.last_line_height = Some(line_height);
            input.last_bounds = Some(bounds);
            input.last_content_height = Some(content_height);
            // 布局更新后重新计算滚动位置，确保光标可见
            input.scroll_to_cursor(cx);
            if height_changed {
                cx.notify();
            }
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let padding_y = px(8.0);
        let min_height = px(20.0) + padding_y * 2.0;
        let content_height = self.last_content_height.unwrap_or(px(20.0)) + padding_y * 2.0;
        let container_height = content_height.max(min_height);

        let final_height = if let Some(max_h) = self.max_height {
            container_height.min(max_h)
        } else {
            container_height
        };

        let needs_scroll = self.max_height.is_some();

        let mut container = div()
            .h(final_height)
            .w_full()
            .px(px(10.0))
            .py(padding_y)
            .rounded(px(8.0))
            .bg(self.bg_color)
            .overflow_y_hidden()
            .child(TextElement { input: cx.entity() });

        if needs_scroll {
            container = container.on_scroll_wheel(cx.listener(Self::on_scroll_wheel));
        }

        div()
            .flex()
            .w_full()
            .debug()
            .key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::insert_newline))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .line_height(px(20.0))
            .text_size(px(14.0))
            .child(container)
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
