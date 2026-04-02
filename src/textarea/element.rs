//! 自定义 Element 渲染实现：文本排版、光标/选区绘制、滚动裁剪。

use std::cmp::{max, min};

use gpui::{
    App, Bounds, Context, CursorStyle, ElementId, ElementInputHandler, Entity, FocusHandle,
    Focusable, GlobalElementId, LayoutId, MouseButton, PaintQuad, Pixels, Style, TextAlign,
    TextRun, UnderlineStyle, Window, div, fill, hsla, point, prelude::*, px, relative, size,
};

use super::TextInput;

/// 文本编辑器的自定义渲染元素，负责将 TextInput 状态绘制到屏幕上。
pub(crate) struct TextElement {
    pub(crate) input: Entity<TextInput>,
}

/// prepaint 阶段的中间状态，缓存排版结果供 paint 阶段使用。
pub(crate) struct PrepaintState {
    /// 经过自动换行后的行列表
    lines: Vec<gpui::WrappedLine>,
    /// 每行在原始文本中的起始字节偏移
    line_starts: Vec<usize>,
    /// 每行在垂直方向上的累计 Y 偏移（像素）
    line_offsets: Vec<Pixels>,
    /// 光标矩形（无选区时）
    cursor: Option<PaintQuad>,
    /// 选区高亮矩形列表（跨行时有多个）
    selections: Vec<PaintQuad>,
    /// 光标闪烁状态：当前是否可见
    cursor_visible: bool,
    /// 垂直滚动偏移量
    scroll_offset: Pixels,
    /// 所有文本内容的总高度
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

    /// 请求布局：设置元素占满父容器的宽高。
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

    /// 预绘制阶段：执行文本排版，计算光标和选区的几何信息。
    /// 这里不实际绘制，只准备 PrepaintState 供 paint 使用。
    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // 读取输入状态的快照
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let cursor_visible = input.cursor_visible;
        let scroll_offset = input.scroll_offset;
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let wrap_width = Some(bounds.size.width);

        // 内容为空时显示半透明的占位文本
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

        // 构建 TextRun 列表：IME 组合输入区域添加下划线样式
        let runs = if !content.is_empty() {
            if let Some(marked_range) = input.marked_range.as_ref() {
                // 将文本分为三段：标记前 / 标记中（带下划线）/ 标记后
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

        // 调用文本排版引擎进行自动换行
        let mut lines = window
            .text_system()
            .shape_text(display_text, font_size, &runs, wrap_width, None)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();

        // 确保至少有一行（空内容时用空格占位，保证光标可见）
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

        // 计算每行在原始文本中的起始字节偏移
        // 按 '\n' 分割，逐行累加字节长度（+1 跳过换行符本身）
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

        // 排版引擎可能因自动换行产生更多行，补齐 line_starts
        if line_starts.len() != lines.len() {
            line_starts.resize(lines.len(), content.len());
        }

        // 计算每行的垂直偏移量（考虑自动换行产生的视觉行数）
        let line_height = window.line_height();
        let mut line_offsets = Vec::with_capacity(lines.len());
        let mut accumulated_y = px(0.0);
        for line in &lines {
            line_offsets.push(accumulated_y);
            accumulated_y += line_height * TextInput::visual_line_count(line) as f32;
        }

        let content_height = accumulated_y;

        let mut selections = Vec::new();

        // 无选区时计算光标矩形，有选区时计算选区高亮矩形
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
            // 遍历选区涉及的每一行，生成对应的高亮矩形
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
                    // 选区在同一视觉行内：单个矩形
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
                    // 选区跨多个视觉行：首行从起点到右边界
                    selections.push(fill(
                        Bounds::from_corners(
                            point(bounds.left() + start_pos.x, top_base + start_pos.y),
                            point(bounds.right(), top_base + start_pos.y + line_height),
                        ),
                        input.selection_color,
                    ));

                    // 中间行：占满整行宽度
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

                    // 末行：从左边界到终点
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

    /// 绘制阶段：将 prepaint 计算好的几何信息实际渲染到屏幕上。
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
        // 注册输入处理器，使该元素可接收 IME 和键盘事件
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        // 先绘制选区高亮（在文本下层）
        for selection in prepaint.selections.drain(..) {
            window.paint_quad(selection);
        }

        // 逐行绘制文本，应用滚动偏移
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

        // 聚焦且闪烁周期为可见时，绘制光标
        if focus_handle.is_focused(window) && prepaint.cursor_visible {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        // 将本帧的排版结果缓存到 TextInput，供编辑操作（光标定位、滚动等）使用
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
            // 内容高度变化时触发重新渲染以更新容器尺寸
            if height_changed {
                cx.notify();
            }
        });
    }
}

/// TextInput 的 Render 实现：构建组件的 DOM 树结构。
impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 根据内容高度动态计算容器尺寸，受 max_height 限制
        let padding_y = px(8.0);
        let min_height = px(20.0) + padding_y * 2.0;
        let content_height = self.last_content_height.unwrap_or(px(20.0)) + padding_y * 2.0;
        let container_height = content_height.max(min_height);

        let final_height = if let Some(max_h) = self.max_height {
            container_height.min(max_h)
        } else {
            container_height
        };

        // 设置了 max_height 时启用滚动
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

        // 外层容器：注册所有键盘/鼠标 action 监听器
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
