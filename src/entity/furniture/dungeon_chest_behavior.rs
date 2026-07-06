//! Behavior of `fdoom.entity.furniture.DungeonChest`. TODO(port:entity-behavior)

use crate::core::game::Game;
use crate::entity::Entity;

pub fn use_furniture(g: &mut Game, e: &mut Entity, player: &mut Entity) -> bool {
    let _ = (g, e, player); // TODO(port:entity-behavior)
    false
}
