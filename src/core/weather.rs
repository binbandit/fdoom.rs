//! Common-ambience weather (rain, tundra snowfall) — original post-port content, no
//! Java counterpart. Layers *under* `core::events`: events are the rare headline acts,
//! weather is the everyday backdrop, and the two coexist (an Ember Rain night merely
//! overrides the rain *visuals* — see `gfx::lighting`).
//!
//! Like the event calendar, the schedule is a **pure** function of the world seed and
//! the session day counter (`g.events.day_number`): nothing is saved. Each day splits
//! into [`SLICES_PER_DAY`] slices; each slice independently rolls rain (one in
//! [`RAIN_SLICE_ODDS`]) with a hash-picked peak strength, and intensity ramps smoothly
//! across slice boundaries (smoothstep into the midpoint of the two adjacent peaks), so
//! rain always fades in and out — never a 0→1 pop.
//!
//! **Biome gating at the player** (presentation): the schedule is world-wide, but what
//! the player sees/feels samples their surface biome — Desert passes only a rare
//! per-slice roll ([`desert_slice_wet`], ~15%), Tundra presents the same intensity as
//! snowfall, and underground layers render no precipitation at all (the render gate
//! lives in `gfx::lighting::render_pass`; audio is deliberately skipped).
//!
//! Consumers:
//! - `gfx::lighting` — rain streaks / snow flecks / cool ambient dim, plus the fish
//!   bubbles drawn from [`fish_presence`].
//! - [`extinguishes_fire`] / [`growth_boost`] / [`fireflies_hidden`] — effects API for
//!   the upcoming fire and mob/crop waves (tile hooks are one-liners on their side).
//! - [`tick`] — the rain-sets-in / rain-clears notification cues, stateless: each tick
//!   compares the pure intensity at the current and previous day-clock positions.

use crate::core::game::Game;
use crate::core::updater::DAY_LENGTH;
use crate::level::infinite_gen::{self, Biome};

/// Weather slices per day (each ~3 real minutes at the classic day length).
pub const SLICES_PER_DAY: i32 = 6;

/// Ticks per weather slice.
pub const SLICE_LEN: i32 = DAY_LENGTH / SLICES_PER_DAY;

/// One slice in this many rains (~20%).
pub const RAIN_SLICE_ODDS: u64 = 5;

/// Intensity threshold for "it is raining" — the cue edge and the boolean queries.
/// Below this the ramp tails are just damp air.
pub const CUE_THRESHOLD: f32 = 0.05;

/// Fish-presence level above which open water hosts visible bubbles
/// (`gfx::lighting::fish_bubbles`) and, later, the fishing wave's hotspots.
pub const FISH_PRESENCE_THRESHOLD: f64 = 0.62;

/// Ramp half-window at each slice boundary: the last/first `RAMP` ticks of a slice
/// ease between the neighboring peaks.
const RAMP: i32 = SLICE_LEN / 8;

/// Hash salt for the rain schedule — distinct from every terrain and event stream.
const WEATHER_SALT: u64 = 0x12A17;

/// Hash salt for the desert "does this rain slice reach the desert" roll.
const DESERT_SALT: u64 = 0xDE5327;

/// Hash salt for the fish-presence field.
const FISH_SALT: u64 = 0xF124;

/// What falls at the player's location, with intensity 0..1. Presentation-gated:
/// Desert mostly blocks rain, Tundra turns it to snow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Precip {
    None,
    Rain(f32),
    Snow(f32),
}

/// Per-slice biome gate for the intensity curve (see [`desert_slice_wet`]).
#[derive(Clone, Copy)]
enum Gate {
    Open,
    Desert,
}

fn norm_slice(day: i32, slice: i32) -> (i32, i32) {
    (
        day + slice.div_euclid(SLICES_PER_DAY),
        slice.rem_euclid(SLICES_PER_DAY),
    )
}

/// Does slice `slice` of `day` rain? Pure. Day 0 (and earlier) is always dry, so a
/// fresh session starts calm — same convention as the event calendar.
pub fn slice_raining(seed: i64, day: i32, slice: i32) -> bool {
    let (day, slice) = norm_slice(day, slice);
    day > 0 && infinite_gen::hash(seed, WEATHER_SALT, day, slice) % RAIN_SLICE_ODDS == 0
}

/// Does this slice's rain reach a desert? A rare (~15%) per-slice roll — the "0.15x"
/// desert multiplier is a gate, not a scaled drizzle: desert storms are rare but real.
pub fn desert_slice_wet(seed: i64, day: i32, slice: i32) -> bool {
    let (day, slice) = norm_slice(day, slice);
    infinite_gen::hash(seed, DESERT_SALT, day, slice) % 100 < 15
}

/// The slice's plateau intensity: 0 when dry, otherwise 0.55..1.0 by hash.
fn gated_peak(seed: i64, day: i32, slice: i32, gate: Gate) -> f32 {
    let (day, slice) = norm_slice(day, slice);
    if !slice_raining(seed, day, slice) {
        return 0.0;
    }
    if matches!(gate, Gate::Desert) && !desert_slice_wet(seed, day, slice) {
        return 0.0;
    }
    let h = infinite_gen::hash(seed, WEATHER_SALT, day, slice);
    0.55 + 0.45 * (((h >> 32) & 0xFFFF) as f32 / 65535.0)
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Continuous intensity for a day-clock position. `tick` outside `0..DAY_LENGTH`
/// carries into the neighboring days, so `(day, tick - 1)` is always well-defined.
fn intensity_gated(seed: i64, day: i32, tick: i32, gate: Gate) -> f32 {
    let day = day + tick.div_euclid(DAY_LENGTH);
    let tick = tick.rem_euclid(DAY_LENGTH);
    let slice = tick / SLICE_LEN;
    let u = tick - slice * SLICE_LEN;
    let cur = gated_peak(seed, day, slice, gate);
    if u < RAMP {
        let edge = 0.5 * (gated_peak(seed, day, slice - 1, gate) + cur);
        edge + (cur - edge) * smooth(u as f32 / RAMP as f32)
    } else if u >= SLICE_LEN - RAMP {
        let edge = 0.5 * (cur + gated_peak(seed, day, slice + 1, gate));
        cur + (edge - cur) * smooth((u - (SLICE_LEN - RAMP)) as f32 / RAMP as f32)
    } else {
        cur
    }
}

/// The world-wide schedule intensity (0..1) — pure `f(seed, day, tick)`, no biome gate.
pub fn schedule_intensity(seed: i64, day: i32, tick: i32) -> f32 {
    intensity_gated(seed, day, tick, Gate::Open)
}

/// The player's surface biome, when they stand on an infinite surface layer. Classic
/// finite surfaces have no biome field — generic rain everywhere.
fn player_biome(g: &Game) -> Option<Biome> {
    let p = g.try_player()?;
    let lvl = p.c.level?;
    let level = g.levels.get(lvl)?.as_ref()?;
    (level.depth == 0 && level.is_infinite())
        .then(|| infinite_gen::biome_at(g.world_seed, p.c.x >> 4, p.c.y >> 4))
}

/// Is the player on a surface (depth 0) layer? Cues are surface-only.
fn player_on_surface(g: &Game) -> bool {
    let surface = || {
        let p = g.try_player()?;
        let lvl = p.c.level?;
        Some(g.levels.get(lvl)?.as_ref()?.depth == 0)
    };
    surface().unwrap_or(false)
}

fn precip_at_clock(g: &Game, day: i32, tick: i32) -> Precip {
    let biome = player_biome(g);
    let gate = if biome == Some(Biome::Desert) {
        Gate::Desert
    } else {
        Gate::Open
    };
    let i = intensity_gated(g.world_seed, day, tick, gate);
    if i <= 0.0 {
        Precip::None
    } else if biome == Some(Biome::Tundra) {
        Precip::Snow(i)
    } else {
        Precip::Rain(i)
    }
}

/// Precipitation as presented at the player's location right now.
pub fn precip(g: &Game) -> Precip {
    precip_at_clock(g, g.events.day_number, g.tick_count)
}

/// Rain intensity at the player, 0 = dry. Tundra snow and desert-blocked slices read
/// as 0 here — snow neither douses fires nor waters crops.
pub fn rain_intensity(g: &Game) -> f32 {
    match precip(g) {
        Precip::Rain(i) => i,
        _ => 0.0,
    }
}

/// Is it raining at the player (above the cue threshold)?
pub fn is_raining(g: &Game) -> bool {
    rain_intensity(g) >= CUE_THRESHOLD
}

/// Heavy rain puts out open flames (fire-wave hook).
pub fn extinguishes_fire(g: &Game) -> bool {
    rain_intensity(g) > 0.5
}

/// Rain doubles crop/berry regrow ticks (tile-side hook is one line per tile).
pub fn growth_boost(g: &Game) -> bool {
    is_raining(g)
}

/// Fireflies (and similar fair-weather ambience) hide from the rain.
pub fn fireflies_hidden(g: &Game) -> bool {
    is_raining(g)
}

/// Deterministic "fish presence" field over the world plane, 0..1 — smooth ~24-tile
/// blobs with finer 7-tile detail. Open-water tiles above
/// [`FISH_PRESENCE_THRESHOLD`] show rising bubbles (`gfx::lighting::fish_bubbles`);
/// the upcoming fishing wave reads the same field for its hotspots.
pub fn fish_presence(seed: i64, x: i32, y: i32) -> f64 {
    0.7 * lattice_noise(seed, FISH_SALT, x, y, 24)
        + 0.3 * lattice_noise(seed, FISH_SALT ^ 0xF00D, x, y, 7)
}

/// Bilinear value noise on a `period`-tile lattice. (A local copy — `infinite_gen`
/// keeps its own private; only `hash`/`unit` are shared crate-wide.)
fn lattice_noise(seed: i64, salt: u64, x: i32, y: i32, period: i32) -> f64 {
    let fx = x.div_euclid(period);
    let fy = y.div_euclid(period);
    let tx = x.rem_euclid(period) as f64 / period as f64;
    let ty = y.rem_euclid(period) as f64 / period as f64;

    let v00 = infinite_gen::unit(infinite_gen::hash(seed, salt, fx, fy));
    let v10 = infinite_gen::unit(infinite_gen::hash(seed, salt, fx + 1, fy));
    let v01 = infinite_gen::unit(infinite_gen::hash(seed, salt, fx, fy + 1));
    let v11 = infinite_gen::unit(infinite_gen::hash(seed, salt, fx + 1, fy + 1));

    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let a = v00 + (v10 - v00) * sx;
    let b = v01 + (v11 - v01) * sx;
    a + (b - a) * sy
}

/// Per-tick weather hook (called from `Game::tick` right after `events::tick`, so the
/// day counter is current). Fires the set-in/clear cues on threshold crossings —
/// **stateless**: the previous intensity is re-derived from the pure schedule at the
/// previous day-clock position, so nothing needs saving and menus can't double-fire
/// (while a menu is open `g.paused` holds and the clock is frozen).
pub fn tick(g: &mut Game) {
    if g.paused {
        return; // day clock frozen (menu open) — no edges can occur
    }
    // Mirror Game::tick's day-cycle divisor: the clock only advanced this tick if
    // game_time hit the divisor (game_time increments *after* this hook, so it still
    // holds the value the set_time gate saw).
    let divisor = match g.settings.get("daycycle").as_str() {
        "Long" => 4,
        "Realtime" => 80,
        _ => 1,
    };
    if g.game_time % divisor != 0 {
        return;
    }
    if !player_on_surface(g) {
        return; // cues are surface-only; underground you hear nothing
    }

    let day = g.events.day_number;
    let t = g.tick_count;
    let cur = precip_at_clock(g, day, t);
    let prev = precip_at_clock(g, day, t - 1);
    let level = |p: Precip| match p {
        Precip::Rain(i) | Precip::Snow(i) => i,
        Precip::None => 0.0,
    };
    let (was, now) = (level(prev) >= CUE_THRESHOLD, level(cur) >= CUE_THRESHOLD);
    if was == now {
        return;
    }
    let snow = matches!(if now { cur } else { prev }, Precip::Snow(_));
    let msg = match (now, snow) {
        (true, false) => "Rain patters down...",
        (true, true) => "Snow drifts down...",
        (false, false) => "The rain clears.",
        (false, true) => "The snow eases.",
    };
    g.notify_all(msg);
}
