/// Demo application showcasing the gpui-editor textarea component.
use gpui::*;
use gpui_editor::textarea::{init, render_textarea, TextInput, Textarea};

struct DemoView {
    textarea: Entity<Textarea>,
}

impl DemoView {
    fn new(cx: &mut Context<Self>) -> Self {
        let textarea = cx.new(|cx| {
            TextInput::new(cx)
                .placeholder("请输入内容...")
                .max_length(500)
                .max_height(px(300.0))
                .bg_color(hsla(0.0, 0.0, 0.137, 1.0))
                .cursor_color(hsla(210.0 / 360.0, 1.0, 0.5, 1.0))
                .text_color(hsla(0.0, 0.0, 0.969, 1.0))
                .selection_color(hsla(250.0 / 360.0, 1.0, 0.5, 0.19))
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
            .bg(rgb(0x1b1b1b))
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
                    .child(render_textarea(&self.textarea)),
            )
    }
}

fn main() {
    Application::new().run(move |cx: &mut App| {
        init(cx);

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
