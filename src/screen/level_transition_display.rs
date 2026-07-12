//! Port of `fdoom.screen.LevelTransitionDisplay` — the sweeping-squares stair animation.

use crate::core::game::Game;
use crate::gfx::Screen;

use super::display::{Display, DisplayBase};

const DURATION: i32 = 30;

/// The `Game.setMenu(new LevelTransitionDisplay(dir))` call site in `Updater.tick`.
pub fn open(g: &mut Game, dir: i32) {
    g.set_menu(LevelTransitionDisplay::new(dir));
}

pub struct LevelTransitionDisplay {
    base: DisplayBase,
    /// Direction that you are changing levels. (going up or down stairs)
    dir: i32,
    /// Time it spends on this menu
    time: i32,
}

impl LevelTransitionDisplay {
    pub fn new(dir: i32) -> LevelTransitionDisplay {
        LevelTransitionDisplay {
            base: DisplayBase::new(false, false, Vec::new()),
            dir,
            time: 0,
        }
    }
}

impl Display for LevelTransitionDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        self.time += 1;
        if self.time == DURATION / 2 {
            // When time equals 30, it will change the level
            crate::core::world::change_level(g, self.dir);
        }
        if self.time == DURATION {
            // When time equals 60, it will get out of this menu
            g.clear_menu();
        }
    }

    fn render(&mut self, screen: &mut Screen, _g: &mut Game) {
        // fixed 200x150 sweep — comfortably covers the whole screen in 8px squares
        for x in 0..200 {
            for y in 0..150 {
                let dd = (y + x % 2 * 2 + x / 3) - self.time * 2; // Used as part of the positioning.
                if dd < 0 && dd > -30 {
                    if self.dir > 0 {
                        // If the direction is upwards then render the squares going up
                        screen.render(x * 8, y * 8, 0, 0, 0);
                    } else {
                        // If the direction is negative, then the squares will go down.
                        screen.render(x * 8, screen.h - y * 8 - 8, 0, 0, 0);
                    }
                }
            }
        }
    }
}
