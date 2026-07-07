//! Port of `fdoom.entity.furniture.Lantern`.

use crate::entity::{Entity, EntityKind};
use crate::gfx::{Sprite, color};

use super::{FurnitureData, furniture_common};

/// Java `Lantern.Type` — extended post-port with `Jacko` (the carved Jack-O-Lantern,
/// crafted from Pumpkin + Torch; no Java origin). Keep new variants at the END of
/// `VALUES`: saves store the lantern type as its `VALUES` ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanternType {
    Norm,
    Iron,
    Gold,
    Jacko,
}

impl LanternType {
    pub const VALUES: [LanternType; 4] = [
        LanternType::Norm,
        LanternType::Iron,
        LanternType::Gold,
        LanternType::Jacko,
    ];

    pub fn title(self) -> &'static str {
        match self {
            LanternType::Norm => "Lantern",
            LanternType::Iron => "Iron Lantern",
            LanternType::Gold => "Gold Lantern",
            LanternType::Jacko => "Jack-O-Lantern",
        }
    }

    pub fn light(self) -> i32 {
        match self {
            LanternType::Norm => 9,
            LanternType::Iron => 12,
            LanternType::Gold => 15,
            LanternType::Jacko => 8, // a flickering carved gourd, dimmer than a real lantern
        }
    }

    pub fn col(self) -> i32 {
        match self {
            LanternType::Norm => color::get4(-1, 0, 222, 555),
            LanternType::Iron => color::get4(-1, 100, 322, 544),
            LanternType::Gold => color::get4(-1, 110, 440, 553),
            // TODO(art): dedicated jack-o-lantern furniture cells + item icon; placeholder
            // reuses the lantern cells (10,8 2x2) in the pumpkin tile's palette.
            LanternType::Jacko => color::get4(-1, 210, 530, 550),
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
    let furniture = FurnitureData::new(
        lantern_type.title(),
        Sprite::new(10, 8, 2, 2, lantern_type.col(), 0),
    );
    let c = furniture_common(furniture.sprite.color, 3, 2);
    Entity::new(
        c,
        EntityKind::Lantern(LanternData {
            furniture,
            lantern_type,
        }),
    )
}
