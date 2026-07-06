//! Port of `fdoom.entity.furniture.DungeonChest`.

use crate::core::game::Game;
use crate::entity::{Entity, EntityKind};
use crate::gfx::color;

use super::chest::ChestData;
use super::furniture_common;

pub fn open_col() -> i32 {
    color::get4(-1, 2, 115, 225)
}
pub fn lock_col() -> i32 {
    color::get4(-1, 222, 333, 555)
}

#[derive(Debug, Clone)]
pub struct DungeonChestData {
    pub chest: ChestData,
    pub is_locked: bool,
}

/// Java `new DungeonChest()` — populates its inventory with random loot.
pub fn new(g: &mut Game) -> Entity {
    let mut chest = ChestData::with_name("Dungeon Chest", lock_col());
    populate_inv(g, &mut chest);
    let c = furniture_common(chest.furniture.sprite.color, 3, 3);
    Entity::new(c, EntityKind::DungeonChest(DungeonChestData { chest, is_locked: true }))
}

/// Java `populateInv()`.
pub fn populate_inv(_g: &mut Game, _chest: &mut ChestData) {
    // TODO(port:entity-behavior): random loot table
}
