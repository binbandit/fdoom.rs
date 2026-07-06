//! Port of `fdoom.screen.entry.BlankEntry` — an empty spacer row.

use crate::core::game::Game;
use crate::gfx::{Screen, sprite_sheet};

use super::{EntryFlags, ListEntry};

pub struct BlankEntry {
    flags: EntryFlags,
}

impl Default for BlankEntry {
    fn default() -> Self {
        Self::new()
    }
}

impl BlankEntry {
    pub fn new() -> BlankEntry {
        let flags = EntryFlags {
            selectable: false,
            ..EntryFlags::default()
        };
        BlankEntry { flags }
    }
}

impl ListEntry for BlankEntry {
    fn flags(&self) -> EntryFlags {
        self.flags
    }

    fn flags_mut(&mut self) -> &mut EntryFlags {
        &mut self.flags
    }

    fn tick(&mut self, _g: &mut Game) {}

    fn render(
        &mut self,
        _screen: &mut Screen,
        _g: &mut Game,
        _x: i32,
        _y: i32,
        _is_selected: bool,
    ) {
    }

    fn get_width(&self, _g: &Game) -> i32 {
        sprite_sheet::BOX_WIDTH
    }

    fn is_blank_entry(&self) -> bool {
        true
    }

    fn to_display_string(&self, _g: &Game) -> String {
        " ".to_string()
    }
}
