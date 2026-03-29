/// Demo application showcasing the gpui-editor textarea component.
///
/// Displays a window with a textarea that supports multi-line editing,
/// keyboard shortcuts, mouse selection, and custom styling.
use gpui::*;
use gpui_editor::style::TextareaStyle;
use gpui_editor::textarea::Textarea;

/// Root view wrapping the textarea.
struct DemoView {
    textarea: Entity<Textarea>,
}

impl DemoView {
    fn new(cx: &mut Context<Self>) -> Self {
        let textarea = cx.new(|cx| {
            let mut ta = Textarea::new(cx);
            ta.set_text("Hello, gpui-editor!\n\nThis is a multi-line textarea.\nTry editing, selecting, and using keyboard shortcuts.\n\nSupported shortcuts:\n  - Cmd+A: Select all\n  - Cmd+C: Copy\n  - Cmd+X: Cut\n  - Cmd+V: Paste\n  - Cmd+Z: Undo (todo)\n  - Arrow keys: Move cursor\n  - Shift+Arrow: Extend selection\n  - Alt+Left/Right: Word movement\n  - Home/End: Line start/end\n  - Double-click: Select word\n  - Triple-click: Select line");
            ta.set_placeholder("Type something here...");

            // Use dark style.
            ta.set_style(TextareaStyle::dark());
            ta
        });

        Self { textarea }
    }
}

impl Render for DemoView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(hsla(0.0, 0.0, 0.08, 1.0))
            .p(px(24.0))
            .gap(px(16.0))
            .child(
                div()
                    .text_color(hsla(0.0, 0.0, 0.9, 1.0))
                    .text_size(px(18.0))
                    .child("gpui-editor Demo"),
            )
            .child(
                div()
                    .flex_1()
                    .border_1()
                    .border_color(hsla(0.0, 0.0, 0.3, 1.0))
                    .rounded(px(8.0))
                    .overflow_hidden()
                    .child(self.textarea.clone()),
            )
    }
}

fn main() {
    Application::new().run(move |cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(800.0), px(600.0)),
                    cx,
                ))),
                ..Default::default()
            },
            |_window, cx| cx.new(|cx| DemoView::new(cx)),
        )
        .unwrap();
    });
}
