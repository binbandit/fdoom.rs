//! Port of `fdoom.screen.entry.KeyInputEntry` — one row of the key-binding screen.
//!
//! Java extended `SelectEntry` with a null action; here it's a standalone `ListEntry`
//! (the select behavior was unused since the action was null).

use crate::core::game::Game;
use crate::gfx::{font, sprite_sheet};

use super::{EntryFlags, ListEntry};

/// Usable width of one row: the live framebuffer minus the two selection-cursor
/// gutters the menu reserves (`> ` and ` <`, one cell each side of the entry box).
/// Claiming the *whole* screen width here is what pushed the Controls list 16px off
/// each edge — the cursors vanished and every mapping lost its last character.
fn row_width(g: &Game) -> i32 {
    (g.screen_size.0 - sprite_sheet::BOX_WIDTH * 4).max(font::text_width(" "))
}

pub struct KeyInputEntry {
    action: String,
    mapping: String,
    flags: EntryFlags,
}

impl KeyInputEntry {
    /// Java `new KeyInputEntry(key)` — `key` is "ACTION;mapping" (see `getKeyPrefs`).
    pub fn new(key: &str) -> KeyInputEntry {
        let idx = key.find(';').unwrap_or(key.len());
        let action = key[..idx].to_string();
        let mapping = if idx < key.len() { &key[idx + 1..] } else { "" };
        KeyInputEntry {
            action,
            mapping: mapping.to_string(),
            flags: EntryFlags::default(),
        }
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

    fn get_width(&self, g: &Game) -> i32 {
        row_width(g)
    }

    /// Action flush left, mapping flush right, padded to the row width. An action with
    /// a pile of alternate bindings can outgrow the row, so the mapping (the half the
    /// player edits) ellipsizes instead of running off the panel.
    fn to_display_string(&self, g: &Game) -> String {
        let action = g.localization.get_localized(&self.action);
        let cols = (row_width(g) / font::text_width(" ")).max(1) as usize;
        let action_len = action.chars().count();
        let mapping = font::fit_chars(&self.mapping, cols.saturating_sub(action_len + 1).max(1));
        let gap = cols
            .saturating_sub(action_len + mapping.chars().count())
            .max(1);
        format!("{action}{}{mapping}", " ".repeat(gap))
    }
}
