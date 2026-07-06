//! Behavior of `fdoom.entity.furniture.Crafter`.

use crate::core::game::Game;
use crate::entity::furniture::crafter::CrafterType;
use crate::entity::{Entity, EntityKind};

/// Java `Crafter.use(player)` — opens the crafting display.
pub fn use_furniture(g: &mut Game, e: &mut Entity, player: &mut Entity) -> bool {
    let EntityKind::Crafter(c) = &e.kind else {
        return false;
    };
    let crafter_type = c.crafter_type;
    // JAVA: Game.setMenu(new CraftingDisplay(type.recipes, type.name(), player));
    let recipes = match crafter_type {
        CrafterType::Workbench => g.recipes.workbench.clone(),
        CrafterType::Oven => g.recipes.oven.clone(),
        CrafterType::Furnace => g.recipes.furnace.clone(),
        CrafterType::Anvil => g.recipes.anvil.clone(),
        CrafterType::Enchanter => g.recipes.enchant.clone(),
        CrafterType::Loom => g.recipes.loom.clone(),
    };
    g.set_menu(crate::screen::crafting_display::CraftingDisplay::new(
        g,
        recipes,
        crafter_type.name(),
        player,
    ));
    true
}
