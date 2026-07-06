//! Port of `fdoom.screen.MultiplayerDisplay`.
//!
//! The full Java display (account login over HTTP, ip entry, GameClient connection
//! states) exists upstream, but this build has no network layer (see PORTING.md
//! "Multiplayer"), so this is a simple "not available" notice. The Java exit behavior
//! (`Game.setMenu(new TitleDisplay())`) is preserved.

use crate::core::game::Game;
use crate::gfx::{Screen, color, font};

use super::display::{Display, DisplayBase};
use super::title_display::TitleDisplay;

pub struct MultiplayerDisplay {
    base: DisplayBase,
}

impl Default for MultiplayerDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiplayerDisplay {
    pub fn new() -> MultiplayerDisplay {
        MultiplayerDisplay {
            base: DisplayBase::new(true, false, Vec::new()),
        }
    }
}

impl Display for MultiplayerDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        // JAVA: if(input.getKey("exit").clicked && !Game.ISHOST) Game.setMenu(new TitleDisplay());
        if g.input.get_key("exit").clicked {
            g.set_menu(TitleDisplay::new(g));
        }
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        screen.clear(0);

        font::draw_centered(
            "Multiplayer is not available",
            screen,
            crate::gfx::screen::H / 2 - font::text_height(),
            color::WHITE,
        );
        font::draw_centered(
            "in this build",
            screen,
            crate::gfx::screen::H / 2,
            color::WHITE,
        );

        // JAVA: the bottom hint shown in the ENTERIP/ERROR states.
        font::draw_centered(
            &format!("Press {} to return", g.input.get_mapping("exit")),
            screen,
            crate::gfx::screen::H - font::text_height() * 2,
            color::GRAY,
        );
    }
}
