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
//! ```rust
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
//!         .enter_mode(EnterMode::Enter)
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

mod editing;
mod element;
mod input_handler;

use std::ops::Range;

use gpui::{
    actions, App, Bounds, Context, Entity, EventEmitter, FocusHandle, Hsla, KeyBinding, Pixels,
    Point, SharedString, Task, Timer, WrappedLine, hsla, point, prelude::*, px,
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
/// ```rust
/// let textarea = cx.new(|cx| {
///     TextInput::new(cx)
///         .placeholder("请输入...")
///         .max_length(200)
///         .max_height(px(300.0))
///         .enter_mode(EnterMode::Enter)
///         .bg_color(hsla(0.0, 0.0, 0.137, 1.0))
///         .cursor_color(hsla(210.0/360.0, 1.0, 0.5, 1.0))
///         .text_color(hsla(0.0, 0.0, 0.969, 1.0))
///         .selection_color(hsla(250.0/360.0, 1.0, 0.5, 0.19))
/// });
/// ```
pub struct TextInput {
    /// 焦点句柄，用于管理键盘输入焦点
    pub(crate) focus_handle: FocusHandle,
    /// 当前输入的文本内容
    pub(crate) content: SharedString,
    /// 占位文本，输入为空时显示
    pub(crate) placeholder: SharedString,
    /// 当前选区的字节范围（start..end），光标无选区时 start == end
    pub(crate) selected_range: Range<usize>,
    /// 选区方向是否反转（true 表示光标在选区起始端）
    pub(crate) selection_reversed: bool,
    /// IME 输入法标记范围（组合输入未确认的文本区域）
    pub(crate) marked_range: Option<Range<usize>>,
    /// 上一次排版的换行布局结果（每个逻辑行一个 WrappedLine）
    pub(crate) last_layout: Option<Vec<WrappedLine>>,
    /// 每个逻辑行在文本中的起始字节偏移量
    pub(crate) last_line_starts: Option<Vec<usize>>,
    /// 每个逻辑行在垂直方向上的像素偏移量
    pub(crate) last_line_offsets: Option<Vec<Pixels>>,
    /// 单行行高（像素）
    pub(crate) last_line_height: Option<Pixels>,
    /// 上一次绘制时的输入框边界矩形
    pub(crate) last_bounds: Option<Bounds<Pixels>>,
    /// 是否正在通过鼠标拖拽选择文本
    pub(crate) is_selecting: bool,
    /// 光标是否可见（用于闪烁）
    pub(crate) cursor_visible: bool,
    /// 光标闪烁定时任务
    pub(crate) _blink_task: Option<Task<()>>,
    /// 输入框背景色（默认：深灰色 0x232323）
    pub(crate) bg_color: Hsla,
    /// 光标颜色（默认：蓝色）
    pub(crate) cursor_color: Hsla,
    /// 文本颜色（默认：白色）
    pub(crate) text_color: Hsla,
    /// 选中背景色（默认：半透明蓝色 0x3311ff30）
    pub(crate) selection_color: Hsla,
    /// 最大输入字符数（按 Unicode 字素计算），None 表示不限制
    pub(crate) max_length: Option<usize>,
    /// 最大高度（像素），超出后内容可滚动。None 表示无限高度自适应
    pub(crate) max_height: Option<Pixels>,
    /// 当前垂直滚动偏移量
    pub(crate) scroll_offset: Pixels,
    /// 上一次排版计算出的内容总高度
    pub(crate) last_content_height: Option<Pixels>,
    /// Enter 键模式
    pub(crate) mode: EnterMode,
    /// 可见文本行数，控制控件的最小高度。默认为 2。
    pub(crate) rows: usize,
}

/// Enter 键模式。控制 Enter 和 Shift+Enter 的行为。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EnterMode {
    /// Enter 插入换行，Shift+Enter 触发提交
    Enter,
    /// Enter 触发提交，Shift+Enter 插入换行（默认）
    #[default]
    ShiftEnter,
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
            bg_color: hsla(0.0, 0.0, 0.137, 1.0),
            cursor_color: hsla(210.0 / 360.0, 1.0, 0.5, 1.0),
            text_color: hsla(0.0, 0.0, 0.969, 1.0),
            selection_color: hsla(250.0 / 360.0, 1.0, 0.5, 0.19),
            max_length: None,
            max_height: None,
            scroll_offset: px(0.0),
            last_content_height: None,
            mode: EnterMode::default(),
            rows: 2,
        };
        this.start_blink_task(cx);
        this
    }

    /// 设置输入框占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置输入框背景色。
    pub fn bg_color(mut self, color: Hsla) -> Self {
        self.bg_color = color;
        self
    }

    /// 设置光标颜色。
    pub fn cursor_color(mut self, color: Hsla) -> Self {
        self.cursor_color = color;
        self
    }

    /// 设置文本颜色。
    pub fn text_color(mut self, color: Hsla) -> Self {
        self.text_color = color;
        self
    }

    /// 设置文本选中背景颜色。通常应使用带透明度的颜色。
    pub fn selection_color(mut self, color: Hsla) -> Self {
        self.selection_color = color;
        self
    }

    /// 设置最大输入字符数（按 Unicode 字素计算）。超出限制的输入将被截断。
    pub fn max_length(mut self, len: usize) -> Self {
        self.max_length = Some(len);
        self
    }

    /// 设置输入框最大高度（像素）。内容超出后可滚动。
    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = Some(height);
        self
    }

    /// 设置 Enter 键模式。
    pub fn enter_mode(mut self, mode: EnterMode) -> Self {
        self.mode = mode;
        self
    }

    /// 设置可见文本行数，控制控件的最小高度。值必须为正整数，默认为 2。
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
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
    pub(crate) fn start_blink_task(&mut self, cx: &mut Context<Self>) {
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
    pub(crate) fn reset_blink(&mut self, cx: &mut Context<Self>) {
        self.cursor_visible = true;
        self.start_blink_task(cx);
    }

    /// 根据 max_length 限制截断输入文本。
    pub(crate) fn clamp_input(&self, new_text: &str, replace_range: &Range<usize>) -> String {
        let Some(max_len) = self.max_length else {
            return new_text.to_string();
        };
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

    /// 确保光标在可视区域内（自动滚动）。
    pub(crate) fn scroll_to_cursor(&mut self, cx: &mut Context<Self>) {
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

    /// 移动光标到指定位置。
    pub(crate) fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.reset_blink(cx);
        self.scroll_to_cursor(cx);
        cx.notify()
    }

    /// 光标当前位置在 selected_range 的哪个边界，取决于 selection_reversed 标志。
    pub(crate) fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    /// 鼠标位置对应的文本索引。
    pub(crate) fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
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

    /// 选择到指定位置。
    pub(crate) fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
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

    /// 查找指定偏移量之前最近的字素边界位置。
    pub(crate) fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    /// 查找指定偏移量之后最近的字素边界位置。
    pub(crate) fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    /// 根据文本偏移量查找所在的逻辑行索引。
    pub(crate) fn line_index_for_offset(line_starts: &[usize], offset: usize) -> usize {
        line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
    }

    /// 计算一个逻辑行包含的视觉行数（含自动换行产生的行）。
    pub(crate) fn visual_line_count(line: &WrappedLine) -> usize {
        line.wrap_boundaries().len() + 1
    }

    /// 计算垂直移动后的文本偏移量。
    /// `direction`: -1 表示上移，1 表示下移。
    pub(crate) fn offset_for_vertical_move(&self, direction: i32) -> Option<usize> {
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

        let current_y = line_offsets[line_index] + cursor_pos.y;
        let current_x = cursor_pos.x;

        let target_y = if direction < 0 {
            if current_y < line_height {
                return Some(0);
            }
            current_y - line_height
        } else {
            let last_line_idx = lines.len() - 1;
            let last_line_bottom = line_offsets[last_line_idx]
                + line_height * Self::visual_line_count(&lines[last_line_idx]) as f32;
            let next_y = current_y + line_height;
            if next_y >= last_line_bottom {
                return Some(self.content.len());
            }
            next_y
        };

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
pub fn init(cx: &mut App) {
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
pub fn render_textarea(textarea: &Entity<Textarea>) -> impl IntoElement {
    textarea.clone()
}
