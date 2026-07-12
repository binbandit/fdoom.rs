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
//! the player sees/feels samples their surface position — Desert passes only a rare
//! per-slice roll ([`desert_slice_wet`], ~15%), cold country presents the same
//! intensity as snowfall (the smooth climate field below [`COLD_REACH`] — all of
//! Tundra plus the cold fringe of its neighbors), and underground layers render no
//! precipitation at all (the render gate lives in `gfx::lighting::render_pass`; audio
//! is deliberately skipped). Where snow falls it also *settles*: the accumulation /
//! thaw random-tick lives in `level::tile::snowfall` and reads [`snowing_at`].
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

/// Cold-reach gate on the smooth climate field (`infinite_gen::climate_at`):
/// precipitation falls as snow below this. Tundra classifies at `< 0.30`, so the
/// 0.30..0.36 band is the *cold fringe* — Plains/Forest country where snow visits
/// during precipitation slices and settles tile by tile (`level::tile::snowfall`).
/// The field's gradient bound keeps 0.36 a comfortable 20+ tiles from the Savanna
/// gate (0.42), so dynamic snow can never reach sand.
pub const COLD_REACH: f64 = 0.36;

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
/// `Storm` is the severe-tier read: only severe slices contribute, so the same ramp
/// machinery yields a smooth storm envelope (see [`storm_intensity`]).
#[derive(Clone, Copy)]
enum Gate {
    Open,
    Desert,
    Storm,
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

/// The slice's plateau intensity: 0 when dry, otherwise 0.55..1.0 by hash. Severe
/// slices ([`slice_severe`]) floor theirs at [`STORM_PEAK_FLOOR`]: a storm *is*
/// heavy precipitation, so every existing intensity consumer (rain dim, fire
/// dousing, temperature chill) escalates with it for free.
fn gated_peak(seed: i64, day: i32, slice: i32, gate: Gate) -> f32 {
    let (day, slice) = norm_slice(day, slice);
    if !slice_raining(seed, day, slice) {
        return 0.0;
    }
    if matches!(gate, Gate::Desert) && !desert_slice_wet(seed, day, slice) {
        return 0.0;
    }
    let severe = slice_severe(seed, day, slice);
    if matches!(gate, Gate::Storm) && !severe {
        return 0.0;
    }
    let h = infinite_gen::hash(seed, WEATHER_SALT, day, slice);
    let peak = 0.55 + 0.45 * (((h >> 32) & 0xFFFF) as f32 / 65535.0);
    if severe {
        peak.max(STORM_PEAK_FLOOR)
    } else {
        peak
    }
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

/// Does precipitation fall as *snow* at this surface tile? Pure climate read
/// ([`COLD_REACH`]); covers all of Tundra plus the cold fringe of its neighbors.
pub fn snow_climate(seed: i64, x: i32, y: i32) -> bool {
    infinite_gen::climate_at(seed, x, y) < COLD_REACH
}

/// Is snow falling on surface tile (x, y) right now? The schedule intensity crossed
/// with the cold-reach climate gate — the driver for `level::tile::snowfall`'s
/// settle rolls. (No desert gate: cold-reach tiles can never classify Desert.)
pub fn snowing_at(g: &Game, x: i32, y: i32) -> bool {
    snow_climate(g.world_seed, x, y)
        && schedule_intensity(g.world_seed, g.events.day_number, g.tick_count) >= CUE_THRESHOLD
}

/// The player's tile position, when they stand on an infinite surface layer. Classic
/// finite surfaces have no biome/climate fields — generic rain everywhere.
fn player_surface_pos(g: &Game) -> Option<(i32, i32)> {
    let p = g.try_player()?;
    let lvl = p.c.level?;
    let level = g.levels.get(lvl)?.as_ref()?;
    (level.depth == 0 && level.is_infinite()).then_some((p.c.x >> 4, p.c.y >> 4))
}

/// The player's surface biome (see [`player_surface_pos`]).
fn player_biome(g: &Game) -> Option<Biome> {
    player_surface_pos(g).map(|(x, y)| infinite_gen::biome_at(g.world_seed, x, y))
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
    let pos = player_surface_pos(g);
    let biome = pos.map(|(x, y)| infinite_gen::biome_at(g.world_seed, x, y));
    let gate = if matches!(biome, Some(Biome::Desert | Biome::Badlands)) {
        Gate::Desert
    } else {
        Gate::Open
    };
    let i = intensity_gated(g.world_seed, day, tick, gate);
    if i <= 0.0 {
        Precip::None
    } else if pos.is_some_and(|(x, y)| snow_climate(g.world_seed, x, y)) {
        // cold-reach: snow in Tundra proper AND the cold fringe of its neighbors —
        // the same gate `tile::snowfall` uses, so flakes fall exactly where they settle
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

/* ---------------------------------- severe weather ---------------------------------- */
//
// The storm tier: a pure severity escalation on top of the precipitation schedule —
// ~[`STORM_SLICE_PCT`]% of precip slices are *severe*, rolled per slice from their
// own salt. Cold country presents a severe slice as a **blizzard** (whiteout veil,
// an extra cold band, snow settling fast), warm country as a **thunderstorm**
// (torrential streaks, sky flashes, rare telegraphed lightning strikes). Everything
// stays `f(seed, day, slice/tick)`: nothing is saved, and the whole tier coexists
// with the effects API — a thunderstorm still `is_raining` (crops drink, fires
// drown), a blizzard is still `Precip::Snow`.
//
// Approachability floors (deliberate, keep them):
// - The storm threshold (0.5) sits far above the rain cue threshold (0.05), so the
//   plain "Rain patters down..." / "Snow drifts down..." cue always lands first —
//   a storm is announced twice before it peaks.
// - The blizzard veil never becomes a wall: `gfx::ambience::blizzard_veil` keeps
//   the player's ~4-tile surroundings readable (clarity rings), and campfires keep
//   their full warmth override (`core::temperature`) — the fire stays home.
// - Lightning is a spectacle and a forest-fire starter, never a player-killer:
//   strikes are suppressed inside [`STRIKE_PLAYER_FLOOR`] tiles of the player, and
//   they never land inside a town footprint (the POIs don't burn down off-screen).
// - Thunder audio is skipped: the engine ships no sound assets (same call as the
//   rain); the sky flash carries the sensory load.

/// Percent of *rainy* slices that escalate to severe (~15%).
pub const STORM_SLICE_PCT: u64 = 15;

/// Severe slices floor their schedule peak here: storms are heavy by definition,
/// which keeps `extinguishes_fire` (> 0.5) live through the whole plateau — a
/// thunderstorm always fights its own lightning fires.
pub const STORM_PEAK_FLOOR: f32 = 0.85;

/// Storm envelope level above which the storm "is on" (cues, presentation, strikes).
/// Well above [`CUE_THRESHOLD`]: rain always sets in before it escalates.
pub const STORM_THRESHOLD: f32 = 0.5;

/// Hash salt for the severe-tier roll — distinct from every weather/fog/event salt.
const STORM_SALT: u64 = 0x570124;

/// Does slice `slice` of `day` escalate to a severe storm? Pure; severe ⊆ rainy.
pub fn slice_severe(seed: i64, day: i32, slice: i32) -> bool {
    let (day, slice) = norm_slice(day, slice);
    slice_raining(seed, day, slice)
        && infinite_gen::hash(seed, STORM_SALT, day, slice) % 100 < STORM_SLICE_PCT
}

/// The storm envelope (0..1) for a day-clock position — the schedule intensity with
/// non-severe slices contributing zero, ramped by the same smoothstep as the rain.
/// Between two severe slices the storm holds; into a plain rain slice it decays
/// mid-ramp (the boundary midpoint sits below [`STORM_THRESHOLD`]) — a storm always
/// dies down *into* rain, never rain -> clear-sky pop.
pub fn storm_intensity(seed: i64, day: i32, tick: i32) -> f32 {
    intensity_gated(seed, day, tick, Gate::Storm)
}

/// Presented storm severity 0..1: 0 at the [`STORM_THRESHOLD`] crossing, 1 at the
/// severe plateau floor. What the renderer drives densities with.
pub fn severity(storm_i: f32) -> f32 {
    ((storm_i - STORM_THRESHOLD) / (STORM_PEAK_FLOOR - STORM_THRESHOLD)).clamp(0.0, 1.0)
}

/// The storm as presented at the player, severity 0..1. Rides [`precip`], so the
/// desert gate and the cold-reach snow gate apply unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Storm {
    None,
    /// Cold-country severe slice: whiteout veil, extra cold band, fast settle.
    Blizzard(f32),
    /// Warm-country severe slice: torrential rain, sky flashes, lightning.
    Thunderstorm(f32),
}

/// The storm at the player's location right now.
pub fn storm(g: &Game) -> Storm {
    let si = storm_intensity(g.world_seed, g.events.day_number, g.tick_count);
    if si < STORM_THRESHOLD {
        return Storm::None;
    }
    match precip(g) {
        Precip::Snow(_) => Storm::Blizzard(severity(si)),
        Precip::Rain(_) => Storm::Thunderstorm(severity(si)),
        Precip::None => Storm::None,
    }
}

/// Is a blizzard raging over surface tile (x, y) right now? Positional (no arena
/// read — safe during the player's own take-out tick): the storm envelope crossed
/// with the same cold-reach gate as [`snowing_at`]. Drivers: the temperature wave's
/// extra cold band and `tile::snowfall`'s fast-settle factor.
pub fn blizzard_at(g: &Game, x: i32, y: i32) -> bool {
    snow_climate(g.world_seed, x, y)
        && storm_intensity(g.world_seed, g.events.day_number, g.tick_count) >= STORM_THRESHOLD
}

/* --------------------------------- sky flash --------------------------------- */

/// Sky-flash scheduling window in ticks (~9 s): each window rolls one 3-tick flash
/// at a hashed offset with ~70% odds — flashes land every ~8-15 s of storm, sparing
/// by design.
pub const FLASH_WINDOW: i32 = 540;

const FLASH_SALT: u64 = 0xF1A54;

/// White-blue sky-flash strength (0..1) at a day-clock instant. Pure schedule side
/// only — callers gate on the thunderstorm actually being on at the player
/// (`gfx::lighting` does). Three-tick pulse: full, then two decay steps.
pub fn sky_flash(seed: i64, day: i32, tick: i32) -> f32 {
    let day = day + tick.div_euclid(DAY_LENGTH);
    let tick = tick.rem_euclid(DAY_LENGTH);
    let w = tick / FLASH_WINDOW;
    let h = infinite_gen::hash(seed, FLASH_SALT, day, w);
    if h % 100 >= 70 {
        return 0.0;
    }
    let off = ((h >> 16) % (FLASH_WINDOW - 4) as u64) as i32;
    match tick - w * FLASH_WINDOW - off {
        0 => 1.0,
        1 => 0.65,
        2 => 0.35,
        _ => 0.0,
    }
}

/* ------------------------------- lightning strikes ------------------------------- */

/// Strike scheduling slot in ticks: one roll per [`STRIKE_CELL`]² world cell per
/// storm minute. Divides `DAY_LENGTH` exactly (64800 / 3600 = 18 slots/day).
pub const STRIKE_SLOT: i32 = 3600;

/// Strike cell size in tiles. With ~50% of cells rolling a strike per slot, the
/// player's couple-of-screens neighborhood sees roughly one strike per storm minute.
pub const STRIKE_CELL: i32 = 32;

/// Telegraph lead in ticks (~2 s): the target tile shimmers this long before the
/// bolt, always — baked into the schedule (a strike can never fire untelegraphed).
pub const STRIKE_TELEGRAPH: i32 = 120;

/// Strikes never land within this many tiles of the player (suppressed at execution
/// and render — lightning is drama on the horizon line, not a player-killer).
pub const STRIKE_PLAYER_FLOOR: i32 = 8;

/// Town footprints get this much extra clearance beyond `kind_radius`.
const STRIKE_TOWN_PAD: i32 = 2;

const STRIKE_SALT: u64 = 0x11A87;

/// One scheduled lightning strike: target tile and the bolt's day-clock tick.
/// The telegraph shimmer runs `tick - STRIKE_TELEGRAPH .. tick`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Strike {
    pub x: i32,
    pub y: i32,
    pub tick: i32,
}

/// Where a strike is in its life at a day-clock instant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StrikePhase {
    Idle,
    /// Telegraph progress 0..1 (1 = the bolt is imminent).
    Telegraph(f32),
    /// Bolt age in ticks: 0 (the flash) .. 2 (dying glow).
    Bolt(i32),
}

pub fn strike_phase(s: &Strike, tick: i32) -> StrikePhase {
    let d = tick - s.tick;
    if (-STRIKE_TELEGRAPH..0).contains(&d) {
        StrikePhase::Telegraph((STRIKE_TELEGRAPH + d) as f32 / STRIKE_TELEGRAPH as f32)
    } else if (0..3).contains(&d) {
        StrikePhase::Bolt(d)
    } else {
        StrikePhase::Idle
    }
}

/// Is this tile inside (or hugging) a town footprint? Lightning spares the POIs.
fn in_town(seed: i64, x: i32, y: i32) -> bool {
    use crate::level::structures_gen::{self, StructureKind};
    let r = structures_gen::MAX_RADIUS + STRIKE_TOWN_PAD;
    structures_gen::placements_in_rect(seed, x - r, y - r, x + r, y + r)
        .iter()
        .any(|p| {
            matches!(p.kind, StructureKind::Hamlet | StructureKind::Village)
                && (x - p.x).abs().max((y - p.y).abs())
                    <= structures_gen::kind_radius(p.kind) + STRIKE_TOWN_PAD
        })
}

/// The strike (if any) a world cell hosts in a slot. Pure: everything hashes from
/// `(seed, day, slot, cell)` — the two-stage hash is the "(seed, day, slice, k)"
/// stream, with the cell as k. Gates (all pure): the storm envelope is on at the
/// bolt tick, the target sits in warm (rain) country, a desert target needs the
/// slice's desert roll, and town footprints are excluded.
fn cell_strike(seed: i64, day: i32, slot: i32, cx: i32, cy: i32) -> Option<Strike> {
    let h1 = infinite_gen::hash(seed, STRIKE_SALT, day, slot);
    let h = infinite_gen::hash(seed, h1, cx, cy);
    if h % 100 >= 50 {
        return None;
    }
    let x = cx * STRIKE_CELL + ((h >> 8) % STRIKE_CELL as u64) as i32;
    let y = cy * STRIKE_CELL + ((h >> 16) % STRIKE_CELL as u64) as i32;
    let tick = slot * STRIKE_SLOT
        + STRIKE_TELEGRAPH
        + ((h >> 32) % (STRIKE_SLOT - STRIKE_TELEGRAPH - 4) as u64) as i32;
    if storm_intensity(seed, day, tick) < STORM_THRESHOLD {
        return None; // the storm isn't on when this bolt would land
    }
    if snow_climate(seed, x, y) {
        return None; // blizzards don't throw lightning
    }
    if infinite_gen::biome_at(seed, x, y) == Biome::Desert
        && !desert_slice_wet(seed, day, tick / SLICE_LEN)
    {
        return None; // no storm reaches a dry desert slice
    }
    if in_town(seed, x, y) {
        return None;
    }
    Some(Strike { x, y, tick })
}

/// Every strike scheduled for `tick`'s slot whose cell overlaps the tile rect.
/// Pure and cheap (a handful of cells per screen); the renderer draws telegraphs
/// and bolts from this, [`tick`] executes the ignition, tests pin it directly.
pub fn strikes_in_rect(
    seed: i64,
    day: i32,
    tick: i32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> Vec<Strike> {
    let day = day + tick.div_euclid(DAY_LENGTH);
    let tick = tick.rem_euclid(DAY_LENGTH);
    let slot = tick / STRIKE_SLOT;
    let mut out = Vec::new();
    for cy in y0.div_euclid(STRIKE_CELL)..=y1.div_euclid(STRIKE_CELL) {
        for cx in x0.div_euclid(STRIKE_CELL)..=x1.div_euclid(STRIKE_CELL) {
            if let Some(s) = cell_strike(seed, day, slot, cx, cy) {
                if s.x >= x0 && s.x <= x1 && s.y >= y0 && s.y <= y1 {
                    out.push(s);
                }
            }
        }
    }
    out
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
/// `pub(crate)` so `gfx::ambience` can drive the mist-patch texture from the same
/// primitive.
pub(crate) fn lattice_noise(seed: i64, salt: u64, x: i32, y: i32, period: i32) -> f64 {
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

/* ---------------------------------- ambient fog ---------------------------------- */
//
// Three everyday fog moods, all pure `f(seed, day, tick, x, y)` like the rain
// schedule (nothing saved), all presentation-first:
//
// - **Morning mist** (~40% of days): from first light until it burns off
//   mid-morning, scaled by regional moisture — marsh and water-adjacent country
//   densest, desert none.
// - **Afternoon haze** (~15% of days): a gentler warm veil through late afternoon
//   into golden hour — more color wash than obscuring fog (`gfx::lighting` renders
//   it as tint/wash only, no patches).
// - **Regional banks** (~35% of days): very humid ground (marsh hearts, water
//   edges) mists up at dawn even when the world-wide roll failed, and shoreline
//   country grows an evening bank — mood placement, not a mechanic.
//
// Rain suppresses all of it (`1 - schedule_intensity`): rain and fog never stack
// into soup. Densities are hard-capped at [`AMBIENT_FOG_MAX`], well below
// [`WHISPER_FOG_FLOOR`] — the Whisper Fog *event* must still land as special;
// [`fog_density`] is the one read future systems (visibility-based mob behavior)
// should consume.

/// Ceiling on everyday fog density (0..1 scale where 1 = whiteout). Deliberately
/// well below [`WHISPER_FOG_FLOOR`]: ambient fog is mood, never a wall.
pub const AMBIENT_FOG_MAX: f32 = 0.55;

/// Ceiling on the afternoon haze — even gentler than mist.
pub const HAZE_MAX: f32 = 0.30;

/// The density [`fog_density`] reports during a Whisper Fog night in marsh country.
/// The rare event owns the top of the scale.
pub const WHISPER_FOG_FLOOR: f32 = 0.85;

/// Mist density above which the dawn cue fires (and a sensible "is it foggy" edge
/// for future consumers).
pub const FOG_CUE_THRESHOLD: f32 = 0.10;

/// Day-fraction windows `(start, full, hold-until, gone)`: intensity smoothsteps in
/// over `start..full`, holds, and fades over `hold-until..gone`. Day clock: 0.0 =
/// morning, 0.25 = day, 0.5 = evening, 0.75 = night (see `lighting::SURFACE_KEYS`).
const MIST_WINDOW: (f32, f32, f32, f32) = (0.000, 0.040, 0.100, 0.170);
/// Haze rides the run-up to golden hour (which begins at 0.53, amber peak 0.575).
const HAZE_WINDOW: (f32, f32, f32, f32) = (0.420, 0.475, 0.555, 0.605);
/// Coastal banks roll in through the evening and dissolve before deep night.
const BANK_EVE_WINDOW: (f32, f32, f32, f32) = (0.540, 0.600, 0.680, 0.740);

/// Hash salts for the fog streams — distinct from every weather/terrain/event salt.
const MIST_SALT: u64 = 0xF06A3;
const HAZE_SALT: u64 = 0xF06B7;
const BANK_SALT: u64 = 0xF06C1;
const FOG_HUMID_SALT: u64 = 0xF06D5;

/// Does `day` open with morning mist? ~40% of days; day 0 (fresh session) stays
/// clear, same convention as the rain schedule.
pub fn mist_day(seed: i64, day: i32) -> bool {
    day > 0 && infinite_gen::hash(seed, MIST_SALT, day, 0) % 100 < 40
}

/// Does `day` haze over in the late afternoon? ~15% of days.
pub fn haze_day(seed: i64, day: i32) -> bool {
    day > 0 && infinite_gen::hash(seed, HAZE_SALT, day, 0) % 100 < 15
}

/// Do the regional banks form today (humid-ground dawn fog + coastal evening
/// banks)? ~35% of days, independent of the mist roll.
pub fn bank_day(seed: i64, day: i32) -> bool {
    day > 0 && infinite_gen::hash(seed, BANK_SALT, day, 0) % 100 < 35
}

/// Peak strength for a fog day, 0.70..1.0 by hash — some mornings are wisps, some
/// are proper murk.
fn fog_peak(seed: i64, salt: u64, day: i32) -> f32 {
    0.70 + 0.30 * (((infinite_gen::hash(seed, salt, day, 0) >> 32) & 0xFFFF) as f32 / 65535.0)
}

/// Smoothstep envelope over a `(start, full, hold, gone)` day-fraction window.
fn window_env(t: f32, w: (f32, f32, f32, f32)) -> f32 {
    let (a, b, c, d) = w;
    if t <= a || t >= d {
        0.0
    } else if t < b {
        smooth((t - a) / (b - a))
    } else if t <= c {
        1.0
    } else {
        1.0 - smooth((t - c) / (d - c))
    }
}

/// Regional moisture for fog, 0..1: a per-biome humidity base modulated by a smooth
/// ~80-tile humidity field, floored by shoreline proximity ([`shore_factor`]) so
/// water-adjacent ground mists up regardless of biome. Marsh reads ~1, desert
/// interior exactly 0.
pub fn fog_moisture(seed: i64, x: i32, y: i32) -> f32 {
    let base = match infinite_gen::biome_at(seed, x, y) {
        Biome::Marsh => 1.0,
        Biome::Beach => 0.85,
        Biome::Ocean | Biome::DeepOcean => 0.80,
        Biome::Forest => 0.75,
        Biome::Plains => 0.60,
        Biome::Tundra => 0.50,
        Biome::Mountains => 0.45,
        Biome::Savanna => 0.25,
        Biome::Desert | Biome::Badlands => 0.0,
    };
    let n = lattice_noise(seed, FOG_HUMID_SALT, x, y, 80) as f32;
    let m = (base * (0.65 + 0.55 * n)).clamp(0.0, 1.0);
    m.max(0.9 * shore_factor(seed, x, y))
}

/// Shoreline proximity 0..1 from the public land/elevation field: 1 at the
/// water/land line (`land ≈ 0.435`), fading out ~coast-strip wide on both sides.
fn shore_factor(seed: i64, x: i32, y: i32) -> f32 {
    (1.0 - ((infinite_gen::land_at(seed, x, y) - 0.435).abs() / 0.075) as f32).clamp(0.0, 1.0)
}

/// The time-side factors of the mist components this instant, each 0..1 and already
/// rain-suppressed. Split from the spatial side ([`mist_from`]) so the renderer
/// computes these once per frame and only pays the per-tile moisture reads on
/// mornings that actually have fog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MistBases {
    /// World-wide morning mist (mist days): scales with plain moisture.
    pub open: f32,
    /// Humid-ground dawn fog (bank days): only very moist ground catches it.
    pub humid: f32,
    /// Coastal evening bank (bank days): scales with shoreline proximity.
    pub coast: f32,
}

impl MistBases {
    pub const NONE: MistBases = MistBases {
        open: 0.0,
        humid: 0.0,
        coast: 0.0,
    };

    pub fn any(&self) -> bool {
        self.open > 0.0 || self.humid > 0.0 || self.coast > 0.0
    }
}

/// Compute the mist time-envelopes for a day-clock instant. Pure.
pub fn mist_bases(seed: i64, day: i32, tick: i32) -> MistBases {
    let day = day + tick.div_euclid(DAY_LENGTH);
    let tick = tick.rem_euclid(DAY_LENGTH);
    let t = tick as f32 / DAY_LENGTH as f32;
    let dry = 1.0 - schedule_intensity(seed, day, tick);
    let morn = window_env(t, MIST_WINDOW) * dry;
    let eve = window_env(t, BANK_EVE_WINDOW) * dry;
    MistBases {
        open: if mist_day(seed, day) {
            morn * fog_peak(seed, MIST_SALT, day)
        } else {
            0.0
        },
        humid: if bank_day(seed, day) {
            morn * fog_peak(seed, BANK_SALT, day)
        } else {
            0.0
        },
        coast: if bank_day(seed, day) {
            eve * 0.85 * fog_peak(seed, BANK_SALT, day)
        } else {
            0.0
        },
    }
}

/// The spatial side of the mist: cross the time bases with this tile's moisture /
/// shoreline reads. Returns the capped density, 0..=[`AMBIENT_FOG_MAX`].
pub fn mist_from(bases: &MistBases, seed: i64, x: i32, y: i32) -> f32 {
    if !bases.any() {
        return 0.0;
    }
    let m = fog_moisture(seed, x, y);
    let open = bases.open * m;
    // only truly humid ground (marsh hearts, water edges) catches the bank-day dawn
    let humid = bases.humid * ((m - 0.70) / 0.30).clamp(0.0, 1.0);
    let coast = bases.coast * shore_factor(seed, x, y);
    open.max(humid).max(coast).min(1.0) * AMBIENT_FOG_MAX
}

/// Morning-mist / fog-bank density at a surface tile, 0..=[`AMBIENT_FOG_MAX`]. Pure.
pub fn mist_at(seed: i64, day: i32, tick: i32, x: i32, y: i32) -> f32 {
    mist_from(&mist_bases(seed, day, tick), seed, x, y)
}

/// Afternoon-haze density at a surface tile, 0..=[`HAZE_MAX`]. Pure. Softly
/// moisture-shaped (deserts still shimmer with dry heat haze, wet country hazes a
/// little thicker), rain-suppressed like the mist.
pub fn haze_at(seed: i64, day: i32, tick: i32, x: i32, y: i32) -> f32 {
    let day = day + tick.div_euclid(DAY_LENGTH);
    let tick = tick.rem_euclid(DAY_LENGTH);
    if !haze_day(seed, day) {
        return 0.0;
    }
    let t = tick as f32 / DAY_LENGTH as f32;
    let env = window_env(t, HAZE_WINDOW);
    if env <= 0.0 {
        return 0.0;
    }
    let dry = 1.0 - schedule_intensity(seed, day, tick);
    let shape = 0.55 + 0.45 * fog_moisture(seed, x, y);
    (env * fog_peak(seed, HAZE_SALT, day) * shape * dry).min(1.0) * HAZE_MAX
}

/// The two ambient-fog components at one spot and instant — what the renderer
/// consumes (`gfx::lighting::fog_grade` + `gfx::ambience::mist_patches`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FogSample {
    /// Cool obscuring mist, 0..=[`AMBIENT_FOG_MAX`].
    pub mist: f32,
    /// Warm translucent haze, 0..=[`HAZE_MAX`].
    pub haze: f32,
}

impl FogSample {
    pub const NONE: FogSample = FogSample {
        mist: 0.0,
        haze: 0.0,
    };

    pub fn any(&self) -> bool {
        self.mist > 0.005 || self.haze > 0.005
    }
}

/// Pure fog sample at a surface tile and day-clock instant.
pub fn fog_sample(seed: i64, day: i32, tick: i32, x: i32, y: i32) -> FogSample {
    FogSample {
        mist: mist_at(seed, day, tick, x, y),
        haze: haze_at(seed, day, tick, x, y),
    }
}

/// Convenience wrapper on the live clock (renderer-side).
pub fn fog_sample_at(g: &Game, x: i32, y: i32) -> FogSample {
    fog_sample(g.world_seed, g.events.day_number, g.tick_count, x, y)
}

/// **Effects API**: ambient fog density on the surface plane at tile `(x, y)`,
/// 0..1 (1 = whiteout). Everyday fog never exceeds [`AMBIENT_FOG_MAX`]; during a
/// Whisper Fog night the marshes report [`WHISPER_FOG_FLOOR`] — the event owns the
/// top of the scale. Future consumers (visibility-based mob behavior, ranged-aim
/// penalties) should read THIS, not the components.
pub fn fog_density(g: &Game, x: i32, y: i32) -> f32 {
    let s = fog_sample_at(g, x, y);
    let mut d = s.mist.max(s.haze);
    if crate::core::events::whisper_fog_active(g)
        && infinite_gen::biome_at(g.world_seed, x, y) == Biome::Marsh
    {
        d = d.max(WHISPER_FOG_FLOOR);
    }
    d
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
    precip_cue(g, day, t);
    storm_cue(g, day, t);
    strike_tick(g, day, t);
    mist_cue(g, day, t);
}

/// The storm-escalation cue edge: fires when the storm envelope crosses
/// [`STORM_THRESHOLD`] at the player — stateless like [`precip_cue`], and always
/// *after* the plain rain/snow cue (the thresholds are ordered). The blizzard
/// set-in earns the centered warning band (it changes what you should do next:
/// get to fire); the thunderstorm set-in stays on the lingering ambient tier —
/// the sky flashes are their own announcement.
fn storm_cue(g: &mut Game, day: i32, t: i32) {
    let seed = g.world_seed;
    let (was, now) = (
        storm_intensity(seed, day, t - 1) >= STORM_THRESHOLD,
        storm_intensity(seed, day, t) >= STORM_THRESHOLD,
    );
    if was == now {
        return;
    }
    // Kind from what's falling at the player; a desert-blocked slice has no storm
    // to announce (precip None on both edges).
    let snow = match precip_at_clock(g, day, if now { t } else { t - 1 }) {
        Precip::Snow(_) => true,
        Precip::Rain(_) => false,
        Precip::None => return,
    };
    match (now, snow) {
        (true, true) => g.push_warning("The wind turns to knives."),
        (true, false) => g.push_cue("Thunder rolls in over the rain..."),
        (false, true) => g.notify_all("The wind eases; the snow falls soft again."),
        (false, false) => g.notify_all("The thunder moves on."),
    }
}

/// Tile radius around the player inside which scheduled bolts actually land
/// (chunks there are streamed in; the far world's strikes stay virtual).
const STRIKE_ACT: i32 = 40;

/// Execute this instant's lightning bolts near the player: ignite the target when
/// it's flammable and throw the smash/fire particles. Presentation-first like the
/// rest of the weather — with the player underground nothing lands (they couldn't
/// see or fight it), and the [`STRIKE_PLAYER_FLOOR`] suppression keeps bolts off
/// the player's camp. No thunder audio: the engine ships no sound assets.
fn strike_tick(g: &mut Game, day: i32, t: i32) {
    let Some((px, py)) = player_surface_pos(g) else {
        return;
    };
    let seed = g.world_seed;
    if storm_intensity(seed, day, t) < STORM_THRESHOLD {
        return;
    }
    let Some(lvl) = g.try_player().and_then(|p| p.c.level) else {
        return;
    };
    for s in strikes_in_rect(
        seed,
        day,
        t,
        px - STRIKE_ACT,
        py - STRIKE_ACT,
        px + STRIKE_ACT,
        py + STRIKE_ACT,
    ) {
        if s.tick != t.rem_euclid(DAY_LENGTH) {
            continue;
        }
        let (dx, dy) = (s.x - px, s.y - py);
        if dx * dx + dy * dy < STRIKE_PLAYER_FLOOR * STRIKE_PLAYER_FLOOR {
            continue; // spectacle floor: lightning never lands beside you
        }
        crate::level::tile::fire::ignite(g, lvl, s.x, s.y); // no-op on non-flammable
        let (cx, cy) = (s.x * 16 + 8, s.y * 16 + 8);
        let smash = crate::entity::particle::new_smash_particle(cx, cy);
        g.level_mut(lvl).add(smash, lvl);
        let flame = crate::entity::particle::new_fire_particle(cx - 4, cy - 4);
        g.level_mut(lvl).add(flame, lvl);
    }
}

/// The rain-sets-in / rain-clears cue edge (see [`tick`]).
fn precip_cue(g: &mut Game, day: i32, t: i32) {
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
    // Cold-reach flavor: outside Tundra proper, snowfall is a visitor — it settles on
    // (and later thaws off) ground that is normally green, so the cue says so.
    let visiting = snow && player_biome(g) != Some(Biome::Tundra);
    let msg = match (now, snow, visiting) {
        (true, false, _) => "Rain patters down...",
        (true, true, false) => "Snow drifts down...",
        (true, true, true) => "The cold creeps in...",
        (false, false, _) => "The rain clears.",
        (false, true, false) => "The snow eases.",
        (false, true, true) => "The snow begins to thaw.",
    };
    g.notify_all(msg);
}

/// The foggy-dawn cue: fires once as the morning mist thickens past
/// [`FOG_CUE_THRESHOLD`] at the player — same stateless previous-instant edge as the
/// rain cue. Morning only (evening coastal banks stay silent), and the burn-off is
/// silent too: the visual is enough.
fn mist_cue(g: &mut Game, day: i32, t: i32) {
    if t.rem_euclid(DAY_LENGTH) >= DAY_LENGTH / 4 {
        return;
    }
    let Some((x, y)) = player_surface_pos(g) else {
        return; // classic finite surfaces have no moisture fields — no fog there
    };
    let seed = g.world_seed;
    let cur = mist_at(seed, day, t, x, y);
    let prev = mist_at(seed, day, t - 1, x, y);
    if prev < FOG_CUE_THRESHOLD && cur >= FOG_CUE_THRESHOLD {
        g.notify_all("Mist hangs over the low ground.");
    }
}
