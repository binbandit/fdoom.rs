//! Fire overlay (fire wave, no Java counterpart): any `TileDef.flammable` tile can
//! burn. The burning state is **not** a tile id — it's the high bit of the tile's
//! per-tile data byte ([`BURN_FLAG`]), with the low bits repurposed as burn progress
//! while alight. That keeps every burning tile rendering/colliding as itself (a
//! burning wall is still a wall) with a flame overlay + flicker light on top.
//!
//! Lifecycle (all on the level's ~1-in-50-per-tile random tick cadence, so one
//! "burn tick" is ~50 game ticks):
//! - **ignite** — campfires ([`crate::entity::furniture::campfire_behavior`]) and
//!   spreading flames call [`ignite`]; it only takes on flammable, not-yet-burning
//!   tiles.
//! - **spread** — each burn tick rolls the four orthogonal neighbors; flammable ones
//!   catch with per-fuel-class odds (light fuel smolders out before it spreads far;
//!   heavy fuel — a wooden building — feeds a real blaze).
//! - **burn out** — after a fuel-class number of burn ticks the tile becomes its burn
//!   product: trees and brush char to dirt, wood walls/doors collapse into plank
//!   flooring, plank floors burn through to dirt (so a house burns in two stages).
//! - **extinguish** — heavy rain (`weather::extinguishes_fire`) douses on the next
//!   burn tick; adjacent water/mud has 1-in-2 odds per burn tick.
//!
//! Entities standing in flames are hurt from `behavior::mob_tick_base` (the same
//! hook lava uses); the hurt-time window paces the damage.

use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::core::weather;
use crate::gfx::Screen;

/// High bit of the tile data byte: this tile is on fire.
pub const BURN_FLAG: i32 = 0x80;

/// Fuel classes: how many burn (random) ticks a tile burns, and the % odds per burn
/// tick that it ignites each flammable orthogonal neighbor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fuel {
    /// Grasses and brush: quick flash, poor spreader — a grass fire runs a handful
    /// of tiles and dies unless the fuel is dense.
    Light,
    /// Trees and worked wood: burns long and spreads hard — dense fuel (a forest
    /// edge, a wooden building) sustains the blaze.
    Heavy,
}

impl Fuel {
    fn burn_ticks(self) -> i32 {
        match self {
            Fuel::Light => 2,
            Fuel::Heavy => 6,
        }
    }

    /// Containment tuning: a light-fuel tile makes ~2 burn ticks x 4 neighbors x 10%
    /// = 0.8 expected new fires even in a *solid* grass field — subcritical, so a
    /// grass fire always dies after a handful of tiles. Heavy fuel reproduces at
    /// ~2-5x — dense wood (a building, a forest edge) genuinely burns down, stopped
    /// only by firebreaks, water, or rain.
    fn catch_odds_pct(self) -> i32 {
        match self {
            Fuel::Light => 10,
            Fuel::Heavy => 40,
        }
    }
}

/// The fuel class of a flammable tile (None = not flammable, can't burn).
pub fn fuel_of(def: &TileDef) -> Option<Fuel> {
    if !def.flammable {
        return None;
    }
    Some(match def.kind {
        TileKind::TallGrass { .. } | TileKind::DryBush | TileKind::BerryBush => Fuel::Light,
        _ => Fuel::Heavy,
    })
}

/// Is the tile at (x, y) on fire?
pub fn is_burning(g: &Game, lvl: usize, x: i32, y: i32) -> bool {
    g.tile_at(lvl, x, y).flammable && g.level(lvl).get_data(x, y) & BURN_FLAG != 0
}

/// Set a flammable tile alight. Returns whether it caught (false: not flammable, or
/// already burning). Overwrites the tile's data byte — burn progress owns it now.
pub fn ignite(g: &mut Game, lvl: usize, x: i32, y: i32) -> bool {
    let def = g.tile_at(lvl, x, y);
    if !def.flammable || g.level(lvl).get_data(x, y) & BURN_FLAG != 0 {
        return false;
    }
    g.level_mut(lvl).set_data(x, y, BURN_FLAG);
    true
}

/// Put a burning tile out, keeping the tile itself (data resets to 0).
pub fn extinguish(g: &mut Game, lvl: usize, x: i32, y: i32) {
    if is_burning(g, lvl, x, y) {
        g.level_mut(lvl).set_data(x, y, 0);
    }
}

/// What a burnt-out tile collapses into.
fn burn_product(def: &TileDef) -> &'static str {
    match def.kind {
        // walls and doors drop into charred plank flooring — the rubble stage, which
        // is itself flammable, so a burning house burns *down*, not just out
        TileKind::Wall { .. } | TileKind::Door { .. } => "Wood Planks",
        // everything else — trees, brush, and the plank floor itself — chars to dirt
        _ => "dirt",
    }
}

/// Does this neighbor tile douse adjacent flames (open water / wet ground)?
fn is_wet(def: &TileDef) -> bool {
    matches!(
        def.name.as_str(),
        "WATER" | "DEEP WATER" | "MUD" | "TIDAL FLAT"
    )
}

/// One burn tick for a burning tile — called from `dispatch::tick` *instead of* the
/// tile's own tick while the [`BURN_FLAG`] is set.
pub fn random_tick(g: &mut Game, lvl: usize, x: i32, y: i32) {
    let def = g.tile_at(lvl, x, y);
    let Some(fuel) = fuel_of(&def) else { return };

    // heavy rain douses open flames
    if weather::extinguishes_fire(g) && g.level(lvl).depth == 0 {
        extinguish(g, lvl, x, y);
        puff_smoke(g, lvl, x, y);
        return;
    }

    // wet neighbors smother the fire (1-in-2 per burn tick)
    let wet_neighbor = [(0, -1), (0, 1), (-1, 0), (1, 0)]
        .iter()
        .any(|&(dx, dy)| is_wet(&g.tile_at(lvl, x + dx, y + dy)));
    if wet_neighbor && g.random.next_int_bound(2) == 0 {
        extinguish(g, lvl, x, y);
        puff_smoke(g, lvl, x, y);
        return;
    }

    // spread to flammable orthogonal neighbors, odds by the *catching* tile's fuel
    for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
        let (nx, ny) = (x + dx, y + dy);
        let ndef = g.tile_at(lvl, nx, ny);
        if let Some(nfuel) = fuel_of(&ndef) {
            if g.level(lvl).get_data(nx, ny) & BURN_FLAG == 0
                && g.random.next_int_bound(100) < nfuel.catch_odds_pct()
            {
                ignite(g, lvl, nx, ny);
            }
        }
    }

    puff_smoke(g, lvl, x, y);

    // advance burn progress; at the end of the fuel, collapse into the burn product
    let progress = (g.level(lvl).get_data(x, y) & !BURN_FLAG) + 1;
    if progress >= fuel.burn_ticks() {
        let product = g.tiles.get(burn_product(&def));
        g.set_tile_default(lvl, x, y, &product);
    } else {
        g.level_mut(lvl).set_data(x, y, BURN_FLAG | progress);
    }
}

/// A gray smoke puff drifting off the flames.
fn puff_smoke(g: &mut Game, lvl: usize, x: i32, y: i32) {
    let jx = g.random.next_int_bound(9) - 4;
    let jy = g.random.next_int_bound(5) - 2;
    let smoke = crate::entity::particle::new_smoke_particle(
        x * 16 + 8 + jx,
        y * 16 + 4 + jy,
        false,
        &mut g.random,
    );
    g.level_mut(lvl).add(smoke, lvl);
}

/// Flame overlay: four 8x8 flame cells over the burning tile, frame + mirror varied
/// per quadrant and game tick so the fire visibly dances.
pub fn render_overlay(g: &Game, screen: &mut Screen, x: i32, y: i32) {
    const FLAME_A: i32 = 10 + 21 * 32;
    const FLAME_B: i32 = 11 + 21 * 32;
    let t = g.tick_count >> 3;
    for (q, (qx, qy)) in [(0, 0), (8, 0), (0, 8), (8, 8)].iter().enumerate() {
        let phase = t + x + y * 3 + q as i32;
        let pos = if phase & 1 == 0 { FLAME_A } else { FLAME_B };
        let mirror = (phase >> 1) & 1; // horizontal flip on alternate beats
        // true-color cells — the palette argument is ignored
        screen.render(x * 16 + qx, y * 16 + qy, pos, 0, mirror);
    }
}

/// Strong flickering light for a burning tile (radius in tiles, torch-and-a-half).
pub fn light_radius(g: &Game, x: i32, y: i32) -> i32 {
    5 + ((g.tick_count >> 2) + x * 3 + y * 5).rem_euclid(2)
}
