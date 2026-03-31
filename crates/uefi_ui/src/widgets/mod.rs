//! Immediate-mode widget **state** (no drawing): pair with `embedded_graphics` + [`crate::theme::Theme`] in your app.
//!
//! Layout: [`crate::layout`] (row/column/grid panels). Overlays: [`crate::popover`].
//!
//! **Composite widgets:** [`DateSelect`] (calendar field); directory picking lives in [`crate::file_picker`] ([`crate::file_picker::FilePickerState`] + [`crate::file_picker::FileIo`]).

mod button;
mod checkbox;
mod date_select;
mod dropdown;
mod graph;
mod icon;
mod icon_button;
mod label;
mod listbox;
mod menu_tree;
mod navbar;
mod number_field;
mod progress;
mod radio;
mod scrollbar;
mod scroll_area;
mod slider;
mod spacer;
mod textarea;
mod toggle;

pub use button::Button;
pub use checkbox::Checkbox;
pub use date_select::{days_in_month, is_leap_year, DateField, DateSelect, DateSelectAction};
pub use dropdown::{Dropdown, DropdownAction};
pub use graph::LineGraph;
pub use icon::Icon;
pub use icon_button::IconButton;
pub use label::Label;
pub use listbox::ListBox;
pub use menu_tree::{MenuAction, MenuEntry, MenuNavigator, MenuTree};
pub use navbar::NavBar;
pub use number_field::NumberField;
pub use progress::ProgressBar;
pub use radio::RadioGroup;
pub use scroll_area::ScrollArea;
pub use scrollbar::{ScrollAxis, ScrollbarHit, ScrollbarState, textarea_sync_vertical_scroll};
pub use slider::Slider;
pub use spacer::Spacer;
pub use textarea::{TextArea, TextAreaAction};
pub use toggle::Toggle;

pub use crate::file_picker::{
    DirEntry, FileIo, FilePickerAction, FilePickerDialogAction, FilePickerDialogState,
    FilePickerFocus, FilePickerState, LineInput, PickerMode,
};
pub use crate::tree_view::{FlatRow, TreeNode, TreeViewState};
