/// Text buffer that stores multi-line text content and supports editing operations.
///
/// The buffer maintains text as a `Vec<String>` where each element represents a line.
/// It provides methods for inserting, deleting, and replacing text at arbitrary positions.
use unicode_segmentation::UnicodeSegmentation;

/// A position in the text buffer, identified by line and column (byte offset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    /// Line index (0-based).
    pub line: usize,
    /// Column byte offset within the line (0-based).
    pub col: usize,
}

impl Position {
    /// Create a new position.
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    /// Position at the start of the buffer.
    pub fn zero() -> Self {
        Self { line: 0, col: 0 }
    }
}

/// A range of text in the buffer (start inclusive, end exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: Position,
    pub end: Position,
}

impl TextRange {
    pub fn new(start: Position, end: Position) -> Self {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        Self { start, end }
    }

    /// Returns true if the range is empty (start == end).
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// The text buffer backing a textarea.
///
/// Stores lines of text and provides editing primitives.
#[derive(Debug, Clone)]
pub struct TextBuffer {
    /// Each element is one line of text (without trailing newline).
    lines: Vec<String>,
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextBuffer {
    /// Create an empty buffer with one empty line.
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
        }
    }

    /// Create a buffer from the given text.
    pub fn from_text(text: &str) -> Self {
        let lines: Vec<String> = if text.is_empty() {
            vec![String::new()]
        } else {
            text.split('\n').map(|s| s.to_string()).collect()
        };
        Self { lines }
    }

    /// Return all text as a single string with newlines.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Number of lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get a reference to a specific line.
    pub fn line(&self, idx: usize) -> &str {
        &self.lines[idx]
    }

    /// Get all lines as a slice.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Length (in bytes) of a given line.
    pub fn line_len(&self, idx: usize) -> usize {
        self.lines[idx].len()
    }

    /// Clamp a position to valid buffer bounds.
    pub fn clamp_position(&self, pos: Position) -> Position {
        let line = pos.line.min(self.lines.len() - 1);
        let col = pos.col.min(self.lines[line].len());
        // Ensure we don't land in the middle of a multi-byte character.
        let col = self.snap_to_grapheme_boundary(&self.lines[line], col);
        Position::new(line, col)
    }

    /// Snap a byte offset to the nearest grapheme cluster boundary.
    fn snap_to_grapheme_boundary(&self, line: &str, byte_offset: usize) -> usize {
        if byte_offset == 0 || byte_offset >= line.len() {
            return byte_offset.min(line.len());
        }
        // Walk grapheme indices to find the closest boundary.
        let mut prev = 0;
        for (idx, _) in line.grapheme_indices(true) {
            if idx == byte_offset {
                return idx;
            }
            if idx > byte_offset {
                return prev;
            }
            prev = idx;
        }
        line.len()
    }

    /// The position at the very end of the buffer.
    pub fn end_position(&self) -> Position {
        let last = self.lines.len() - 1;
        Position::new(last, self.lines[last].len())
    }

    /// Insert text at the given position. Handles newlines.
    /// Returns the position after the inserted text.
    pub fn insert(&mut self, pos: Position, text: &str) -> Position {
        let pos = self.clamp_position(pos);
        if text.is_empty() {
            return pos;
        }

        let after = self.lines[pos.line].split_off(pos.col);
        let insert_lines: Vec<&str> = text.split('\n').collect();

        if insert_lines.len() == 1 {
            self.lines[pos.line].push_str(insert_lines[0]);
            let new_col = self.lines[pos.line].len();
            self.lines[pos.line].push_str(&after);
            Position::new(pos.line, new_col)
        } else {
            // First fragment goes on the current line.
            self.lines[pos.line].push_str(insert_lines[0]);

            // Middle lines are inserted as new lines.
            let mut new_lines: Vec<String> = insert_lines[1..insert_lines.len() - 1]
                .iter()
                .map(|s| s.to_string())
                .collect();

            // Last fragment + remainder.
            let last_insert = insert_lines[insert_lines.len() - 1];
            let new_col = last_insert.len();
            let mut last_line = last_insert.to_string();
            last_line.push_str(&after);
            new_lines.push(last_line);

            let insert_at = pos.line + 1;
            let new_line_idx = pos.line + insert_lines.len() - 1;
            for (i, line) in new_lines.into_iter().enumerate() {
                self.lines.insert(insert_at + i, line);
            }

            Position::new(new_line_idx, new_col)
        }
    }

    /// Delete text in the given range. Returns the start position.
    pub fn delete(&mut self, range: TextRange) -> Position {
        if range.is_empty() {
            return range.start;
        }
        let start = self.clamp_position(range.start);
        let end = self.clamp_position(range.end);

        if start.line == end.line {
            self.lines[start.line].replace_range(start.col..end.col, "");
        } else {
            let after = self.lines[end.line][end.col..].to_string();
            self.lines[start.line].truncate(start.col);
            self.lines[start.line].push_str(&after);
            // Remove lines between start+1 and end (inclusive).
            self.lines.drain((start.line + 1)..=end.line);
        }
        start
    }

    /// Replace text in the given range with new text.
    /// Returns the position after the replacement.
    pub fn replace(&mut self, range: TextRange, text: &str) -> Position {
        let start = self.delete(range);
        self.insert(start, text)
    }

    /// Get the text within a range.
    pub fn text_in_range(&self, range: TextRange) -> String {
        let start = self.clamp_position(range.start);
        let end = self.clamp_position(range.end);

        if start.line == end.line {
            self.lines[start.line][start.col..end.col].to_string()
        } else {
            let mut result = String::new();
            result.push_str(&self.lines[start.line][start.col..]);
            for line_idx in (start.line + 1)..end.line {
                result.push('\n');
                result.push_str(&self.lines[line_idx]);
            }
            result.push('\n');
            result.push_str(&self.lines[end.line][..end.col]);
            result
        }
    }

    /// Move position left by one grapheme cluster.
    pub fn move_left(&self, pos: Position) -> Position {
        let pos = self.clamp_position(pos);
        if pos.col == 0 {
            if pos.line == 0 {
                pos
            } else {
                Position::new(pos.line - 1, self.line_len(pos.line - 1))
            }
        } else {
            let line = &self.lines[pos.line];
            let mut prev_boundary = 0;
            for (idx, _) in line.grapheme_indices(true) {
                if idx >= pos.col {
                    break;
                }
                prev_boundary = idx;
            }
            Position::new(pos.line, prev_boundary)
        }
    }

    /// Move position right by one grapheme cluster.
    pub fn move_right(&self, pos: Position) -> Position {
        let pos = self.clamp_position(pos);
        let line = &self.lines[pos.line];
        if pos.col >= line.len() {
            if pos.line >= self.lines.len() - 1 {
                pos
            } else {
                Position::new(pos.line + 1, 0)
            }
        } else {
            let mut found_current = false;
            for (idx, _) in line.grapheme_indices(true) {
                if found_current {
                    return Position::new(pos.line, idx);
                }
                if idx == pos.col {
                    found_current = true;
                }
            }
            Position::new(pos.line, line.len())
        }
    }

    /// Move position up by one line, preserving column as best as possible.
    pub fn move_up(&self, pos: Position, preferred_col: usize) -> Position {
        if pos.line == 0 {
            Position::new(0, 0)
        } else {
            let new_line = pos.line - 1;
            let col = preferred_col.min(self.line_len(new_line));
            self.clamp_position(Position::new(new_line, col))
        }
    }

    /// Move position down by one line, preserving column as best as possible.
    pub fn move_down(&self, pos: Position, preferred_col: usize) -> Position {
        if pos.line >= self.lines.len() - 1 {
            self.end_position()
        } else {
            let new_line = pos.line + 1;
            let col = preferred_col.min(self.line_len(new_line));
            self.clamp_position(Position::new(new_line, col))
        }
    }

    /// Move to the beginning of the current line.
    pub fn line_start(&self, pos: Position) -> Position {
        Position::new(pos.line.min(self.lines.len() - 1), 0)
    }

    /// Move to the end of the current line.
    pub fn line_end(&self, pos: Position) -> Position {
        let line = pos.line.min(self.lines.len() - 1);
        Position::new(line, self.line_len(line))
    }

    /// Move left by one word (skip whitespace, then word characters).
    pub fn move_word_left(&self, pos: Position) -> Position {
        let pos = self.clamp_position(pos);
        if pos.col == 0 {
            if pos.line == 0 {
                return pos;
            }
            return Position::new(pos.line - 1, self.line_len(pos.line - 1));
        }

        let line = &self.lines[pos.line][..pos.col];
        let chars: Vec<(usize, &str)> = line.grapheme_indices(true).collect();
        let mut i = chars.len();

        // Skip whitespace backwards.
        while i > 0 && chars[i - 1].1.trim().is_empty() {
            i -= 1;
        }
        // Skip word characters backwards.
        while i > 0 && !chars[i - 1].1.trim().is_empty() {
            i -= 1;
        }

        if i == 0 {
            Position::new(pos.line, 0)
        } else {
            Position::new(pos.line, chars[i].0)
        }
    }

    /// Move right by one word.
    pub fn move_word_right(&self, pos: Position) -> Position {
        let pos = self.clamp_position(pos);
        let line = &self.lines[pos.line];
        if pos.col >= line.len() {
            if pos.line >= self.lines.len() - 1 {
                return pos;
            }
            return Position::new(pos.line + 1, 0);
        }

        let rest = &line[pos.col..];
        let graphemes: Vec<(usize, &str)> = rest.grapheme_indices(true).collect();
        let mut i = 0;

        // Skip word characters.
        while i < graphemes.len() && !graphemes[i].1.trim().is_empty() {
            i += 1;
        }
        // Skip whitespace.
        while i < graphemes.len() && graphemes[i].1.trim().is_empty() {
            i += 1;
        }

        if i >= graphemes.len() {
            Position::new(pos.line, line.len())
        } else {
            Position::new(pos.line, pos.col + graphemes[i].0)
        }
    }

    /// Select all: returns range covering entire buffer.
    pub fn select_all(&self) -> TextRange {
        TextRange::new(Position::zero(), self.end_position())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_buffer() {
        let buf = TextBuffer::new();
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.text(), "");
        assert_eq!(buf.line(0), "");
    }

    #[test]
    fn test_from_text() {
        let buf = TextBuffer::from_text("hello\nworld");
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.line(0), "hello");
        assert_eq!(buf.line(1), "world");
        assert_eq!(buf.text(), "hello\nworld");
    }

    #[test]
    fn test_insert_single_line() {
        let mut buf = TextBuffer::new();
        let pos = buf.insert(Position::zero(), "hello");
        assert_eq!(pos, Position::new(0, 5));
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn test_insert_multiline() {
        let mut buf = TextBuffer::new();
        let pos = buf.insert(Position::zero(), "hello\nworld\nfoo");
        assert_eq!(pos, Position::new(2, 3));
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.text(), "hello\nworld\nfoo");
    }

    #[test]
    fn test_insert_middle() {
        let mut buf = TextBuffer::from_text("helo");
        let pos = buf.insert(Position::new(0, 3), "l");
        assert_eq!(pos, Position::new(0, 4));
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn test_delete_single_line() {
        let mut buf = TextBuffer::from_text("hello world");
        let pos = buf.delete(TextRange::new(Position::new(0, 5), Position::new(0, 11)));
        assert_eq!(pos, Position::new(0, 5));
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn test_delete_multiline() {
        let mut buf = TextBuffer::from_text("hello\nbeautiful\nworld");
        let pos = buf.delete(TextRange::new(Position::new(0, 5), Position::new(2, 0)));
        assert_eq!(pos, Position::new(0, 5));
        assert_eq!(buf.text(), "helloworld");
    }

    #[test]
    fn test_replace() {
        let mut buf = TextBuffer::from_text("hello world");
        let pos = buf.replace(
            TextRange::new(Position::new(0, 6), Position::new(0, 11)),
            "rust",
        );
        assert_eq!(pos, Position::new(0, 10));
        assert_eq!(buf.text(), "hello rust");
    }

    #[test]
    fn test_text_in_range() {
        let buf = TextBuffer::from_text("hello\nworld\nfoo");
        let text = buf.text_in_range(TextRange::new(Position::new(0, 3), Position::new(1, 3)));
        assert_eq!(text, "lo\nwor");
    }

    #[test]
    fn test_move_left_right() {
        let buf = TextBuffer::from_text("ab\ncd");
        // Move right from start
        let pos = buf.move_right(Position::new(0, 0));
        assert_eq!(pos, Position::new(0, 1));
        // Move right to next line
        let pos = buf.move_right(Position::new(0, 2));
        assert_eq!(pos, Position::new(1, 0));
        // Move left wraps to previous line
        let pos = buf.move_left(Position::new(1, 0));
        assert_eq!(pos, Position::new(0, 2));
        // Move left at start stays
        let pos = buf.move_left(Position::new(0, 0));
        assert_eq!(pos, Position::new(0, 0));
    }

    #[test]
    fn test_move_up_down() {
        let buf = TextBuffer::from_text("hello\nhi\nworld");
        let pos = buf.move_down(Position::new(0, 4), 4);
        assert_eq!(pos, Position::new(1, 2)); // "hi" only has 2 chars
        let pos = buf.move_down(Position::new(1, 2), 4);
        assert_eq!(pos, Position::new(2, 4));
        let pos = buf.move_up(Position::new(2, 4), 4);
        assert_eq!(pos, Position::new(1, 2));
    }

    #[test]
    fn test_word_movement() {
        let buf = TextBuffer::from_text("hello world foo");
        let pos = buf.move_word_right(Position::new(0, 0));
        assert_eq!(pos, Position::new(0, 6));
        let pos = buf.move_word_right(Position::new(0, 6));
        assert_eq!(pos, Position::new(0, 12));
        let pos = buf.move_word_left(Position::new(0, 12));
        assert_eq!(pos, Position::new(0, 6));
    }

    #[test]
    fn test_select_all() {
        let buf = TextBuffer::from_text("hello\nworld");
        let range = buf.select_all();
        assert_eq!(range.start, Position::zero());
        assert_eq!(range.end, Position::new(1, 5));
    }

    #[test]
    fn test_unicode() {
        let mut buf = TextBuffer::from_text("héllo");
        let pos = buf.move_right(Position::new(0, 0));
        assert_eq!(pos, Position::new(0, 1));
        // é is 2 bytes in UTF-8
        let pos = buf.move_right(pos);
        assert_eq!(pos, Position::new(0, 3));
        let end = buf.insert(Position::new(0, 6), " 🌍");
        assert_eq!(buf.text(), "héllo 🌍");
        assert!(end.col > 6);
    }

    #[test]
    fn test_delete_newline() {
        let mut buf = TextBuffer::from_text("hello\nworld");
        buf.delete(TextRange::new(Position::new(0, 5), Position::new(1, 0)));
        assert_eq!(buf.text(), "helloworld");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_insert_newline() {
        let mut buf = TextBuffer::from_text("helloworld");
        buf.insert(Position::new(0, 5), "\n");
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.line(0), "hello");
        assert_eq!(buf.line(1), "world");
    }
}
