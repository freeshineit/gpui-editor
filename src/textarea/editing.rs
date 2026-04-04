//! 文本编辑操作：光标移动、选择、删除、剪贴板、提交和鼠标/滚轮事件处理。

use gpui::{ClipboardItem, Context, EntityInputHandler, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, ScrollWheelEvent, Window};

use super::{
    Backspace, Copy, Cut, Delete, Down, End, EnterMode, Home, InsertNewline, Left, Paste, Right,
    SelectAll, SelectDown, SelectLeft, SelectRight, SelectUp, ShowCharacterPalette, Submit,
    TextInput, TextInputEvent, Up,
};

impl TextInput {
    /// 向左移动光标。如果有选区，则移动到选区起始位置。
    pub(crate) fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    /// 向右移动光标。如果有选区，则移动到选区结束位置。
    pub(crate) fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    /// 向上移动光标。移动到上一视觉行的相同 x 位置。
    pub(crate) fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.offset_for_vertical_move(-1) {
            self.move_to(offset, cx);
        }
    }

    /// 向下移动光标。移动到下一视觉行的相同 x 位置。
    pub(crate) fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.offset_for_vertical_move(1) {
            self.move_to(offset, cx);
        }
    }

    /// 向左选择文本。
    pub(crate) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    /// 向右选择文本。
    pub(crate) fn select_right(
        &mut self,
        _: &SelectRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    /// 向上选择文本。
    pub(crate) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.offset_for_vertical_move(-1) {
            self.select_to(offset, cx);
        }
    }

    /// 向下选择文本。
    pub(crate) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.offset_for_vertical_move(1) {
            self.select_to(offset, cx);
        }
    }

    /// 选择所有文本。
    pub(crate) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    /// 移动光标到行首。
    pub(crate) fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    /// 移动光标到行尾。
    pub(crate) fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    /// 删除光标前的字符。
    pub(crate) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    /// 删除光标后的字符。
    pub(crate) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    /// 提交输入内容（由 Enter 键触发）。
    pub(crate) fn submit(&mut self, _: &Submit, window: &mut Window, cx: &mut Context<Self>) {
        match self.mode {
            EnterMode::ShiftEnter => cx.emit(TextInputEvent::Submit),
            EnterMode::Enter => {
                cx.emit(TextInputEvent::InsertNewline);
                self.replace_text_in_range(None, "\n", window, cx);
            }
        }
    }

    /// 插入换行符（由 Shift+Enter 触发）。
    pub(crate) fn insert_newline(
        &mut self,
        _: &InsertNewline,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.mode {
            EnterMode::ShiftEnter => {
                cx.emit(TextInputEvent::InsertNewline);
                self.replace_text_in_range(None, "\n", window, cx);
            }
            EnterMode::Enter => cx.emit(TextInputEvent::Submit),
        }
    }

    /// 粘贴内容。
    pub(crate) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    /// 复制内容。
    pub(crate) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    /// 剪切内容。
    pub(crate) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    /// 显示字符面板。
    pub(crate) fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    /// 鼠标按下时，开始选择文本。
    pub(crate) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    /// 鼠标抬起时，结束选择文本。
    pub(crate) fn on_mouse_up(
        &mut self,
        _: &MouseUpEvent,
        _window: &mut Window,
        _: &mut Context<Self>,
    ) {
        self.is_selecting = false;
    }

    /// 鼠标移动时，更新选择文本。
    pub(crate) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    /// 处理鼠标滚轮事件，更新滚动偏移。
    pub(crate) fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(max_height) = self.max_height else {
            return;
        };
        let content_height = self.last_content_height.unwrap_or(Pixels::ZERO);
        if content_height <= max_height {
            return;
        }
        let line_height = window.line_height();
        let delta = event.delta.pixel_delta(line_height);
        let new_offset = self.scroll_offset - delta.y;
        let max_scroll = content_height - max_height;
        self.scroll_offset = new_offset.max(Pixels::ZERO).min(max_scroll);
        cx.notify();
    }
}
