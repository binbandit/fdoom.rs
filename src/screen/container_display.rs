//! Port of `fdoom.screen.ContainerDisplay` — the two-inventory chest screen.

use crate::core::game::Game;
use crate::entity::Entity;
use crate::gfx::{Screen, screen};

use super::display::{Display, DisplayBase, display_render_default, display_tick_default};
use super::inventory_menu;

const PADDING: i32 = 10;

pub struct ContainerDisplay {
    base: DisplayBase,
    player_eid: i32,
    chest_eid: i32,
}

impl ContainerDisplay {
    /// Java `new ContainerDisplay(player, chest)`.
    pub fn new(g: &Game, player: &Entity, chest: &Entity) -> ContainerDisplay {
        let chest_name = chest
            .furniture()
            .expect("chest must be furniture")
            .name
            .clone();
        let menu0 = inventory_menu::new(g, chest, &chest_name);
        let mut menu1 = inventory_menu::new(g, player, "Inventory");

        menu1.translate(menu0.get_bounds().width() + PADDING, 0);

        let mut display = ContainerDisplay {
            base: DisplayBase::new(false, true, vec![menu0, menu1]),
            player_eid: player.c.eid,
            chest_eid: chest.c.eid,
        };

        if display.base.menus[0].get_num_options() == 0 {
            display.on_selection_change(0, 1);
        }

        display
    }

    /// Java `onSelectionChange(oldSel, newSel)`.
    fn on_selection_change(&mut self, old_sel: i32, new_sel: i32) {
        self.base.selection = new_sel; // JAVA: super.onSelectionChange
        // JAVA: "this also serves as a protection against access to menus[0] when such
        // may not exist" (it always does in this port).
        if old_sel == new_sel {
            return;
        }
        let mut shift = 0;
        if new_sel == 0 {
            shift = PADDING - self.base.menus[0].get_bounds().left();
        }
        if new_sel == 1 {
            shift = (screen::W - PADDING) - self.base.menus[1].get_bounds().right();
        }
        for m in &mut self.base.menus {
            m.translate(shift, 0);
        }
    }

    /// Java `getOtherIdx()`.
    fn get_other_idx(&self) -> i32 {
        (self.base.selection + 1) % 2
    }

    /// Java `update()` — rebuilds both menus from the live inventories.
    fn update(&mut self, g: &mut Game) {
        let sel0 = self.base.menus[0].get_selection();
        let sel1 = self.base.menus[1].get_selection();
        let title0 = self.base.menus[0].get_title().to_string();
        let title1 = self.base.menus[1].get_title().to_string();

        let Some(chest) = g.entities.get(self.chest_eid) else {
            return;
        };
        let Some(player) = g.entities.get(self.player_eid) else {
            return;
        };

        let menu0 = inventory_menu::rebuilt(g, chest, &title0, sel0);
        let mut menu1 = inventory_menu::rebuilt(g, player, &title1, sel1);
        menu1.translate(menu0.get_bounds().width() + PADDING, 0);
        self.base.menus[0] = menu0;
        self.base.menus[1] = menu1;

        self.on_selection_change(0, self.base.selection);
    }
}

impl Display for ContainerDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        // JAVA: super.tick(input) — the default tick, plus the onSelectionChange override
        // when the left/right keys switch menus, plus InventoryMenu.tick's drop handling
        // (which only ran when Display.tick reached menus[selection].tick).
        let prev_sel = self.base.selection;
        let exit_clicked = self.base.can_exit && g.input.get_key("exit").clicked;
        display_tick_default(&mut self.base, g);
        if prev_sel != self.base.selection {
            let new_sel = self.base.selection;
            self.on_selection_change(prev_sel, new_sel);
        } else if !exit_clicked {
            let sel = self.base.selection;
            let holder_eid = if sel == 0 {
                self.chest_eid
            } else {
                self.player_eid
            };
            // JAVA: dropOne is disabled inside a ContainerDisplay.
            inventory_menu::tick_drops(g, &mut self.base.menus[sel as usize], holder_eid, false);
        }

        let chest_removed = g
            .entities
            .get(self.chest_eid)
            .map(|c| c.c.removed)
            .unwrap_or(true);
        if g.input.get_key("menu").clicked || chest_removed {
            g.clear_menu();
            return;
        }

        let selection = self.base.selection as usize;
        let other_idx = self.get_other_idx() as usize;

        if self.base.menus[selection].get_num_options() > 0
            && (g.input.get_key("attack").clicked || g.input.get_key("drop-one").clicked)
        {
            // switch inventories
            let to_sel = self.base.menus[other_idx].get_selection();
            let from_sel = self.base.menus[selection].get_selection();

            // JAVA: the isValidClient removeFromChest/addToChest branches are dead — offline.

            let creative = g.is_mode("creative");
            let attack_clicked = g.input.get_key("attack").clicked;

            // Take the chest out so both inventories can be borrowed (see PORTING.md).
            let Some(mut chest) = g.entities.take(self.chest_eid) else {
                return;
            };
            {
                let Some(player) = g.entities.get_mut(self.player_eid) else {
                    g.entities.put_back(chest);
                    return;
                };
                let chest_inv = &mut chest.chest_mut().expect("chest entity").inventory;
                let player_inv = &mut player.player_mut().inventory;
                let (from, to, from_is_player) = if selection == 0 {
                    (chest_inv, player_inv, false)
                } else {
                    (player_inv, chest_inv, true)
                };

                let from_item = from.get_mut(from_sel);

                let transfer_all =
                    attack_clicked || !from_item.is_stackable() || from_item.count() == 1;

                let mut to_item = from_item.clone();

                if !transfer_all {
                    from_item.set_count(from_item.count() - 1); // this is known to be valid.
                    to_item.set_count(1);
                    // items are setup for sending.
                } else {
                    // transfer whole item/stack.
                    if !(creative && from_is_player) {
                        from.remove(from_sel); // remove it
                    }
                }

                to.add_at(to_sel, to_item);
            }
            g.entities.put_back(chest);
            self.update(g);
        }
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        display_render_default(&mut self.base, screen, g);
        // pinned selected-item line for whichever inventory panel has focus
        let sel = self.base.selection as usize;
        inventory_menu::render_selected_info(&self.base.menus[sel], screen, g);
    }
}
