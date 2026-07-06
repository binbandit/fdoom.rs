//! Behavior of `fdoom.entity.furniture.Chest` (+DeathChest use). TODO(port:entity-behavior)

use crate::core::game::Game;
use crate::entity::Entity;

/// Java `Chest.use(player)` — opens the container display.
pub fn use_furniture(g: &mut Game, e: &mut Entity, player: &mut Entity) -> bool {
    let _ = (g, e, player); // TODO(port:screen): ContainerDisplay
    false
}
