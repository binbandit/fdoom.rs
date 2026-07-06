//! Port of `fdoom.screen.entry.KeyInputEntry` — one row of the key-binding screen.
//!
//! Java extended `SelectEntry` with a null action; here it's a standalone `ListEntry`
//! (the select behavior was unused since the action was null).

use crate::core::game::Game;
use crate::gfx::{font, screen};

use super::{EntryFlags, ListEntry};

pub struct KeyInputEntry {
    action: String,
    mapping: String,
    buffer: String,
    flags: EntryFlags,
}

impl KeyInputEntry {
    /// Java `new KeyInputEntry(key)` — `key` is "ACTION;mapping" (see `getKeyPrefs`).
    pub fn new(key: &str) -> KeyInputEntry {
        let idx = key.find(';').unwrap_or(key.len());
        let action = key[..idx].to_string();
        let mapping = if idx < key.len() { &key[idx + 1..] } else { "" };
        let mut entry = KeyInputEntry {
            action,
            mapping: String::new(),
            buffer: String::new(),
            flags: EntryFlags::default(),
        };
        entry.set_mapping(mapping);
        entry
    }

    fn set_mapping(&mut self, mapping: &str) {
        self.mapping = mapping.to_string();

        let total = screen::W / font::text_width(" ")
            - self.action.chars().count() as i32
            - self.mapping.chars().count() as i32;
        let mut buffer = String::new();
        let mut spaces = 0;
        while spaces < total {
            buffer.push(' ');
            spaces += 1;
        }
        self.buffer = buffer;
    }
}

impl ListEntry for KeyInputEntry {
    fn flags(&self) -> EntryFlags {
        self.flags
    }

    fn flags_mut(&mut self) -> &mut EntryFlags {
        &mut self.flags
    }

    fn tick(&mut self, g: &mut Game) {
        if g.input.get_key("c").clicked || g.input.get_key("enter").clicked {
            g.input.change_key_binding(&self.action);
        } else if g.input.get_key("a").clicked {
            // add a binding, don't remove previous.
            g.input.add_key_binding(&self.action);
        }
    }

    fn get_width(&self, _g: &Game) -> i32 {
        screen::W
    }

    fn to_display_string(&self, g: &Game) -> String {
        format!(
            "{}{}{}",
            g.localization.get_localized(&self.action),
            self.buffer,
            self.mapping
        )
    }
}
