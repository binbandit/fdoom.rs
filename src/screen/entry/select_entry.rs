//! Port of `fdoom.screen.entry.SelectEntry` — a "button" row that runs an action.

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::gfx::font;

use super::{EntryFlags, ListEntry};

pub type SelectAction = Box<dyn FnMut(&mut Game)>;

pub struct SelectEntry {
    on_select: Option<SelectAction>,
    text: String,
    localize: bool,
    flags: EntryFlags,
}

impl SelectEntry {
    pub fn new(text: &str, on_select: impl FnMut(&mut Game) + 'static) -> SelectEntry {
        Self::with_localize(text, on_select, true)
    }

    pub fn with_localize(
        text: &str,
        on_select: impl FnMut(&mut Game) + 'static,
        localize: bool,
    ) -> SelectEntry {
        SelectEntry {
            on_select: Some(Box::new(on_select)),
            text: text.to_string(),
            localize,
            flags: EntryFlags::default(),
        }
    }

    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
    }
}

impl ListEntry for SelectEntry {
    fn flags(&self) -> EntryFlags {
        self.flags
    }

    fn flags_mut(&mut self) -> &mut EntryFlags {
        &mut self.flags
    }

    fn tick(&mut self, g: &mut Game) {
        if g.input.get_key("select").clicked {
            if let Some(mut action) = self.on_select.take() {
                g.play_sound(Sound::Confirm);
                action(g);
                self.on_select = Some(action);
            }
        }
    }

    fn to_display_string(&self, g: &Game) -> String {
        if self.localize {
            g.localization.get_localized(&self.text)
        } else {
            self.text.clone()
        }
    }

    fn get_width(&self, g: &Game) -> i32 {
        font::text_width(&self.to_display_string(g))
    }
}
