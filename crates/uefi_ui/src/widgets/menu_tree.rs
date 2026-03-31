//! Hierarchical menu (menus + submenus) with keyboard navigation.

use crate::input::{Key, KeyEvent};

/// One row in the tree (leaf or nested submenu).
#[derive(Debug, Clone)]
pub enum MenuEntry<'a> {
    Item {
        label: &'a str,
        id: u32,
    },
    Submenu {
        label: &'a str,
        children: &'a [MenuEntry<'a>],
    },
}

/// Static tree description (data only).
#[derive(Debug, Clone)]
pub struct MenuTree<'a> {
    pub roots: &'a [MenuEntry<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Activated(u32),
    SubmenuOpened,
    SubmenuClosed,
    Moved,
}

/// Focus: either a root index, or inside `(root_i, child_i)` submenu.
#[derive(Debug, Clone, Default)]
pub struct MenuNavigator {
    pub root_index: usize,
    /// When `Some`, keyboard moves among submenu entries.
    pub sub: Option<(usize, usize)>,
}

impl MenuNavigator {
    fn n_roots(&self, tree: &MenuTree<'_>) -> usize {
        tree.roots.len()
    }

    pub fn apply_key(&mut self, tree: &MenuTree<'_>, key: Key) -> Option<MenuAction> {
        self.apply_key_event(tree, &KeyEvent::new(key))
    }

    pub fn apply_key_event(&mut self, tree: &MenuTree<'_>, ev: &KeyEvent) -> Option<MenuAction> {
        let key = ev.key;
        let nr = self.n_roots(tree);
        if nr == 0 {
            return None;
        }
        match key {
            Key::Up | Key::Down => {
                if let Some((ri, si)) = self.sub {
                    let children = match tree.roots.get(ri) {
                        Some(MenuEntry::Submenu { children, .. }) => children,
                        _ => return None,
                    };
                    let nc = children.len();
                    if nc == 0 {
                        return None;
                    }
                    let delta = if matches!(key, Key::Down) { 1isize } else { -1 };
                    let nsi = (si as isize + delta).clamp(0, nc as isize - 1) as usize;
                    self.sub = Some((ri, nsi));
                    Some(MenuAction::Moved)
                } else {
                    let delta = if matches!(key, Key::Down) { 1isize } else { -1 };
                    self.root_index = (self.root_index as isize + delta).clamp(0, nr as isize - 1)
                        as usize;
                    Some(MenuAction::Moved)
                }
            }
            Key::Right | Key::Enter => {
                if let Some((ri, si)) = self.sub {
                    let children = match tree.roots.get(ri) {
                        Some(MenuEntry::Submenu { children, .. }) => children,
                        _ => return None,
                    };
                    return match children.get(si) {
                        Some(MenuEntry::Item { id, .. }) => Some(MenuAction::Activated(*id)),
                        Some(MenuEntry::Submenu { .. }) => None,
                        None => None,
                    };
                }
                let cur = tree.roots.get(self.root_index)?;
                match cur {
                    MenuEntry::Submenu { children, .. } if !children.is_empty() => {
                        self.sub = Some((self.root_index, 0));
                        Some(MenuAction::SubmenuOpened)
                    }
                    MenuEntry::Item { id, .. } => Some(MenuAction::Activated(*id)),
                    _ => None,
                }
            }
            Key::Left | Key::Escape => {
                if self.sub.take().is_some() {
                    Some(MenuAction::SubmenuClosed)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_submenu_and_activate() {
        let children = [MenuEntry::Item {
            label: "x",
            id: 99,
        }];
        let roots = [
            MenuEntry::Submenu {
                label: "File",
                children: &children,
            },
            MenuEntry::Item {
                label: "Quit",
                id: 1,
            },
        ];
        let tree = MenuTree { roots: &roots };
        let mut nav = MenuNavigator::default();
        assert_eq!(
            nav.apply_key(&tree, Key::Right),
            Some(MenuAction::SubmenuOpened)
        );
        assert_eq!(
            nav.apply_key(&tree, Key::Enter),
            Some(MenuAction::Activated(99))
        );
    }
}
