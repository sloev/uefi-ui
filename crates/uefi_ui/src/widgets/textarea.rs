//! Multiline text buffer with scroll and cursor (UTF-8 safe).
//!
//! Selection modes:
//! - **Stream** (hold **Shift**, arrows / Home / End / PgUp / PgDn): contiguous range from the
//!   anchor caret to the moving caret (standard editor behavior).
//! - **Block / column** (**Alt+Shift** + arrows): axis-aligned rectangle in (line, column) space;
//!   useful for column edits on multiple lines.

use alloc::string::String;
use alloc::vec::Vec;

use crate::input::{Key, KeyEvent};

/// Actions the app may react to (e.g. play beep, commit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextAreaAction {
    /// Text or cursor changed
    Edited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionState {
    None,
    /// Contiguous [lo, hi) in byte space (normalized from anchor vs cursor).
    Stream { anchor: usize },
    /// Rectangle in line × column space; second corner is always the current cursor.
    Block {
        anchor_line: usize,
        anchor_col: usize,
    },
}

#[derive(Debug, Clone)]
pub struct TextArea {
    pub text: String,
    /// Byte index into `text`, always on a char boundary.
    cursor: usize,
    sel: SelectionState,
    /// First visible line (0-based) for vertical scrolling.
    pub scroll_top_line: usize,
    preferred_column: Option<usize>,
}

impl Default for TextArea {
    fn default() -> Self {
        Self::new()
    }
}

impl TextArea {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            sel: SelectionState::None,
            scroll_top_line: 0,
            preferred_column: None,
        }
    }

    pub fn from_str(s: &str) -> Self {
        Self {
            text: String::from(s),
            cursor: s.len(),
            sel: SelectionState::None,
            scroll_top_line: 0,
            preferred_column: None,
        }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_cursor(&mut self, byte: usize) {
        self.cursor = floor_char_boundary(&self.text, byte.min(self.text.len()));
        self.sel = SelectionState::None;
    }

    pub fn clear_selection(&mut self) {
        self.sel = SelectionState::None;
    }

    /// True when using column/block selection ([`SelectionState::Block`]).
    pub fn is_block_selection(&self) -> bool {
        matches!(self.sel, SelectionState::Block { .. })
    }

    /// Select the entire buffer (keyboard shortcut helper when firmware does not send Ctrl+A).
    pub fn select_all(&mut self) {
        self.sel = SelectionState::Stream { anchor: 0 };
        self.cursor = self.text.len();
        self.preferred_column = None;
    }

    /// Column range `[start_col, end_col)` within `line_idx` to highlight (stream or block).
    pub fn selection_highlight_on_line(&self, line_idx: usize) -> Option<(usize, usize)> {
        match self.sel {
            SelectionState::None => None,
            SelectionState::Stream { anchor } => {
                let (lo, hi) = stream_range_bytes(anchor, self.cursor, &self.text)?;
                let lines = self.lines();
                let _line = *lines.get(line_idx)?;
                let line_start = line_byte_start(&self.text, &lines, line_idx);
                let line_slice = lines.get(line_idx).copied().unwrap_or("");
                let line_end = line_start + line_slice.len();
                let a = lo.max(line_start).min(line_end);
                let b = hi.min(line_end).max(line_start);
                if a >= b {
                    return None;
                }
                let col_a = self.text[line_start..a].chars().count();
                let col_b = self.text[line_start..b].chars().count();
                Some((col_a, col_b))
            }
            SelectionState::Block {
                anchor_line,
                anchor_col,
            } => {
                let (cl, cc) = self.cursor_line_col();
                let line_lo = anchor_line.min(cl);
                let line_hi = anchor_line.max(cl);
                if line_idx < line_lo || line_idx > line_hi {
                    return None;
                }
                let col_lo = anchor_col.min(cc);
                let col_hi_excl = anchor_col.max(cc) + 1;
                let lines = self.lines();
                let line_slice = lines.get(line_idx).copied().unwrap_or("");
                let line_len = line_slice.chars().count();
                let start = col_lo.min(line_len);
                let end = col_hi_excl.min(line_len).max(start);
                if start >= end {
                    None
                } else {
                    Some((start, end))
                }
            }
        }
    }

    /// Half-open byte range `[lo, hi)` for **stream** selection only.
    pub fn selection_range_bytes(&self) -> Option<(usize, usize)> {
        match self.sel {
            SelectionState::Stream { anchor } => stream_range_bytes(anchor, self.cursor, &self.text),
            SelectionState::Block { .. } | SelectionState::None => None,
        }
    }

    pub fn has_selection(&self) -> bool {
        match self.sel {
            SelectionState::None => false,
            SelectionState::Stream { anchor } => anchor != self.cursor,
            SelectionState::Block {
                anchor_line,
                anchor_col,
            } => {
                let (cl, cc) = self.cursor_line_col();
                anchor_line != cl || anchor_col != cc
            }
        }
    }

    /// Selected characters (stream or column block), suitable for copy.
    pub fn selected_text(&self) -> Option<String> {
        match self.sel {
            SelectionState::None => None,
            SelectionState::Stream { anchor } => {
                let (lo, hi) = stream_range_bytes(anchor, self.cursor, &self.text)?;
                Some(String::from(&self.text[lo..hi]))
            }
            SelectionState::Block {
                anchor_line,
                anchor_col,
            } => {
                let (cl, cc) = self.cursor_line_col();
                let line_lo = anchor_line.min(cl);
                let line_hi = anchor_line.max(cl);
                let col_lo = anchor_col.min(cc);
                let col_hi_excl = anchor_col.max(cc) + 1;
                let lines = self.lines();
                let mut out = String::new();
                for li in line_lo..=line_hi {
                    let line = lines.get(li).copied().unwrap_or("");
                    let len = line.chars().count();
                    let start = col_lo.min(len);
                    let end = col_hi_excl.min(len).max(start);
                    if li > line_lo {
                        out.push('\n');
                    }
                    let segment: String = line.chars().skip(start).take(end - start).collect();
                    out.push_str(&segment);
                }
                if out.is_empty() {
                    None
                } else {
                    Some(out)
                }
            }
        }
    }

    /// Replace the current selection with `s`, or insert at the caret if there is none.
    pub fn replace_selection_with_str(&mut self, s: &str) -> Option<TextAreaAction> {
        if matches!(self.sel, SelectionState::Block { .. }) {
            let _ = self.delete_block_selection();
            self.insert_str_at_cursor(s);
            return Some(TextAreaAction::Edited);
        }
        if let Some((lo, hi)) = self.selection_range_bytes() {
            self.text.replace_range(lo..hi, s);
            self.cursor = lo + s.len();
            self.cursor = floor_char_boundary(&self.text, self.cursor.min(self.text.len()));
            self.sel = SelectionState::None;
            return Some(TextAreaAction::Edited);
        }
        self.insert_str_at_cursor(s);
        Some(TextAreaAction::Edited)
    }

    fn insert_str_at_cursor(&mut self, s: &str) {
        self.cursor = floor_char_boundary(&self.text, self.cursor);
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
    }

    /// Lines split by `\n` (uses `split` so a trailing newline yields a final empty line).
    pub fn lines(&self) -> Vec<&str> {
        self.text.split('\n').collect()
    }

    pub fn line_count(&self) -> usize {
        self.lines().len()
    }

    /// **S15.1** — Soft-wrapped display rows for a viewport of `cols` characters wide.
    ///
    /// Each logical line (split by `\n`) is broken into one or more display rows:
    /// - Break at the last space before `cols` (word-wrap).
    /// - Fall back to a hard break at `cols` if no space exists.
    ///
    /// `scroll_top_line` continues to refer to **display rows** in the returned `Vec`
    /// when the demo uses this method instead of `lines()`.
    pub fn wrapped_lines(&self, cols: usize) -> Vec<&str> {
        if cols == 0 {
            return self.lines();
        }
        let mut out: Vec<&str> = Vec::new();
        for logical in self.text.split('\n') {
            if logical.is_empty() {
                out.push(logical);
                continue;
            }
            let mut rem = logical;
            while !rem.is_empty() {
                if rem.chars().count() <= cols {
                    out.push(rem);
                    break;
                }
                // Find last space at or before `cols` char boundary
                let break_pos = {
                    let mut pos = 0usize; // byte pos
                    let mut last_space_byte = None;
                    let mut char_count = 0usize;
                    for (byte_off, ch) in rem.char_indices() {
                        if char_count == cols {
                            break;
                        }
                        if ch == ' ' {
                            last_space_byte = Some(byte_off);
                        }
                        pos = byte_off + ch.len_utf8();
                        char_count += 1;
                    }
                    last_space_byte.unwrap_or(pos)
                };
                out.push(&rem[..break_pos]);
                rem = rem[break_pos..].trim_start_matches(' ');
            }
        }
        if out.is_empty() {
            out.push("");
        }
        out
    }

    /// Number of display rows when wrapped to `cols` characters.
    pub fn wrapped_line_count(&self, cols: usize) -> usize {
        self.wrapped_lines(cols).len()
    }

    /// Ensure scroll so cursor line is visible given `visible_lines`.
    pub fn scroll_to_cursor(&mut self, visible_lines: usize) {
        let cur_line = line_index_for_byte(&self.text, self.cursor);
        if cur_line < self.scroll_top_line {
            self.scroll_top_line = cur_line;
        } else if cur_line >= self.scroll_top_line + visible_lines.max(1) {
            self.scroll_top_line = cur_line + 1 - visible_lines.max(1);
        }
    }

    pub fn apply_key(&mut self, key: Key) -> Option<TextAreaAction> {
        self.apply_key_event(&KeyEvent::new(key))
    }

    pub fn apply_key_event(&mut self, ev: &KeyEvent) -> Option<TextAreaAction> {
        let m = ev.modifiers;
        let k = ev.key;

        // Ctrl+A: select all
        if m.ctrl && matches!(k, Key::Character('a') | Key::Character('A')) {
            self.sel = SelectionState::Stream { anchor: 0 };
            self.cursor = self.text.len();
            self.preferred_column = None;
            return Some(TextAreaAction::Edited);
        }

        let stream_shift = m.shift && !m.alt;
        let block_shift = m.shift && m.alt;

        match k {
            Key::Left
            | Key::Right
            | Key::Up
            | Key::Down
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown => {
                if block_shift {
                    // Column / block selection: anchor is fixed at (line,col) until cleared.
                    if !matches!(self.sel, SelectionState::Block { .. }) {
                        let (l, c) = self.cursor_line_col();
                        self.sel = SelectionState::Block {
                            anchor_line: l,
                            anchor_col: c,
                        };
                    }
                    self.apply_navigation(k);
                    return Some(TextAreaAction::Edited);
                }
                if stream_shift {
                    if !matches!(self.sel, SelectionState::Stream { .. }) {
                        self.sel = SelectionState::Stream {
                            anchor: self.cursor,
                        };
                    }
                    self.apply_navigation(k);
                    return Some(TextAreaAction::Edited);
                }
                self.sel = SelectionState::None;
                self.apply_navigation(k);
                Some(TextAreaAction::Edited)
            }
            Key::Character(c) if !m.ctrl => {
                self.replace_selection_or_insert(c);
                Some(TextAreaAction::Edited)
            }
            Key::Backspace => {
                if self.delete_selection_or_backspace() {
                    return Some(TextAreaAction::Edited);
                }
                None
            }
            Key::Delete => {
                if self.delete_selection_or_delete_forward() {
                    return Some(TextAreaAction::Edited);
                }
                None
            }
            Key::Enter => {
                self.replace_selection_or_insert('\n');
                Some(TextAreaAction::Edited)
            }
            Key::Escape | Key::Tab => None,
            Key::Function(_) | Key::Other(_) => None,
            Key::Character(_) => None,
        }
    }

    fn cursor_line_col(&self) -> (usize, usize) {
        let li = line_index_for_byte(&self.text, self.cursor);
        let col = column_for_byte(&self.text, self.cursor);
        (li, col)
    }

    fn apply_navigation(&mut self, key: Key) {
        match key {
            Key::Left => self.move_horiz(-1),
            Key::Right => self.move_horiz(1),
            Key::Up => self.move_vert(-1),
            Key::Down => self.move_vert(1),
            Key::Home => self.cursor_to_line_home(),
            Key::End => self.cursor_to_line_end(),
            Key::PageUp => self.move_vert(-10),
            Key::PageDown => self.move_vert(10),
            _ => {}
        }
    }

    fn replace_selection_or_insert(&mut self, c: char) {
        match self.sel {
            SelectionState::Block { .. } => {
                self.delete_block_selection();
            }
            SelectionState::Stream { .. } => {
                if let Some((lo, hi)) = self.selection_range_bytes() {
                    self.text.replace_range(lo..hi, "");
                    self.cursor = lo;
                    self.sel = SelectionState::None;
                }
            }
            SelectionState::None => {}
        }
        self.insert_char(c);
    }

    fn delete_selection_or_backspace(&mut self) -> bool {
        match self.sel {
            SelectionState::Block { .. } => self.delete_block_selection(),
            SelectionState::Stream { .. } => {
                if let Some((lo, hi)) = self.selection_range_bytes() {
                    self.text.replace_range(lo..hi, "");
                    self.cursor = lo;
                    self.sel = SelectionState::None;
                    return true;
                }
                false
            }
            SelectionState::None => self.backspace(),
        }
    }

    fn delete_selection_or_delete_forward(&mut self) -> bool {
        match self.sel {
            SelectionState::Block { .. } => self.delete_block_selection(),
            SelectionState::Stream { .. } => {
                if let Some((lo, hi)) = self.selection_range_bytes() {
                    self.text.replace_range(lo..hi, "");
                    self.cursor = lo;
                    self.sel = SelectionState::None;
                    return true;
                }
                false
            }
            SelectionState::None => self.delete(),
        }
    }

    /// Removes the current block selection; returns true if something was removed.
    fn delete_block_selection(&mut self) -> bool {
        let SelectionState::Block {
            anchor_line,
            anchor_col,
        } = self.sel
        else {
            return false;
        };
        let (cl, cc) = self.cursor_line_col();
        let line_lo = anchor_line.min(cl);
        let line_hi = anchor_line.max(cl);
        let col_lo = anchor_col.min(cc);
        let col_hi_excl = anchor_col.max(cc) + 1;

        let src = self.text.clone();
        let lines: Vec<&str> = src.split('\n').collect();
        if lines.is_empty() {
            self.sel = SelectionState::None;
            return false;
        }

        let mut new_lines: Vec<String> = Vec::with_capacity(lines.len());
        for (i, line) in lines.iter().enumerate() {
            if i < line_lo || i > line_hi {
                new_lines.push(String::from(*line));
                continue;
            }
            let len_c = line.chars().count();
            let start = col_lo.min(len_c);
            let end = col_hi_excl.min(len_c);
            if start >= end {
                new_lines.push(String::from(*line));
            } else {
                let mut out = String::new();
                for (j, ch) in line.chars().enumerate() {
                    if j < start || j >= end {
                        out.push(ch);
                    }
                }
                new_lines.push(out);
            }
        }

        self.text = new_lines.join("\n");
        let new_cursor = byte_at_line_column(
            &self.text,
            &self.lines(),
            line_lo,
            col_lo.min(lines.get(line_lo).map(|l| l.chars().count()).unwrap_or(0)),
        );
        self.cursor = new_cursor;
        self.sel = SelectionState::None;
        self.preferred_column = Some(column_for_byte(&self.text, self.cursor));
        true
    }

    fn insert_char(&mut self, c: char) {
        self.cursor = floor_char_boundary(&self.text, self.cursor);
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    fn backspace(&mut self) -> bool {
        self.cursor = floor_char_boundary(&self.text, self.cursor);
        if self.cursor == 0 {
            return false;
        }
        let prev = prev_char_boundary(&self.text, self.cursor);
        self.text.replace_range(prev..self.cursor, "");
        self.cursor = prev;
        true
    }

    fn delete(&mut self) -> bool {
        self.cursor = floor_char_boundary(&self.text, self.cursor);
        if self.cursor >= self.text.len() {
            return false;
        }
        let next = next_char_boundary(&self.text, self.cursor);
        self.text.replace_range(self.cursor..next, "");
        true
    }

    fn move_horiz(&mut self, dir: isize) {
        self.cursor = floor_char_boundary(&self.text, self.cursor);
        if dir < 0 {
            if self.cursor == 0 {
                return;
            }
            self.cursor = prev_char_boundary(&self.text, self.cursor);
        } else {
            if self.cursor >= self.text.len() {
                return;
            }
            self.cursor = next_char_boundary(&self.text, self.cursor);
        }
        self.preferred_column = Some(column_for_byte(&self.text, self.cursor));
    }

    fn move_vert(&mut self, delta: isize) {
        let li = line_index_for_byte(&self.text, self.cursor);
        let col = self
            .preferred_column
            .unwrap_or_else(|| column_for_byte(&self.text, self.cursor));
        let new_cursor = {
            let lines = self.lines();
            if lines.is_empty() {
                return;
            }
            let new_li = (li as isize + delta).clamp(0, lines.len().saturating_sub(1) as isize) as usize;
            if new_li == li {
                return;
            }
            byte_at_line_column(&self.text, &lines, new_li, col)
        };
        self.cursor = new_cursor;
        self.preferred_column = Some(col);
    }

    fn cursor_to_line_home(&mut self) {
        let li = line_index_for_byte(&self.text, self.cursor);
        let start = {
            let lines = self.lines();
            line_byte_start(&self.text, &lines, li)
        };
        self.cursor = start;
        self.preferred_column = Some(0);
    }

    fn cursor_to_line_end(&mut self) {
        let li = line_index_for_byte(&self.text, self.cursor);
        let (end_byte, col) = {
            let lines = self.lines();
            let line_slice = lines.get(li).copied().unwrap_or("");
            let start = line_byte_start(&self.text, &lines, li);
            (start + line_slice.len(), line_slice.chars().count())
        };
        self.cursor = floor_char_boundary(&self.text, end_byte);
        self.preferred_column = Some(col);
    }
}

fn stream_range_bytes(anchor: usize, cursor: usize, text: &str) -> Option<(usize, usize)> {
    let lo = anchor.min(cursor);
    let hi = anchor.max(cursor);
    if lo == hi {
        return None;
    }
    let lo = floor_char_boundary(text, lo);
    let hi = floor_char_boundary(text, hi);
    if lo >= hi {
        return None;
    }
    Some((lo, hi))
}

fn floor_char_boundary(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn prev_char_boundary(s: &str, i: usize) -> usize {
    let mut i = i;
    if i == 0 {
        return 0;
    }
    i -= 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_char_boundary(s: &str, i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    let mut i = i + 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn line_index_for_byte(s: &str, byte: usize) -> usize {
    let b = floor_char_boundary(s, byte);
    s[..b].bytes().filter(|&x| x == b'\n').count()
}

fn line_byte_start(text: &str, lines: &[&str], line: usize) -> usize {
    if line == 0 || lines.is_empty() {
        return 0;
    }
    let mut off = 0usize;
    for l in lines.iter().take(line) {
        off += l.len();
        if off < text.len() && text.as_bytes().get(off) == Some(&b'\n') {
            off += 1;
        }
    }
    off.min(text.len())
}

fn column_for_byte(s: &str, byte: usize) -> usize {
    let b = floor_char_boundary(s, byte);
    let start = s[..b].rfind('\n').map(|i| i + 1).unwrap_or(0);
    s[start..b].chars().count()
}

fn byte_at_line_column(text: &str, lines: &[&str], line: usize, col: usize) -> usize {
    let start = line_byte_start(text, lines, line);
    let line_slice = lines.get(line).copied().unwrap_or("");
    let mut pos = start;
    let mut c = 0;
    for ch in line_slice.chars() {
        if c >= col {
            break;
        }
        pos += ch.len_utf8();
        c += 1;
    }
    floor_char_boundary(text, pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyEvent, Modifiers};

    #[test]
    fn multiline_and_cursor() {
        let mut t = TextArea::from_str("a\nbc");
        t.set_cursor(0);
        assert_eq!(t.lines(), vec!["a", "bc"]);
        t.apply_key(Key::Down);
        assert!(t.cursor() > 0);
    }

    #[test]
    fn scroll_clamps() {
        let mut t = TextArea::new();
        for _ in 0..30 {
            t.apply_key(Key::Character('x'));
            t.apply_key(Key::Enter);
        }
        t.scroll_top_line = 100;
        t.scroll_to_cursor(5);
        assert!(t.scroll_top_line <= t.line_count());
    }

    #[test]
    fn shift_right_extends_stream_selection() {
        let mut t = TextArea::from_str("hello");
        t.set_cursor(0);
        let ev = KeyEvent::with_modifiers(
            Key::Right,
            Modifiers {
                shift: true,
                ctrl: false,
                alt: false,
            },
        );
        t.apply_key_event(&ev).unwrap();
        assert!(t.has_selection());
        assert_eq!(t.selection_range_bytes(), Some((0, 1)));
    }

    #[test]
    fn typing_replaces_selection() {
        let mut t = TextArea::from_str("abcdef");
        t.sel = SelectionState::Stream { anchor: 0 };
        t.cursor = 3;
        t.apply_key_event(&KeyEvent::new(Key::Character('z'))).unwrap();
        assert_eq!(t.text, "zdef");
    }

    #[test]
    fn select_all_ctrl_a() {
        let mut t = TextArea::from_str("hi");
        let ev = KeyEvent {
            key: Key::Character('a'),
            modifiers: Modifiers {
                shift: false,
                ctrl: true,
                alt: false,
            },
        };
        t.apply_key_event(&ev).unwrap();
        assert_eq!(t.selection_range_bytes(), Some((0, 2)));
    }

    #[test]
    fn alt_shift_block_columns() {
        let mut t = TextArea::from_str("abcd\nefgh");
        t.set_cursor(0);
        let m = Modifiers {
            shift: true,
            ctrl: false,
            alt: true,
        };
        t.apply_key_event(&KeyEvent::with_modifiers(Key::Right, m)).unwrap();
        t.apply_key_event(&KeyEvent::with_modifiers(Key::Right, m)).unwrap();
        t.apply_key_event(&KeyEvent::with_modifiers(Key::Down, m)).unwrap();
        assert!(t.is_block_selection());
        let h0 = t.selection_highlight_on_line(0).unwrap();
        let h1 = t.selection_highlight_on_line(1).unwrap();
        // Columns [0, 3): chars at visual columns 0,1,2 after two Shift+Alt+Right from col 0.
        assert_eq!(h0, (0, 3));
        assert_eq!(h1, (0, 3));
    }
}
