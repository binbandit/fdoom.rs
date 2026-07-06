//! Port of `fdoom.entity.furniture.Tnt`.
//!
//! Java used a 300ms swing Timer to restore exploded tiles; the port counts it in game
//! ticks via `explode_ticks_left` (18 ticks at 60/s = 300ms).

use crate::entity::{Entity, EntityKind};
use crate::gfx::{Sprite, color};

use super::{FurnitureData, furniture_common};

pub const FUSE_TIME: i32 = 90;
pub const BLAST_RADIUS: i32 = 32;
pub const BLAST_DAMAGE: i32 = 30;

pub fn tnt_color() -> i32 {
    color::get4(-1, 200, 300, 555)
}

#[derive(Debug, Clone)]
pub struct TntData {
    pub furniture: FurnitureData,
    pub ftik: i32,
    pub fuse_lit: bool,
    /// JAVA: Java removed the Tnt at explosion and restored the tiles from a 300ms swing
    /// Timer (`explodeTimer`/`levelSave`). The port instead keeps the (exploded, invisible)
    /// entity alive for this countdown, restoring the tiles and removing it when it hits 0.
    pub explode_ticks_left: Option<i32>,
}

/// Java `new Tnt()`.
pub fn new() -> Entity {
    let furniture = FurnitureData::new("Tnt", Sprite::new(14, 8, 2, 2, tnt_color(), 0));
    let c = furniture_common(furniture.sprite.color, 3, 2);
    Entity::new(
        c,
        EntityKind::Tnt(TntData {
            furniture,
            ftik: 0,
            fuse_lit: false,
            explode_ticks_left: None,
        }),
    )
}
