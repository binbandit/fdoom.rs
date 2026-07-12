//! Tidal Flat (sandbox era, no Java counterpart): the intertidal band where the ocean
//! meets the beach.
//!
//! Generation carves the band out of the upper Ocean strip and the lower Beach edge:
//! every tile whose `land` field value (see `infinite_gen::land_at`) falls in
//! `[BAND_LOW, BAND_HIGH)` becomes a Tidal Flat. At runtime the global tide level —
//! a pure cosine of the day clock, two tides per in-game day — sweeps through exactly
//! that range, so each flat tile is **submerged** while `land < tide_level` and
//! **exposed** otherwise. Because submersion compares per-tile elevation against one
//! global level, the waterline creeps tile-by-tile across the flat over the hours
//! instead of snapping.
//!
//! - Submerged: renders as (slightly darkened) water, passable by swimmers only,
//!   counts as swimming (`behavior::is_swimming`).
//! - Exposed: renders as wet sand (dedicated `tiles/wet_sand_texture` cells under
//!   the sand connector shapes in a damp palette) with phase-driven puddle glints,
//!   walkable, and rarely washes up a beachcombing find (Grass Fibers / Stone /
//!   rare gem) on random tile ticks — throttled so the shore never accumulates
//!   litter.

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::core::updater::DAY_LENGTH;
use crate::entity::behavior::can_swim;
use crate::entity::{Entity, EntityKind};
use crate::gfx::{Screen, Sprite, color};
use crate::level::drop_item;
use crate::level::infinite_gen::{hash, land_at, unit};

/// The tidal band in `land`-field units: low tide exposes down to `BAND_LOW`, high
/// tide submerges up to `BAND_HIGH`. Sits inside the Ocean strip (`land < 0.42`) and
/// the lower Beach edge (`land < 0.445`), leaving the ocean permanently wet shallows
/// below 0.405 (where seaweed/coral live) and the beach a permanently dry top above
/// 0.435 (where the palms stand).
pub const BAND_LOW: f64 = 0.405;
pub const BAND_HIGH: f64 = 0.435;

/// Beachcombing: 1-in-N chance per random tile tick on an exposed flat.
const FIND_ODDS: i32 = 80;
/// ...but only while fewer than this many item entities lie within `FIND_RADIUS`.
const FIND_MAX_NEARBY: usize = 2;
/// Throttle radius in tiles.
const FIND_RADIUS: i32 = 8;

pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::TidalFlat);
    // blend into both neighbors it straddles: open water below, beach sand above
    def.connects_to_water = true;
    def.connects_to_sand = true;
    def
}

/// The global tide level, in `land`-field units: a pure function of the day clock,
/// two smooth (cosine) tides per in-game day, sweeping `[BAND_LOW, BAND_HIGH]`.
/// High tides at tick 0 and `DAY_LENGTH / 2`, low tides at 1/4 and 3/4 day.
pub fn tide_level(g: &Game) -> f64 {
    let t = g.tick_count.rem_euclid(DAY_LENGTH) as f64 / DAY_LENGTH as f64;
    let mid = (BAND_LOW + BAND_HIGH) / 2.0;
    let amp = (BAND_HIGH - BAND_LOW) / 2.0;
    mid + amp * (t * 4.0 * std::f64::consts::PI).cos()
}

/// Is the flat at tile `(xt, yt)` currently under water? Compares the tile's own
/// elevation (the exact `land` field world gen thresholded, `infinite_gen::land_at`)
/// against the global tide level.
pub fn is_submerged(g: &Game, xt: i32, yt: i32) -> bool {
    land_at(g.world_seed, xt, yt) < tide_level(g)
}

pub fn render(g: &mut Game, screen: &mut Screen, lvl: usize, x: i32, y: i32) {
    if is_submerged(g, x, y) {
        // under the tide: ride on the regular water art, a touch darker than open
        // water so the flooded flat still reads as "shallow shore"
        let water = g.tiles.get("water");
        dispatch::render(g, screen, &water, lvl, x, y);
        screen.darken_rect(x * 16, y * 16, 16, 16, 40);
        return;
    }
    // exposed: dedicated wet-sand cells. Each 8x8 quarter picks independently from
    // the `tiles/wet_sand_texture` variant row (a ripple-and-glint cluster, a damp
    // patch, a plain damp cell, a lone ripple) by position hash, so detail arrives
    // in scattered clumps instead of a uniform dither. The edges reuse the sand
    // connector *shapes* in a damp palette, so the flat still blends seamlessly
    // into the dry beach above it. Shade roles: 0 damp patch, 1 base, 2 sheen,
    // 3 ripple shadow.
    let mut wet = (*g.tiles.get("sand")).clone();
    let cs = wet.csprite.as_mut().expect("sand has a csprite");
    cs.sparse = Sprite::new(11, 0, 3, 3, color::get4(330, 431, 330, 210), 3);
    cs.sides = cs.sparse.clone();
    let base = crate::assets::sprite_cell("tiles/wet_sand_texture").pos();
    let h = hash(g.world_seed, 0x5745_5453, x, y);
    let coords = [
        base + (h & 3) as i32,
        base + ((h >> 2) & 3) as i32,
        base + ((h >> 4) & 3) as i32,
        base + ((h >> 6) & 3) as i32,
    ];
    cs.full = crate::gfx::sprite::make_sprite(
        2,
        2,
        color::get4(320, 431, 455, 321),
        ((h >> 8) & 1) as i32,
        false,
        &coords,
    );
    dispatch::csprite_render(g, screen, &wet, lvl, x, y, None);
    // puddle glints: brief shimmer strips on a positional phase offset, the same
    // trick as the deep-water wave crests
    let phase = ((g.tick_count / 7) + (x * 5 + y * 11)) & 63;
    if phase < 3 {
        let row = (x * 3 + y * 7) & 7;
        screen.darken_rect(x * 16 + 2, y * 16 + row * 2, 12, 2, 50);
    }
}

/// Walkable when exposed; swimmers only while the tide is in.
pub fn may_pass(g: &Game, x: i32, y: i32, e: &Entity) -> bool {
    if is_submerged(g, x, y) {
        can_swim(e)
    } else {
        true
    }
}

/// Fossicking: an exposed flat is prime panning ground (the tide re-sorts the sand
/// twice a day). Submerged, there is nothing to stand on.
#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut crate::entity::Entity,
    item: &mut crate::item::Item,
    _attack_dir: crate::entity::Direction,
) -> bool {
    if is_submerged(g, xt, yt) {
        return false;
    }
    super::fossick::try_pan(g, lvl, xt, yt, player, item)
}

/// Random tile tick: beachcombing. While the flat is exposed, the receding tide
/// rarely leaves a find behind — throttled by the number of item entities already
/// lying nearby, so ignored shores don't silt up with litter.
pub fn tick(g: &mut Game, lvl: usize, xt: i32, yt: i32) {
    if is_submerged(g, xt, yt) {
        return;
    }
    if g.random.next_int_bound(FIND_ODDS) != 0 {
        return;
    }

    let (cx, cy) = (xt * 16 + 8, yt * 16 + 8);
    let near = |e: &Entity| {
        matches!(e.kind, EntityKind::ItemEntity(_))
            && (e.c.x - cx).abs() <= FIND_RADIUS * 16
            && (e.c.y - cy).abs() <= FIND_RADIUS * 16
    };
    let nearby = g
        .entities
        .entities_on_level(lvl)
        .filter(|e| near(e))
        .count()
        + g.level(lvl)
            .entities_to_add
            .iter()
            .filter(|e| near(e))
            .count();
    if nearby >= FIND_MAX_NEARBY {
        return;
    }

    // pick the find by hash (position + moment): washed-up weed most often, a
    // sea-tumbled stone next, and on a lucky day a gem glinting in the sand
    let roll = unit(hash(
        g.world_seed ^ i64::from(g.tick_count),
        0x71DA_1F1A,
        xt,
        yt,
    ));
    let name = if roll < 0.55 {
        "Grass Fibers"
    } else if roll < 0.95 {
        "Stone"
    } else {
        "gem"
    };
    let item = crate::item::registry::get(g, name);
    drop_item(g, lvl, cx, cy, item);
}
