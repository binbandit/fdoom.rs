//! Port of `fdoom.screen.ItemListMenu` — the shared Menu configuration for item lists.
//! Java made it a Menu subclass; in Rust it's the builder plus constructor functions.

use crate::core::game::Game;
use crate::gfx::Point;

use super::entry::EntryHandle;
use super::menu::{Menu, MenuBuilder};
use super::rel_pos::RelPos;

/// Java `ItemListMenu.getBuilder()`. Centered on the live framebuffer, nudged up-left
/// by half a cell so the frame reads as centered once its border is drawn.
pub fn get_builder(g: &Game) -> MenuBuilder {
    let (w, h) = g.screen_size;
    MenuBuilder::new(true, 0, RelPos::Left, Vec::new())
        .set_positioning(Point::new((w - 8) / 2, (h - 8) / 2), RelPos::Center)
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
    new_with_builder(g, get_builder(g), entries, title)
}
