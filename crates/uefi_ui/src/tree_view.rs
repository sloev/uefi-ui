//! Directory tree state for the file picker left pane (File Browser).
//!
//! Build a [`TreeViewState`] from your filesystem, then pass it to
//! [`crate::bedrock_controls::draw_tree_view`] each frame.

use alloc::string::String;
use alloc::vec::Vec;

use crate::input::Key;

/// One node in the directory tree.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Display label (e.g. folder name).
    pub label: String,
    /// Path component for this node (joined to build a path for [`crate::file_picker::FileIo`]).
    pub path_component: String,
    /// Child nodes (populated lazily when `expanded` is set to `true`).
    pub children: Vec<TreeNode>,
    /// Whether the subtree is currently shown.
    pub expanded: bool,
}

impl TreeNode {
    pub fn new(label: impl Into<String>, path_component: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            path_component: path_component.into(),
            children: Vec::new(),
            expanded: false,
        }
    }

    /// Convenience: mark expanded and give children in one call.
    pub fn with_children(mut self, children: Vec<TreeNode>) -> Self {
        self.children = children;
        self
    }
}

/// Full tree state: roots + selected path + vertical scroll.
#[derive(Debug, Clone)]
pub struct TreeViewState {
    pub roots: Vec<TreeNode>,
    /// Path components of the currently selected node (empty = nothing selected).
    pub selected_path: Vec<String>,
    /// First visible flat row (0-based) for vertical scrolling.
    pub scroll_top: usize,
}

impl TreeViewState {
    pub fn new(roots: Vec<TreeNode>) -> Self {
        Self { roots, selected_path: Vec::new(), scroll_top: 0 }
    }

    /// Count total visible (flattened) rows — used to size the scrollbar.
    pub fn flat_row_count(&self) -> usize {
        count_rows(&self.roots)
    }

    /// Move selection to the next/previous visible row (Up/Down arrows).
    pub fn apply_key(&mut self, key: Key, visible_rows: usize) {
        match key {
            Key::Up => self.nav(-1, visible_rows),
            Key::Down => self.nav(1, visible_rows),
            Key::Left => self.collapse_selected(),
            Key::Right => self.expand_selected(),
            _ => {}
        }
    }

    fn nav(&mut self, delta: isize, visible_rows: usize) {
        let rows = self.flat_rows();
        let current_idx = rows.iter().position(|r| r.path == self.selected_path);
        let n = rows.len();
        if n == 0 { return; }
        let new_idx = match current_idx {
            None => 0,
            Some(i) => ((i as isize + delta).clamp(0, n as isize - 1)) as usize,
        };
        self.selected_path = rows[new_idx].path.clone();
        // Keep scroll window
        if new_idx < self.scroll_top {
            self.scroll_top = new_idx;
        } else if new_idx >= self.scroll_top + visible_rows.max(1) {
            self.scroll_top = new_idx + 1 - visible_rows.max(1);
        }
    }

    fn collapse_selected(&mut self) {
        let path = self.selected_path.clone();
        set_expanded(&mut self.roots, &path, false);
    }

    fn expand_selected(&mut self) {
        let path = self.selected_path.clone();
        set_expanded(&mut self.roots, &path, true);
    }

    /// Flatten the visible tree into rows for painting.
    pub fn flat_rows(&self) -> Vec<FlatRow> {
        let mut out = Vec::new();
        flatten(&self.roots, &[], 0, 0, &self.selected_path, &mut out);
        out
    }
}

// ── Flat row (used by draw_tree_view) ─────────────────────────────────────────

/// One visible row produced by flattening the tree.
#[derive(Debug, Clone)]
pub struct FlatRow {
    pub level: usize,
    pub label: String,
    pub path: Vec<String>,
    pub has_children: bool,
    pub expanded: bool,
    pub selected: bool,
    /// Bitmask: bit L is set when the vertical connector at indent level L
    /// should extend through this row (i.e. there are more siblings below at level L).
    pub continues_mask: u32,
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn count_rows(nodes: &[TreeNode]) -> usize {
    let mut n = 0;
    for node in nodes {
        n += 1;
        if node.expanded {
            n += count_rows(&node.children);
        }
    }
    n
}

/// Recursively flatten visible nodes into `out`.
/// `parent_path` — path components above this level.
/// `continues_mask` — bitmask of ancestor levels whose connector line still runs.
fn flatten(
    nodes: &[TreeNode],
    parent_path: &[String],
    level: usize,
    continues_mask: u32,
    selected_path: &[String],
    out: &mut Vec<FlatRow>,
) {
    let count = nodes.len();
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == count - 1;
        // Build full path for this node
        let mut path = parent_path.to_vec();
        path.push(node.path_component.clone());

        let selected = path == selected_path;

        // The vertical line at `level` continues through this row only if there are
        // more siblings below (not the last child).
        let my_mask = if is_last {
            continues_mask
        } else {
            continues_mask | (1u32 << level)
        };

        out.push(FlatRow {
            level,
            label: node.label.clone(),
            path: path.clone(),
            has_children: !node.children.is_empty(),
            expanded: node.expanded,
            selected,
            continues_mask: my_mask,
        });

        if node.expanded && !node.children.is_empty() {
            flatten(&node.children, &path, level + 1, my_mask, selected_path, out);
        }
    }
}

fn set_expanded(nodes: &mut Vec<TreeNode>, path: &[String], expanded: bool) {
    if path.is_empty() { return; }
    for node in nodes.iter_mut() {
        if node.path_component == path[0] {
            if path.len() == 1 {
                node.expanded = expanded;
            } else {
                set_expanded(&mut node.children, &path[1..], expanded);
            }
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> TreeViewState {
        TreeViewState::new(vec![
            TreeNode::new("C:", "C:").with_children(vec![
                TreeNode::new("EFI", "EFI").with_children(vec![
                    TreeNode::new("Boot", "Boot"),
                    TreeNode::new("Microsoft", "Microsoft"),
                ]),
                TreeNode::new("Windows", "Windows"),
            ]),
        ])
    }

    #[test]
    fn flat_collapsed() {
        let t = sample_tree();
        assert_eq!(t.flat_row_count(), 1); // only root visible
        let rows = t.flat_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "C:");
    }

    #[test]
    fn flat_expanded_one_level() {
        let mut t = sample_tree();
        t.roots[0].expanded = true;
        let rows = t.flat_rows();
        assert_eq!(rows.len(), 3); // C: + EFI + Windows
        assert_eq!(rows[1].level, 1);
        assert_eq!(rows[1].label, "EFI");
        // EFI is not last → continues_mask has bit 1 set
        assert_eq!(rows[1].continues_mask & 0b10, 0b10);
        // Windows is last → bit 1 not set
        assert_eq!(rows[2].continues_mask & 0b10, 0);
    }

    #[test]
    fn nav_wraps() {
        let mut t = sample_tree();
        t.roots[0].expanded = true;
        t.selected_path = vec![String::from("C:")];
        t.apply_key(Key::Up, 5);
        // already at top — stays at C:
        assert_eq!(t.selected_path, vec![String::from("C:")]);
        t.apply_key(Key::Down, 5);
        assert_eq!(t.selected_path, vec![String::from("C:"), String::from("EFI")]);
    }

    #[test]
    fn expand_collapse_via_key() {
        let mut t = sample_tree();
        t.selected_path = vec![String::from("C:")];
        t.apply_key(Key::Right, 5); // expand C:
        assert!(t.roots[0].expanded);
        t.apply_key(Key::Left, 5);  // collapse C:
        assert!(!t.roots[0].expanded);
    }
}
