//! Port of `fdoom.screen.InfoDisplay` — the "Player Stats" panel.

use crate::core::game::Game;
use crate::core::updater::NORM_SPEED;
use crate::gfx::{Point, sprite_sheet};

use super::display::{Display, DisplayBase};
use super::entry::StringEntry;
use super::menu::MenuBuilder;
use super::rel_pos::RelPos;

pub struct InfoDisplay {
    base: DisplayBase,
}

impl InfoDisplay {
    pub fn new(g: &Game) -> InfoDisplay {
        // (Post-port cleanup: the "{select}/{exit}:Exit" hint line is gone — navigation
        // hints are title-screen-only now.)
        let lines = [
            "----------------------------".to_string(),
            format!("Time Played: {}", get_time_string(g)),
            format!("Current Score: {}", g.player().player().get_score()),
            "----------------------------".to_string(),
        ];

        let menu = MenuBuilder::new(true, 4, RelPos::Left, StringEntry::use_lines(&lines))
            .set_title("Player Stats")
            .set_title_pos(RelPos::TopLeft)
            .set_positioning(
                Point::new(sprite_sheet::BOX_WIDTH, sprite_sheet::BOX_WIDTH),
                RelPos::BottomRight,
            )
            .create_menu(g);

        InfoDisplay {
            base: DisplayBase::new(false, true, vec![menu]),
        }
    }
}

impl Display for InfoDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        if g.input.get_key("select").clicked || g.input.get_key("exit").clicked {
            g.exit_menu();
        }
    }
}

/// Java `InfoDisplay.getTimeString()`.
pub fn get_time_string(g: &Game) -> String {
    let seconds = g.game_time / NORM_SPEED;
    let mut minutes = seconds / 60;
    let hours = minutes / 60;
    minutes %= 60;
    let seconds = seconds % 60;

    if hours > 0 {
        format!("{hours}h{}{minutes}m", if minutes < 10 { "0" } else { "" })
    } else {
        format!(
            "{minutes}m {}{seconds}s",
            if seconds < 10 { "0" } else { "" }
        )
    }
}
