//! Ambient heat/cold — the temperature wave. Original post-port content.
//!
//! The whole system is one **pure score** in "band units" (each 1.0 = one comfort
//! band), assembled from systems that already exist: the continental climate field
//! (`infinite_gen`'s octave-0 noise — the same field the biome gates threshold), the
//! day clock, the weather schedule, tiles (tree canopy, interior floors, snow, water),
//! the campfire, and the worn-armor slot. Nothing here is saved: body temperature is
//! recomputed from the world every tick, so saves are untouched.
//!
//! Band table (score thresholds at ±0.5 / ±1.5 / ±2.5):
//!
//! | band      | score        | effect                                              |
//! |-----------|--------------|-----------------------------------------------------|
//! | Freezing  | <= -2.5      | slow damage (stops at 3 hearts unless score <= -3.8)|
//! | Cold      | (-2.5,-1.5]  | stamina regains slower; breath-fog cue; shivers     |
//! | Chilly    | (-1.5,-0.5]  | HUD tint only                                       |
//! | Comfort   | (-0.5, 0.5)  | nothing (indicator hidden)                          |
//! | Warm      | [0.5, 1.5)   | HUD tint only                                       |
//! | Hot       | [1.5, 2.5)   | stamina regains slower; heat cue; sweat             |
//! | Scorching | >= 2.5       | slow damage (stops at 3 hearts unless score >= 3.8) |
//!
//! Mitigations (cheap and thematic, all one place to look):
//! - Fur Coat worn: cold shifted **two** bands toward comfort.
//! - Straw Hat worn: heat shifted one band toward comfort.
//! - Tree canopy / an interior floor underfoot: shade, one heat band.
//! - Swimming: heat clamped to comfort outright (water breaks heat).
//! - A lit campfire within resting range: cold clamped to comfort outright.
//!
//! Consumers: `entity::mob::player_behavior::temperature_tick` (effects) and the
//! renderer's small thermometer dot (`core::renderer`). Tests: `tests/temperature.rs`.

use crate::core::game::Game;
use crate::core::updater::DAY_LENGTH;
use crate::core::weather::{self, Precip};
use crate::entity::{Entity, EntityKind};
use crate::level::infinite_gen::{self, Biome};
use crate::level::tile::TileKind;

/* --------------------------------- the climate field --------------------------------- */

/// Salt of the continental climate field — MUST match `infinite_gen::climate_at`
/// (octave 0 of the salt-6 fractal, period 512). A local copy of the single-octave
/// value noise, same convention as `weather::lattice_noise` (only `hash`/`unit` are
/// shared crate-wide). `tests/temperature.rs` pins this to the biome gates: Tundra
/// tiles always read `< 0.30` here and Desert tiles `> 0.70`.
const CLIMATE_SALT: u64 = 6;
const CLIMATE_PERIOD: i32 = 512;

/// The smooth continental climate at a tile, 0 (polar) .. 1 (equatorial).
pub fn climate(seed: i64, x: i32, y: i32) -> f64 {
    let period = CLIMATE_PERIOD;
    let fx = x.div_euclid(period);
    let fy = y.div_euclid(period);
    let tx = x.rem_euclid(period) as f64 / period as f64;
    let ty = y.rem_euclid(period) as f64 / period as f64;

    let v00 = infinite_gen::unit(infinite_gen::hash(seed, CLIMATE_SALT, fx, fy));
    let v10 = infinite_gen::unit(infinite_gen::hash(seed, CLIMATE_SALT, fx + 1, fy));
    let v01 = infinite_gen::unit(infinite_gen::hash(seed, CLIMATE_SALT, fx, fy + 1));
    let v11 = infinite_gen::unit(infinite_gen::hash(seed, CLIMATE_SALT, fx + 1, fy + 1));

    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let a = v00 + (v10 - v00) * sx;
    let b = v01 + (v11 - v01) * sx;
    a + (b - a) * sy
}

/* ------------------------------------ the model ------------------------------------ */

/// Climate-to-score span: climate 0..1 maps to -2.5..+2.5 before the day/weather
/// terms. The biome gates land at: tundra gate (0.30) -> -1.0, desert gate (0.70)
/// -> +1.0, deep tundra (~0.10) -> -2.0, deep desert (~0.90) -> +2.0.
const CLIMATE_SCALE: f64 = 5.0;

/// Midday warmth: small everywhere (forest noon stays comfort) but grows with hot
/// climate — deep desert noon lands in Scorching.
const SUN_BASE: f64 = 0.35;
const SUN_DRY: f64 = 0.7;

/// Night chill: every biome's nights dip about one band (the campfire becomes home),
/// and dry hot country loses far more — desert nights are properly chilly.
const NIGHT_BASE: f64 = 0.8;
const NIGHT_DRY: f64 = 1.2;

/// Falling weather chills: rain damp, snowfall worse.
const RAIN_CHILL: f64 = 0.6;
const SNOW_CHILL: f64 = 1.0;

/// Underground layers: constant cave-cool (Chilly — a tint, never a mechanic).
pub const MINE_SCORE: f64 = -0.8;

/// Snow-covered ground underfoot chills half a band.
const SNOW_GROUND_CHILL: f64 = 0.5;

/// Fur Coat: cold shifted two bands toward comfort. Straw Hat / shade: one heat band.
const COAT_SHIFT: f64 = 2.0;
const HAT_SHIFT: f64 = 1.0;
const SHADE_SHIFT: f64 = 1.0;

/// Past this |score| the 3-heart mercy floor no longer holds. Only the deepest
/// climate extremes stacked with the worst weather/time reach it — death by
/// temperature means ignoring every signal for minutes.
pub const DEADLY_SCORE: f64 = 3.8;

/// Day-clock warmth wave in [-1, 1]: peak at 0.375 of the day (midday — the Day
/// quarter's midpoint) and trough at 0.875 (deep night).
fn day_wave(day_tick: i32) -> f64 {
    let frac = day_tick.rem_euclid(DAY_LENGTH) as f64 / DAY_LENGTH as f64;
    (std::f64::consts::TAU * (frac - 0.375)).cos()
}

/// The pure ambient score on an infinite surface: climate + time of day + weather.
/// Everything a tile "is" before the player's own mitigations.
pub fn ambient_score(seed: i64, xt: i32, yt: i32, day_tick: i32, precip: Precip) -> f64 {
    let c = (climate(seed, xt, yt) - 0.5) * CLIMATE_SCALE;
    let dry = c.max(0.0); // hot-dry country swings harder, day and night
    let wave = day_wave(day_tick);
    let sun = wave.max(0.0);
    let night = (-wave).max(0.0);

    let mut s = c + sun * (SUN_BASE + dry * SUN_DRY) - night * (NIGHT_BASE + dry * NIGHT_DRY);
    s += match precip {
        Precip::None => 0.0,
        Precip::Rain(i) => -RAIN_CHILL * i as f64,
        Precip::Snow(i) => -SNOW_CHILL * i as f64,
    };
    s
}

/* ------------------------------------- bands ------------------------------------- */

/// Comfort bands, coldest to hottest. `steps()` counts bands away from Comfort
/// (negative = cold side).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Band {
    Freezing,
    Cold,
    Chilly,
    Comfort,
    Warm,
    Hot,
    Scorching,
}

impl Band {
    pub fn from_score(s: f64) -> Band {
        if s <= -2.5 {
            Band::Freezing
        } else if s <= -1.5 {
            Band::Cold
        } else if s <= -0.5 {
            Band::Chilly
        } else if s < 0.5 {
            Band::Comfort
        } else if s < 1.5 {
            Band::Warm
        } else if s < 2.5 {
            Band::Hot
        } else {
            Band::Scorching
        }
    }

    /// Bands away from Comfort: Freezing -3 .. Scorching +3.
    pub fn steps(self) -> i32 {
        self as i32 - 3
    }
}

/* -------------------------------- the player's body -------------------------------- */

/// Weather as felt at a tile, from the public schedule pieces (`weather::precip`
/// reads the player out of the arena, which is empty during the player's own
/// take-out tick — so re-derive it positionally; same gates, ramp edges ignored).
fn precip_at(g: &Game, xt: i32, yt: i32) -> Precip {
    let seed = g.world_seed;
    let (day, t) = (g.events.day_number, g.tick_count);
    let i = weather::schedule_intensity(seed, day, t);
    if i <= 0.0 {
        return Precip::None;
    }
    match infinite_gen::biome_at(seed, xt, yt) {
        Biome::Desert if !weather::desert_slice_wet(seed, day, t / weather::SLICE_LEN) => {
            Precip::None
        }
        Biome::Tundra => Precip::Snow(i),
        _ => Precip::Rain(i),
    }
}

/// The BODY-worn item's name, if the entity is a player wearing one.
fn worn(e: &Entity) -> Option<&str> {
    match &e.kind {
        EntityKind::Player(pd) => pd.cur_armor.as_ref().map(|a| a.get_name()),
        _ => None,
    }
}

/// The HEAD-worn item's name (the wear-slot split moved hats off the armor slot;
/// the body reads stay on `cur_armor`).
fn worn_head(e: &Entity) -> Option<&str> {
    match &e.kind {
        EntityKind::Player(pd) => pd.worn_head.as_ref().map(|a| a.get_name()),
        _ => None,
    }
}

/// Tree canopy beside/under the player, or an interior floor underfoot — shade.
fn shaded(g: &Game, lvl: usize, xt: i32, yt: i32) -> bool {
    if matches!(g.tile_at(lvl, xt, yt).kind, TileKind::Floor { .. }) {
        return true; // built floor = roofed interior
    }
    for dy in -1..=1 {
        for dx in -1..=1 {
            if matches!(
                g.tile_at(lvl, xt + dx, yt + dy).kind,
                TileKind::Tree | TileKind::TreeSpecies { .. } | TileKind::SnowTree
            ) {
                return true;
            }
        }
    }
    false
}

/// Everything the player's body/gear/surroundings contribute on top of the ambient
/// score — gathered from the live world by [`modifiers_for`], applied by the pure
/// [`apply_modifiers`] (which tests drive directly with pinned inputs).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub snow_underfoot: bool,
    pub swimming: bool,
    pub shaded: bool,
    pub straw_hat: bool,
    pub fur_coat: bool,
    pub near_fire: bool,
}

/// The pure mitigation pipeline: ambient score in, felt score out.
pub fn apply_modifiers(ambient: f64, m: &Modifiers) -> f64 {
    let mut s = ambient;
    if m.snow_underfoot {
        s -= SNOW_GROUND_CHILL;
    }
    // water breaks heat outright (and never punishes — cold water is not modeled)
    if s > 0.0 && m.swimming {
        s = 0.0;
    }
    // shade and a straw hat each pull one heat band, never past comfort
    if s > 0.0 && m.shaded {
        s = (s - SHADE_SHIFT).max(0.0);
    }
    if s > 0.0 && m.straw_hat {
        s = (s - HAT_SHIFT).max(0.0);
    }
    // a fur coat pulls two cold bands, never past comfort
    if s < 0.0 && m.fur_coat {
        s = (s + COAT_SHIFT).min(0.0);
    }
    // resting range of a lit campfire overrides cold entirely
    if s < 0.0 && m.near_fire {
        s = 0.0;
    }
    s
}

/// Read the [`Modifiers`] off the live world for a (player) entity. Works during
/// the take-out tick (the entity is passed in, not read from the arena).
pub fn modifiers_for(g: &Game, e: &Entity) -> Modifiers {
    let Some(lvl) = e.c.level else {
        return Modifiers::default();
    };
    let (xt, yt) = (e.c.x >> 4, e.c.y >> 4);
    let on_surface = g
        .levels
        .get(lvl)
        .and_then(|l| l.as_ref())
        .is_some_and(|l| l.depth == 0 && l.is_infinite());
    Modifiers {
        // snow chill is a surface phenomenon (mine floors are never snow anyway)
        snow_underfoot: on_surface && matches!(g.tile_at(lvl, xt, yt).kind, TileKind::Snow),
        swimming: crate::entity::behavior::is_swimming(g, e),
        shaded: shaded(g, lvl, xt, yt),
        straw_hat: worn_head(e) == Some("Straw Hat") || worn(e) == Some("Straw Hat"),
        fur_coat: worn(e) == Some("Fur Coat"),
        near_fire: crate::entity::furniture::campfire_behavior::near_lit_campfire(g, e),
    }
}

/// The ambient (pre-mitigation) score at the entity's position: the surface model,
/// cave-cool underground, temperate on the finite set-piece layers.
pub fn ambient_for(g: &Game, e: &Entity) -> f64 {
    let Some(lvl) = e.c.level else { return 0.0 };
    let Some(level) = g.levels.get(lvl).and_then(|l| l.as_ref()) else {
        return 0.0;
    };
    if level.depth < 0 {
        MINE_SCORE // mines and dungeon: constant cave-cool
    } else if level.depth == 0 && level.is_infinite() {
        let (xt, yt) = (e.c.x >> 4, e.c.y >> 4);
        ambient_score(g.world_seed, xt, yt, g.tick_count, precip_at(g, xt, yt))
    } else {
        0.0 // sky set-piece / classic finite surfaces: temperate
    }
}

/// The full felt temperature for a (player) entity: ambient + ground + gear + shade
/// + water + fire. Works during the take-out tick (the entity is passed in).
pub fn score_for(g: &Game, e: &Entity) -> f64 {
    apply_modifiers(ambient_for(g, e), &modifiers_for(g, e))
}

/// [`score_for`] banded.
pub fn band_for(g: &Game, e: &Entity) -> Band {
    Band::from_score(score_for(g, e))
}
