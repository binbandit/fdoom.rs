//! Port of `fdoom.entity.furniture.Chest`.

use crate::entity::{Entity, EntityKind};
use crate::gfx::{color, Sprite};
use crate::item::Inventory;

use super::{furniture_common, FurnitureData};

#[derive(Debug, Clone)]
pub struct ChestData {
    pub furniture: FurnitureData,
    pub inventory: Inventory,
}

impl ChestData {
    /// Java `new Chest(name, color)`.
    pub fn with_name(name: &str, color: i32) -> ChestData {
        ChestData {
            furniture: FurnitureData::new(name, Sprite::new(2, 8, 2, 2, color, 0)),
            inventory: Inventory::new(),
        }
    }
}

/// Java `new Chest()`.
pub fn new() -> Entity {
    let data = ChestData::with_name("Chest", color::get4(-1, 220, 331, 552));
    let c = furniture_common(data.furniture.sprite.color, 3, 3);
    Entity::new(c, EntityKind::Chest(data))
}
