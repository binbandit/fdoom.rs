//! Snow accumulation and thaw (snow wave, no Java counterpart): while snow falls on
//! a loaded surface chunk (`weather::snowing_at` — the schedule crossed with the
//! cold-reach climate gate), the natural ground slowly whitens one tile at a time;
//! when the sky clears, visiting snow thaws back off at a gentler pace. The world
//! breathes with the weather, like the tides.
//!
//! Runs on the level's ~1-in-50-per-tile random tick cadence, interposed in
//! `dispatch::tick` the same way the fire overlay is. Rates are tuned against the
//! slice length (a tile sees ~215 random ticks per weather slice): one snowy slice
//! dusts roughly a quarter of a clearing — visibly wintering, never a whiteout pop —
//! and a thaw takes several clear slices to undo.
//!
//! Correctness rules:
//! - **Natural families only.** Converts Grass / TallGrass → Snow and the broadleaf
//!   Tree → Snow Tree; thaws Snow → Grass and Snow Tree → Tree. Nothing else — never
//!   floors, farmland, paths, walls, sand, or any player-worked tile. (Bare dirt is
//!   left alone too: there is no snow-dusted dirt art, and dug dirt is player work.)
//! - **Native snow never thaws.** Inside Tundra — and on Mountain snow caps — snow is
//!   home ([`snow_native`] checks both `biome_at` and the render-facing
//!   `biome_at_blended`, so the patchy generated boundary is protected as well).
//!   Outside, snow is a visitor and melts back.
//! - Surface infinite layers only; a burning tile is owned by the fire overlay
//!   (`dispatch::tick` checks fire first).

use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::core::weather;
use crate::level::infinite_gen::{self, Biome};

/// Settle odds per random tick: grass (and tufts) under falling snow.
const SETTLE_GROUND_ODDS: i32 = 700;

/// Settle odds per random tick: a broadleaf canopy whitens a bit faster — fewer,
/// bigger landmarks make the accumulation legible.
const SETTLE_TREE_ODDS: i32 = 450;

/// Thaw odds per random tick, once the snow stops: roughly half the settle pace, so
/// a dusting lingers for a few clear slices before the green returns.
const THAW_GROUND_ODDS: i32 = 1500;
const THAW_TREE_ODDS: i32 = 1100;

/// Is snow *at home* at this position? Tundra proper and Mountains (summit caps) —
/// checked through both the plain and the domain-warped biome lookups, so the
/// generated patchy boundary snow counts as native and never erodes.
pub fn snow_native(seed: i64, x: i32, y: i32) -> bool {
    let home = |b: Biome| matches!(b, Biome::Tundra | Biome::Mountains);
    home(infinite_gen::biome_at(seed, x, y)) || home(infinite_gen::biome_at_blended(seed, x, y))
}

/// One accumulation/thaw attempt for this tile's random tick. Returns true when the
/// tile was converted (the caller stops — the new tile takes over next pass).
pub fn random_tick(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32) -> bool {
    {
        let level = g.level(lvl);
        if level.depth != 0 || !level.is_infinite() {
            return false;
        }
    }
    let (odds, to) = match def.kind {
        // settle: falling snow buries the meadow and whitens broadleaf canopies
        TileKind::Grass | TileKind::TallGrass { .. } if weather::snowing_at(g, x, y) => {
            (SETTLE_GROUND_ODDS, "snow")
        }
        TileKind::Tree if weather::snowing_at(g, x, y) => (SETTLE_TREE_ODDS, "snow tree"),
        // thaw: once the snow stops, visiting snow melts back off — never at home
        TileKind::Snow | TileKind::SnowTree
            if weather::snowing_at(g, x, y) || snow_native(g.world_seed, x, y) =>
        {
            return false;
        }
        TileKind::Snow => (THAW_GROUND_ODDS, "grass"),
        TileKind::SnowTree => (THAW_TREE_ODDS, "tree"),
        _ => return false,
    };
    if g.random.next_int_bound(odds) != 0 {
        return false;
    }
    let tile = g.tiles.get(to);
    g.set_tile_default(lvl, x, y, &tile);
    true
}
