//! Behavior of `fdoom.entity.furniture.Spawner`. TODO(port:entity-behavior)

use crate::core::game::Game;
use crate::entity::Entity;

pub fn tick(g: &mut Game, e: &mut Entity) {
    super::behavior::tick(g, e); // TODO(port:entity-behavior): spawn logic
}
