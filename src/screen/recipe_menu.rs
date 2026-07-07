//! Port of `fdoom.screen.RecipeMenu` (an ItemListMenu of RecipeEntries).

use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

use crate::core::game::Game;
use crate::item::{Inventory, Recipe};

use super::entry::EntryHandle;
use super::entry::recipe_entry::RecipeEntry;
use super::item_list_menu;
use super::menu::Menu;

/// Java `getAndSortRecipes(recipes, player)`. Sorts the shared list in place (craftable
/// recipes first) and builds the entries.
fn get_and_sort_recipes(
    g: &Game,
    recipes: &mut [Rc<RefCell<Recipe>>],
    inventory: &Inventory,
) -> Vec<EntryHandle> {
    // refresh every recipe's canCraft flag once up front; the sort below only reads it
    for r in recipes.iter() {
        r.borrow_mut().check_can_craft(g, inventory);
    }
    recipes.sort_by(|r1, r2| {
        let craft1 = r1.borrow().get_can_craft();
        let craft2 = r2.borrow().get_can_craft();
        if craft1 == craft2 {
            Ordering::Equal
        } else if craft1 {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    });

    RecipeEntry::use_recipes(g, recipes)
}

/// Java `new RecipeMenu(recipes, title, player)`.
pub fn new(
    g: &Game,
    recipes: &mut [Rc<RefCell<Recipe>>],
    title: &str,
    inventory: &Inventory,
) -> Menu {
    let entries = get_and_sort_recipes(g, recipes, inventory);
    item_list_menu::new(g, entries, title)
}

/// Java `new RecipeMenu(recipes, title, player, fillCol, edgeStrokeCol, edgeFillCol)`.
pub fn with_frame(
    g: &Game,
    recipes: &mut [Rc<RefCell<Recipe>>],
    title: &str,
    inventory: &Inventory,
    fill_col: i32,
    edge_stroke_col: i32,
    edge_fill_col: i32,
) -> Menu {
    let entries = get_and_sort_recipes(g, recipes, inventory);
    item_list_menu::new_with_builder(
        g,
        item_list_menu::get_builder().set_frame_colors(fill_col, edge_stroke_col, edge_fill_col),
        entries,
        title,
    )
}
