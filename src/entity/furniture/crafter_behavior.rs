//! Behavior of `fdoom.entity.furniture.Crafter`. TODO(port:entity-behavior)

use crate::core::game::Game;
use crate::entity::Entity;

/// Java `Crafter.use(player)` — opens the crafting display.
pub fn use_furniture(g: &mut Game, e: &mut Entity, player: &mut Entity) -> bool {
    let _ = (g, e, player); // TODO(port:screen): CraftingDisplay
    false
}
