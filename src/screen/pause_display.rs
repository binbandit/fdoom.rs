//! Port of `fdoom.screen.PauseDisplay`.

use crate::core::game::Game;
use crate::gfx::color;

use super::display::{Display, DisplayBase, display_tick_default};
use super::entry::{BlankEntry, EntryHandle, SelectEntry, StringEntry, handle};
use super::menu::MenuBuilder;
use super::options_display::OptionsDisplay;
use super::rel_pos::RelPos;
use super::title_display::TitleDisplay;

pub struct PauseDisplay {
    base: DisplayBase,
}

impl PauseDisplay {
    pub fn new(g: &Game) -> PauseDisplay {
        let up_string = format!(
            "{}{}{}{}",
            g.input.get_mapping("up"),
            g.localization.get_localized(" and "),
            g.input.get_mapping("down"),
            g.localization.get_localized(" to Scroll")
        );
        let select_string = format!(
            "{}{}",
            g.input.get_mapping("select"),
            g.localization.get_localized(": Choose")
        );

        let mut entries: Vec<EntryHandle> = vec![
            handle(BlankEntry::new()),
            handle(SelectEntry::new("Return to Game", |g: &mut Game| {
                g.clear_menu()
            })),
            handle(SelectEntry::new("Options", |g: &mut Game| {
                g.set_menu(OptionsDisplay::new(g))
            })),
        ];

        // JAVA: if(!Game.ISONLINE) — always true; Network.startMultiplayerServer() has no
        // network layer in this build (see PORTING.md "Multiplayer").
        entries.push(handle(SelectEntry::new(
            "Make World Multiplayer",
            |g: &mut Game| {
                g.clear_menu();
                g.notify_all("Multiplayer is not available in this build");
            },
        )));

        // JAVA: if(!Game.isValidClient()) — always true.
        entries.push(handle(SelectEntry::new("Save Game", |g: &mut Game| {
            g.clear_menu();
            // isValidServer() is always false, so this is the singleplayer branch.
            let world_name = super::world_select::get_world_name(g);
            crate::saveload::save::save_world_named(g, &world_name);
            // JAVA: SaveTest.backupGame(...) — TODO(port:saveload): backups pending.
        })));

        entries.push(handle(SelectEntry::new("Main Menu", |g: &mut Game| {
            // JAVA: MyUtils.fromNetworkStatus(offline, client, server) — always offline.
            let mut items: Vec<EntryHandle> = StringEntry::use_lines(&[
                "Are you sure you want to".to_string(),
                "Exit the Game?".to_string(),
            ]);

            items.extend(StringEntry::use_lines_color(
                color::RED,
                &[
                    String::new(),
                    "All unsaved progress".to_string(),
                    "will be lost!".to_string(),
                    String::new(),
                ],
            ));

            items.push(handle(BlankEntry::new()));
            items.push(handle(SelectEntry::new("No", |g: &mut Game| g.exit_menu())));
            items.push(handle(SelectEntry::new("Yes", |g: &mut Game| {
                g.set_menu(TitleDisplay::new(g))
            })));

            let menu = MenuBuilder::new(true, 8, RelPos::Center, items).create_menu(g);
            g.set_menu(super::plain_display(false, true, vec![menu]));
        })));

        entries.push(handle(BlankEntry::new()));
        entries.push(handle(StringEntry::with_color(&up_string, color::GRAY)));
        entries.push(handle(StringEntry::with_color(&select_string, color::GRAY)));

        let menu = MenuBuilder::new(true, 4, RelPos::Center, entries)
            .set_title_color("Paused", 550, false)
            .create_menu(g);

        PauseDisplay {
            base: DisplayBase::new(false, true, vec![menu]),
        }
    }
}

impl Display for PauseDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn init(&mut self, g: &mut Game) {
        // JAVA: super.init(null) — ignore; pause menus always lead back to the game.
        g.display.stack.clear();
    }

    fn tick(&mut self, g: &mut Game) {
        display_tick_default(&mut self.base, g);
        if g.input.get_key("pause").clicked {
            g.exit_menu();
        }
    }
}
