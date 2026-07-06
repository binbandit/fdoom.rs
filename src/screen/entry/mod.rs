//! Port of the `fdoom.screen.entry` package — the widget rows that make up menus.
//!
//! Java menus and `Settings` share the *same* mutable entry objects, so entries are held
//! as `Rc<RefCell<dyn ListEntry>>` (`EntryHandle`).

pub mod array_entry;
pub mod blank_entry;
pub mod input_entry;
pub mod item_entry;
pub mod item_listing;
pub mod key_input_entry;
pub mod recipe_entry;
pub mod select_entry;
pub mod string_entry;

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::game::Game;
use crate::gfx::{Screen, color, font};

pub use array_entry::{ArrayEntry, Value};
pub use blank_entry::BlankEntry;
pub use select_entry::SelectEntry;
pub use string_entry::StringEntry;

pub type EntryHandle = Rc<RefCell<dyn ListEntry>>;

pub fn handle(entry: impl ListEntry + 'static) -> EntryHandle {
    Rc::new(RefCell::new(entry))
}

pub const COL_UNSLCT: i32 = color::GRAY;
pub const COL_SLCT: i32 = color::WHITE;

/// Java `ListEntry.getHeight()` (static).
pub fn entry_height() -> i32 {
    font::text_height()
}

/// The `selectable`/`visible` flags from the Java `ListEntry` base class.
#[derive(Debug, Clone, Copy)]
pub struct EntryFlags {
    pub selectable: bool,
    pub visible: bool,
}

impl Default for EntryFlags {
    fn default() -> Self {
        EntryFlags {
            selectable: true,
            visible: true,
        }
    }
}

pub trait ListEntry {
    fn flags(&self) -> EntryFlags;
    fn flags_mut(&mut self) -> &mut EntryFlags;

    /// Java `tick(input)` — input is reached through `g.input`.
    fn tick(&mut self, g: &mut Game);

    /// Java `toString()` — the display text.
    fn to_display_string(&self, g: &Game) -> String;

    /// Java `render(screen, x, y, isSelected)`.
    fn render(&mut self, screen: &mut Screen, g: &mut Game, x: i32, y: i32, is_selected: bool) {
        if self.flags().visible {
            let text = self.to_display_string(g);
            font::draw(&text, screen, x, y, self.get_color(is_selected));
        }
    }

    /// Java `getColor(isSelected)`.
    fn get_color(&self, is_selected: bool) -> i32 {
        if is_selected { COL_SLCT } else { COL_UNSLCT }
    }

    /// Java `getWidth()`.
    fn get_width(&self, g: &Game) -> i32 {
        font::text_width(&self.to_display_string(g))
    }

    /// Java `isSelectable()` — visible && selectable.
    fn is_selectable(&self) -> bool {
        let f = self.flags();
        f.selectable && f.visible
    }

    /// Java `instanceof ArrayEntry` (used by `Display.tick` for shift-arrow navigation).
    fn is_array_entry(&self) -> bool {
        false
    }

    /// Java `instanceof BlankEntry` (used by `Menu.render` to skip arrows/positioning).
    fn is_blank_entry(&self) -> bool {
        false
    }

    /// True for text-entry rows: while selected, letters type instead of navigating, so
    /// the menu only navigates with the physical arrow keys.
    fn captures_typing(&self) -> bool {
        false
    }

    fn set_selectable(&mut self, selectable: bool) {
        self.flags_mut().selectable = selectable;
    }

    fn set_visible(&mut self, visible: bool) {
        self.flags_mut().visible = visible;
    }
}
