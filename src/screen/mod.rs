//! Port of the `fdoom.screen` package: displays (menus/screens) and their widgets.

pub mod book_data;
pub mod book_display;
pub mod container_display;
pub mod crafting_display;
pub mod display;
pub mod end_game_display;
pub mod entry;
pub mod info_display;
pub mod inventory_menu;
pub mod item_list_menu;
pub mod key_input_display;
pub mod level_transition_display;
pub mod loading_display;
pub mod map_menu;
pub mod menu;
pub mod multiplayer_display;
pub mod options_display;
pub mod pause_display;
pub mod player_death_display;
pub mod player_inv_display;
pub mod recipe_menu;
pub mod rel_pos;
pub mod splash_menu;
pub mod temp_display;
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
