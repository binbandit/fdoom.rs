//! Port of `fdoom.screen.CraftingDisplay` — the recipe list with "Have:"/"Cost:" panels.
//!
//! Java kept the two `Menu.Builder`s around to rebuild the panels on each selection
//! change; the Rust builder is consumed by `create_menu`, so the anchors are stored and
//! the builders are reconstructed. The recipes are shared with the entries via
//! `Rc<RefCell<Recipe>>` (Java shared the `Recipe` objects).

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::game::Game;
use crate::entity::Entity;
use crate::gfx::{Point, sprite_sheet};
use crate::item::{Inventory, Recipe, registry};

use super::display::{Display, DisplayBase, display_tick_default};
use super::entry::item_listing::ItemListing;
use super::entry::{EntryHandle, handle};
use super::menu::MenuBuilder;
use super::recipe_menu;
use super::rel_pos::RelPos;

pub struct CraftingDisplay {
    base: DisplayBase,
    player_eid: i32,
    recipes: Vec<Rc<RefCell<Recipe>>>,
    is_personal_crafter: bool,
    /// Java `itemCountMenu`/`costsMenu` builder positioning.
    item_count_anchor: Point,
    costs_anchor: Point,
}

impl CraftingDisplay {
    /// Java `new CraftingDisplay(recipes, title, player)`.
    pub fn new(g: &Game, recipes: Vec<Recipe>, title: &str, player: &Entity) -> CraftingDisplay {
        Self::with_personal(g, recipes, title, player, false)
    }

    /// Java `new CraftingDisplay(recipes, title, player, isPersonal)`.
    pub fn with_personal(
        g: &Game,
        recipes: Vec<Recipe>,
        title: &str,
        player: &Entity,
        is_personal: bool,
    ) -> CraftingDisplay {
        let inventory = &player.player().inventory;

        let mut recipes: Vec<Rc<RefCell<Recipe>>> = recipes
            .into_iter()
            .map(|r| Rc::new(RefCell::new(r)))
            .collect();
        for recipe in &recipes {
            recipe.borrow_mut().check_can_craft(g, inventory);
        }

        let recipe_menu = if !is_personal {
            recipe_menu::new(g, &mut recipes, title, inventory)
        } else {
            recipe_menu::with_frame(g, &mut recipes, title, inventory, 300, 1, 400)
        };

        let bounds = recipe_menu.get_bounds();
        let item_count_anchor =
            Point::new(bounds.left() + sprite_sheet::BOX_WIDTH, bounds.bottom());
        // JAVA: itemCountMenu.createMenu().getBounds().getLeft() + 90 — probe an empty
        // menu for its position.
        let probe = Self::item_count_builder(item_count_anchor).create_menu(g);
        let costs_anchor = Point::new(probe.get_bounds().left() + 90, bounds.bottom());

        let item_count_menu = Self::item_count_builder(item_count_anchor).create_menu(g);
        let costs_menu = Self::costs_builder(costs_anchor).create_menu(g);

        let mut display = CraftingDisplay {
            base: DisplayBase::new(false, true, vec![recipe_menu, item_count_menu, costs_menu]),
            player_eid: player.c.eid,
            recipes,
            is_personal_crafter: is_personal,
            item_count_anchor,
            costs_anchor,
        };
        display.refresh_data(g, inventory);
        display
    }

    /// Java `itemCountMenu` builder.
    fn item_count_builder(anchor: Point) -> MenuBuilder {
        MenuBuilder::new(true, 0, RelPos::Left, Vec::new())
            .set_title("Have:")
            .set_title_pos(RelPos::TopLeft)
            .set_positioning(anchor, RelPos::BottomRight)
    }

    /// Java `costsMenu` builder.
    fn costs_builder(anchor: Point) -> MenuBuilder {
        MenuBuilder::new(true, 0, RelPos::Left, Vec::new())
            .set_title("Cost:")
            .set_title_pos(RelPos::TopLeft)
            .set_positioning(anchor, RelPos::BottomRight)
    }

    /// Java `refreshData()`.
    fn refresh_data(&mut self, g: &Game, inventory: &Inventory) {
        let mut costs_menu = Self::costs_builder(self.costs_anchor)
            .set_entries(self.get_cur_item_costs(g, inventory))
            .create_menu(g);

        let product = self.selected_recipe().borrow().get_product(g);
        let count = inventory.count(&product); // JAVA: getCurItemCount()
        let mut item_count_menu = Self::item_count_builder(self.item_count_anchor)
            .set_entries(vec![handle(ItemListing::new(product, &count.to_string()))])
            .create_menu(g);

        {
            let prev = &self.base.menus[2];
            costs_menu.set_frame_colors_from(prev);
            item_count_menu.set_frame_colors_from(prev);
        }
        self.base.menus[2] = costs_menu;
        self.base.menus[1] = item_count_menu;
    }

    /// Java `recipes[recipeMenu.getSelection()]`.
    fn selected_recipe(&self) -> &Rc<RefCell<Recipe>> {
        &self.recipes[self.base.menus[0].get_selection() as usize]
    }

    /// Java `getCurItemCosts()`.
    fn get_cur_item_costs(&self, g: &Game, inventory: &Inventory) -> Vec<EntryHandle> {
        let mut cost_list = Vec::new();
        // JAVA: iterates the costs HashMap's keySet (unspecified order); the port's cost
        // list preserves the recipe's insertion order.
        let recipe = self.selected_recipe().borrow();
        for (item_name, amount) in recipe.get_costs() {
            let cost = registry::get(g, item_name);
            let text = format!("{}/{}", amount, inventory.count(&cost));
            cost_list.push(handle(ItemListing::new(cost, &text)));
        }
        cost_list
    }
}

impl Display for CraftingDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        if g.input.get_key("menu").clicked
            || (self.is_personal_crafter && g.input.get_key("craft").clicked)
        {
            g.exit_menu();
            return;
        }

        let prev_sel = self.base.menus[0].get_selection();
        display_tick_default(&mut self.base, g);
        if prev_sel != self.base.menus[0].get_selection() {
            // JAVA: refreshData() — reads player.getInventory().
            if let Some(mut player) = g.entities.take(self.player_eid) {
                self.refresh_data(g, &player.player_mut().inventory);
                g.entities.put_back(player);
            }
        }

        if (g.input.get_key("select").clicked || g.input.get_key("attack").clicked)
            && self.base.menus[0].get_selection() >= 0
        {
            // check the selected recipe
            let recipe = self.selected_recipe().clone();
            if recipe.borrow().get_can_craft() {
                // JAVA: r.craft(player) — take the player out so the inventory can be
                // borrowed alongside `g`.
                if let Some(mut player) = g.entities.take(self.player_eid) {
                    {
                        let inventory = &mut player.player_mut().inventory;
                        recipe.borrow().craft(g, inventory);

                        self.refresh_data(g, inventory);
                        for recipe in &self.recipes {
                            recipe.borrow_mut().check_can_craft(g, inventory);
                        }
                    }
                    g.entities.put_back(player);
                }
            }
        }
    }
}
