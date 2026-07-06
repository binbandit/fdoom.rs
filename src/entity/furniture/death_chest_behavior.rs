//! Behavior of `fdoom.entity.furniture.DeathChest`. TODO(port:entity-behavior)

use crate::core::game::Game;
use crate::entity::Entity;
use crate::gfx::Screen;

pub fn tick(g: &mut Game, e: &mut Entity) {
    super::behavior::tick(g, e); // TODO(port:entity-behavior): expiry countdown
}

pub fn render(g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    super::behavior::render(g, screen, e); // TODO(port:entity-behavior): red flash + timer text
}
