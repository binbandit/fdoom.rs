//! The intro splash before the title screen.
//!
//! (The Java fork drew a full-screen animated diagonal pattern here; replaced with the
//! game logo fading in over the title flyover world.)

use crate::core::game::Game;
use crate::gfx::{Screen, color, font};

use super::display::{Display, DisplayBase};
use super::title_display::TitleDisplay;

pub struct SplashMenu {
    base: DisplayBase,
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
        // a shorter beat than the Java fork's 200-tick pattern show
        if self.tickc >= 90 || g.input.get_key("select").clicked {
            g.set_menu(TitleDisplay::new(g));
        }
    }

    fn render(&mut self, screen: &mut Screen, _g: &mut Game) {
        // The flyover world is already on screen (clear_screen=false); fade the logo in
        // by stepping its shade with time.
        let t = self.tickc.min(60);
        let shade = match t {
            0..=14 => 111,
            15..=29 => 222,
            30..=44 => 333,
            _ => 444,
        };
        let logo_shade = color::get4(-1, 0, shade, 500);
        let (w, h) = (14, 2);
        let xo = (crate::gfx::screen::W - w * 8) / 2;
        let yo = 56;
        font::draw_centered(
            "* F O S S I C K E R S *",
            screen,
            yo - 8,
            color::get(-1, 500),
        );
        for y in 0..h {
            for x in 0..w {
                screen.render(xo + x * 8, yo + y * 8, x + (y + 6) * 32, logo_shade, 0);
            }
        }
        if t >= 45 {
            font::draw_centered("A FOSSICKERS TALE", screen, 96, color::get(-1, 333));
        }
    }
}
