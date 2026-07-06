//! Port of `fdoom.level.tile.WaterTile`.

use super::{ConnectorSprite, TileDef, TileKind, dirt, dispatch};
use crate::core::game::Game;
use crate::entity::Entity;
use crate::entity::behavior::can_swim;
use crate::gfx::{Screen, Sprite, color};

/// Java `WaterTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Water);
    def.csprite = Some(ConnectorSprite::simple(
        Sprite::new(14, 0, 3, 3, color::get4(3, 105, 211, 321), 3),
        Sprite::dots(color::get4(5, 105, 115, 115)),
    ));
    def.connects_to_sand = true;
    def.connects_to_water = true;
    def
}

/// Java anonymous `ConnectorSprite.connectsTo` override.
pub fn connects_to(_def: &TileDef, other: &TileDef, _is_side: bool) -> bool {
    other.connects_to_water
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, e: &Entity) -> bool {
    can_swim(e)
}

/// Java anonymous `ConnectorSprite.getSparseColor` override.
pub fn get_sparse_color(_def: &TileDef, tile: &TileDef, orig_col: i32) -> i32 {
    if !tile.connects_to_water && tile.connects_to_sand {
        color::get4(3, 105, 440, 550)
    } else {
        orig_col
    }
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    // JAVA: `(tickCount + (x / 2 - y) * 4311) / 10` is int math; the `* 54687121l`
    // promotes to long.
    let int_part = g
        .tile_tick_count
        .wrapping_add((x / 2 - y).wrapping_mul(4311))
        / 10;
    let seed = (int_part as i64)
        .wrapping_mul(54687121)
        .wrapping_add(x as i64 * 3271612)
        .wrapping_add(y as i64 * 3412987161);

    let mut tmp = def.clone();
    let cs = tmp.csprite.as_mut().expect("water has a csprite");
    cs.full = Sprite::random_dots(seed, cs.full.color);
    let full = cs.full.color;
    let sparse = color::get4(3, 105, 211, dirt::d_col(g.level(lvl).depth));
    // JAVA: `sides` aliases `sparse` (two-sprite ConnectorSprite), so the side color
    // follows the sparse recolor.
    dispatch::csprite_render(g, screen, &tmp, lvl, x, y, Some((sparse, sparse, full)));
}

pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let mut xn = xt;
    let mut yn = yt;

    if g.random.next_boolean() {
        xn += g.random.next_int_bound(2) * 2 - 1;
    } else {
        yn += g.random.next_int_bound(2) * 2 - 1;
    }

    if g.tile_at(lvl, xn, yn).same_tile(&g.tiles.get("hole")) {
        g.set_tile_default(lvl, xn, yn, def);
    }
    if g.tile_at(lvl, xn, yn).same_tile(&g.tiles.get("lava")) {
        // JAVA: "Stone Bricks" is not a registered tile name; Tiles.get logs an error and
        // falls back to tile 0 (grass).
        let t = g.tiles.get("Stone Bricks");
        g.set_tile_default(lvl, xn, yn, &t);
    }
}
