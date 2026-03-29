# gpui-editor

A multi-line textarea library built with [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui).

## Features

- **Multi-line editing** — insert, delete, replace across lines
- **Keyboard shortcuts** — HTML textarea-like behavior (see table below)
- **Mouse interaction** — click to position cursor, drag to select, double-click to select word, triple-click to select line
- **Custom cursor** — configurable color and width
- **Selection styling** — custom selection background and text color
- **Theming** — fully customizable background, text color, font family, font size, border, corner radius; includes a dark theme preset
- **Placeholder text** — displayed when the buffer is empty
- **Read-only mode**
- **Unicode support** — correct grapheme cluster handling for CJK, emoji, etc.

## Requirements

- **Rust** ≥ 1.70
- **macOS** (gpui currently targets macOS)

## Getting Started

### Install dependencies

```bash
cargo build
```

### Run the demo

```bash
cargo run --example demo
# or
./scripts/demo.sh
```

### Run tests

```bash
cargo test
# or
./scripts/test.sh
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
gpui-editor = { path = "." }
```

Create a textarea in a gpui view:

```rust
use gpui::*;
use gpui_editor::textarea::Textarea;
use gpui_editor::style::TextareaStyle;

struct MyView {
    textarea: Entity<Textarea>,
}

impl MyView {
    fn new(cx: &mut Context<Self>) -> Self {
        let textarea = cx.new(|cx| {
            let mut ta = Textarea::new(cx);
            ta.set_text("Hello, world!");
            ta.set_placeholder("Type here...");
            ta.set_style(TextareaStyle::dark());
            ta
        });
        Self { textarea }
    }
}

impl Render for MyView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.textarea.clone())
    }
}
```

### Listen for changes

```rust
cx.subscribe(&textarea, |_view, _textarea, event: &TextareaEvent, _cx| {
    match event {
        TextareaEvent::Changed(text) => {
            println!("Text changed: {text}");
        }
    }
}).detach();
```

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `← / →` | Move cursor left / right |
| `↑ / ↓` | Move cursor up / down |
| `Shift + Arrow` | Extend selection |
| `Cmd + ← / →` | Move to line start / end |
| `Cmd + ↑ / ↓` | Move to buffer start / end |
| `Alt + ← / →` | Move by word |
| `Shift + Alt + ← / →` | Select by word |
| `Home / End` | Line start / end |
| `Ctrl + Home / End` | Buffer start / end |
| `Backspace` | Delete character before cursor |
| `Alt + Backspace` | Delete word before cursor |
| `Delete` | Delete character after cursor |
| `Enter` | Insert newline |
| `Tab` | Insert 4 spaces |
| `Cmd + A` | Select all |
| `Cmd + C` | Copy |
| `Cmd + X` | Cut |
| `Cmd + V` | Paste |

## Styling

### Default (light) theme

```rust
let style = TextareaStyle::default();
```

### Dark theme

```rust
let style = TextareaStyle::dark();
```

### Custom style

```rust
use gpui::hsla;
use gpui_editor::TextareaStyle;

let mut style = TextareaStyle::default();
style.background = hsla(0.0, 0.0, 0.12, 1.0);
style.text_color = hsla(0.0, 0.0, 0.9, 1.0);
style.cursor_color = hsla(0.15, 1.0, 0.5, 1.0);
style.font_family = "JetBrains Mono".to_string();
style.font_size = 16.0;
style.line_height = 24.0;
```

## Project Structure

```
gpui-editor/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs          # Library root, re-exports
│   ├── buffer.rs       # TextBuffer — line-based text storage & editing
│   ├── cursor.rs       # Cursor — caret position & selection
│   ├── style.rs        # TextareaStyle — colors, fonts, theming
│   └── textarea.rs     # Textarea — gpui Render component
├── examples/
│   └── demo.rs         # Demo application
├── tests/
│   └── integration_test.rs
└── scripts/
    ├── build.sh        # cargo build
    ├── test.sh         # cargo test
    ├── fmt.sh          # cargo fmt
    ├── lint.sh         # cargo clippy
    ├── demo.sh         # cargo run --example demo
    └── doc.sh          # cargo doc --open
```

## Scripts

| Script | Description |
|---|---|
| `./scripts/build.sh` | Build the library |
| `./scripts/test.sh` | Run all tests (unit + integration + doc) |
| `./scripts/fmt.sh` | Format code with `rustfmt` |
| `./scripts/lint.sh` | Lint with `clippy` |
| `./scripts/demo.sh` | Run the demo application |
| `./scripts/doc.sh` | Generate and open documentation |

## License

MIT
