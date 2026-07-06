//! Port of `fdoom.screen.OptionsDisplay`.

use crate::core::game::Game;

use super::display::{Display, DisplayBase};
use super::entry::{EntryHandle, SelectEntry, handle};
use super::key_input_display::KeyInputDisplay;
use super::menu::MenuBuilder;
use super::rel_pos::RelPos;

pub struct OptionsDisplay {
    base: DisplayBase,
}

impl OptionsDisplay {
    pub fn new(g: &Game) -> OptionsDisplay {
        // The settings entries are the same shared objects Java passed around
        // (Settings.getEntry returns the live entry).
        let entries: Vec<EntryHandle> = vec![
            g.settings.get_entry("diff"),
            g.settings.get_entry("fps"),
            g.settings.get_entry("sound"),
            g.settings.get_entry("autosave"),
            g.settings.get_entry("skinon"),
            handle(SelectEntry::new("Change Key Bindings", |g: &mut Game| {
                g.set_menu(KeyInputDisplay::new(g));
            })),
            g.settings.get_entry("language"),
        ];

        let menu = MenuBuilder::new(false, 6, RelPos::Left, entries)
            .set_title("Options")
            .create_menu(g);

        OptionsDisplay {
            base: DisplayBase::new(true, true, vec![menu]),
        }
    }
}

impl Display for OptionsDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn on_exit(&mut self, g: &mut Game) {
        let language = g.settings.get("language").as_str().to_string();
        g.localization.change_language(&language);
        // JAVA: new Save() — TODO(port:saveload): global preferences save pending.
        g.max_fps = g.settings.get("fps").as_int();
    }
}
