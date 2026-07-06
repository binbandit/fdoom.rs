//! Port of `fdoom.core.Renderer` — draws each frame into the 288x192 software screen.
//! The platform layer scales/blits `self.screen.pixels` to the window.

use std::sync::Arc;

use crate::core::game::Game;
use crate::gfx::ellipsis::Ellipsis;
use crate::gfx::screen::{self, Screen};
use crate::gfx::sprite_sheet::SpriteSheet;
use crate::gfx::{color, font};

pub const HEIGHT: i32 = screen::H;
pub const WIDTH: i32 = screen::W;

pub struct Renderer {
    pub screen: Screen,
    /// The darkness/fog-of-war overlay screen (JAVA: the overlay call is commented out in
    /// this fork, but the screen is still constructed).
    pub light_screen: Screen,
    ellipsis: Ellipsis,
}

impl Renderer {
    pub fn new(sheet: Arc<SpriteSheet>) -> Renderer {
        Renderer {
            screen: Screen::new(sheet.clone()),
            light_screen: Screen::new(sheet),
            ellipsis: Ellipsis::smooth_tick(),
        }
    }

    /// Java `Renderer.render()` — called from the game loop, a bit after tick().
    pub fn render(&mut self, g: &mut Game) {
        if !g.has_gui {
            return; // no point in this if there's no gui
        }

        if g.ready_to_render_gameplay {
            self.render_level(g);
            self.render_gui(g);
        }

        // renders menu, if present (top display only, as in Java)
        if let Some(mut top) = g.display.stack.pop() {
            top.render(&mut self.screen, g);
            g.display.stack.push(top);
        }

        if !g.has_focus && !g.is_online() && !g.continous {
            self.render_focus_nagger(g);
        }
    }

    fn render_level(&mut self, _g: &mut Game) {
        // TODO(port:level,entity): scroll clamping, sky background, renderBackground,
        // renderSprites; the light overlay is commented out in the Java fork.
    }

    fn render_gui(&mut self, _g: &mut Game) {
        // TODO(port:entity,item): arrow count, status frames, hearts/stamina/hunger,
        // notifications, score mode HUD, potion overlay, current item.
    }

    #[allow(dead_code)]
    fn render_debug_info(&mut self, _g: &mut Game) {
        // TODO(port:level,entity): F3 debug info panel.
    }

    /// Java `renderFocusNagger()` — the "Come Back!" box when the window loses focus.
    fn render_focus_nagger(&mut self, g: &mut Game) {
        let msg = "Come Back!"; // the message when you click off the screen
        g.paused = true; // perhaps paused is only used for this
        let xx = (screen::W - font::text_width(msg)) / 2; // the width of the box
        let yy = (HEIGHT - 8) / 2; // the height of the box
        let w = msg.len() as i32; // length of message in characters
        let h = 1;
        let txtcolor = color::get4(-1, 1, color::hex("#2c2c2c"), 445);

        let screen = &mut self.screen;

        // renders the four corners of the box
        screen.render(xx - 8, yy - 8, 13 * 32, txtcolor, 0);
        screen.render(xx + w * 8, yy - 8, 13 * 32, txtcolor, 1);
        screen.render(xx - 8, yy + 8, 13 * 32, txtcolor, 2);
        screen.render(xx + w * 8, yy + 8, 13 * 32, txtcolor, 3);

        // renders each part of the box...
        for x in 0..w {
            screen.render(xx + x * 8, yy - 8, 1 + 13 * 32, txtcolor, 0); // ...top part
            screen.render(xx + x * 8, yy + 8, 1 + 13 * 32, txtcolor, 2); // ...bottom part
        }
        for y in 0..h {
            screen.render(xx - 8, yy + y * 8, 2 + 13 * 32, txtcolor, 0); // ...left part
            screen.render(xx + w * 8, yy + y * 8, 2 + 13 * 32, txtcolor, 1); // ...right part
        }

        // renders the focus nagger text with a flash effect...
        if (g.tick_count / 20) % 2 == 0 {
            font::draw(msg, screen, xx, yy, color::get(color::hex("#2c2c2c"), 333));
        } else {
            font::draw(msg, screen, xx, yy, color::get(color::hex("#2c2c2c"), 555));
        }
    }

    /// Access for gameplay screens that need the ellipsis (server waiting screen).
    pub fn ellipsis_update(&mut self, tick_count: i32) -> String {
        self.ellipsis.update_and_get(tick_count)
    }
}
