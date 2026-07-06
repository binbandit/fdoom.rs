//! Port of `fdoom.screen.entry.RecipeEntry` (an ItemEntry subclass).
//!
//! Java entries shared the `Recipe` objects with `CraftingDisplay`, so `checkCanCraft`
//! calls there changed the entry's render color too; the port shares them the same way
//! via `Rc<RefCell<Recipe>>`.

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::game::Game;
use crate::gfx::{Screen, font};
use crate::item::{Item, Recipe};

use super::{COL_SLCT, COL_UNSLCT, EntryFlags, EntryHandle, ListEntry, handle};

pub struct RecipeEntry {
    recipe: Rc<RefCell<Recipe>>,
    /// Java `super(r.getProduct())` — the ItemEntry layer's item.
    item: Item,
    flags: EntryFlags,
}

impl RecipeEntry {
    /// Java `new RecipeEntry(r)`.
    pub fn new(g: &Game, recipe: Rc<RefCell<Recipe>>) -> RecipeEntry {
        let item = recipe.borrow().get_product(g);
        RecipeEntry {
            recipe,
            item,
            flags: EntryFlags::default(),
        }
    }

    /// Java `RecipeEntry.useRecipes(recipes)`.
    pub fn use_recipes(g: &Game, recipes: &[Rc<RefCell<Recipe>>]) -> Vec<EntryHandle> {
        recipes
            .iter()
            .map(|r| handle(RecipeEntry::new(g, r.clone())))
            .collect()
    }

    /// Java `getItem()` (from the ItemEntry layer).
    pub fn get_item(&self) -> &Item {
        &self.item
    }
}

impl ListEntry for RecipeEntry {
    fn flags(&self) -> EntryFlags {
        self.flags
    }

    fn flags_mut(&mut self) -> &mut EntryFlags {
        &mut self.flags
    }

    fn tick(&mut self, _g: &mut Game) {}

    fn render(&mut self, screen: &mut Screen, g: &mut Game, x: i32, y: i32, _is_selected: bool) {
        if self.flags.visible {
            let col = if self.recipe.borrow().get_can_craft() {
                COL_SLCT
            } else {
                COL_UNSLCT
            };
            let text = self.to_display_string(g);
            font::draw(&text, screen, x, y, col);
            self.item.sprite.render(screen, x, y);
        }
    }

    fn to_display_string(&self, g: &Game) -> String {
        let amount = self.recipe.borrow().get_amount();
        let suffix = if amount > 1 {
            format!(" x{amount}")
        } else {
            String::new()
        };
        format!("{}{}", self.item.get_display_name(g), suffix)
    }
}
