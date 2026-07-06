//! Port of `fdoom.level.tile.ExplodedTile` — "for tiles WHILE THEY ARE EXPLODING".

use super::{ConnectorSprite, TileDef, TileKind};
use crate::core::game::Game;
use crate::entity::Entity;
use crate::entity::behavior::mob_hurt_tile;
use crate::gfx::{Sprite, color};

/// Java `ExplodedTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Exploded);
    // JAVA: the sparse sprite's color is the literal 0 (not a Color.get value).
    def.csprite = Some(ConnectorSprite::simple(
        Sprite::new(14, 0, 3, 3, 0, 3),
        Sprite::dots(color::get4(555, 555, 555, 550)),
    ));
    def.connects_to_sand = true;
    def.connects_to_water = true;
    def.connects_to_lava = true;
    def
}

/// Java anonymous `ConnectorSprite.connectsTo` override.
pub fn connects_to(_def: &TileDef, other: &TileDef, is_side: bool) -> bool {
    !is_side || other.connects_to_liquid()
}

pub fn stepped_on(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, e: &mut Entity) {
    let _ = lvl;
    if e.mob().is_some() {
        mob_hurt_tile(g, e, def, x, y, 50);
    }
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    true
}
