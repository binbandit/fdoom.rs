//! Behavior of `fdoom.entity.furniture.Crafter`.

use crate::core::game::Game;
use crate::entity::furniture::crafter::CrafterType;
use crate::entity::{Entity, EntityKind};

/// Using a station opens the survival screen on CRAFT with the station's recipe
/// set and its name as a sub-header (UI_REDESIGN §3.5) — the other tabs stay
/// live, so the pack is reachable without stepping away from the bench.
pub fn use_furniture(g: &mut Game, e: &mut Entity, player: &mut Entity) -> bool {
    let EntityKind::Crafter(c) = &e.kind else {
        return false;
    };
    let crafter_type = c.crafter_type;
    let recipes = match crafter_type {
        CrafterType::Workbench => g.recipes.workbench.clone(),
        CrafterType::Oven => g.recipes.oven.clone(),
        CrafterType::Furnace => g.recipes.furnace.clone(),
        CrafterType::Anvil => g.recipes.anvil.clone(),
        CrafterType::Enchanter => g.recipes.enchant.clone(),
        CrafterType::Loom => g.recipes.loom.clone(),
    };
    g.set_menu(
        crate::screen::survival_display::SurvivalDisplay::at_station(
            g,
            player,
            crafter_type.name(),
            recipes,
        ),
    );
    true
}
