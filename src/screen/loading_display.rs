//! Port of `fdoom.screen.LoadingDisplay`.
//!
//! Java started a one-shot 500ms Swing timer whose callback ran `World.initWorld()` on
//! another thread and then closed the menu. Per PORTING.md ("Threads → incremental state
//! machines") the Rust port stays single-threaded: tick 1 lets one frame render, tick 2
//! runs `init_world` synchronously and closes the menu. The Java percentage/message
//! statics live on `Game` (`loading_percentage` / `loading_message`).

use crate::core::game::Game;
use crate::gfx::ellipsis::Ellipsis;
use crate::gfx::{FontStyle, Screen, color, font};

use super::display::{Display, DisplayBase, display_render_default, display_tick_default};

/// Java `LoadingDisplay.setPercentage(percent)`.
pub fn set_percentage(g: &mut Game, percent: f32) {
    g.loading_percentage = percent;
}

/// Java `LoadingDisplay.getPercentage()`.
pub fn get_percentage(g: &Game) -> f32 {
    g.loading_percentage
}

/// Java `LoadingDisplay.setMessage(progressType)` — "World", "Level B3", ...
pub fn set_message(g: &mut Game, progress_type: &str) {
    g.loading_message = progress_type.to_string();
}

/// Java `LoadingDisplay.progress(amt)`.
pub fn progress(g: &mut Game, amt: f32) {
    g.loading_percentage = (g.loading_percentage + amt).min(100.0);
}

pub struct LoadingDisplay {
    base: DisplayBase,
    /// Replaces the Java 500ms timer: counts ticks since init.
    ticks: i32,
    msg: String,
    ellipsis: Ellipsis,
}

impl Default for LoadingDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadingDisplay {
    pub fn new() -> LoadingDisplay {
        LoadingDisplay {
            base: DisplayBase::new(true, false, Vec::new()),
            ticks: 0,
            msg: String::new(),
            ellipsis: Ellipsis::smooth_time(),
        }
    }
}

impl Display for LoadingDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn init(&mut self, g: &mut Game) {
        self.ticks = 0;
        g.loading_percentage = 0.0;
        g.loading_message = "World".to_string();
        self.msg = if super::world_select::loaded_world(g) {
            "Loading".to_string()
        } else {
            "Generating".to_string()
        };
    }

    fn tick(&mut self, g: &mut Game) {
        self.ticks += 1;
        if self.ticks == 2 {
            // one frame of "Loading..." has been drawn; now do the real work
            crate::core::world::init_world(g);
            g.clear_menu();
            return;
        }

        display_tick_default(&mut self.base, g);
    }

    fn on_exit(&mut self, g: &mut Game) {
        g.loading_percentage = 0.0;
        if !super::world_select::loaded_world(g) {
            self.msg = "Saving".to_string();
            g.loading_message = "World".to_string();
            let world_name = super::world_select::get_world_name(g);
            crate::saveload::save::save_world_named(g, &world_name);
            g.notifications.clear();
        }
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        display_render_default(&mut self.base, screen, g);

        let percent = g.loading_percentage.round() as i32;
        let mut line = g.localization.get_localized(&self.msg);
        if !g.loading_message.is_empty() {
            line.push(' ');
            line.push_str(&g.localization.get_localized(&g.loading_message));
        }
        line.push_str(&self.ellipsis.update_and_get(g.tick_count));

        let para = format!("{line}\n{percent}%");
        font::draw_paragraph_str(&para, screen, &mut FontStyle::new(color::RED), 6);
    }
}
