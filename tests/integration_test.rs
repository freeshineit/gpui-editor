/// Integration tests for gpui-editor textarea.
///
/// Tests that combine buffer, cursor, and editing operations together
/// to simulate realistic editing workflows.
use gpui_editor::buffer::{Position, TextBuffer, TextRange};
use gpui_editor::cursor::Cursor;

#[test]
fn test_full_editing_workflow() {
    let mut buf = TextBuffer::new();
    let mut cursor = Cursor::new();

    // Type "Hello, World!"
    let text = "Hello, World!";
    let pos = buf.insert(cursor.position, text);
    cursor.set_position(pos);
    assert_eq!(buf.text(), "Hello, World!");
    assert_eq!(cursor.position, Position::new(0, 13));

    // Press Enter to create a new line.
    let pos = buf.insert(cursor.position, "\n");
    cursor.set_position(pos);
    assert_eq!(buf.line_count(), 2);
    assert_eq!(cursor.position, Position::new(1, 0));

    // Type on new line.
    let pos = buf.insert(cursor.position, "This is line 2.");
    cursor.set_position(pos);
    assert_eq!(buf.text(), "Hello, World!\nThis is line 2.");

    // Move to beginning of line.
    cursor.move_home(&buf, false);
    assert_eq!(cursor.position, Position::new(1, 0));

    // Select to end of line.
    cursor.move_end(&buf, true);
    assert!(cursor.has_selection());
    let sel = cursor.selection().unwrap();
    assert_eq!(sel.start, Position::new(1, 0));
    assert_eq!(sel.end, Position::new(1, 15));

    // Delete selection.
    let pos = buf.delete(sel);
    cursor.set_position(pos);
    assert_eq!(buf.text(), "Hello, World!\n");

    // Type replacement text.
    let pos = buf.insert(cursor.position, "Goodbye!");
    cursor.set_position(pos);
    assert_eq!(buf.text(), "Hello, World!\nGoodbye!");
}

#[test]
fn test_multiline_selection_and_delete() {
    let mut buf = TextBuffer::from_text("Line 1\nLine 2\nLine 3\nLine 4");
    let mut cursor = Cursor::new();

    // Select from middle of line 1 to middle of line 3.
    cursor.set_position(Position::new(0, 4));
    cursor.select_to(Position::new(2, 4));

    let sel = cursor.selection().unwrap();
    let selected = buf.text_in_range(sel);
    assert_eq!(selected, " 1\nLine 2\nLine");

    // Delete the selection.
    let pos = buf.delete(sel);
    cursor.set_position(pos);
    assert_eq!(buf.text(), "Line 3\nLine 4");
    assert_eq!(buf.line_count(), 2);
}

#[test]
fn test_word_navigation() {
    let buf = TextBuffer::from_text("fn hello_world(arg1: u32) {");
    let mut cursor = Cursor::at(Position::new(0, 0));

    // Move word by word to the right.
    cursor.move_word_right(&buf, false);
    // After "fn ", cursor should be at "hello_world"
    assert!(cursor.position.col > 0);

    // Move to end.
    cursor.move_to_end(&buf, false);
    assert_eq!(cursor.position, buf.end_position());

    // Move word by word to the left.
    cursor.move_word_left(&buf, false);
    assert!(cursor.position.col < buf.line_len(0));
}

#[test]
fn test_select_all_and_replace() {
    let mut buf = TextBuffer::from_text("old content\nwith multiple\nlines");
    let mut cursor = Cursor::new();

    // Select all.
    cursor.select_all(&buf);
    let sel = cursor.selection().unwrap();

    // Replace all with new content.
    let pos = buf.replace(sel, "new content");
    cursor.set_position(pos);
    assert_eq!(buf.text(), "new content");
    assert_eq!(buf.line_count(), 1);
}

#[test]
fn test_backspace_at_line_boundary() {
    let mut buf = TextBuffer::from_text("Line 1\nLine 2");
    let mut cursor = Cursor::at(Position::new(1, 0));

    // Backspace at start of line 2 should join lines.
    let prev = buf.move_left(cursor.position);
    let range = TextRange::new(prev, cursor.position);
    let pos = buf.delete(range);
    cursor.set_position(pos);

    assert_eq!(buf.text(), "Line 1Line 2");
    assert_eq!(buf.line_count(), 1);
    assert_eq!(cursor.position, Position::new(0, 6));
}

#[test]
fn test_cursor_movement_across_lines() {
    let buf = TextBuffer::from_text("abc\ndef\nghi");
    let mut cursor = Cursor::at(Position::new(0, 3));

    // Move right at end of line wraps to next.
    cursor.move_right(&buf, false);
    assert_eq!(cursor.position, Position::new(1, 0));

    // Move left at start of line wraps to previous.
    cursor.move_left(&buf, false);
    assert_eq!(cursor.position, Position::new(0, 3));

    // Move down then up.
    cursor.move_down(&buf, false);
    assert_eq!(cursor.position, Position::new(1, 3));
    cursor.move_up(&buf, false);
    assert_eq!(cursor.position, Position::new(0, 3));
}

#[test]
fn test_unicode_editing() {
    let mut buf = TextBuffer::from_text("你好世界");
    let mut cursor = Cursor::at(Position::new(0, 0));

    // Move right through CJK characters.
    cursor.move_right(&buf, false);
    assert_eq!(cursor.position.col, 3); // Each CJK char is 3 bytes in UTF-8.

    cursor.move_right(&buf, false);
    assert_eq!(cursor.position.col, 6);

    // Insert at cursor position.
    let pos = buf.insert(cursor.position, "，");
    cursor.set_position(pos);
    assert_eq!(buf.text(), "你好，世界");

    // Select all and verify.
    cursor.select_all(&buf);
    let sel = cursor.selection().unwrap();
    let text = buf.text_in_range(sel);
    assert_eq!(text, "你好，世界");
}

#[test]
fn test_empty_buffer_operations() {
    let mut buf = TextBuffer::new();
    let mut cursor = Cursor::new();

    // Moving in empty buffer should be safe.
    cursor.move_left(&buf, false);
    assert_eq!(cursor.position, Position::zero());

    cursor.move_right(&buf, false);
    assert_eq!(cursor.position, Position::zero());

    cursor.move_up(&buf, false);
    assert_eq!(cursor.position, Position::zero());

    cursor.move_down(&buf, false);
    assert_eq!(cursor.position, Position::zero());

    // Delete in empty buffer should be safe.
    let prev = buf.move_left(cursor.position);
    assert_eq!(prev, cursor.position);

    // Type something.
    let pos = buf.insert(cursor.position, "a");
    cursor.set_position(pos);
    assert_eq!(buf.text(), "a");
}

#[test]
fn test_tab_insertion() {
    let mut buf = TextBuffer::new();
    let mut cursor = Cursor::new();

    let pos = buf.insert(cursor.position, "    "); // 4 spaces for tab
    cursor.set_position(pos);
    assert_eq!(buf.text(), "    ");
    assert_eq!(cursor.position, Position::new(0, 4));

    let pos = buf.insert(cursor.position, "code");
    cursor.set_position(pos);
    assert_eq!(buf.text(), "    code");
}

#[test]
fn test_style_creation() {
    use gpui_editor::style::TextareaStyle;

    let default_style = TextareaStyle::default();
    assert!(default_style.font_size > 0.0);
    assert!(default_style.line_height > 0.0);

    let dark_style = TextareaStyle::dark();
    assert!(dark_style.font_size > 0.0);
    assert!(dark_style.line_height > 0.0);
}
