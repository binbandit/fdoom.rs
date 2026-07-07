//! Port of `fdoom.screen.OptionsDisplay`.

use crate::core::game::Game;
use crate::screen::settings_widgets::{self, SettingEntry};

use super::display::{Display, DisplayBase, display_tick_default};
use super::entry::{EntryHandle, SelectEntry, handle};
use super::key_input_display::KeyInputDisplay;
use super::menu::MenuBuilder;
use super::rel_pos::RelPos;

pub struct OptionsDisplay {
    base: DisplayBase,
    settings: Vec<SettingEntry>,
}

impl OptionsDisplay {
    pub fn new(g: &Game) -> OptionsDisplay {
        let settings: Vec<SettingEntry> =
            ["diff", "daycycle", "fps", "sound", "autosave", "language"]
                .iter()
                .map(|key| settings_widgets::make_entry(g, key))
                .collect();

        let entries: Vec<EntryHandle> = vec![
            settings[0].1.clone(),
            settings[1].1.clone(),
            settings[2].1.clone(),
            settings[3].1.clone(),
            settings[4].1.clone(),
            handle(SelectEntry::new("Change Key Bindings", |g: &mut Game| {
                g.set_menu(KeyInputDisplay::new(g));
            })),
        ];

        let menu = MenuBuilder::new(false, 6, RelPos::Left, entries)
            .set_title("Options")
            .create_menu(g);

        OptionsDisplay {
            base: DisplayBase::new(false, true, vec![menu]),
            settings,
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

    fn tick(&mut self, g: &mut Game) {
        display_tick_default(&mut self.base, g);
        settings_widgets::sync(g, &self.settings);
    }

    fn on_exit(&mut self, g: &mut Game) {
        settings_widgets::sync(g, &self.settings);
        let language = g.settings.get("language").as_str().to_string();
        g.localization.change_language(&language);
        crate::saveload::save::save_prefs(g); // JAVA: new Save()
        g.max_fps = g.settings.get("fps").as_int();
    }
}
