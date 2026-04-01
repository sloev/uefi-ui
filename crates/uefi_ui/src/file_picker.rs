//! Load / save file picker **state** (populate `entries` from [`FileIo`] or UEFI).
//!
//! Composite behavior: [`FilePickerState::reload`], list navigation + [`FilePickerState::interact`]
//! mirror [`crate::widgets::ListBox`] patterns (scroll slice + keyboard).
//!
//! [`LineInput`] is a single-line text input widget state used for the filename field.

use alloc::string::String;
use alloc::vec::Vec;

use crate::input::{Key, KeyEvent};

/// One row in the directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerMode {
    Load,
    Save,
}

/// Abstract filesystem for tests and for UEFI adapters.
pub trait FileIo {
    type Error: core::fmt::Debug;

    /// List `path` (slash-separated components, empty = volume root).
    fn list(&mut self, path: &[String]) -> Result<Vec<DirEntry>, Self::Error>;

    fn read_file(&mut self, path: &[String], name: &str) -> Result<Vec<u8>, Self::Error>;

    fn write_file(&mut self, path: &[String], name: &str, data: &[u8]) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone)]
pub struct FilePickerState {
    pub path: Vec<String>,
    /// Navigation cannot go above this path. Empty = no restriction (volume root is the limit).
    pub root: Vec<String>,
    pub entries: Vec<DirEntry>,
    pub selected: usize,
    /// First visible row when the listing is taller than the on-screen rows.
    pub scroll_top: usize,
    pub mode: PickerMode,
    /// Filename field when [`PickerMode::Save`].
    pub save_as: String,
}

impl FilePickerState {
    pub fn new(mode: PickerMode) -> Self {
        Self {
            path: Vec::new(),
            root: Vec::new(),
            entries: Vec::new(),
            selected: 0,
            scroll_top: 0,
            mode,
            save_as: String::new(),
        }
    }

    pub fn new_rooted(mode: PickerMode, root: Vec<String>) -> Self {
        let path = root.clone();
        Self {
            path,
            root,
            entries: Vec::new(),
            selected: 0,
            scroll_top: 0,
            mode,
            save_as: String::new(),
        }
    }

    /// Returns true when `path` is at the root level (cannot navigate further up).
    pub fn at_root(&self) -> bool {
        self.path.len() <= self.root.len()
    }

    /// Refresh `entries` from `io`, prepend `..` when not at root, sort dirs then files.
    pub fn reload<F: FileIo>(&mut self, io: &mut F) -> Result<(), F::Error> {
        let mut entries = io.list(&self.path)?;
        sort_dir_entries(&mut entries);
        if !self.at_root() {
            entries.insert(
                0,
                DirEntry {
                    name: String::from(".."),
                    is_dir: true,
                },
            );
        }
        self.entries = entries;
        self.scroll_top = 0;
        self.selected = self
            .selected
            .min(self.entries.len().saturating_sub(1));
        Ok(())
    }

    /// Move selection up; keeps `scroll_top` in range for `visible_rows`.
    pub fn nav_up(&mut self, visible_rows: usize) {
        let n = self.entries.len();
        if n == 0 {
            return;
        }
        if self.selected > 0 {
            self.selected -= 1;
        }
        if self.selected < self.scroll_top {
            self.scroll_top = self.selected;
        }
        let _ = visible_rows;
    }

    /// Move selection down; scrolls when the selection moves past the visible window.
    pub fn nav_down(&mut self, visible_rows: usize) {
        let n = self.entries.len();
        if n == 0 {
            return;
        }
        if self.selected + 1 < n {
            self.selected += 1;
        }
        let vr = visible_rows.max(1);
        if self.selected >= self.scroll_top + vr {
            self.scroll_top = (self.selected + 1).saturating_sub(vr);
        }
        let max_top = n.saturating_sub(vr);
        if self.scroll_top > max_top {
            self.scroll_top = max_top;
        }
    }

    /// One keyboard step: navigation, activate selection, or parent directory (Backspace).
    /// Reloads from `io` after path changes.
    pub fn interact<F: FileIo>(
        &mut self,
        io: &mut F,
        ev: &KeyEvent,
        visible_rows: usize,
    ) -> Result<Option<FilePickerAction>, F::Error> {
        match ev.key {
            Key::Up => {
                self.nav_up(visible_rows);
                Ok(None)
            }
            Key::Down => {
                self.nav_down(visible_rows);
                Ok(None)
            }
            Key::Enter => {
                let a = self.activate();
                match a {
                    FilePickerAction::Navigated => {
                        self.reload(io)?;
                        Ok(Some(FilePickerAction::Navigated))
                    }
                    FilePickerAction::PickedFile(_) => Ok(Some(a)),
                    FilePickerAction::None => Ok(None),
                }
            }
            Key::Backspace => {
                if self.at_root() {
                    return Ok(None);
                }
                self.path.pop();
                self.selected = 0;
                self.scroll_top = 0;
                self.reload(io)?;
                Ok(Some(FilePickerAction::Navigated))
            }
            _ => Ok(None),
        }
    }

    pub fn selected_name(&self) -> Option<&str> {
        self.entries.get(self.selected).map(|e| e.name.as_str())
    }

    /// Enter directory or pick file depending on selection.
    pub fn activate(&mut self) -> FilePickerAction {
        let Some(e) = self.entries.get(self.selected) else {
            return FilePickerAction::None;
        };
        if e.is_dir {
            if e.name == ".." {
                self.path.pop();
            } else {
                self.path.push(e.name.clone());
            }
            self.selected = 0;
            FilePickerAction::Navigated
        } else {
            FilePickerAction::PickedFile(e.name.clone())
        }
    }

    /// Confirm save using `save_as` in current `path`.
    pub fn confirm_save(&self) -> Option<FilePickerAction> {
        if self.mode != PickerMode::Save {
            return None;
        }
        let name = self.save_as.trim();
        if name.is_empty() {
            return None;
        }
        Some(FilePickerAction::PickedFile(String::from(name)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilePickerAction {
    None,
    Navigated,
    PickedFile(String),
}

// ── LineInput ─────────────────────────────────────────────────────────────────

/// Single-line text input state — filename field in the file picker dialog.
///
/// Handles printable characters, Backspace (delete-left), Delete (delete-right),
/// Left / Right arrows, Home / End.  No newlines accepted.
#[derive(Debug, Clone)]
pub struct LineInput {
    pub text: String,
    /// Byte offset of the cursor; always on a UTF-8 char boundary.
    pub cursor: usize,
}

impl LineInput {
    pub fn new() -> Self {
        Self { text: String::new(), cursor: 0 }
    }

    pub fn from_str(s: &str) -> Self {
        Self { text: String::from(s), cursor: s.len() }
    }

    /// Apply one key; returns `true` if the text changed.
    pub fn apply_key(&mut self, key: Key) -> bool {
        match key {
            Key::Character(c) => {
                self.text.insert(self.cursor, c);
                self.cursor += c.len_utf8();
                true
            }
            Key::Backspace => {
                if self.cursor == 0 { return false; }
                let prev = prev_char_byte(&self.text, self.cursor);
                self.text.replace_range(prev..self.cursor, "");
                self.cursor = prev;
                true
            }
            Key::Delete => {
                if self.cursor >= self.text.len() { return false; }
                let next = next_char_byte(&self.text, self.cursor);
                self.text.replace_range(self.cursor..next, "");
                true
            }
            Key::Left => {
                if self.cursor > 0 {
                    self.cursor = prev_char_byte(&self.text, self.cursor);
                }
                false
            }
            Key::Right => {
                if self.cursor < self.text.len() {
                    self.cursor = next_char_byte(&self.text, self.cursor);
                }
                false
            }
            Key::Home => { self.cursor = 0; false }
            Key::End  => { self.cursor = self.text.len(); false }
            _ => false,
        }
    }

    /// Column index (char count) of the cursor.
    pub fn cursor_col(&self) -> usize {
        self.text[..self.cursor].chars().count()
    }
}

impl Default for LineInput {
    fn default() -> Self { Self::new() }
}

fn prev_char_byte(s: &str, mut i: usize) -> usize {
    if i == 0 { return 0; }
    i -= 1;
    while i > 0 && !s.is_char_boundary(i) { i -= 1; }
    i
}

fn next_char_byte(s: &str, mut i: usize) -> usize {
    if i >= s.len() { return s.len(); }
    i += 1;
    while i < s.len() && !s.is_char_boundary(i) { i += 1; }
    i
}

// ── Focus zone + dialog coordinator ───────────────────────────────────────────

/// Which interactive zone of the file picker dialog currently has keyboard focus.
///
/// Tab / Shift+Tab cycles through this order:
/// `List → FilenameField → FiletypeDropdown → OkButton → CancelButton → List`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerFocus {
    List,
    FilenameField,
    FiletypeDropdown,
    OkButton,
    CancelButton,
}

impl FilePickerFocus {
    const ORDER: [FilePickerFocus; 5] = [
        FilePickerFocus::List,
        FilePickerFocus::FilenameField,
        FilePickerFocus::FiletypeDropdown,
        FilePickerFocus::OkButton,
        FilePickerFocus::CancelButton,
    ];

    /// Advance focus (Tab).
    pub fn next(self) -> Self {
        let i = Self::ORDER.iter().position(|&f| f == self).unwrap_or(0);
        Self::ORDER[(i + 1) % Self::ORDER.len()]
    }

    /// Retreat focus (Shift+Tab).
    pub fn prev(self) -> Self {
        let i = Self::ORDER.iter().position(|&f| f == self).unwrap_or(0);
        Self::ORDER[(i + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
}

/// Outcome returned by [`FilePickerDialogState::handle_key`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilePickerDialogAction {
    /// Nothing to report — keep showing the dialog.
    None,
    /// User confirmed a selection: full path components + filename.
    Confirm {
        /// Directory path (e.g. `["EFI", "Boot"]`).
        path: Vec<String>,
        /// File name (e.g. `"BOOTX64.EFI"`).
        name: String,
    },
    /// User pressed Cancel or Escape.
    Cancel,
}

/// Complete file picker dialog state — owns [`FilePickerState`], [`LineInput`],
/// file-type selection, and focus.
///
/// Call [`handle_key`](Self::handle_key) every key event; call
/// [`crate::bedrock_controls::draw_file_picker`] every frame, passing
/// `self.focus` and `&self.filename.text`.
///
/// ```ignore
/// let mut dlg = FilePickerDialogState::new(PickerMode::Load, 3, io)?;
/// // per-frame:
/// match dlg.handle_key(ev, visible_rows)? {
///     FilePickerDialogAction::Confirm { path, name } => { /* use it */ }
///     FilePickerDialogAction::Cancel => { /* dismiss */ }
///     FilePickerDialogAction::None => {}
/// }
/// ```
#[derive(Debug, Clone)]
pub struct FilePickerDialogState {
    pub picker: FilePickerState,
    /// Filename text field (pre-populated from selected file on Enter).
    pub filename: LineInput,
    /// Currently selected index in the file-type dropdown.
    pub filetype_sel: usize,
    /// Total number of file-type options (for wrapping arrow keys).
    pub filetype_count: usize,
    /// Which zone has keyboard focus.
    pub focus: FilePickerFocus,
    /// Last directory navigated to; persisted by the caller between dialog open/close cycles.
    /// On Confirm or navigation it is updated to the current path so the caller can save it.
    pub last_dir: Vec<String>,
}

impl FilePickerDialogState {
    /// Create and immediately load the root listing from `io`.
    pub fn new<F: FileIo>(
        mode: PickerMode,
        filetype_count: usize,
        io: &mut F,
    ) -> Result<Self, F::Error> {
        Self::with_root(mode, filetype_count, Vec::new(), Vec::new(), io)
    }

    /// Create with a navigation root (cannot navigate above `root`) and an optional
    /// previously-visited directory (`last_dir`) to restore on open.
    ///
    /// If `last_dir` is non-empty the picker starts there; otherwise it starts at `root`.
    pub fn with_root<F: FileIo>(
        mode: PickerMode,
        filetype_count: usize,
        root: Vec<String>,
        last_dir: Vec<String>,
        io: &mut F,
    ) -> Result<Self, F::Error> {
        let mut picker = FilePickerState::new_rooted(mode, root);
        // If last_dir is deeper than root, restore it; otherwise stay at root.
        if last_dir.len() > picker.root.len() {
            picker.path = last_dir.clone();
        }
        picker.reload(io)?;
        Ok(Self {
            last_dir: picker.path.clone(),
            picker,
            filename: LineInput::new(),
            filetype_sel: 0,
            filetype_count: filetype_count.max(1),
            focus: FilePickerFocus::List,
        })
    }

    /// Route a key event to the focused sub-widget.
    ///
    /// Returns `Ok(FilePickerDialogAction::Confirm { .. })` when the user confirms
    /// a selection, `Ok(FilePickerDialogAction::Cancel)` when they dismiss, or
    /// `Ok(FilePickerDialogAction::None)` otherwise.
    ///
    /// `visible_rows` — number of rows visible in the list (from
    /// [`crate::bedrock_controls::FilePickerLayout::visible_rows`]).
    pub fn handle_key<F: FileIo>(
        &mut self,
        ev: &KeyEvent,
        visible_rows: usize,
        io: &mut F,
    ) -> Result<FilePickerDialogAction, F::Error> {
        let k = ev.key;
        let shift = ev.modifiers.shift;

        // ── Global keys ───────────────────────────────────────────────────────
        if k == Key::Escape {
            return Ok(FilePickerDialogAction::Cancel);
        }
        if k == Key::Tab {
            self.focus = if shift { self.focus.prev() } else { self.focus.next() };
            return Ok(FilePickerDialogAction::None);
        }

        // ── Per-zone routing ──────────────────────────────────────────────────
        match self.focus {
            FilePickerFocus::List => {
                match k {
                    Key::Enter => {
                        let action = self.picker.activate();
                        match action {
                            FilePickerAction::Navigated => {
                                self.picker.reload(io)?;
                                self.last_dir = self.picker.path.clone();
                                self.filename.text.clear();
                                self.filename.cursor = 0;
                            }
                            FilePickerAction::PickedFile(ref name) => {
                                // Populate filename field and move focus there
                                self.filename = LineInput::from_str(name);
                                self.focus = FilePickerFocus::FilenameField;
                            }
                            FilePickerAction::None => {}
                        }
                    }
                    Key::Backspace => {
                        if !self.picker.at_root() {
                            self.picker.path.pop();
                            self.picker.selected = 0;
                            self.picker.scroll_top = 0;
                            self.picker.reload(io)?;
                            self.last_dir = self.picker.path.clone();
                        }
                    }
                    Key::Up => self.picker.nav_up(visible_rows),
                    Key::Down => self.picker.nav_down(visible_rows),
                    _ => {}
                }
            }

            FilePickerFocus::FilenameField => {
                match k {
                    Key::Enter => return Ok(self.try_confirm()),
                    _ => { self.filename.apply_key(k); }
                }
            }

            FilePickerFocus::FiletypeDropdown => {
                match k {
                    Key::Up | Key::Left => {
                        if self.filetype_sel > 0 { self.filetype_sel -= 1; }
                    }
                    Key::Down | Key::Right => {
                        if self.filetype_sel + 1 < self.filetype_count {
                            self.filetype_sel += 1;
                        }
                    }
                    _ => {}
                }
            }

            FilePickerFocus::OkButton => {
                if matches!(k, Key::Enter | Key::Character(' ')) {
                    return Ok(self.try_confirm());
                }
            }

            FilePickerFocus::CancelButton => {
                if matches!(k, Key::Enter | Key::Character(' ')) {
                    return Ok(FilePickerDialogAction::Cancel);
                }
            }
        }

        Ok(FilePickerDialogAction::None)
    }

    /// Attempt to confirm: filename field must be non-empty, or a file must be selected.
    fn try_confirm(&mut self) -> FilePickerDialogAction {
        let name = String::from(self.filename.text.trim());
        if !name.is_empty() {
            self.last_dir = self.picker.path.clone();
            return FilePickerDialogAction::Confirm {
                path: self.picker.path.clone(),
                name,
            };
        }
        // Fall back to selected entry if it is a file
        if let Some(entry) = self.picker.entries.get(self.picker.selected) {
            if !entry.is_dir {
                self.last_dir = self.picker.path.clone();
                return FilePickerDialogAction::Confirm {
                    path: self.picker.path.clone(),
                    name: entry.name.clone(),
                };
            }
        }
        FilePickerDialogAction::None
    }
}

fn sort_dir_entries(entries: &mut Vec<DirEntry>) {
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => core::cmp::Ordering::Less,
        (false, true) => core::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEvent;

    struct TreeFs;
    impl FileIo for TreeFs {
        type Error = ();
        fn list(&mut self, path: &[String]) -> Result<Vec<DirEntry>, ()> {
            if path.is_empty() {
                return Ok(alloc::vec![
                    DirEntry {
                        name: String::from("z_dir"),
                        is_dir: true,
                    },
                    DirEntry {
                        name: String::from("a.txt"),
                        is_dir: false,
                    },
                ]);
            }
            if path == [String::from("z_dir")] {
                return Ok(alloc::vec![DirEntry {
                    name: String::from("nested.txt"),
                    is_dir: false,
                }]);
            }
            Ok(alloc::vec![])
        }
        fn read_file(&mut self, _: &[String], _: &str) -> Result<Vec<u8>, ()> {
            Ok(alloc::vec![])
        }
        fn write_file(&mut self, _: &[String], _: &str, _: &[u8]) -> Result<(), ()> {
            Ok(())
        }
    }

    #[test]
    fn pick_file() {
        let mut p = FilePickerState::new(PickerMode::Load);
        p.entries = alloc::vec![
            DirEntry {
                name: String::from(".."),
                is_dir: true,
            },
            DirEntry {
                name: String::from("cfg.txt"),
                is_dir: false,
            },
        ];
        p.selected = 1;
        assert_eq!(
            p.activate(),
            FilePickerAction::PickedFile(String::from("cfg.txt"))
        );
    }

    #[test]
    fn reload_sorts_dirs_before_files_and_prepends_parent() {
        let mut p = FilePickerState::new(PickerMode::Load);
        p.path.push(String::from("z_dir"));
        let mut fs = TreeFs;
        p.reload(&mut fs).unwrap();
        assert_eq!(p.entries[0].name, "..");
        assert_eq!(p.entries[1].name, "nested.txt");
    }

    #[test]
    fn interact_opens_dir_and_picks_file() {
        let mut p = FilePickerState::new(PickerMode::Load);
        let mut fs = TreeFs;
        p.reload(&mut fs).unwrap();
        // sorted: z_dir (dir), a.txt — dirs first
        assert_eq!(p.entries[0].name, "z_dir");
        p.selected = 0;
        assert_eq!(
            p.interact(&mut fs, &KeyEvent::new(Key::Enter), 8).unwrap(),
            Some(FilePickerAction::Navigated)
        );
        assert_eq!(p.path, alloc::vec![String::from("z_dir")]);
        assert_eq!(p.entries[1].name, "nested.txt");
        p.selected = 1;
        assert_eq!(
            p.interact(&mut fs, &KeyEvent::new(Key::Enter), 8).unwrap(),
            Some(FilePickerAction::PickedFile(String::from("nested.txt")))
        );
    }

    // ── FilePickerDialogState tests ───────────────────────────────────────────

    #[test]
    fn focus_cycles_with_tab() {
        use crate::input::Modifiers;
        let mut dlg = FilePickerDialogState::new(PickerMode::Load, 2, &mut TreeFs).unwrap();
        assert_eq!(dlg.focus, FilePickerFocus::List);
        dlg.handle_key(&KeyEvent::new(Key::Tab), 8, &mut TreeFs).unwrap();
        assert_eq!(dlg.focus, FilePickerFocus::FilenameField);
        dlg.handle_key(&KeyEvent::new(Key::Tab), 8, &mut TreeFs).unwrap();
        assert_eq!(dlg.focus, FilePickerFocus::FiletypeDropdown);
        dlg.handle_key(&KeyEvent::new(Key::Tab), 8, &mut TreeFs).unwrap();
        assert_eq!(dlg.focus, FilePickerFocus::OkButton);
        dlg.handle_key(&KeyEvent::new(Key::Tab), 8, &mut TreeFs).unwrap();
        assert_eq!(dlg.focus, FilePickerFocus::CancelButton);
        dlg.handle_key(&KeyEvent::new(Key::Tab), 8, &mut TreeFs).unwrap();
        assert_eq!(dlg.focus, FilePickerFocus::List); // wraps back
        // Shift+Tab goes backwards
        dlg.handle_key(
            &KeyEvent::with_modifiers(Key::Tab, Modifiers { shift: true, ..Default::default() }),
            8, &mut TreeFs,
        ).unwrap();
        assert_eq!(dlg.focus, FilePickerFocus::CancelButton);
    }

    #[test]
    fn escape_returns_cancel() {
        let mut dlg = FilePickerDialogState::new(PickerMode::Load, 1, &mut TreeFs).unwrap();
        let action = dlg.handle_key(&KeyEvent::new(Key::Escape), 8, &mut TreeFs).unwrap();
        assert_eq!(action, FilePickerDialogAction::Cancel);
    }

    #[test]
    fn enter_on_file_populates_filename_and_shifts_focus() {
        let mut dlg = FilePickerDialogState::new(PickerMode::Load, 1, &mut TreeFs).unwrap();
        // entries: z_dir (0), a.txt (1)
        dlg.picker.selected = 1; // a.txt
        dlg.handle_key(&KeyEvent::new(Key::Enter), 8, &mut TreeFs).unwrap();
        assert_eq!(dlg.filename.text, "a.txt");
        assert_eq!(dlg.focus, FilePickerFocus::FilenameField);
    }

    #[test]
    fn ok_button_confirms_filename_text() {
        let mut dlg = FilePickerDialogState::new(PickerMode::Load, 1, &mut TreeFs).unwrap();
        dlg.filename = LineInput::from_str("a.txt");
        dlg.focus = FilePickerFocus::OkButton;
        let action = dlg.handle_key(&KeyEvent::new(Key::Enter), 8, &mut TreeFs).unwrap();
        assert!(matches!(action, FilePickerDialogAction::Confirm { ref name, .. } if name == "a.txt"));
    }

    #[test]
    fn cancel_button_returns_cancel() {
        let mut dlg = FilePickerDialogState::new(PickerMode::Load, 1, &mut TreeFs).unwrap();
        dlg.focus = FilePickerFocus::CancelButton;
        let action = dlg.handle_key(&KeyEvent::new(Key::Enter), 8, &mut TreeFs).unwrap();
        assert_eq!(action, FilePickerDialogAction::Cancel);
    }

    #[test]
    fn enter_on_dir_navigates() {
        let mut dlg = FilePickerDialogState::new(PickerMode::Load, 1, &mut TreeFs).unwrap();
        dlg.picker.selected = 0; // z_dir
        dlg.handle_key(&KeyEvent::new(Key::Enter), 8, &mut TreeFs).unwrap();
        assert_eq!(dlg.picker.path, alloc::vec![String::from("z_dir")]);
        assert_eq!(dlg.focus, FilePickerFocus::List);
    }

    #[test]
    fn filetype_arrow_cycles() {
        let mut dlg = FilePickerDialogState::new(PickerMode::Load, 3, &mut TreeFs).unwrap();
        dlg.focus = FilePickerFocus::FiletypeDropdown;
        assert_eq!(dlg.filetype_sel, 0);
        dlg.handle_key(&KeyEvent::new(Key::Down), 8, &mut TreeFs).unwrap();
        assert_eq!(dlg.filetype_sel, 1);
        dlg.handle_key(&KeyEvent::new(Key::Up), 8, &mut TreeFs).unwrap();
        assert_eq!(dlg.filetype_sel, 0);
        dlg.handle_key(&KeyEvent::new(Key::Up), 8, &mut TreeFs).unwrap();
        assert_eq!(dlg.filetype_sel, 0); // clamps at 0
    }

    #[test]
    fn line_input_typing_and_backspace() {
        let mut inp = LineInput::new();
        inp.apply_key(Key::Character('h'));
        inp.apply_key(Key::Character('i'));
        assert_eq!(inp.text, "hi");
        inp.apply_key(Key::Backspace);
        assert_eq!(inp.text, "h");
        assert_eq!(inp.cursor_col(), 1);
    }

    #[test]
    fn backspace_pops_path() {
        let mut p = FilePickerState::new(PickerMode::Load);
        let mut fs = TreeFs;
        p.reload(&mut fs).unwrap();
        p.selected = 0;
        p.interact(&mut fs, &KeyEvent::new(Key::Enter), 8).unwrap();
        assert_eq!(p.path.len(), 1);
        assert_eq!(
            p.interact(&mut fs, &KeyEvent::new(Key::Backspace), 8).unwrap(),
            Some(FilePickerAction::Navigated)
        );
        assert!(p.path.is_empty());
    }

    // ── Root clamping (T-29) ──────────────────────────────────────────────────

    #[test]
    fn root_clamps_backspace() {
        // Root is set to ["z_dir"]; picker starts there and cannot navigate above.
        let root = alloc::vec![String::from("z_dir")];
        let mut p = FilePickerState::new_rooted(PickerMode::Load, root.clone());
        let mut fs = TreeFs;
        p.reload(&mut fs).unwrap();
        assert!(p.at_root(), "should be at root on creation");
        // Backspace at root does nothing
        let r = p.interact(&mut fs, &KeyEvent::new(Key::Backspace), 8).unwrap();
        assert_eq!(r, None);
        assert_eq!(p.path, root, "path must not change");
    }

    #[test]
    fn root_no_parent_entry_shown() {
        // At root, no ".." entry should appear.
        let root = alloc::vec![String::from("z_dir")];
        let mut p = FilePickerState::new_rooted(PickerMode::Load, root);
        let mut fs = TreeFs;
        p.reload(&mut fs).unwrap();
        assert!(p.entries.iter().all(|e| e.name != ".."), ".. must not appear at root");
    }

    #[test]
    fn root_empty_means_volume_root_unclamped() {
        // With empty root, ".." appears when path is non-empty (original behavior).
        let mut p = FilePickerState::new(PickerMode::Load);
        let mut fs = TreeFs;
        p.path.push(String::from("z_dir"));
        p.reload(&mut fs).unwrap();
        assert!(p.entries.iter().any(|e| e.name == ".."), ".. must appear below volume root");
    }

    #[test]
    fn with_root_restores_last_dir() {
        let root = alloc::vec![];
        let last = alloc::vec![String::from("z_dir")];
        let mut fs = TreeFs;
        let dlg = FilePickerDialogState::with_root(PickerMode::Load, 1, root, last.clone(), &mut fs).unwrap();
        assert_eq!(dlg.picker.path, last, "should start in last_dir");
        assert_eq!(dlg.last_dir, last);
    }

    #[test]
    fn last_dir_updated_on_navigation() {
        let mut dlg = FilePickerDialogState::new(PickerMode::Load, 1, &mut TreeFs).unwrap();
        // Navigate into z_dir
        dlg.picker.selected = 0; // z_dir
        dlg.handle_key(&KeyEvent::new(Key::Enter), 8, &mut TreeFs).unwrap();
        assert_eq!(dlg.last_dir, alloc::vec![String::from("z_dir")]);
    }

    #[test]
    fn last_dir_updated_on_confirm() {
        let mut dlg = FilePickerDialogState::new(PickerMode::Load, 1, &mut TreeFs).unwrap();
        dlg.picker.selected = 1; // a.txt
        dlg.handle_key(&KeyEvent::new(Key::Enter), 8, &mut TreeFs).unwrap();
        // Now in FilenameField with "a.txt"
        let action = dlg.handle_key(&KeyEvent::new(Key::Enter), 8, &mut TreeFs).unwrap();
        assert!(matches!(action, FilePickerDialogAction::Confirm { .. }));
        assert_eq!(dlg.last_dir, alloc::vec![] as Vec<String>, "confirmed at root");
    }
}
