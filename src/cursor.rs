/// Cursor and selection state for the textarea.
///
/// Manages the caret position, text selection, and provides methods
/// for extending/collapsing selections.
use crate::buffer::{Position, TextBuffer, TextRange};

/// Describes the cursor and optional selection in the textarea.
#[derive(Debug, Clone)]
pub struct Cursor {
    /// The current caret position (where text is inserted).
    pub position: Position,
    /// If set, defines the anchor of the selection. The selection range is
    /// between `anchor` and `position`.
    pub anchor: Option<Position>,
    /// The preferred column (in bytes) when moving vertically.
    /// This preserves the column when moving through lines of different lengths.
    pub preferred_col: usize,
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

impl Cursor {
    /// Create a cursor at position (0, 0) with no selection.
    pub fn new() -> Self {
        Self {
            position: Position::zero(),
            anchor: None,
            preferred_col: 0,
        }
    }

    /// Create a cursor at the given position.
    pub fn at(pos: Position) -> Self {
        Self {
            position: pos,
            anchor: None,
            preferred_col: pos.col,
        }
    }

    /// Returns the selected text range, or None if no selection.
    pub fn selection(&self) -> Option<TextRange> {
        self.anchor
            .map(|anchor| TextRange::new(anchor, self.position))
    }

    /// Returns true if there is an active selection.
    pub fn has_selection(&self) -> bool {
        self.anchor.is_some() && self.anchor != Some(self.position)
    }

    /// Set position and clear selection.
    pub fn set_position(&mut self, pos: Position) {
        self.position = pos;
        self.anchor = None;
        self.preferred_col = pos.col;
    }

    /// Extend selection from current anchor (or set anchor to current position) to new position.
    pub fn select_to(&mut self, pos: Position) {
        if self.anchor.is_none() {
            self.anchor = Some(self.position);
        }
        self.position = pos;
        self.preferred_col = pos.col;
    }

    /// Collapse selection to the caret position.
    pub fn collapse(&mut self) {
        self.anchor = None;
    }

    /// Set the selection range explicitly.
    pub fn set_selection(&mut self, range: TextRange) {
        self.anchor = Some(range.start);
        self.position = range.end;
        self.preferred_col = range.end.col;
    }

    // ── Movement helpers ──────────────────────────────────

    /// Move cursor left. If `extend` is true, extends selection.
    pub fn move_left(&mut self, buf: &TextBuffer, extend: bool) {
        if !extend && self.has_selection() {
            let sel = self.selection().unwrap();
            self.set_position(sel.start);
            return;
        }
        let new_pos = buf.move_left(self.position);
        if extend {
            self.select_to(new_pos);
        } else {
            self.set_position(new_pos);
        }
    }

    /// Move cursor right. If `extend` is true, extends selection.
    pub fn move_right(&mut self, buf: &TextBuffer, extend: bool) {
        if !extend && self.has_selection() {
            let sel = self.selection().unwrap();
            self.set_position(sel.end);
            return;
        }
        let new_pos = buf.move_right(self.position);
        if extend {
            self.select_to(new_pos);
        } else {
            self.set_position(new_pos);
        }
    }

    /// Move cursor up. If `extend` is true, extends selection.
    pub fn move_up(&mut self, buf: &TextBuffer, extend: bool) {
        let saved_col = self.preferred_col;
        let new_pos = buf.move_up(self.position, self.preferred_col);
        if extend {
            self.select_to(new_pos);
        } else {
            self.set_position(new_pos);
        }
        // Restore preferred_col so vertical movement is smooth.
        self.preferred_col = saved_col;
    }

    /// Move cursor down. If `extend` is true, extends selection.
    pub fn move_down(&mut self, buf: &TextBuffer, extend: bool) {
        let saved_col = self.preferred_col;
        let new_pos = buf.move_down(self.position, self.preferred_col);
        if extend {
            self.select_to(new_pos);
        } else {
            self.set_position(new_pos);
        }
        self.preferred_col = saved_col;
    }

    /// Move to start of line. If `extend` is true, extends selection.
    pub fn move_home(&mut self, buf: &TextBuffer, extend: bool) {
        let new_pos = buf.line_start(self.position);
        if extend {
            self.select_to(new_pos);
        } else {
            self.set_position(new_pos);
        }
    }

    /// Move to end of line. If `extend` is true, extends selection.
    pub fn move_end(&mut self, buf: &TextBuffer, extend: bool) {
        let new_pos = buf.line_end(self.position);
        if extend {
            self.select_to(new_pos);
        } else {
            self.set_position(new_pos);
        }
    }

    /// Move left by one word. If `extend` is true, extends selection.
    pub fn move_word_left(&mut self, buf: &TextBuffer, extend: bool) {
        let new_pos = buf.move_word_left(self.position);
        if extend {
            self.select_to(new_pos);
        } else {
            self.set_position(new_pos);
        }
    }

    /// Move right by one word. If `extend` is true, extends selection.
    pub fn move_word_right(&mut self, buf: &TextBuffer, extend: bool) {
        let new_pos = buf.move_word_right(self.position);
        if extend {
            self.select_to(new_pos);
        } else {
            self.set_position(new_pos);
        }
    }

    /// Move to the very start of the buffer.
    pub fn move_to_start(&mut self, _buf: &TextBuffer, extend: bool) {
        let new_pos = Position::zero();
        if extend {
            self.select_to(new_pos);
        } else {
            self.set_position(new_pos);
        }
    }

    /// Move to the very end of the buffer.
    pub fn move_to_end(&mut self, buf: &TextBuffer, extend: bool) {
        let new_pos = buf.end_position();
        if extend {
            self.select_to(new_pos);
        } else {
            self.set_position(new_pos);
        }
    }

    /// Select all text.
    pub fn select_all(&mut self, buf: &TextBuffer) {
        let range = buf.select_all();
        self.set_selection(range);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_default() {
        let c = Cursor::new();
        assert_eq!(c.position, Position::zero());
        assert!(!c.has_selection());
    }

    #[test]
    fn test_cursor_selection() {
        let mut c = Cursor::at(Position::new(0, 0));
        c.select_to(Position::new(0, 5));
        assert!(c.has_selection());
        let sel = c.selection().unwrap();
        assert_eq!(sel.start, Position::new(0, 0));
        assert_eq!(sel.end, Position::new(0, 5));
    }

    #[test]
    fn test_move_collapses_selection() {
        let buf = TextBuffer::from_text("hello world");
        let mut c = Cursor::at(Position::new(0, 0));
        c.select_to(Position::new(0, 5));
        c.move_right(&buf, false);
        assert!(!c.has_selection());
        assert_eq!(c.position, Position::new(0, 5));
    }

    #[test]
    fn test_move_up_down_preferred_col() {
        let buf = TextBuffer::from_text("hello\nhi\nworld");
        let mut c = Cursor::at(Position::new(0, 4));
        c.preferred_col = 4;
        c.move_down(&buf, false);
        assert_eq!(c.position, Position::new(1, 2));
        c.move_down(&buf, false);
        assert_eq!(c.position, Position::new(2, 4));
    }

    #[test]
    fn test_select_all() {
        let buf = TextBuffer::from_text("hello\nworld");
        let mut c = Cursor::new();
        c.select_all(&buf);
        assert!(c.has_selection());
        let sel = c.selection().unwrap();
        assert_eq!(sel.start, Position::zero());
        assert_eq!(sel.end, Position::new(1, 5));
    }
}
