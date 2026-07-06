//! Port of the `fdoom.screen` package: displays (menus/screens) and their widgets.

pub mod display;
pub mod entry;
pub mod menu;
pub mod rel_pos;
pub mod splash_menu;
pub mod title_display;

pub use display::{Display, DisplayBase};
pub use menu::{Menu, MenuBuilder};
pub use rel_pos::RelPos;

/// Java `new Display(clearScreen, canExit, menus...)` used directly (no subclass).
pub struct PlainDisplay {
    base: DisplayBase,
}

impl Display for PlainDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }
}

pub fn plain_display(clear_screen: bool, can_exit: bool, menus: Vec<Menu>) -> PlainDisplay {
    PlainDisplay { base: DisplayBase::new(clear_screen, can_exit, menus) }
}
