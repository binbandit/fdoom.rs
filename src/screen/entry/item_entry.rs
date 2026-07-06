//! Port of `fdoom.screen.entry.ItemEntry` — a menu row showing an item's sprite and name.

use crate::core::game::Game;
use crate::gfx::{Screen, font};
use crate::item::Item;

use super::{EntryFlags, EntryHandle, ListEntry, handle};

pub struct ItemEntry {
    item: Item,
    flags: EntryFlags,
}

impl ItemEntry {
    /// Java `new ItemEntry(i)`. Holds a clone (Java shared the inventory's Item object;
    /// callers refresh the entry when the backing item changes).
    pub fn new(item: Item) -> ItemEntry {
        ItemEntry {
            item,
            flags: EntryFlags::default(),
        }
    }

    /// Java `ItemEntry.useItems(items)`.
    pub fn use_items(items: &[Item]) -> Vec<EntryHandle> {
        items
            .iter()
            .map(|i| handle(ItemEntry::new(i.clone())))
            .collect()
    }

    /// Java `getItem()`.
    pub fn get_item(&self) -> &Item {
        &self.item
    }
}

impl ListEntry for ItemEntry {
    fn flags(&self) -> EntryFlags {
        self.flags
    }

    fn flags_mut(&mut self) -> &mut EntryFlags {
        &mut self.flags
    }

    fn tick(&mut self, _g: &mut Game) {}

    fn render(&mut self, screen: &mut Screen, g: &mut Game, x: i32, y: i32, _is_selected: bool) {
        // JAVA: super.render(screen, x, y, true) — always drawn in the selected color.
        if self.flags.visible {
            let text = self.to_display_string(g);
            font::draw(&text, screen, x, y, self.get_color(true));
        }
        // JAVA: the sprite renders outside the visibility check.
        self.item.sprite.render(screen, x, y);
    }

    // JAVA: "if you add to the length of the string, and therefore the width of the
    // entry, then it will actually move the entry RIGHT in the inventory, instead of the
    // intended left, because it is auto-positioned to the left side."
    fn to_display_string(&self, g: &Game) -> String {
        self.item.get_display_name(g)
    }
}
