//! 系统输入法（IME）交互接口实现及 UTF-16 编码转换工具。

use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};

use super::TextInput;

impl TextInput {
    /// 将 UTF-16 代码单元偏移量转换为 UTF-8 字节偏移量。
    pub(crate) fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    /// 将 UTF-8 字节偏移量转换为 UTF-16 代码单元偏移量。
    pub(crate) fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    /// 将 UTF-8 字节范围转换为 UTF-16 范围。
    pub(crate) fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    /// 将 UTF-16 范围转换为 UTF-8 字节范围。
    pub(crate) fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        let new_text = self.clamp_input(new_text, &range);

        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        // 清除旧布局缓存，避免 scroll_to_cursor 使用过期数据
        self.last_layout = None;
        self.last_line_starts = None;
        self.last_line_offsets = None;
        self.reset_blink(cx);
        self.scroll_to_cursor(cx);
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
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        let new_text = self.clamp_input(new_text, &range);

        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        // 清除旧布局缓存，避免 scroll_to_cursor 使用过期数据
        self.last_layout = None;
        self.last_line_starts = None;
        self.last_line_offsets = None;
        self.reset_blink(cx);
        self.scroll_to_cursor(cx);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layouts = self.last_layout.as_ref()?;
        let last_line_starts = self.last_line_starts.as_ref()?;
        let last_line_offsets = self.last_line_offsets.as_ref()?;
        let line_height = self.last_line_height?;
        if last_layouts.is_empty() || last_line_starts.is_empty() || last_line_offsets.is_empty() {
            return None;
        }

        let range = self.range_from_utf16(&range_utf16);

        let start_line = Self::line_index_for_offset(last_line_starts, range.start);
        let end_line = Self::line_index_for_offset(last_line_starts, range.end);

        let start_line_start = last_line_starts[start_line];
        let end_line_start = last_line_starts[end_line];

        let start_pos = last_layouts[start_line]
            .position_for_index(range.start.saturating_sub(start_line_start), line_height)
            .unwrap_or(gpui::point(gpui::px(0.0), gpui::px(0.0)));
        let end_pos = last_layouts[end_line]
            .position_for_index(range.end.saturating_sub(end_line_start), line_height)
            .unwrap_or(gpui::point(gpui::px(0.0), gpui::px(0.0)));

        Some(Bounds::from_corners(
            gpui::point(
                bounds.left() + start_pos.x,
                bounds.top() + last_line_offsets[start_line] + start_pos.y,
            ),
            gpui::point(
                bounds.left() + end_pos.x,
                bounds.top() + last_line_offsets[end_line] + end_pos.y + line_height,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.offset_to_utf16(self.index_for_mouse_position(point)))
    }
}
