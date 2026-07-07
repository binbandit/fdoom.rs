//! Tnt: a placeable bomb with a fuse, a blast, and a brief "exploding" tile overlay
//! that is restored shortly after the bang (see `explode_ticks_left`).

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
    /// After the blast the (invisible, already-exploded) entity stays alive for this
    /// countdown — 18 ticks, ~300ms — then restores the "exploding" overlay tiles and
    /// removes itself. Some(n) = counting down; None = not yet exploded.
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
