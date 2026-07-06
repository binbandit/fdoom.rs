//! Port of `fdoom.screen.entry.StringEntry` — an unselectable text line.

use crate::core::game::Game;
use crate::gfx::color;

use super::{EntryFlags, EntryHandle, ListEntry, handle};

const DEFAULT_COLOR: i32 = color::WHITE;

pub struct StringEntry {
    text: String,
    color: i32,
    flags: EntryFlags,
}

impl StringEntry {
    pub fn new(text: &str) -> StringEntry {
        Self::with_color(text, DEFAULT_COLOR)
    }

    pub fn with_color(text: &str, color: i32) -> StringEntry {
        let flags = EntryFlags { selectable: false, ..EntryFlags::default() };
        StringEntry { text: text.to_string(), color, flags }
    }

    /// Java `StringEntry.useLines(lines...)`.
    pub fn use_lines(lines: &[String]) -> Vec<EntryHandle> {
        Self::use_lines_color(DEFAULT_COLOR, lines)
    }

    /// Java `StringEntry.useLines(color, lines...)`.
    pub fn use_lines_color(color: i32, lines: &[String]) -> Vec<EntryHandle> {
        lines.iter().map(|l| handle(Self::with_color(l, color))).collect()
    }
}

impl ListEntry for StringEntry {
    fn flags(&self) -> EntryFlags {
        self.flags
    }

    fn flags_mut(&mut self) -> &mut EntryFlags {
        &mut self.flags
    }

    fn tick(&mut self, _g: &mut Game) {}

    fn get_color(&self, _is_selected: bool) -> i32 {
        self.color
    }

    fn to_display_string(&self, _g: &Game) -> String {
        self.text.clone()
    }
}
