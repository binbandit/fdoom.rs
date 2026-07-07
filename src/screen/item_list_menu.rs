//! Port of `fdoom.screen.ItemListMenu` — the shared Menu configuration for item lists.
//! Java made it a Menu subclass; in Rust it's the builder plus constructor functions.

use crate::core::game::Game;
use crate::gfx::{Point, screen};

use super::entry::EntryHandle;
use super::menu::{Menu, MenuBuilder};
use super::rel_pos::RelPos;

/// Java `ItemListMenu.getBuilder()`.
pub fn get_builder() -> MenuBuilder {
    MenuBuilder::new(true, 0, RelPos::Left, Vec::new())
        .set_positioning(
            Point::new((screen::W - 8) / 2, (screen::H - 8) / 2),
            RelPos::Center,
        )
        .set_display_length(9)
        .set_selectable(true)
        .set_scroll_policies(1.0, false)
}

/// Java `new ItemListMenu(b, entries, title)`.
pub fn new_with_builder(g: &Game, b: MenuBuilder, entries: Vec<EntryHandle>, title: &str) -> Menu {
    b.set_entries(entries).set_title(title).create_menu(g)
}

/// Java `new ItemListMenu(entries, title)`.
pub fn new(g: &Game, entries: Vec<EntryHandle>, title: &str) -> Menu {
    new_with_builder(g, get_builder(), entries, title)
}
