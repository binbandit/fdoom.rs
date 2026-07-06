//! Port of `fdoom.screen.TempDisplay` — a display that auto-exits after a delay.
//!
//! Java spawned a thread that slept `milliDelay` ms and then exited the menu if this
//! display was still current; the Rust port checks elapsed wall-clock time in `tick`
//! (only the current display ticks, so the "still current" check is implicit).

use std::time::Instant;

use crate::core::game::Game;

use super::display::{Display, DisplayBase, display_tick_default};
use super::menu::Menu;

pub struct TempDisplay {
    base: DisplayBase,
    milli_delay: i32,
    start: Option<Instant>,
}

impl TempDisplay {
    /// Java `new TempDisplay(milliDelay)`.
    pub fn new(milli_delay: i32) -> TempDisplay {
        Self::with_display(milli_delay, false, true, Vec::new())
    }

    /// Java `new TempDisplay(milliDelay, menus...)`.
    pub fn with_menus(milli_delay: i32, menus: Vec<Menu>) -> TempDisplay {
        Self::with_display(milli_delay, false, true, menus)
    }

    /// Java `new TempDisplay(milliDelay, clearScreen, canExit, menus...)` (and the other
    /// constructor flavors).
    pub fn with_display(
        milli_delay: i32,
        clear_screen: bool,
        can_exit: bool,
        menus: Vec<Menu>,
    ) -> TempDisplay {
        TempDisplay {
            base: DisplayBase::new(clear_screen, can_exit, menus),
            milli_delay,
            start: None,
        }
    }
}

impl Display for TempDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn init(&mut self, _g: &mut Game) {
        self.start = Some(Instant::now());
    }

    fn tick(&mut self, g: &mut Game) {
        if let Some(start) = self.start {
            if start.elapsed().as_millis() as i64 >= self.milli_delay as i64 {
                g.exit_menu();
                return;
            }
        }

        display_tick_default(&mut self.base, g);
    }
}
