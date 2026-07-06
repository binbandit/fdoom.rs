//! Port of `fdoom.entity.furniture` — data structs + constructors; behaviors in the
//! furniture behavior functions.

pub mod bed;
pub mod chest;
pub mod crafter;
pub mod death_chest;
pub mod dungeon_chest;
pub mod lantern;
pub mod spawner;
pub mod tnt;

use crate::entity::Direction;
use crate::gfx::Sprite;

/// Fields of the Java `Furniture` base class.
#[derive(Debug, Clone)]
pub struct FurnitureData {
    pub push_time: i32,
    pub multi_push_time: i32,
    pub push_dir: Direction,
    pub sprite: Sprite,
    pub name: String,
}

impl FurnitureData {
    /// Java `Furniture(name, sprite)`/`Furniture(name, sprite, xr, yr)` — the xr/yr go on
    /// `EntityCommon`; `col` is set from the sprite color by the caller.
    pub fn new(name: &str, sprite: Sprite) -> FurnitureData {
        FurnitureData {
            push_time: 0,
            multi_push_time: 0,
            push_dir: Direction::None,
            sprite,
            name: name.to_string(),
        }
    }
}

/// Helper shared by all furniture constructors: builds the entity common with the
/// Java `Furniture` super call semantics (col = sprite color).
pub fn furniture_common(sprite_color: i32, xr: i32, yr: i32) -> crate::entity::EntityCommon {
    let mut c = crate::entity::EntityCommon::new(xr, yr);
    c.col = sprite_color;
    c
}
