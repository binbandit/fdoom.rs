//! Port of `fdoom.screen.PlayerDeathDisplay`. The Java `shouldRespawn` static is
//! `g.should_respawn`.

use crate::core::game::Game;
use crate::gfx::{Point, sprite_sheet};

use super::display::{Display, DisplayBase};
use super::entry::{BlankEntry, EntryHandle, SelectEntry, StringEntry, handle};
use super::info_display;
use super::menu::MenuBuilder;
use super::rel_pos::RelPos;
use super::title_display::TitleDisplay;

/// The `Game.setMenu(new PlayerDeathDisplay())` call site in `Updater.tick`.
pub fn open(g: &mut Game) {
    let display = PlayerDeathDisplay::new(g);
    g.set_menu(display);
}

pub struct PlayerDeathDisplay {
    base: DisplayBase,
}

impl PlayerDeathDisplay {
    pub fn new(g: &Game) -> PlayerDeathDisplay {
        let mut entries: Vec<EntryHandle> = vec![
            handle(StringEntry::new(&format!(
                "Time: {}",
                info_display::get_time_string(g)
            ))),
            handle(StringEntry::new(&format!(
                "Score: {}",
                g.player().player().get_score()
            ))),
            handle(BlankEntry::new()),
        ];

        // JAVA: !Settings.get("mode").equals("hardcore") — a case-sensitive check against
        // the canonical "Hardcore" value; ported as the intended mode comparison.
        if !g.is_mode("hardcore") {
            entries.push(handle(SelectEntry::new("Respawn", |g: &mut Game| {
                crate::core::world::reset_game(g, true);
                // JAVA: if(!Game.isValidClient()) — always true.
                g.clear_menu(); // sets the menu to nothing
            })));
        }

        // JAVA: if hardcore || !Game.isValidClient() — always true.
        entries.push(handle(SelectEntry::new("Quit", |g: &mut Game| {
            g.set_menu(TitleDisplay::new(g))
        })));

        let menu = MenuBuilder::new(true, 0, RelPos::Left, entries)
            .set_positioning(
                Point::new(sprite_sheet::BOX_WIDTH, sprite_sheet::BOX_WIDTH * 3),
                RelPos::BottomRight,
            )
            .set_title("You died! Aww!")
            .set_title_pos(RelPos::TopLeft)
            .create_menu(g);

        PlayerDeathDisplay {
            base: DisplayBase::new(false, false, vec![menu]),
        }
    }
}

impl Display for PlayerDeathDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }
}
