//! Port of `fdoom.entity.furniture.Crafter`.

use crate::entity::{Entity, EntityKind};
use crate::gfx::{Sprite, color};

use super::{FurnitureData, furniture_common};

/// Java `Crafter.Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrafterType {
    Workbench,
    Oven,
    Furnace,
    Anvil,
    Enchanter,
    Loom,
}

impl CrafterType {
    pub const VALUES: [CrafterType; 6] = [
        CrafterType::Workbench,
        CrafterType::Oven,
        CrafterType::Furnace,
        CrafterType::Anvil,
        CrafterType::Enchanter,
        CrafterType::Loom,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CrafterType::Workbench => "Workbench",
            CrafterType::Oven => "Oven",
            CrafterType::Furnace => "Furnace",
            CrafterType::Anvil => "Anvil",
            CrafterType::Enchanter => "Enchanter",
            CrafterType::Loom => "Loom",
        }
    }

    pub fn sprite(self) -> Sprite {
        match self {
            CrafterType::Workbench => Sprite::new(8, 8, 2, 2, color::get4(-1, 100, 321, 431), 0),
            CrafterType::Oven => Sprite::new(4, 8, 2, 2, color::get4(-1, 0, 332, 442), 0),
            CrafterType::Furnace => Sprite::new(6, 8, 2, 2, color::get4(-1, 0, 222, 333), 0),
            CrafterType::Anvil => Sprite::new(0, 8, 2, 2, color::get4(-1, 0, 222, 333), 0),
            CrafterType::Enchanter => Sprite::new(12, 8, 2, 2, color::get4(-1, 623, 999, 111), 0),
            CrafterType::Loom => Sprite::new(18, 8, 2, 2, color::get4(-1, 100, 333, 211), 0),
        }
    }

    pub fn radius(self) -> (i32, i32) {
        match self {
            CrafterType::Enchanter | CrafterType::Loom => (7, 2),
            _ => (3, 2),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CrafterData {
    pub furniture: FurnitureData,
    pub crafter_type: CrafterType,
}

/// Java `new Crafter(type)`.
pub fn new(crafter_type: CrafterType) -> Entity {
    let furniture = FurnitureData::new(crafter_type.name(), crafter_type.sprite());
    let (xr, yr) = crafter_type.radius();
    let c = furniture_common(furniture.sprite.color, xr, yr);
    Entity::new(
        c,
        EntityKind::Crafter(CrafterData {
            furniture,
            crafter_type,
        }),
    )
}
