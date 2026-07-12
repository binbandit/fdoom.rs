//! Spring Water (content wave, no Java counterpart): the water of a geothermal hot
//! spring — rare warm pools in Tundra and Mountains (placed by `features_gen`). Rides
//! the ordinary water machinery (connector shoreline art, swimming) under a warm
//! mineral-teal palette, breathes gentle steam wisps on its random tick, and never
//! freezes or snows over (`snowfall` only converts the natural ground families).
//!
//! Warmth is the point: `core::temperature` clamps cold toward comfort within
//! basking range of any spring tile (`Modifiers::near_spring`) — a found sanctuary
//! in the coldest country, the campfire's wild cousin.
//!
//! TODO(art): dedicated spring cells — the render reuses the water connector cells
//! (14, 0) and dot shimmer under a teal palette.

use super::{ConnectorSprite, TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::entity::Entity;
use crate::entity::behavior::can_swim;
use crate::entity::particle::new_smoke_particle;
use crate::gfx::{Screen, Sprite, color};

/// Interior shimmer: luminous milky turquoise over a deep teal bed (kept two
/// steps brighter than plain water so the pool reads WARM against snow).
const FULL_COL: i32 = color::get4(33, 144, 255, 255);
/// Shoreline: dark line, bright teal body, pale turquoise, and a warm ochre lap
/// line on the bank — the mineral terrace a hot spring stains around itself.
const SPARSE_COL: i32 = color::get4(3, 144, 244, 432);

pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::SpringWater);
    def.csprite = Some(ConnectorSprite::simple(
        Sprite::new(14, 0, 3, 3, SPARSE_COL, 3),
        Sprite::dots(FULL_COL),
    ));
    // water-family: pool cells merge with each other (and with any adjacent water)
    def.connects_to_water = true;
    def
}

/// Same water-family rule as `water::connects_to`.
pub fn connects_to(_def: &TileDef, other: &TileDef, _is_side: bool) -> bool {
    other.connects_to_water
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, e: &Entity) -> bool {
    can_swim(e)
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    // the water tile's ripple-shimmer seed, unchanged (see water.rs for the rationale
    // behind the truncating i32 math)
    let int_part = g
        .tile_tick_count
        .wrapping_add((x / 2 - y).wrapping_mul(4311))
        / 10;
    let seed = (int_part as i64)
        .wrapping_mul(54687121)
        .wrapping_add(x as i64 * 3271612)
        .wrapping_add(y as i64 * 3412987161);

    let mut tmp = def.clone();
    let cs = tmp.csprite.as_mut().expect("spring water has a csprite");
    cs.full = Sprite::random_dots(seed, FULL_COL);
    dispatch::csprite_render(
        g,
        screen,
        &tmp,
        lvl,
        x,
        y,
        Some((SPARSE_COL, SPARSE_COL, FULL_COL)),
    );
}

/// Random tick: a thin steam wisp curls up off the warm water (the campfire's smoke
/// particle, wispy variant). At the ~1-in-50 tile-tick cadence a small pool keeps
/// two or three wisps drifting — steaming, never billowing.
pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let jx = g.random.next_int_bound(9);
    let jy = g.random.next_int_bound(5);
    // the fat puff cell (two gray shades) so steam stays readable on white snow
    let steam = new_smoke_particle(xt * 16 + jx, yt * 16 + jy - 2, false, &mut g.random);
    g.level_mut(lvl).add(steam, lvl);
}
