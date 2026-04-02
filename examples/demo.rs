/// Demo application showcasing the gpui-editor textarea component.
use gpui::*;
use gpui_editor::textarea::{Copy, Cut, EnterMode, Paste, Quit, SelectAll, TextInput, Textarea, init, render_textarea};

// Define menu actions
actions!(
    demo,
    [About, Hide, HideOthers, ShowAll, Minimize, Zoom, CloseWindow, Undo, Redo]
);

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
                .enter_mode(EnterMode::EnterNewline)
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

fn open_main_window(cx: &mut App) {
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
}

fn main() {
    let app = Application::new();

    app.on_reopen(|cx| {
        if let Some(window) = cx.active_window() {
            window
                .update(cx, |_root, window, _cx| {
                    window.activate_window();
                })
                .ok();
        } else if cx.windows().is_empty() {
            open_main_window(cx);
        } else if let Some(window) = cx.windows().first().copied() {
            window
                .update(cx, |_root, window, _cx| {
                    window.activate_window();
                })
                .ok();
        }
    });

    app.run(move |cx: &mut App| {
        init(cx);

        // Register global action handlers
        cx.on_action(|_: &About, _cx| {
            // About dialog handled by OS on macOS
        });
        cx.on_action(|_: &Hide, cx| {
            cx.hide();
        });
        cx.on_action(|_: &HideOthers, cx| {
            cx.hide_other_apps();
        });
        cx.on_action(|_: &ShowAll, cx| {
            cx.unhide_other_apps();
        });
        cx.on_action(|_: &Quit, cx| {
            cx.quit();
        });

        cx.set_menus(vec![
            Menu {
                name: "gpui-editor".into(),
                items: vec![
                    MenuItem::action("About gpui-editor", About),
                    MenuItem::separator(),
                    MenuItem::os_submenu("Services", SystemMenuType::Services),
                    MenuItem::separator(),
                    MenuItem::action("Hide gpui-editor", Hide),
                    MenuItem::action("Hide Others", HideOthers),
                    MenuItem::action("Show All", ShowAll),
                    MenuItem::separator(),
                    MenuItem::action("Quit gpui-editor", Quit),
                ],
            },
            Menu {
                name: "File".into(),
                items: vec![
                    MenuItem::action("Close Window", CloseWindow),
                ],
            },
            Menu {
                name: "Edit".into(),
                items: vec![
                    MenuItem::os_action("Undo", Undo, OsAction::Undo),
                    MenuItem::os_action("Redo", Redo, OsAction::Redo),
                    MenuItem::separator(),
                    MenuItem::os_action("Cut", Cut, OsAction::Cut),
                    MenuItem::os_action("Copy", Copy, OsAction::Copy),
                    MenuItem::os_action("Paste", Paste, OsAction::Paste),
                    MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
                ],
            },
            Menu {
                name: "Window".into(),
                items: vec![
                    MenuItem::action("Minimize", Minimize),
                    MenuItem::action("Zoom", Zoom),
                ],
            },
        ]);

        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-h", Hide, None),
            KeyBinding::new("alt-cmd-h", HideOthers, None),
            KeyBinding::new("cmd-w", CloseWindow, None),
            KeyBinding::new("cmd-m", Minimize, None),
            KeyBinding::new("cmd-z", Undo, None),
            KeyBinding::new("shift-cmd-z", Redo, None),
        ]);

        open_main_window(cx);
    });
}
