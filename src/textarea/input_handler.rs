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

/// 实现 gpui 的 EntityInputHandler trait，处理系统输入法（IME）与文本编辑器之间的交互。
/// 所有与系统的交互使用 UTF-16 编码，内部存储使用 UTF-8，需要在两者之间转换。
impl EntityInputHandler for TextInput {
    /// 返回指定 UTF-16 范围内的文本内容，供输入法查询上下文使用。
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

    /// 返回当前选区的 UTF-16 范围，供输入法定位候选窗口位置。
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

    /// 返回当前 IME 组合文本（marked text）的 UTF-16 范围。
    /// 组合文本是用户正在通过输入法输入但尚未确认的文本（如拼音输入中的下划线部分）。
    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    /// 清除 IME 组合标记，表示输入法已确认当前组合文本。
    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    /// 替换指定范围的文本（已确认的输入）。
    /// 替换范围优先级：显式指定范围 > IME 组合范围 > 当前选区。
    /// 用于普通字符输入、粘贴、IME 确认输入等场景。
    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 确定替换范围：显式范围 → marked 范围 → 选区范围
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        // 截断超出最大长度限制的输入
        let new_text = self.clamp_input(new_text, &range);

        // 拼接新内容：前段 + 新文本 + 后段
        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        // 光标移到新文本末尾，清除 IME 组合标记
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

    /// 替换文本并设置新的 IME 组合标记（marked text）。
    /// 用于输入法组合过程中实时更新候选文本（如拼音输入时显示的临时字符）。
    /// 与 replace_text_in_range 不同，此方法会保留 marked 状态表示文本尚未最终确认。
    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 确定替换范围：显式范围 → marked 范围 → 选区范围
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        let new_text = self.clamp_input(new_text, &range);

        // 拼接新内容
        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        // 有组合文本时设置 marked 范围，空文本时清除标记
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        // 更新选区：使用输入法指定的选区范围，或默认移到新文本末尾
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

    /// 返回指定 UTF-16 范围对应的屏幕矩形区域。
    /// 系统输入法用此信息定位候选词窗口的显示位置。
    /// 需要依赖上一帧的布局缓存（行布局、行起始偏移等）来计算。
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

        // 根据字节偏移找到起止行号及各行的起始字节偏移
        let start_line = Self::line_index_for_offset(last_line_starts, range.start);
        let end_line = Self::line_index_for_offset(last_line_starts, range.end);

        let start_line_start = last_line_starts[start_line];
        let end_line_start = last_line_starts[end_line];

        // 通过行内布局计算起止位置的像素坐标
        let start_pos = last_layouts[start_line]
            .position_for_index(range.start.saturating_sub(start_line_start), line_height)
            .unwrap_or(gpui::point(gpui::px(0.0), gpui::px(0.0)));
        let end_pos = last_layouts[end_line]
            .position_for_index(range.end.saturating_sub(end_line_start), line_height)
            .unwrap_or(gpui::point(gpui::px(0.0), gpui::px(0.0)));

        // 组合为绝对坐标矩形：加上容器偏移和行的纵向偏移
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

    /// 将屏幕坐标转换为 UTF-16 字符索引，供输入法通过点击位置查询文本位置。
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.offset_to_utf16(self.index_for_mouse_position(point)))
    }
}
