//! Port of the `fdoom.screen` package: displays (menus/screens) and their widgets.
//!
//! # Module index
//!
//! - Shell and routing: `display`, `menu`, `rel_pos`, `key_input_display`.
//! - Entry flow: `entry`, `splash_menu`, `world_select`, `world_gen_display`,
//!   `loading_display`, `level_transition_display`.
//! - In-game views: `survival_display`, `container_display`, `map_menu`,
//!   `pause_display`, `player_death_display`, `info_display`.
//! - Reusable widgets and data: `settings_widgets`, `item_list_menu`, `book_data`,
//!   `book_display`.
//! - Development-only UI: `dev_console`.

pub mod book_data;
pub mod book_display;
pub mod container_display;
pub mod dev_console;
pub mod display;
pub mod entry;
pub mod info_display;
pub mod item_list_menu;
pub mod key_input_display;
pub mod level_transition_display;
pub mod loading_display;
pub mod map_menu;
pub mod menu;
pub mod options_display;
pub mod pause_display;
pub mod player_death_display;
pub mod rel_pos;
pub mod settings_widgets;
pub mod splash_menu;
pub mod survival_display;
pub mod title_display;
pub mod world_gen_display;
pub mod world_select;

pub use display::{Display, DisplayBase};
pub use menu::{Menu, MenuBuilder};
pub use rel_pos::RelPos;

/// Java `new Display(clearScreen, canExit, menus...)` used directly (no subclass).
pub struct PlainDisplay {
    base: DisplayBase,
}

impl Display for PlainDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }
}

pub fn plain_display(clear_screen: bool, can_exit: bool, menus: Vec<Menu>) -> PlainDisplay {
    PlainDisplay {
        base: DisplayBase::new(clear_screen, can_exit, menus),
    }
}
