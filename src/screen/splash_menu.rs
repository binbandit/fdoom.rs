//! Port of `fdoom.screen.SplashMenu` — the animated splash before the title screen.

use crate::core::game::Game;
use crate::core::renderer;
use crate::gfx::{Screen, color};

use super::display::{Display, DisplayBase};
use super::title_display::TitleDisplay;

pub struct SplashMenu {
    base: DisplayBase,
    rdm: i32,
    tickc: i32,
}

impl Default for SplashMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl SplashMenu {
    pub fn new() -> SplashMenu {
        SplashMenu {
            base: DisplayBase::default(),
            rdm: 0,
            tickc: 0,
        }
    }
}

impl Display for SplashMenu {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn init(&mut self, g: &mut Game) {
        // JAVA: super.init(null) — can't have a parent for a splash screen.
        g.display.stack.clear();
    }

    fn tick(&mut self, g: &mut Game) {
        self.tickc += 1;
        if self.tickc >= 200 {
            g.set_menu(TitleDisplay::new(g));
        }
    }

    fn render(&mut self, screen: &mut Screen, _g: &mut Game) {
        let mut h = 5;
        let mut w = 46;
        self.rdm += 1;
        screen.clear(0);
        for y in 3..h {
            for x in 17..w {
                let title_color = color::get4(
                    self.rdm + x * 8,
                    self.rdm + x * 8,
                    self.rdm + x * 8,
                    self.rdm + x * 8,
                );
                screen.render(x * 4, y * 8, 352, title_color, 0);
            }
        }
        h = renderer::HEIGHT;
        w = renderer::WIDTH;
        self.rdm += 1;
        for y in 0..h {
            for x in 0..w {
                let title_color = color::get4(0, 0, self.rdm + x * 5 + y * 2, 551);
                screen.render(x * 8, y * 8, 355, title_color, 0);
            }
        }
    }
}
