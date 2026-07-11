//! Port of `fdoom.screen.PlayerInvDisplay` — the player's inventory screen.

use crate::core::game::Game;
use crate::entity::Entity;
use crate::gfx::{Screen, color, font, sprite_sheet};

use super::display::{Display, DisplayBase, display_render_default, display_tick_default};
use super::inventory_menu;

pub struct PlayerInvDisplay {
    base: DisplayBase,
    player_eid: i32,
}

impl PlayerInvDisplay {
    /// Java `new PlayerInvDisplay(player)`.
    pub fn new(g: &Game, player: &Entity) -> PlayerInvDisplay {
        let menu = inventory_menu::new(g, player, "Inventory");
        PlayerInvDisplay {
            base: DisplayBase::new(false, true, vec![menu]),
            player_eid: player.c.eid,
        }
    }
}

impl Display for PlayerInvDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        // drop handling must not run on the exit key press (the press closes the menu)
        let exit_clicked = self.base.can_exit && g.input.get_key("exit").clicked;
        display_tick_default(&mut self.base, g);
        if !exit_clicked {
            inventory_menu::tick_drops(g, &mut self.base.menus[0], self.player_eid, true);
        }

        if g.input.get_key("menu").clicked {
            g.clear_menu();
            return;
        }

        if g.input.get_key("attack").clicked && self.base.menus[0].get_num_options() > 0 {
            let sel = self.base.menus[0].get_selection();
            if let Some(player) = g.entities.get_mut(self.player_eid) {
                let pd = player.player_mut();
                pd.active_item = Some(pd.inventory.remove(sel));
            }
            g.clear_menu();
        }
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        display_render_default(&mut self.base, screen, g);
        let menu = &self.base.menus[0];
        if menu.get_num_options() == 0 {
            // an empty inventory is a featureless black panel otherwise; one dim
            // line of direction, wrapped and centered in the panel interior
            let b = menu.get_bounds();
            let w = b.width() - 2 * sprite_sheet::BOX_WIDTH;
            let lines = font::get_lines("Empty - gather something.", w, b.height(), 1);
            let line_h = font::text_height() + 1;
            let mut y = b.top() + (b.height() - lines.len() as i32 * line_h) / 2;
            for line in &lines {
                let x = b.left() + (b.width() - font::text_width(line)) / 2;
                font::draw(line, screen, x, y, color::DARK_GRAY);
                y += line_h;
            }
        }
        inventory_menu::render_selected_info(&self.base.menus[0], screen, g);
    }
}
