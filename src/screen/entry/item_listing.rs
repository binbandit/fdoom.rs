//! Port of `fdoom.screen.entry.ItemListing` — an unselectable ItemEntry whose text is a
//! caller-supplied info string (used for the "Have:"/"Cost:" crafting panels).

use crate::core::game::Game;
use crate::gfx::{Screen, font};
use crate::item::Item;

use super::{EntryFlags, ListEntry};

pub struct ItemListing {
    /// The ItemEntry layer's item (only its sprite is shown).
    item: Item,
    info: String,
    flags: EntryFlags,
}

impl ItemListing {
    /// Java `new ItemListing(i, text)`.
    pub fn new(item: Item, text: &str) -> ItemListing {
        ItemListing {
            item,
            info: text.to_string(),
            flags: EntryFlags {
                selectable: false, // JAVA: setSelectable(false)
                ..EntryFlags::default()
            },
        }
    }

    /// Java `setText(text)`.
    pub fn set_text(&mut self, text: &str) {
        self.info = text.to_string();
    }
}

impl ListEntry for ItemListing {
    fn flags(&self) -> EntryFlags {
        self.flags
    }

    fn flags_mut(&mut self) -> &mut EntryFlags {
        &mut self.flags
    }

    fn tick(&mut self, _g: &mut Game) {}

    fn render(&mut self, screen: &mut Screen, g: &mut Game, x: i32, y: i32, _is_selected: bool) {
        // JAVA: inherited ItemEntry.render — text always in the selected color, sprite
        // outside the visibility check.
        if self.flags.visible {
            let text = self.to_display_string(g);
            font::draw(&text, screen, x, y, self.get_color(true));
        }
        self.item.sprite.render(screen, x, y);
    }

    fn to_display_string(&self, _g: &Game) -> String {
        format!(" {}", self.info)
    }
}
