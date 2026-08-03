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
        // a splash screen never has a parent display
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
        // The flyover world is already on screen (clear_screen=false). The title lockup
        // is true-color sheet art (artgen `logo`), so instead of a palette fade the two
        // strips are revealed in beats: kicker first, then the DOOM wordmark.
        let t = self.tickc.min(60);
        let logo_color = color::get4(-1, 0, 333, 500);
        let kicker_w = 17; // "FOSSICKERS" strip, cells (15..31,6..7)
        let doom_w = 15; // "DOOM" strip, cells (0..14,6..7)
        // the lockup hangs off the middle of the live framebuffer: 48px above the
        // midline is where it sits at the classic 288x192
        let mid_x = screen.w / 2;
        let mid_y = screen.h / 2;
        let kicker_x = mid_x - kicker_w * 8 / 2;
        let kicker_y = mid_y - 48;
        let doom_x = mid_x - doom_w * 8 / 2;
        let doom_y = kicker_y + 18;
        for y in 0..2 {
            for x in 0..kicker_w {
                screen.render(
                    kicker_x + x * 8,
                    kicker_y + y * 8,
                    15 + x + (y + 6) * 32,
                    logo_color,
                    0,
                );
            }
            if t >= 15 {
                for x in 0..doom_w {
                    screen.render(
                        doom_x + x * 8,
                        doom_y + y * 8,
                        x + (y + 6) * 32,
                        logo_color,
                        0,
                    );
                }
            }
        }
        if t >= 45 {
            font::draw_centered("A FOSSICKERS TALE", screen, mid_y, color::get(-1, 333));
        }
    }
}
