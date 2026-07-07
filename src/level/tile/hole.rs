//! Port of `fdoom.level.tile.HoleTile`.

use super::{ConnectorSprite, TileDef, TileKind, dirt, dispatch};
use crate::core::game::Game;
use crate::entity::Entity;
use crate::entity::behavior::can_swim;
use crate::gfx::{Screen, Sprite, color};

/// Java `HoleTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Hole);
    def.csprite = Some(ConnectorSprite::simple(
        Sprite::new(14, 0, 3, 3, color::get4(3, 222, 211, 321), 3),
        Sprite::dots(color::get4(222, 222, 220, 220)),
    ));
    def.connects_to_sand = true;
    def.connects_to_water = true;
    def.connects_to_lava = true;
    def
}

/// Java anonymous `ConnectorSprite.connectsTo` override.
pub fn connects_to(_def: &TileDef, other: &TileDef, _is_side: bool) -> bool {
    other.connects_to_liquid()
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, e: &Entity) -> bool {
    can_swim(e)
}

/// Java anonymous `ConnectorSprite.getSparseColor` override.
pub fn get_sparse_color(_def: &TileDef, tile: &TileDef, orig_col: i32) -> i32 {
    if !tile.connects_to_liquid() && tile.connects_to_sand {
        color::get4(3, 222, 440, 550)
    } else {
        orig_col
    }
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let sparse = color::get4(3, 222, 211, dirt::d_col(g.level(lvl).depth));
    let full = def.csprite.as_ref().map(|cs| cs.full.color).unwrap_or(0);
    // two-sprite ConnectorSprite: sides share the sparse sprite, so the side color
    // must follow the sparse recolor
    dispatch::csprite_render(g, screen, def, lvl, x, y, Some((sparse, sparse, full)));
}
