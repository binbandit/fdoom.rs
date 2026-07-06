//! Port of `fdoom.entity.furniture.Lantern`.

use crate::entity::{Entity, EntityKind};
use crate::gfx::{color, Sprite};

use super::{furniture_common, FurnitureData};

/// Java `Lantern.Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanternType {
    Norm,
    Iron,
    Gold,
}

impl LanternType {
    pub const VALUES: [LanternType; 3] = [LanternType::Norm, LanternType::Iron, LanternType::Gold];

    pub fn title(self) -> &'static str {
        match self {
            LanternType::Norm => "Lantern",
            LanternType::Iron => "Iron Lantern",
            LanternType::Gold => "Gold Lantern",
        }
    }

    pub fn light(self) -> i32 {
        match self {
            LanternType::Norm => 9,
            LanternType::Iron => 12,
            LanternType::Gold => 15,
        }
    }

    pub fn col(self) -> i32 {
        match self {
            LanternType::Norm => color::get4(-1, 0, 222, 555),
            LanternType::Iron => color::get4(-1, 100, 322, 544),
            LanternType::Gold => color::get4(-1, 110, 440, 553),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LanternData {
    pub furniture: FurnitureData,
    pub lantern_type: LanternType,
}

/// Java `new Lantern(type)`.
pub fn new(lantern_type: LanternType) -> Entity {
    let furniture =
        FurnitureData::new(lantern_type.title(), Sprite::new(10, 8, 2, 2, lantern_type.col(), 0));
    let c = furniture_common(furniture.sprite.color, 3, 2);
    Entity::new(c, EntityKind::Lantern(LanternData { furniture, lantern_type }))
}
