//! 多行文本输入组件。
//!
//! 提供基于 gpui 的多行文本编辑器，支持：
//!
//! - 多行文本编辑、自动换行
//! - IME（输入法）支持，兼容中文/日文/韩文等
//! - 键盘快捷键：方向键、Backspace/Delete、Home/End、Cmd+A/C/V/X
//! - 鼠标点击定位光标、拖拽选择、Shift+点击扩展选区
//! - 上下键跨行移动光标（含自动换行的视觉行）
//! - 光标闪烁动画（500ms 间隔）
//! - 内容自适应高度，可设置最大高度并启用滚动
//! - 输入字符数限制（按 Unicode 字素计算）
//! - Enter 键模式切换（Enter 提交/换行）
//! - 完全可定制的颜色主题（背景、光标、文本、选区）
//!
//! # 快速开始
//!
//! ```ignore
//! use gpui_editor::textarea::{TextInput, Textarea, EnterMode, init, render_textarea};
//!
//! // 1. 注册快捷键（在 App 初始化时调用一次）
//! init(cx);
//!
//! // 2. 创建输入实体
//! let textarea = cx.new(|cx| {
//!     TextInput::new(cx)
//!         .placeholder("请输入内容...")
//!         .max_length(500)
//!         .max_height(px(300.0))
//!         .enter_mode(EnterMode::EnterNewline)
//! });
//!
//! // 3. 监听事件
//! cx.subscribe(&textarea, |this, _textarea, event, cx| {
//!     match event {
//!         TextInputEvent::Submit => { /* 提交逻辑 */ }
//!         TextInputEvent::InsertNewline => { /* 换行回调 */ }
//!     }
//! }).detach();
//!
//! // 4. 渲染
//! render_textarea(&textarea)
//! ```

use std::cmp::{max, min};
use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, Hsla, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    ScrollWheelEvent, SharedString, Style, Task, TextAlign, TextRun, Timer, UTF16Selection,
    UnderlineStyle, Window, WrappedLine, actions, div, fill, hsla, point, prelude::*, px, relative,
    size,
};
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Submit,
        InsertNewline,
        Paste,
        Cut,
        Copy,
        Quit,
    ]
);

/// 多行文本输入组件状态。
///
/// `TextInput` 持有编辑器的所有状态，包括文本内容、光标位置、选区、
/// 布局缓存和配置项。通过 builder 模式进行配置。
///
/// # 示例
///
/// ```ignore
/// let textarea = cx.new(|cx| {
///     TextInput::new(cx)
///         .placeholder("请输入...")
///         .max_length(200)
///         .max_height(px(300.0))
///         .enter_mode(EnterMode::EnterNewline)
///         .bg_color(hsla(0.0, 0.0, 0.137, 1.0))
///         .cursor_color(hsla(210.0/360.0, 1.0, 0.5, 1.0))
///         .text_color(hsla(0.0, 0.0, 0.969, 1.0))
///         .selection_color(hsla(250.0/360.0, 1.0, 0.5, 0.19))
/// });
/// ```
pub struct TextInput {
    /// 焦点句柄，用于管理键盘输入焦点
    focus_handle: FocusHandle,
    /// 当前输入的文本内容
    content: SharedString,
    /// 占位文本，输入为空时显示
    placeholder: SharedString,
    /// 当前选区的字节范围（start..end），光标无选区时 start == end
    selected_range: Range<usize>,
    /// 选区方向是否反转（true 表示光标在选区起始端）
    selection_reversed: bool,
    /// IME 输入法标记范围（组合输入未确认的文本区域）
    marked_range: Option<Range<usize>>,
    /// 上一次排版的换行布局结果（每个逻辑行一个 WrappedLine）
    last_layout: Option<Vec<WrappedLine>>,
    /// 每个逻辑行在文本中的起始字节偏移量
    last_line_starts: Option<Vec<usize>>,
    /// 每个逻辑行在垂直方向上的像素偏移量
    last_line_offsets: Option<Vec<Pixels>>,
    /// 单行行高（像素）
    last_line_height: Option<Pixels>,
    /// 上一次绘制时的输入框边界矩形
    last_bounds: Option<Bounds<Pixels>>,
    /// 是否正在通过鼠标拖拽选择文本
    is_selecting: bool,
    /// 光标是否可见（用于闪烁）
    cursor_visible: bool,
    /// 光标闪烁定时任务
    _blink_task: Option<Task<()>>,
    /// 输入框背景色（默认：深灰色 0x232323）
    bg_color: Hsla,
    /// 光标颜色（默认：蓝色）
    cursor_color: Hsla,
    /// 文本颜色（默认：白色）
    text_color: Hsla,
    /// 选中背景色（默认：半透明蓝色 0x3311ff30）
    selection_color: Hsla,
    /// 最大输入字符数（按 Unicode 字素计算），None 表示不限制
    max_length: Option<usize>,
    /// 最大高度（像素），超出后内容可滚动。None 表示无限高度自适应
    max_height: Option<Pixels>,
    /// 当前垂直滚动偏移量
    scroll_offset: Pixels,
    /// 上一次排版计算出的内容总高度
    last_content_height: Option<Pixels>,
    /// Enter 键模式
    mode: EnterMode,
}

/// Enter 键模式。控制 Enter 和 Shift+Enter 的行为。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EnterMode {
    /// Enter 提交，Shift+Enter 换行（默认）
    #[default]
    EnterSubmit,
    /// Enter 换行，Shift+Enter 提交
    EnterNewline,
}

#[derive(Clone, Debug)]
pub enum TextInputEvent {
    /// 提交事件（由 Enter 或 Shift+Enter 触发，取决于 EnterMode）
    Submit,
    /// 换行事件（由 Enter 或 Shift+Enter 触发，取决于 EnterMode）
    InsertNewline,
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl TextInput {
    /// 创建一个基础文本输入组件状态。
    /// ```ignore
    /// let input = TextInput::new(cx);
    /// ```
    /// 返回一个可聚焦、支持键盘输入、选区和剪贴板操作的文本输入状态。
    /// 默认占位文本为 `请输入内容...`。
    /// ```ignore
    /// let input = TextInput::new(cx);
    /// // 默认颜色配置：深灰背景、蓝色光标、白色文本、蓝色选中
    /// ```
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            focus_handle: cx.focus_handle(),
            content: "".into(),
            placeholder: "请输入内容...".into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_line_starts: None,
            last_line_offsets: None,
            last_line_height: None,
            last_bounds: None,
            is_selecting: false,
            cursor_visible: true,
            _blink_task: None,
            bg_color: hsla(0.0, 0.0, 0.137, 1.0), // 深灰色 #232323
            cursor_color: hsla(210.0 / 360.0, 1.0, 0.5, 1.0), // 蓝色 #0099ff
            text_color: hsla(0.0, 0.0, 0.969, 1.0), // 浅白色 #f7f7f7
            selection_color: hsla(250.0 / 360.0, 1.0, 0.5, 0.19), // 半透明蓝 #3311ff30
            max_length: None,
            max_height: None,
            scroll_offset: px(0.0),
            last_content_height: None,
            mode: EnterMode::default(),
        };
        this.start_blink_task(cx);
        this
    }

    /// 设置输入框占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置输入框背景色。支持链式调用配置主题。
    ///
    /// 深色主题示例:
    /// ```ignore
    /// Textarea::new(cx)
    ///     .bg_color(hsla(0.0, 0.0, 0.137, 1.0))         // 深灰 #232323
    ///     .cursor_color(hsla(210.0/360.0, 1.0, 0.5, 1.0))   // 蓝 #0099ff
    ///     .text_color(hsla(0.0, 0.0, 0.969, 1.0))       // 白 #f7f7f7
    ///     .selection_color(hsla(250.0/360.0, 1.0, 0.5, 0.19)) // 半透明蓝
    /// ```
    pub fn bg_color(mut self, color: Hsla) -> Self {
        self.bg_color = color;
        self
    }

    /// 设置光标颜色。
    ///
    /// 常用颜色值:
    /// - 蓝色: hsla(210.0/360.0, 1.0, 0.5, 1.0)
    /// - 绿色: hsla(120.0/360.0, 1.0, 0.6, 1.0)
    /// - 橙色: hsla(30.0/360.0, 1.0, 0.5, 1.0)
    /// - 红色: hsla(0.0, 1.0, 0.5, 1.0)
    pub fn cursor_color(mut self, color: Hsla) -> Self {
        self.cursor_color = color;
        self
    }

    /// 设置文本颜色。
    ///
    /// 常用颜色值:
    /// - 纯白: hsla(0.0, 0.0, 1.0, 1.0)
    /// - 深灰: hsla(0.0, 0.0, 0.2, 1.0)
    /// - 浅绿: hsla(120.0/360.0, 0.8, 0.7, 1.0)
    /// - 黄色: hsla(60.0/360.0, 1.0, 0.5, 1.0)
    pub fn text_color(mut self, color: Hsla) -> Self {
        self.text_color = color;
        self
    }

    /// 设置文本选中背景颜色。通常应使用带透明度的颜色。
    ///
    /// 常用颜色值（建议透明度 0.2-0.4）:
    /// - 半透明蓝: hsla(250.0/360.0, 1.0, 0.5, 0.19) 默认值
    /// - 半透明绿: hsla(120.0/360.0, 1.0, 0.4, 0.3)
    /// - 半透明黄: hsla(60.0/360.0, 1.0, 0.5, 0.25)
    /// - 半透明紫: hsla(270.0/360.0, 1.0, 0.55, 0.4)
    pub fn selection_color(mut self, color: Hsla) -> Self {
        self.selection_color = color;
        self
    }

    /// 设置最大输入字符数（按 Unicode 字素计算）。超出限制的输入将被截断。
    ///
    /// ```ignore
    /// TextInput::new(cx).max_length(100) // 最多输入100个字符
    /// ```
    pub fn max_length(mut self, len: usize) -> Self {
        self.max_length = Some(len);
        self
    }

    /// 设置输入框最大高度（像素）。内容超出后可滚动。
    /// 不设置时输入框高度随内容自适应增长。
    ///
    /// ```ignore
    /// TextInput::new(cx).max_height(px(200.0)) // 最大高度200px
    /// ```
    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = Some(height);
        self
    }

    /// 设置 Enter 键模式。
    ///
    /// - `EnterMode::EnterSubmit`（默认）：Enter 提交，Shift+Enter 换行
    /// - `EnterMode::EnterNewline`：Enter 换行，Shift+Enter 提交
    ///
    /// 提交时触发 `TextInputEvent::Submit`，换行时触发 `TextInputEvent::InsertNewline`。
    pub fn enter_mode(mut self, mode: EnterMode) -> Self {
        self.mode = mode;
        self
    }

    /// 读取当前输入内容。
    pub fn value(&self) -> SharedString {
        self.content.clone()
    }

    /// 清空输入框内容并重置光标位置。
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.content = "".into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
    }

    /// 启动光标闪烁定时任务。
    fn start_blink_task(&mut self, cx: &mut Context<Self>) {
        let blink_interval = Duration::from_millis(500);
        let task = cx.spawn(async move |this: gpui::WeakEntity<TextInput>, cx: &mut gpui::AsyncApp| {
            loop {
                Timer::after(blink_interval).await;
                let Ok(()) = this.update(cx, |view, cx| {
                    view.cursor_visible = !view.cursor_visible;
                    cx.notify();
                }) else {
                    break;
                };
            }
        });
        self._blink_task = Some(task);
    }

    /// 重置光标闪烁（输入/移动后让光标立即可见并重启定时器）。
    fn reset_blink(&mut self, cx: &mut Context<Self>) {
        self.cursor_visible = true;
        self.start_blink_task(cx);
    }

    /// 根据 max_length 限制截断输入文本。
    /// 计算替换后剩余可容纳的字素数量，截断超出部分。
    fn clamp_input(&self, new_text: &str, replace_range: &Range<usize>) -> String {
        let Some(max_len) = self.max_length else {
            return new_text.to_string();
        };
        // 计算替换后保留的文本字素数
        let before = self.content[..replace_range.start].graphemes(true).count();
        let after = self.content[replace_range.end..].graphemes(true).count();
        let existing = before + after;
        if existing >= max_len {
            return String::new();
        }
        let allowed = max_len - existing;
        let new_graphemes: String = new_text.graphemes(true).take(allowed).collect();
        new_graphemes
    }

    /// 处理鼠标滚轮事件，更新滚动偏移。
    fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 只在有 max_height 限制时才滚动
        let Some(max_height) = self.max_height else {
            return;
        };
        let content_height = self.last_content_height.unwrap_or(px(0.0));
        if content_height <= max_height {
            return;
        }
        let line_height = window.line_height();
        let delta = event.delta.pixel_delta(line_height);
        let new_offset = self.scroll_offset - delta.y;
        let max_scroll = content_height - max_height;
        self.scroll_offset = new_offset.max(px(0.0)).min(max_scroll);
        cx.notify();
    }

    /// 确保光标在可视区域内（自动滚动）。
    fn scroll_to_cursor(&mut self, cx: &mut Context<Self>) {
        let Some(max_height) = self.max_height else {
            self.scroll_offset = px(0.0);
            return;
        };
        let (Some(line_starts), Some(line_offsets), Some(lines), Some(line_height)) = (
            self.last_line_starts.as_ref(),
            self.last_line_offsets.as_ref(),
            self.last_layout.as_ref(),
            self.last_line_height,
        ) else {
            return;
        };
        if lines.is_empty() || line_starts.is_empty() || line_offsets.is_empty() {
            return;
        }
        let cursor = self.cursor_offset();
        let line_index = Self::line_index_for_offset(line_starts, cursor);
        let line_start = line_starts[line_index];
        let cursor_pos = lines[line_index]
            .position_for_index(cursor.saturating_sub(line_start), line_height)
            .unwrap_or(point(px(0.0), px(0.0)));
        let cursor_top = line_offsets[line_index] + cursor_pos.y;
        let cursor_bottom = cursor_top + line_height;

        if cursor_top < self.scroll_offset {
            self.scroll_offset = cursor_top;
            cx.notify();
        } else if cursor_bottom > self.scroll_offset + max_height {
            self.scroll_offset = cursor_bottom - max_height;
            cx.notify();
        }
    }

    /// 向左移动光标。 如果有选区，则移动到选区起始位置。
    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    /// 向右移动光标。 如果有选区，则移动到选区结束位置。
    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    /// 向上移动光标。移动到上一视觉行的相同 x 位置。
    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.offset_for_vertical_move(-1) {
            self.move_to(offset, cx);
        }
    }

    /// 向下移动光标。移动到下一视觉行的相同 x 位置。
    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.offset_for_vertical_move(1) {
            self.move_to(offset, cx);
        }
    }

    /// 向左选择文本。 如果没有选区，则从光标当前位置开始选择；如果已有选区，则扩展选区到左边界。
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    /// 向右选择文本。 如果没有选区，则从光标当前位置开始选择；如果已有选区，则扩展选区到右边界。
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    /// 向上选择文本。
    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.offset_for_vertical_move(-1) {
            self.select_to(offset, cx);
        }
    }

    /// 向下选择文本。
    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.offset_for_vertical_move(1) {
            self.select_to(offset, cx);
        }
    }

    /// 选择所有文本。
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    /// 移动光标到行首。
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    /// 移动光标到行尾。
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    /// 删除光标前的字符。
    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    /// 删除光标后的字符。
    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    /// 鼠标按下时，开始选择文本。
    fn on_mouse_down(
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
    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    /// 鼠标移动时，更新选择文本。
    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    /// 显示字符面板。
    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    /// 提交输入内容（由 Enter 键触发）。
    /// 根据 mode 决定是提交还是换行。
    fn submit(&mut self, _: &Submit, window: &mut Window, cx: &mut Context<Self>) {
        match self.mode {
            EnterMode::EnterSubmit => cx.emit(TextInputEvent::Submit),
            EnterMode::EnterNewline => {
                self.replace_text_in_range(None, "\n", window, cx);
                cx.emit(TextInputEvent::InsertNewline);
            }
        }
    }

    /// 插入换行符（由 Shift+Enter 触发）。
    /// 根据 mode 决定是换行还是提交。
    fn insert_newline(&mut self, _: &InsertNewline, window: &mut Window, cx: &mut Context<Self>) {
        match self.mode {
            EnterMode::EnterSubmit => {
                self.replace_text_in_range(None, "\n", window, cx);
                cx.emit(TextInputEvent::InsertNewline);
            }
            EnterMode::EnterNewline => cx.emit(TextInputEvent::Submit),
        }
    }

    /// 粘贴内容。
    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    /// 复制内容。
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    /// 剪切内容。
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    /// 移动光标到指定位置。
    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.reset_blink(cx);
        self.scroll_to_cursor(cx);
        cx.notify()
    }

    /// 光标当前位置在 selected_range 的哪个边界，取决于 selection_reversed 标志。
    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    /// 鼠标位置对应的文本索引。根据最后一次布局计算文本行和位置，返回对应的文本索引。
    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(lines), Some(line_starts), Some(line_offsets), Some(line_height)) = (
            self.last_bounds.as_ref(),
            self.last_layout.as_ref(),
            self.last_line_starts.as_ref(),
            self.last_line_offsets.as_ref(),
            self.last_line_height,
        ) else {
            return 0;
        };
        if lines.is_empty() || line_starts.is_empty() || line_offsets.is_empty() {
            return 0;
        }
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }

        let y = position.y - bounds.top() + self.scroll_offset;
        let line_index = line_offsets
            .iter()
            .enumerate()
            .find_map(|(ix, line_offset)| {
                let line_bottom =
                    *line_offset + line_height * Self::visual_line_count(&lines[ix]) as f32;
                (y < line_bottom).then_some(ix)
            })
            .unwrap_or(lines.len().saturating_sub(1));

        let line = &lines[line_index];
        let line_start = line_starts[line_index];
        let local_y = y - line_offsets[line_index];
        let local_index = line
            .closest_index_for_position(point(position.x - bounds.left(), local_y), line_height)
            .unwrap_or_else(|index| index);

        (line_start + local_index).min(self.content.len())
    }

    /// 选择到指定位置。根据 selection_reversed 标志更新 selected_range 的起始或结束位置，并确保 start <= end。
    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    /// 将 UTF-8 字节偏移量转换为 UTF-16 代码单元偏移量。
    /// 用于和系统 IME 接口交互（macOS IME 使用 UTF-16 编码）。
    fn offset_from_utf16(&self, offset: usize) -> usize {
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
    fn offset_to_utf16(&self, offset: usize) -> usize {
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
    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    /// 将 UTF-16 范围转换为 UTF-8 字节范围。
    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    /// 查找指定偏移量之前最近的字素边界位置。
    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    /// 查找指定偏移量之后最近的字素边界位置。
    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    /// 根据文本偏移量查找所在的逻辑行索引。
    fn line_index_for_offset(line_starts: &[usize], offset: usize) -> usize {
        line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
    }

    /// 计算一个逻辑行包含的视觉行数（含自动换行产生的行）。
    fn visual_line_count(line: &WrappedLine) -> usize {
        line.wrap_boundaries().len() + 1
    }

    /// 计算垂直移动后的文本偏移量。
    /// `direction`: -1 表示上移，1 表示下移。
    /// 利用布局信息找到光标当前视觉位置，移动到上/下一行同一 x 坐标处。
    fn offset_for_vertical_move(&self, direction: i32) -> Option<usize> {
        let lines = self.last_layout.as_ref()?;
        let line_starts = self.last_line_starts.as_ref()?;
        let line_offsets = self.last_line_offsets.as_ref()?;
        let line_height = self.last_line_height?;
        if lines.is_empty() || line_starts.is_empty() || line_offsets.is_empty() {
            return None;
        }

        let cursor = self.cursor_offset();
        let line_index = Self::line_index_for_offset(line_starts, cursor);
        let line_start = line_starts[line_index];
        let cursor_pos = lines[line_index]
            .position_for_index(cursor.saturating_sub(line_start), line_height)
            .unwrap_or(point(px(0.0), px(0.0)));

        // 当前光标在全局坐标中的 y 和 x
        let current_y = line_offsets[line_index] + cursor_pos.y;
        let current_x = cursor_pos.x;

        // 目标行的 y
        let target_y = if direction < 0 {
            // 上移：移到上一行（当前行顶部上方半个行高）
            if current_y < line_height {
                return Some(0); // 已在第一行，移到开头
            }
            current_y - line_height
        } else {
            // 下移：移到下一行
            let last_line_idx = lines.len() - 1;
            let last_line_bottom = line_offsets[last_line_idx]
                + line_height * Self::visual_line_count(&lines[last_line_idx]) as f32;
            let next_y = current_y + line_height;
            if next_y >= last_line_bottom {
                return Some(self.content.len()); // 已在最后一行，移到末尾
            }
            next_y
        };

        // 找到目标 y 所在的逻辑行
        let target_line_index = line_offsets
            .iter()
            .enumerate()
            .rev()
            .find_map(|(ix, offset)| {
                if target_y >= *offset {
                    Some(ix)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        let target_line_start = line_starts[target_line_index];
        let local_y = target_y - line_offsets[target_line_index];

        let local_index = lines[target_line_index]
            .closest_index_for_position(point(current_x, local_y), line_height)
            .unwrap_or_else(|index| index);

        Some((target_line_start + local_index).min(self.content.len()))
    }
}

/// `TextInput` 的类型别名，方便在语义上区分多行输入场景。
pub type Textarea = TextInput;

/// 初始化文本输入相关的全局快捷键绑定。
/// ```ignore
/// input::init(cx);
/// ```
/// 会注册文本编辑常用按键，包括：
/// - `backspace` / `delete`
/// - `enter`
/// - `shift-enter`
/// - `left` / `right`
/// - `shift-left` / `shift-right`
/// - `cmd-a` / `cmd-c` / `cmd-v` / `cmd-x`
/// - `home` / `end`
/// - `ctrl-cmd-space`
pub fn init(cx: &mut App) {
    // 绑定文本输入相关的快捷键到 `TextInput` 组件。按键事件会被 `TextInput` 实例捕获并调用对应的方法处理输入逻辑。
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, None),
        KeyBinding::new("delete", Delete, None),
        KeyBinding::new("enter", Submit, None),
        KeyBinding::new("shift-enter", InsertNewline, None),
        KeyBinding::new("left", Left, None),
        KeyBinding::new("right", Right, None),
        KeyBinding::new("up", Up, None),
        KeyBinding::new("down", Down, None),
        KeyBinding::new("shift-left", SelectLeft, None),
        KeyBinding::new("shift-right", SelectRight, None),
        KeyBinding::new("shift-up", SelectUp, None),
        KeyBinding::new("shift-down", SelectDown, None),
        KeyBinding::new("cmd-a", SelectAll, None),
        KeyBinding::new("cmd-v", Paste, None),
        KeyBinding::new("cmd-c", Copy, None),
        KeyBinding::new("cmd-x", Cut, None),
        KeyBinding::new("home", Home, None),
        KeyBinding::new("end", End, None),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, None),
    ]);
}

/// 渲染一个 `Textarea` 输入实体。
/// ```ignore
/// let element = render_textarea(&textarea);
/// ```
/// - `textarea`: 文本输入实体句柄
/// 返回一个可直接挂载到布局中的输入组件元素。
/// 该函数本身不创建状态，只负责把已有实体转成可渲染元素。
pub fn render_textarea(textarea: &Entity<Textarea>) -> impl IntoElement {
    textarea.clone()
}

impl EntityInputHandler for TextInput {
    /// 获取指定范围的文本。
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
            .unwrap_or(point(px(0.0), px(0.0)));
        let end_pos = last_layouts[end_line]
            .position_for_index(range.end.saturating_sub(end_line_start), line_height)
            .unwrap_or(point(px(0.0), px(0.0)));

        Some(Bounds::from_corners(
            point(
                bounds.left() + start_pos.x,
                bounds.top() + last_line_offsets[start_line] + start_pos.y,
            ),
            point(
                bounds.left() + end_pos.x,
                bounds.top() + last_line_offsets[end_line] + end_pos.y + line_height,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.offset_to_utf16(self.index_for_mouse_position(point)))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    lines: Vec<WrappedLine>,
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
            // 占位符文本：使用配置文本颜色的50%透明度
            let Hsla { h, s, l, a } = input.text_color;
            (input.placeholder.clone(), hsla(h, s, l, a * 0.5))
        } else {
            // 正常文本：使用配置的文本颜色
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

        // 计算内容总高度
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
            if height_changed {
                cx.notify();
            }
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 计算内容区高度：padding_y(8px) * 2 + 内容高度，最低一行高度
        let padding_y = px(8.0);
        let min_height = px(20.0) + padding_y * 2.0; // 至少一行
        let content_height = self.last_content_height.unwrap_or(px(20.0)) + padding_y * 2.0;
        let container_height = content_height.max(min_height);

        // 如果设置了 max_height，限制高度并启用滚动
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
