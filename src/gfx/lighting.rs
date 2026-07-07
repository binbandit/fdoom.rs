//! Software lighting/atmosphere post-process — original post-port work, no Java
//! counterpart. (The Java fork's disabled `Screen::overlay` darkness pass is preserved
//! untouched; this module supersedes it visually without reusing it.)
//!
//! Runs at the end of `Renderer::render_level`, after the world and entities are drawn
//! but *before* the HUD and menus, so UI text stays crisp. Four stages:
//!
//! 0. **Ground blend** — on the infinite surface layer, each visible tile's pixels are
//!    multiplied by a mild color factor, bilinearly interpolated per pixel between the
//!    tile's four corners (each corner averages the factors of the 4 tiles sharing it):
//!    Minecraft-style smooth ground-color transitions. A tile's factor comes from its
//!    ground family (sand/snow/mud read as themselves wherever they are), falling back
//!    to the per-biome tint (`biome_at_blended`, domain-warped so boundaries are patchy)
//!    for grass/dirt/water/everything else — so a grass→sand boundary grades over ~2
//!    tiles and snow edges soften into a frosty fringe, while the tile art itself stays
//!    pixel-crisp (only the multiply factor is interpolated).
//! 1. **Time-of-day grading** — a per-frame ambient (brightness + RGB tint) derived from
//!    the day clock: rose-gold dawn, neutral day, a two-stage amber→violet sunset over
//!    ~15% of the day, deep-blue night at ~35% brightness. Applied as a per-pixel
//!    channel multiply through per-frame 256-entry lookup tables.
//! 2. **Radiance** — emitters (player, held torch/lantern, torch tiles, lava, lit
//!    pumpkins, lanterns, furnaces/ovens...) stamp radial falloff into the light screen;
//!    the final pixel is `grade(pixel) * max(ambient, light)`, quantized into bands with
//!    Bayer 4x4 ordered dithering on the falloff so light edges read as pixel-art, not
//!    smooth banding. Underground levels get a near-black ambient — real cave darkness —
//!    that only emitters can push back. Stamping is **occlusion-aware** (light &
//!    shelter wave): walls, rock, and closed doors (`dispatch::blocks_light`) shadow
//!    the falloff via a per-emitter tile-grid line-of-sight mask, so a torch in a
//!    stone room lights the room and spills beams through the doorway and windows,
//!    but never glows through the walls. Emitters with no blocker in reach skip the
//!    mask entirely — open terrain costs what it always did.
//! 3. **Event skies** — Aurora nights (`core::events`) drift slow green/teal bands over
//!    the scene, additively and subtly.
//!
//! The visual-excellence wave adds, all quantized/dithered pixel-art (never smooth
//! alpha): a **seam color-carry** inside the ground blend (ground families speckle
//! their base color across family borders, so snow bleeds white stipple into grass
//! instead of ending in a hard connector edge); **night emitter halos** (one extra
//! half-band dither ring beyond strong emitters — bloom without blur); **torch
//! breathing** (flame emitters swell/settle their radius on a slow per-emitter
//! phase; lanterns stay steady); and **mine depth fog** (underground, darkness
//! deepens in two hash-dithered bands beyond each emitter's reach instead of a flat
//! ambient floor). Scene-space cousins (golden-hour long shadows, contact shadows,
//! water glitter, heat shimmer, drifting motes) live in `gfx::ambience` and are
//! driven from [`render_pass`].

use std::sync::atomic::{AtomicU32, Ordering};

use crate::core::game::Game;
use crate::core::updater::DAY_LENGTH;
use crate::core::weather::{self, Precip};
use crate::entity::EntityKind;
use crate::entity::furniture::crafter::CrafterType;
use crate::gfx::ambience;
use crate::gfx::screen::{self, Screen};
use crate::item::ItemKind;
use crate::level::tile::TileKind;

/// Number of light bands above ambient (band 0 = pure ambient, band `BANDS` = full
/// warm light). Few enough that the dithered steps read as deliberate pixel-art.
const BANDS: usize = 10;

/// Bayer 4x4 ordered-dither thresholds (0-15), same matrix as `Screen::DITHER`.
pub(crate) const BAYER: [i32; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];

/* ------------------------- effect toggles (test support) ------------------------- */

pub const FX_SEAM_BLEND: u32 = 1 << 0;
pub const FX_LONG_SHADOWS: u32 = 1 << 1;
pub const FX_CONTACT_SHADOWS: u32 = 1 << 2;
pub const FX_EMITTER_HALO: u32 = 1 << 3;
pub const FX_WATER_GLITTER: u32 = 1 << 4;
pub const FX_HEAT_SHIMMER: u32 = 1 << 5;
pub const FX_MOTES: u32 = 1 << 6;
pub const FX_TORCH_BREATHING: u32 = 1 << 7;
pub const FX_DEPTH_FOG: u32 = 1 << 8;

/// Bitmask of effects currently disabled. The game never touches this — it exists so
/// `tests/visuals.rs` can render true A/B pairs of the same frame.
static DISABLED_FX: AtomicU32 = AtomicU32::new(0);

/// Disable a set of effects (`FX_*` bitmask; 0 = everything on). Test support only.
pub fn set_disabled_fx(mask: u32) {
    DISABLED_FX.store(mask, Ordering::Relaxed);
}

/// Whether an effect (one `FX_*` bit) is enabled this frame.
pub fn fx_on(bit: u32) -> bool {
    DISABLED_FX.load(Ordering::Relaxed) & bit == 0
}

/// The tint a fully-lit pixel converges to: warm torchlight — extra red, eased green,
/// pulled-back blue. (Tint only; band brightness ramps separately in `build_luts`.)
const WARM_TINT: [f32; 3] = [1.24, 0.78, 0.48];

/// Pixel radius per tile-unit of light radius (`get_light_radius` speaks in tiles).
const PX_PER_RADIUS: i32 = 8;

/// Per-frame ambient: a floor brightness (0-1) for the light `max()`, and per-channel
/// gains (1.0 = untouched) that already include that brightness.
#[derive(Debug, Clone, Copy)]
pub struct Ambient {
    pub brightness: f32,
    pub tint: [f32; 3],
    pub gain: [f32; 3],
    /// Additive atmosphere wash (0-255 per channel): the luminous haze of dawn rose
    /// and sunset amber that a pure multiply can't produce over dark/green terrain.
    pub wash: [f32; 3],
}

impl Ambient {
    fn from_tint(brightness: f32, tint: [f32; 3]) -> Ambient {
        Ambient {
            brightness,
            tint,
            gain: [
                brightness * tint[0],
                brightness * tint[1],
                brightness * tint[2],
            ],
            wash: [0.0; 3],
        }
    }

    /// Whether grading through this ambient would leave every pixel unchanged.
    fn is_identity(&self) -> bool {
        self.gain.iter().all(|g| (g - 1.0).abs() < 1.0 / 255.0)
            && self.wash.iter().all(|w| *w < 0.5)
    }
}

/// Surface day-cycle keyframes: `(day_fraction, brightness, [r, g, b] tint,
/// [r, g, b] additive wash)`. The day clock: 0.0 = morning, 0.25 = day, 0.5 = evening,
/// 0.75 = night. First and last entries match so the midnight wrap is seamless.
#[rustfmt::skip]
const SURFACE_KEYS: &[(f32, f32, [f32; 3], [f32; 3])] = &[
    (0.000, 0.34, [0.60, 0.66, 1.14], [0.0, 0.0, 0.0]),   // pre-dawn deep blue
    (0.035, 0.46, [1.02, 0.70, 0.86], [12.0, 2.0, 8.0]),  // first blush of rose
    (0.085, 0.72, [1.16, 0.82, 0.72], [12.0, 4.0, 5.0]),  // rose-gold dawn peak
    (0.160, 0.93, [1.04, 0.97, 0.90], [3.0, 1.0, 0.0]),   // warm morning
    (0.250, 1.00, [1.00, 1.00, 1.00], [0.0, 0.0, 0.0]),   // full day
    (0.480, 1.00, [1.00, 1.00, 1.00], [0.0, 0.0, 0.0]),   // late afternoon, still neutral
    (0.530, 0.97, [1.09, 0.96, 0.80], [8.0, 3.0, 0.0]),   // golden hour begins
    (0.575, 0.90, [1.22, 0.80, 0.50], [20.0, 7.0, 0.0]),  // sunset stage 1: amber blaze
    (0.615, 0.68, [1.02, 0.60, 0.90], [14.0, 3.0, 10.0]), // sunset stage 2: rose-violet
    (0.650, 0.52, [0.80, 0.58, 1.08], [5.0, 1.0, 9.0]),   // violet dusk
    (0.700, 0.41, [0.64, 0.65, 1.12], [0.0, 0.0, 4.0]),   // blue hour
    (0.750, 0.35, [0.56, 0.64, 1.18], [0.0, 0.0, 0.0]),   // night falls: deep blue @ ~35%
    (0.960, 0.34, [0.56, 0.64, 1.18], [0.0, 0.0, 0.0]),   // deep night hold
    (1.000, 0.34, [0.60, 0.66, 1.14], [0.0, 0.0, 0.0]),   // wraps to 0.000
];

/// Continuous surface ambient for a day-clock position (`tick_count` in
/// `0..DAY_LENGTH`). Piecewise-linear over [`SURFACE_KEYS`] — smooth, no pops.
pub fn surface_ambient(tick_count: i32) -> Ambient {
    let t = (tick_count.rem_euclid(DAY_LENGTH)) as f32 / DAY_LENGTH as f32;
    let mut prev = SURFACE_KEYS[0];
    for &key in &SURFACE_KEYS[1..] {
        if t <= key.0 {
            let span = (key.0 - prev.0).max(1e-6);
            let w = (t - prev.0) / span;
            let lerp = |a: f32, b: f32| a + (b - a) * w;
            let mut a = Ambient::from_tint(
                lerp(prev.1, key.1),
                [
                    lerp(prev.2[0], key.2[0]),
                    lerp(prev.2[1], key.2[1]),
                    lerp(prev.2[2], key.2[2]),
                ],
            );
            a.wash = [
                lerp(prev.3[0], key.3[0]),
                lerp(prev.3[1], key.3[1]),
                lerp(prev.3[2], key.3[2]),
            ];
            return a;
        }
        prev = key;
    }
    let mut a = Ambient::from_tint(prev.1, prev.2);
    a.wash = prev.3;
    a
}

/// The frame's ambient for a level. Level layout (see `Screen::overlay` provenance):
/// 0-2 = caves, 3 = surface, 4 = sky, 5 = dungeon.
pub fn ambient_for(g: &Game, lvl: usize) -> Ambient {
    match lvl {
        0..=2 => Ambient::from_tint(0.06, [0.80, 0.90, 1.15]), // cave darkness, cool
        5 => Ambient::from_tint(0.09, [1.00, 0.80, 1.08]),     // dungeon: faint violet
        4 => {
            // sky level: same cycle, but nights stay moonlit above the clouds
            let a = surface_ambient(g.tick_count);
            if a.brightness < 0.48 {
                Ambient::from_tint(0.48, a.tint)
            } else {
                a
            }
        }
        _ => surface_ambient(g.tick_count),
    }
}

/// Per-frame lighting context, shared between [`render_pass`] and [`stamp_emitters`]
/// so the halo/fog math sees the exact ambient the compose uses.
struct FrameCtx {
    amb: Ambient,
    precip: Precip,
    aurora: bool,
    ember: bool,
}

/// The frame's graded ambient + which sky show (weather/event) owns the visuals.
/// Weather is surface-only presentation, and event skies own the night: an Ember
/// Rain's warm glow and falling embers would clash with cool rain streaks, and an
/// aurora shimmers over a *clear* sky by definition. The schedule itself coexists
/// (weather::is_raining stays live); only the visuals yield.
fn frame_ctx(g: &Game, lvl: usize) -> FrameCtx {
    let aurora = crate::core::events::aurora_active(g) && lvl >= 3;
    let ember = crate::core::events::ember_rain_active(g);
    let precip = if lvl == 3 && !aurora && !ember {
        weather::precip(g)
    } else {
        Precip::None
    };
    FrameCtx {
        amb: weather_grade(ambient_for(g, lvl), precip),
        precip,
        aurora,
        ember,
    }
}

/// Underground layers (caves + dungeon) get the depth-fog treatment.
fn is_underground(lvl: usize) -> bool {
    matches!(lvl, 0..=2 | 5)
}

/// The whole pipeline: grade + radiance + event sky + scene ambience. `screen` is the
/// freshly drawn world frame; `light` is the scratch light buffer (raw 0-255
/// brightness).
pub fn render_pass(
    screen: &mut Screen,
    light: &mut Screen,
    g: &Game,
    lvl: usize,
    x_scroll: i32,
    y_scroll: i32,
) {
    // Ground blend first: it shifts the *base* image the grade then operates on, and
    // must apply even at neutral noon (it is how biomes read at midday).
    if lvl == 3 && g.levels[lvl].as_ref().is_some_and(|l| l.is_infinite()) {
        ground_blend_pass(screen, g, lvl, x_scroll, y_scroll);
    }

    let ctx = frame_ctx(g, lvl);
    let amb = ctx.amb;

    // Golden-hour long shadows: a low clear sun stretches soft dithered strips off
    // trees and walls. Before the grade, so the amber/rose tint sits on top of them.
    if lvl == 3 && ctx.precip == Precip::None && fx_on(FX_LONG_SHADOWS) {
        if let Some((dir, q)) = ambience::golden_hour(g.tick_count) {
            ambience::long_shadows(screen, g, lvl, x_scroll, y_scroll, dir, q);
        }
    }

    if !amb.is_identity() || ctx.aurora {
        let a8 = ((amb.brightness * 255.0).round() as i32).clamp(0, 254);
        // Near full daylight the light term can never exceed ambient — skip the stamp.
        let stamp = a8 < 240;
        if stamp {
            stamp_emitters(light, g, lvl, x_scroll, y_scroll);
        }

        let fog = stamp && is_underground(lvl) && fx_on(FX_DEPTH_FOG);
        let luts = build_luts(&amb, fog);
        compose(screen, light, &luts, a8, stamp, fog, x_scroll, y_scroll);

        if ctx.aurora {
            aurora_bands(screen, g, x_scroll);
        }
    }

    // Heat shimmer wobbles the finished frame (lava rows on any level; desert sand
    // only under a punishing clear noon sun). Runs pre-HUD, so UI rows never bend.
    if fx_on(FX_HEAT_SHIMMER) {
        let desert_noon =
            lvl == 3 && ctx.precip == Precip::None && ambience::high_noon(g.tick_count);
        ambience::heat_shimmer(screen, g, lvl, x_scroll, y_scroll, desert_noon);
    }

    // Ambience on top of the graded frame (surface only): sun/moon glitter and fish
    // bubbles on open water, then whatever falls from the sky (or drifts through it).
    if lvl == 3 {
        if ctx.precip == Precip::None && !ctx.ember && fx_on(FX_WATER_GLITTER) {
            ambience::water_glitter(screen, g, lvl, x_scroll, y_scroll, amb.brightness);
        }
        fish_bubbles(screen, g, lvl, x_scroll, y_scroll, amb.brightness);
        match ctx.precip {
            Precip::Rain(i) => rain_streaks(screen, g, i, amb.brightness, x_scroll, y_scroll),
            Precip::Snow(i) => snow_flecks(screen, g, i, amb.brightness, x_scroll, y_scroll),
            Precip::None => {
                if fx_on(FX_MOTES) {
                    ambience::drift_motes(screen, g, x_scroll, y_scroll, amb.brightness);
                }
            }
        }
    }
}

/// Rain's cool dim, multiplied into the ambient the grade LUTs are built from:
/// darker, cooler, and the dawn/sunset atmosphere wash rained out. Snow dims far less.
fn weather_grade(amb: Ambient, precip: Precip) -> Ambient {
    let d = match precip {
        Precip::Rain(i) => i,
        Precip::Snow(i) => 0.4 * i,
        Precip::None => return amb,
    };
    let mut a = Ambient::from_tint(
        amb.brightness * (1.0 - 0.26 * d),
        [
            amb.tint[0] * (1.0 - 0.10 * d),
            amb.tint[1] * (1.0 - 0.04 * d),
            amb.tint[2] * (1.0 + 0.05 * d),
        ],
    );
    let wash = 1.0 - 0.7 * d;
    a.wash = [amb.wash[0] * wash, amb.wash[1] * wash, amb.wash[2] * wash];
    a
}

/// World-anchored diagonal rain streaks. Streaks live on lanes of constant
/// `d = 3*wx + wy` (falling steeply down-left); activation is Bayer-ordered over the
/// (lane, epoch) cell grid so partial intensity reads as deliberate pixel-art dither,
/// while per-cell hash jitter (phase along the lane, offset within it, 3-4 px length)
/// keeps the full downpour from reading as a ruled grid. The epoch coordinate rides
/// the fall offset, so each drop streaks down its lane and re-rolls next cycle.
fn rain_streaks(
    screen: &mut Screen,
    g: &Game,
    intensity: f32,
    ambient: f32,
    x_scroll: i32,
    y_scroll: i32,
) {
    const RAIN_SALT: u64 = 0xDA0B5;
    const LANE: i32 = 11; // d-units between lanes
    const SEG: i32 = 22; // y-period between drops on a lane

    let fall = g.game_time as i64 * 3; // 3 px/tick
    // scale the additive streak by daylight so night rain doesn't glow
    let lift = 0.35 + 0.65 * ambient.clamp(0.0, 1.0);
    let boost = lift * (0.55 + 0.45 * intensity);
    let (ar, ag, ab) = (
        (52.0 * boost) as i32,
        (64.0 * boost) as i32,
        (86.0 * boost) as i32,
    );
    let coverage = (intensity * 16.0).ceil() as i32; // Bayer level 0..16

    let d0 = 3 * x_scroll + y_scroll;
    let d1 = 3 * (x_scroll + screen::W) + y_scroll + screen::H;
    let e0 = (y_scroll as i64 - fall).div_euclid(SEG as i64) as i32 - 1;
    let e1 = ((y_scroll + screen::H) as i64 - fall).div_euclid(SEG as i64) as i32 + 1;
    for q in (d0.div_euclid(LANE) - 1)..=(d1.div_euclid(LANE) + 1) {
        for e in e0..=e1 {
            if BAYER[((q & 3) + ((e & 3) << 2)) as usize] >= coverage {
                continue;
            }
            let h = crate::level::infinite_gen::hash(g.world_seed, RAIN_SALT, q, e);
            let len = 3 + (h & 1) as i32; // 3-4 px streaks
            let s0 = ((h >> 8) % (SEG - len) as u64) as i32; // phase along the lane
            let dq = q * LANE + ((h >> 24) % (LANE - 2) as u64) as i32; // offset in lane
            let wy0 = (e as i64 * SEG as i64 + fall) as i32 + s0;
            for k in 0..len {
                let wy = wy0 + k;
                let wx = (dq - wy).div_euclid(3);
                screen.add_rgb(wx - x_scroll, wy - y_scroll, ar, ag, ab);
            }
        }
    }
}

/// Tundra snowfall: slow-drifting single white specks on a world-anchored cell grid —
/// same Bayer-ordered activation as the rain, but falling at a third of the speed
/// with a gentle side-to-side sway.
fn snow_flecks(
    screen: &mut Screen,
    g: &Game,
    intensity: f32,
    ambient: f32,
    x_scroll: i32,
    y_scroll: i32,
) {
    const SNOW_SALT: u64 = 0x5A02;
    const CELL: i32 = 13;

    let t = g.game_time as i64;
    let fall = t / 3; // ~0.33 px/tick — lazy drift
    let coverage = (intensity * 16.0).ceil() as i32;
    let lift = 0.45 + 0.55 * ambient.clamp(0.0, 1.0);
    let a = (105.0 * lift) as i32;

    let i0 = x_scroll.div_euclid(CELL) - 1;
    let i1 = (x_scroll + screen::W).div_euclid(CELL) + 1;
    let j0 = (y_scroll as i64 - fall).div_euclid(CELL as i64) as i32 - 1;
    let j1 = ((y_scroll + screen::H) as i64 - fall).div_euclid(CELL as i64) as i32 + 1;
    for j in j0..=j1 {
        for i in i0..=i1 {
            if BAYER[((i & 3) + ((j & 3) << 2)) as usize] >= coverage {
                continue;
            }
            let h = crate::level::infinite_gen::hash(g.world_seed, SNOW_SALT, i, j);
            let sway = [0, 1, 0, -1][((t / 14 + ((h >> 40) & 0xF) as i64) & 3) as usize];
            let wx = i * CELL + ((h >> 8) % CELL as u64) as i32 + sway;
            let wy = (j as i64 * CELL as i64 + fall) as i32 + ((h >> 16) % CELL as u64) as i32;
            let (sx, sy) = (wx - x_scroll, wy - y_scroll);
            screen.add_rgb(sx, sy, a, a, a + 10);
            screen.add_rgb(sx, sy + 1, a / 2, a / 2, a / 2 + 6); // soft tail
        }
    }
}

/// Fish bubbles: open-water tiles whose deterministic fish-presence field
/// (`weather::fish_presence`) runs high host 2-3 tiny bubbles rising through a
/// phase window — the same phase-offset trick as `deep_water_render`'s wave crests,
/// but living in the post pass and marking the future fishing wave's hotspots.
pub fn fish_bubbles(
    screen: &mut Screen,
    g: &Game,
    lvl: usize,
    x_scroll: i32,
    y_scroll: i32,
    ambient: f32,
) {
    const BUBBLE_SALT: u64 = 0xB0BB1E5;
    const CYCLE: i32 = 96; // phase steps per cycle (one step per 2 ticks)
    const WINDOW: i32 = 30; // bubbles visible for this many steps of the cycle

    let seed = g.world_seed;
    let lift = 0.4 + 0.6 * ambient.clamp(0.0, 1.0);
    let a = (46.0 * lift) as i32;
    for ty in (y_scroll >> 4)..=((y_scroll + screen::H - 1) >> 4) {
        for tx in (x_scroll >> 4)..=((x_scroll + screen::W - 1) >> 4) {
            if crate::core::weather::fish_presence(seed, tx, ty)
                <= crate::core::weather::FISH_PRESENCE_THRESHOLD
            {
                continue;
            }
            let name = &g.tile_at(lvl, tx, ty).name;
            if name != "WATER" && name != "DEEP WATER" {
                continue;
            }
            let h = crate::level::infinite_gen::hash(seed, BUBBLE_SALT, tx, ty);
            let phase = (g.tick_count / 2 + (h & 0xFF) as i32).rem_euclid(CYCLE);
            if phase >= WINDOW {
                continue;
            }
            let n = 2 + (h >> 9 & 1) as i32; // 2-3 bubbles
            for k in 0..n {
                let hk = h >> (12 + 10 * k as u64);
                let bx = tx * 16 + 3 + (hk % 10) as i32;
                let start = ((hk >> 4) % 5) as i32 + k * 4; // staggered release
                let by = ty * 16 + 13 - (phase - start).clamp(0, 11);
                screen.add_rgb(bx - x_scroll, by - y_scroll, a, a + 6, a + 12);
            }
        }
    }
}

/// Neutral ground-blend factor (8.8 fixed point, 256 = 1.0): multiply by 1, no shift.
const NEUTRAL_F: [i32; 3] = [256; 3];

/// Sand family: warm — deliberately identical to the Desert biome tint, so desert
/// interiors look exactly as before and beach/oasis sand grades toward the same hue.
const SAND_F: [i32; 3] = [271, 256, 230]; // 1.06, 1.00, 0.90

/// Snow family: bright frost, a touch cooler and lighter than the Tundra grass tint —
/// the small difference is what makes snow edges grade into a frosty fringe.
const SNOW_F: [i32; 3] = [256, 264, 281]; // 1.00, 1.03, 1.10

/// Mud: dark peaty brown, slightly deeper than the Marsh tint around it.
const MUD_F: [i32; 3] = [227, 233, 218]; // 0.89, 0.91, 0.85

/// Per-biome ground tint factors (8.8 fixed point), Minecraft-style: a mild palette
/// shift so biome transitions read on the ground itself, not just the flora.
/// Deliberately subtle (all factors stay within ~0.85-1.10).
fn biome_factor(b: crate::level::infinite_gen::Biome) -> [i32; 3] {
    use crate::level::infinite_gen::Biome;
    let f = |r: f32, g: f32, b: f32| [(r * 256.0) as i32, (g * 256.0) as i32, (b * 256.0) as i32];
    match b {
        Biome::Forest => f(0.88, 1.00, 1.02),    // deeper, cooler green
        Biome::Savanna => f(1.08, 1.02, 0.86),   // warmer, yellower
        Biome::Marsh => f(0.90, 0.94, 0.86),     // darker sage
        Biome::Tundra => f(0.94, 0.98, 1.08),    // cooler
        Biome::Desert => f(1.06, 1.00, 0.90),    // warmer sand
        Biome::Mountains => f(0.97, 0.98, 1.03), // faintly cool stone
        // plains stay the neutral reference; water/beach keep their painted colors
        Biome::Plains | Biome::Beach | Biome::Ocean | Biome::DeepOcean => NEUTRAL_F,
    }
}

/// Ground family of a surface tile — the unit the seam color-carry blends between.
/// A family groups the tiles that share a base ground look (a tree still stands on
/// grass; a cactus on sand), so borders are detected where the *ground* changes, not
/// where props do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroundFam {
    /// Water, rock, floors... — never carries and never receives (their connector
    /// art already handles those seams).
    Other,
    Grass,
    Dirt,
    Sand,
    Snow,
    Mud,
}

/// Representative on-screen base color per ground family (what the tile art reads
/// as at noon). The seam carry lerps border pixels toward the *neighbor's* color.
fn fam_color(f: GroundFam) -> i32 {
    match f {
        GroundFam::Grass => 0x5FA85A, // mid meadow green
        GroundFam::Dirt => 0x9A7A55,  // dry earth tan
        GroundFam::Sand => 0xE3D06A,  // warm dune yellow
        GroundFam::Snow => 0xEFF4F9,  // frost white, faintly blue
        GroundFam::Mud => 0x584A38,   // dark peat
        GroundFam::Other => 0,
    }
}

/// The ground-blend factor *and* family of one surface tile. Hard ground families
/// read as themselves wherever they sit (a lone snow patch is frosty even outside
/// tundra); grass, dirt, water, and everything else take the biome tint — that is
/// where the per-biome hue direction (deep forest green, warm savanna...) comes from.
fn tile_ground(g: &Game, lvl: usize, seed: i64, tx: i32, ty: i32) -> ([i32; 3], GroundFam) {
    let biome = || biome_factor(crate::level::infinite_gen::biome_at_blended(seed, tx, ty));
    match g.tile_at(lvl, tx, ty).kind {
        TileKind::Sand | TileKind::Cactus | TileKind::FruitingCactus | TileKind::QuickSand => {
            (SAND_F, GroundFam::Sand)
        }
        TileKind::Snow | TileKind::SnowTree => (SNOW_F, GroundFam::Snow),
        TileKind::Mud => (MUD_F, GroundFam::Mud),
        TileKind::Dirt | TileKind::Farm => (biome(), GroundFam::Dirt),
        TileKind::Grass
        | TileKind::Flower
        | TileKind::TallGrass { .. }
        | TileKind::Tree
        | TileKind::TreeSpecies { .. }
        | TileKind::Sapling { .. }
        | TileKind::BerryBush
        | TileKind::DryBush => (biome(), GroundFam::Grass),
        _ => (biome(), GroundFam::Other),
    }
}

/// Minecraft-style seamless ground blending: multiply each visible tile's 16x16 pixel
/// block by a color factor bilinearly interpolated between the tile's four corners,
/// where a corner's factor is the average of the 4 tiles sharing it. Ground-color
/// transitions grade over ~2 tiles instead of switching per tile; the tile art itself
/// stays pixel-crisp because only the multiply factor is interpolated.
///
/// Cost: one factor lookup per tile of the visible grid + margin (~315/frame),
/// corner averages once per frame (~280), then integer bilinear interpolation in the
/// pixel loop. Tiles whose four corners agree take a flat-multiply fast path; fully
/// neutral tiles (deep plains interior) skip entirely.
fn ground_blend_pass(screen: &mut Screen, g: &Game, lvl: usize, x_scroll: i32, y_scroll: i32) {
    let seed = g.world_seed;
    let tx0 = x_scroll >> 4;
    let ty0 = y_scroll >> 4;
    // Span math stays in i32 until the end: at negative scroll (west/north of the
    // origin — half the infinite world) casting the floored tile coords to usize
    // first wrapped huge and the subtraction overflowed.
    let nx = (((x_scroll + screen::W - 1) >> 4) - tx0 + 1) as usize;
    let ny = (((y_scroll + screen::H - 1) >> 4) - ty0 + 1) as usize;

    // Per-tile factors + ground families over the visible grid plus a one-tile
    // margin (edge corners average tiles just off screen; edge seams carry from it).
    let tw = nx + 2;
    let mut tf = vec![NEUTRAL_F; tw * (ny + 2)];
    let mut fam = vec![GroundFam::Other; tw * (ny + 2)];
    for j in 0..ny + 2 {
        for i in 0..tw {
            let (f, fm) = tile_ground(g, lvl, seed, tx0 + i as i32 - 1, ty0 + j as i32 - 1);
            tf[j * tw + i] = f;
            fam[j * tw + i] = fm;
        }
    }

    // Corner factors: corner (i, j) sits at world tile corner (tx0 + i, ty0 + j) and
    // averages the 4 tiles around it.
    let cw = nx + 1;
    let mut cf = vec![NEUTRAL_F; cw * (ny + 1)];
    for j in 0..=ny {
        for i in 0..=nx {
            let (a, b, c, d) = (
                tf[j * tw + i],
                tf[j * tw + i + 1],
                tf[(j + 1) * tw + i],
                tf[(j + 1) * tw + i + 1],
            );
            for ch in 0..3 {
                cf[j * cw + i][ch] = (a[ch] + b[ch] + c[ch] + d[ch] + 2) >> 2;
            }
        }
    }

    for tj in 0..ny {
        let ty = ty0 + tj as i32;
        let y0 = (ty * 16 - y_scroll).max(0);
        let y1 = (ty * 16 + 16 - y_scroll).min(screen::H);
        for ti in 0..nx {
            let tx = tx0 + ti as i32;
            let c00 = cf[tj * cw + ti];
            let c10 = cf[tj * cw + ti + 1];
            let c01 = cf[(tj + 1) * cw + ti];
            let c11 = cf[(tj + 1) * cw + ti + 1];
            let flat = c00 == c10 && c00 == c01 && c00 == c11;
            if flat && c00 == NEUTRAL_F {
                continue;
            }
            let x0 = (tx * 16 - x_scroll).max(0);
            let x1 = (tx * 16 + 16 - x_scroll).min(screen::W);
            for y in y0..y1 {
                let row = (y * screen::W) as usize;
                let px = &mut screen.pixels[row + x0 as usize..row + x1 as usize];
                if flat {
                    let [fr, fg, fb] = c00;
                    for p in px.iter_mut() {
                        let r = ((((*p >> 16) & 0xFF) * fr) >> 8).min(255);
                        let g2 = ((((*p >> 8) & 0xFF) * fg) >> 8).min(255);
                        let b = (((*p & 0xFF) * fb) >> 8).min(255);
                        *p = (r << 16) | (g2 << 8) | b;
                    }
                    continue;
                }
                // Bilinear, sampled at pixel centers: wv/wu in 1..32 of 32 (5.5 bits
                // of subtile weight), so the two >>10 shifts return to 8.8 factors.
                let wv = 2 * (y + y_scroll - ty * 16) + 1;
                let mut l = [0i32; 3];
                let mut r = [0i32; 3];
                for ch in 0..3 {
                    l[ch] = c00[ch] * (32 - wv) + c01[ch] * wv;
                    r[ch] = c10[ch] * (32 - wv) + c11[ch] * wv;
                }
                let mut wu = 2 * (x0 + x_scroll - tx * 16) + 1;
                for p in px.iter_mut() {
                    let fr = (l[0] * (32 - wu) + r[0] * wu) >> 10;
                    let fg = (l[1] * (32 - wu) + r[1] * wu) >> 10;
                    let fb = (l[2] * (32 - wu) + r[2] * wu) >> 10;
                    let rr = ((((*p >> 16) & 0xFF) * fr) >> 8).min(255);
                    let g2 = ((((*p >> 8) & 0xFF) * fg) >> 8).min(255);
                    let b = (((*p & 0xFF) * fb) >> 8).min(255);
                    *p = (rr << 16) | (g2 << 8) | b;
                    wu += 2;
                }
            }
        }
    }

    // The multiplier blend above nudges hue; the seam carry below is what actually
    // bridges strongly different grounds (snow-white into grass-green).
    if fx_on(FX_SEAM_BLEND) {
        seam_carry(screen, &fam, tw, nx, ny, tx0, ty0, x_scroll, y_scroll);
    }
}

/// Seam carry depth: how many pixels each side of a family border blends.
const CARRY_DEPTH: i32 = 5;

/// Bayer coverage (of 16) per pixel of depth from the seam: near-solid on the border
/// row, gone 5 px in. Quantized ramp — the dither *is* the gradient.
const CARRY_COV: [i32; CARRY_DEPTH as usize] = [13, 10, 7, 4, 2];

/// How far a carried pixel lerps toward the neighbor family's color (of 256).
/// Strong enough to read as that ground, weak enough to keep the art's shading.
const CARRY_LERP: i32 = 168;

/// Dithered color-carry across ground-family seams: wherever two adjacent tiles
/// belong to different ground families, pixels within [`CARRY_DEPTH`] of the shared
/// edge (on *both* sides — each tile paints its own strip) lerp toward the
/// neighbor's representative base color, masked by ordered Bayer dither whose
/// coverage ramps ~100% -> 0% across the strip. Snow bleeds white speckle into the
/// grass edge and grass speckles green back into the snow — the hard connector
/// border dissolves into pixel-art interpenetration, and isolated one-tile freckles
/// read as soft-edged patches. Corners get both axes (the two strips overlap).
#[allow(clippy::too_many_arguments)]
fn seam_carry(
    screen: &mut Screen,
    fam: &[GroundFam],
    tw: usize,
    nx: usize,
    ny: usize,
    tx0: i32,
    ty0: i32,
    x_scroll: i32,
    y_scroll: i32,
) {
    #[inline]
    fn lerp_px(p: &mut i32, target: i32, k: i32) {
        let (pr, pg, pb) = ((*p >> 16) & 0xFF, (*p >> 8) & 0xFF, *p & 0xFF);
        let (tr, tg, tb) = ((target >> 16) & 0xFF, (target >> 8) & 0xFF, target & 0xFF);
        let r = pr + (((tr - pr) * k) >> 8);
        let g = pg + (((tg - pg) * k) >> 8);
        let b = pb + (((tb - pb) * k) >> 8);
        *p = (r << 16) | (g << 8) | b;
    }

    // One strip: `horiz` = seam runs horizontally (neighbor above/below).
    // `d`-th pixel row/column from the seam, world-anchored Bayer.
    let mut strip = |tx: i32, ty: i32, dx: i32, dy: i32, ncol: i32| {
        for d in 0..CARRY_DEPTH {
            let cov = CARRY_COV[d as usize];
            if dy != 0 {
                // horizontal seam: rows into the tile from its top (dy<0) or bottom
                let wy = if dy < 0 {
                    ty * 16 + d
                } else {
                    ty * 16 + 15 - d
                };
                let y = wy - y_scroll;
                if !(0..screen::H).contains(&y) {
                    continue;
                }
                let x0 = (tx * 16 - x_scroll).max(0);
                let x1 = (tx * 16 + 16 - x_scroll).min(screen::W);
                let row = (y * screen::W) as usize;
                let by = ((wy & 3) << 2) as usize;
                for x in x0..x1 {
                    if BAYER[((x + x_scroll) & 3) as usize + by] < cov {
                        lerp_px(&mut screen.pixels[row + x as usize], ncol, CARRY_LERP);
                    }
                }
            } else {
                // vertical seam: columns into the tile from its left/right edge
                let wx = if dx < 0 {
                    tx * 16 + d
                } else {
                    tx * 16 + 15 - d
                };
                let x = wx - x_scroll;
                if !(0..screen::W).contains(&x) {
                    continue;
                }
                let y0 = (ty * 16 - y_scroll).max(0);
                let y1 = (ty * 16 + 16 - y_scroll).min(screen::H);
                let bx = (wx & 3) as usize;
                for y in y0..y1 {
                    if BAYER[bx + (((y + y_scroll) & 3) << 2) as usize] < cov {
                        lerp_px(
                            &mut screen.pixels[(y * screen::W) as usize + x as usize],
                            ncol,
                            CARRY_LERP,
                        );
                    }
                }
            }
        }
    };

    for tj in 0..ny {
        for ti in 0..nx {
            let f = fam[(tj + 1) * tw + ti + 1];
            if f == GroundFam::Other {
                continue;
            }
            let (tx, ty) = (tx0 + ti as i32, ty0 + tj as i32);
            // (grid dx, grid dy) of the four neighbors; the strip hugs that edge.
            for (dx, dy) in [(0i32, -1i32), (0, 1), (-1, 0), (1, 0)] {
                let n = fam[(tj as i32 + 1 + dy) as usize * tw + (ti as i32 + 1 + dx) as usize];
                if n != GroundFam::Other && n != f {
                    strip(tx, ty, dx, dy, fam_color(n));
                }
            }
        }
    }
}

/// Depth-fog band multipliers on the ambient grade (underground only): the fringe
/// band just beyond an emitter's reach, then the deep dark. Two quantized steps —
/// the hash-dithered edge between them does the "gradient" work.
const FOG_FRINGE: f32 = 0.60;
const FOG_DEEP: f32 = 0.32;

/// Build the per-frame grading tables: for each light band, a 256-entry map per
/// channel. Band 0 is the pure ambient grade; higher bands ramp brightness toward
/// full while the tint converges on [`WARM_TINT`] *faster* than the brightness
/// (sqrt bias) — so even the dim outer falloff already reads as firelight, not as a
/// neutral "hole" in the darkness that lets the terrain's own hue glow green.
///
/// With `fog`, two extra tables follow the light bands: `BANDS+1` = fog fringe,
/// `BANDS+2` = fog deep (the ambient grade scaled by [`FOG_FRINGE`]/[`FOG_DEEP`]).
fn build_luts(amb: &Ambient, fog: bool) -> Vec<[[u8; 256]; 3]> {
    let n = BANDS + 1 + if fog { 2 } else { 0 };
    let mut luts = vec![[[0u8; 256]; 3]; n];
    for (k, lut) in luts.iter_mut().enumerate() {
        let (ws, wt, extra) = if k <= BANDS {
            let w = k as f32 / BANDS as f32;
            let ws = w * w * (3.0 - 2.0 * w); // smoothstep: soft shoulder at both ends
            (ws, ws.powf(0.4), 1.0) // tint leads brightness: dim falloff reads warm
        } else if k == BANDS + 1 {
            (0.0, 0.0, FOG_FRINGE)
        } else {
            (0.0, 0.0, FOG_DEEP)
        };
        let brightness = (amb.brightness + (1.0 - amb.brightness) * ws) * extra;
        for c in 0..3 {
            let tint = amb.tint[c] + (WARM_TINT[c] - amb.tint[c]) * wt;
            let gain_fp = (tint * brightness * 256.0).round() as i32;
            // the atmosphere wash fades out where real light takes over
            let wash = (amb.wash[c] * (1.0 - ws) * extra).round() as i32;
            for (v, out) in lut[c].iter_mut().enumerate() {
                *out = (((v as i32 * gain_fp) >> 8) + wash).min(255) as u8;
            }
        }
    }
    luts
}

/// Per-emitter tile-grid line-of-sight mask: `vis` is a `(2*rt+1)²` grid centered on
/// the emitter's tile, true where the emitter can see the tile.
struct Occlusion {
    vis: Vec<bool>,
    etx: i32,
    ety: i32,
    rt: i32,
}

impl Occlusion {
    fn visible(&self, tx: i32, ty: i32) -> bool {
        let w = 2 * self.rt + 1;
        let dx = tx - self.etx + self.rt;
        let dy = ty - self.ety + self.rt;
        dx >= 0 && dy >= 0 && dx < w && dy < w && self.vis[(dy * w + dx) as usize]
    }
}

/// Whether the center-to-center segment from the origin tile to `(dx, dy)`
/// (mask-local coords) crosses a blocked cell. A supercover grid walk: at each step
/// the segment's next grid-line crossing decides an x-step, a y-step, or (exactly
/// through a corner) a diagonal step. Endpoints are exempt — the emitter's own tile
/// never blocks, and a wall's *face* still catches light.
fn line_clear(blocked: &[bool], rt: i32, dx: i32, dy: i32) -> bool {
    let w = 2 * rt + 1;
    let (sx, sy) = (dx.signum(), dy.signum());
    let (adx, ady) = (dx.abs(), dy.abs());
    let (mut x, mut y) = (0i32, 0i32);
    let (mut ix, mut iy) = (0i32, 0i32); // vertical / horizontal grid lines crossed
    while (x, y) != (dx, dy) {
        // Fractions along the segment of the next crossings: (2ix+1)/(2adx) vs
        // (2iy+1)/(2ady), compared via cross-multiplication (no division).
        let dec = (2 * ix + 1) * ady - (2 * iy + 1) * adx;
        match dec.cmp(&0) {
            std::cmp::Ordering::Equal => {
                x += sx;
                y += sy;
                ix += 1;
                iy += 1;
            }
            std::cmp::Ordering::Less => {
                x += sx;
                ix += 1;
            }
            std::cmp::Ordering::Greater => {
                y += sy;
                iy += 1;
            }
        }
        if (x, y) == (dx, dy) {
            break;
        }
        if blocked[((y + rt) * w + x + rt) as usize] {
            return false;
        }
    }
    true
}

/// The line-of-sight mask around an emitter, or `None` when no tile in reach blocks
/// light — the common open-terrain case, which then stamps mask-free at the exact
/// pre-occlusion cost.
fn occlusion_mask(g: &Game, lvl: usize, etx: i32, ety: i32, rt: i32) -> Option<Occlusion> {
    let w = 2 * rt + 1;
    let mut blocked = vec![false; (w * w) as usize];
    let mut any = false;
    for dy in -rt..=rt {
        for dx in -rt..=rt {
            let (tx, ty) = (etx + dx, ety + dy);
            let def = g.tile_at(lvl, tx, ty);
            if crate::level::tile::dispatch::blocks_light(g, &def, lvl, tx, ty) {
                blocked[((dy + rt) * w + dx + rt) as usize] = true;
                any = true;
            }
        }
    }
    if !any {
        return None;
    }
    let mut vis = vec![false; (w * w) as usize];
    for dy in -rt..=rt {
        for dx in -rt..=rt {
            vis[((dy + rt) * w + dx + rt) as usize] = line_clear(&blocked, rt, dx, dy);
        }
    }
    Some(Occlusion { vis, etx, ety, rt })
}

/// Night-halo ring width beyond the falloff radius, in pixels.
const HALO_W: i32 = 8;

/// Depth-fog reach beyond the falloff radius: how far an emitter pushes the
/// underground fog back before the fringe/deep bands take over.
const FOG_REACH: i32 = 26;

/// Stamp one emitter's radial falloff (the `Screen::render_light` curve) into the
/// light buffer, masked per tile by the emitter's line-of-sight when it has one.
///
/// `halo_v > 0` adds the night bloom ring: everything out to `r + HALO_W` is floored
/// at `halo_v` (half a band above ambient), which the compose dithers into a single
/// warm stipple ring hugging the lit edge. `fog_a8 > 0` (underground) additionally
/// writes a sub-ambient ramp out to `r + FOG_REACH` — values below ambient never
/// brighten anything, they only encode "how close is the nearest light" for the
/// depth-fog bands in [`compose`].
#[allow(clippy::too_many_arguments)]
fn stamp_falloff(
    light: &mut Screen,
    cx: i32,
    cy: i32,
    r: i32,
    halo_v: i32,
    fog_a8: i32,
    occ: Option<&Occlusion>,
    x_scroll: i32,
    y_scroll: i32,
) {
    let rh = if halo_v > 0 { r + HALO_W } else { r };
    let rf = if fog_a8 > 0 { r + FOG_REACH } else { r };
    let reach = rh.max(rf);
    let x = cx - x_scroll;
    let y = cy - y_scroll;
    let x0 = (x - reach).max(0);
    let x1 = (x + reach).min(screen::W);
    let y0 = (y - reach).max(0);
    let y1 = (y + reach).min(screen::H);
    let rr = r * r;
    let rh2 = rh * rh;
    let rf2 = rf * rf;
    for yy in y0..y1 {
        let ty = (yy + y_scroll) >> 4;
        let yd = (yy - y) * (yy - y);
        let row = (yy * screen::W) as usize;
        for xx in x0..x1 {
            let xd = xx - x;
            let dist = xd * xd + yd;
            if dist > rr && dist > rh2 && dist > rf2 {
                continue;
            }
            if let Some(o) = occ {
                if !o.visible((xx + x_scroll) >> 4, ty) {
                    continue;
                }
            }
            let mut br = if dist <= rr { 255 - dist * 255 / rr } else { 0 };
            if fog_a8 > 0 && dist <= rf2 {
                br = br.max((fog_a8 * (rf2 - dist) / (rf2 - rr).max(1)).min(fog_a8));
            }
            if halo_v > 0 && dist > rr && dist <= rh2 {
                br = br.max(halo_v);
            }
            let px = &mut light.pixels[row + xx as usize];
            if *px < br {
                *px = br;
            }
        }
    }
}

/// Slow per-emitter breathing offset for flame light (±1/4 tile in 2 px quantized
/// steps over ~2 s): firelight swells and settles instead of holding a perfect
/// circle. Kept under the menu's ±0.5 tile — side-by-side torches must still read
/// as equals in a still frame. Steady emitters (lanterns, pumpkins, lava) skip it.
fn flame_breath(g: &Game, cx: i32, cy: i32) -> i32 {
    const WAVE: [i32; 8] = [-4, -2, 0, 2, 4, 2, 0, -2];
    let h = crate::level::infinite_gen::hash(g.world_seed, 0xB4EA7, cx, cy);
    WAVE[(((g.game_time / 16) + (h & 7) as i32) & 7) as usize]
}

/// Collect this frame's emitters and stamp radial falloff into the light buffer,
/// shadowed by light-blocking tiles (see [`Occlusion`]). Public for the shelter
/// tests (`tests/light_shelter.rs`); the game only calls it via [`render_pass`].
pub fn stamp_emitters(light: &mut Screen, g: &Game, lvl: usize, x_scroll: i32, y_scroll: i32) {
    light.clear(0);

    let xo = x_scroll >> 4;
    let yo = y_scroll >> 4;
    let w = (screen::W + 15) >> 4;
    let h = (screen::H + 15) >> 4;
    const MARGIN: i32 = 8; // widest emitter (gold lantern, r=15) reaches ~8 tiles

    // Halo/fog context: the same graded ambient the compose will use.
    let amb = frame_ctx(g, lvl).amb;
    let a8 = ((amb.brightness * 255.0).round() as i32).clamp(0, 254);
    // Half a compose band above ambient — dithers to a 50% stipple ring.
    let half_band = (((255 - a8) / (BANDS as i32 * 2)).max(1) + a8).min(254);
    let halo_on = a8 <= 120 && fx_on(FX_EMITTER_HALO);
    let fog_a8 = if is_underground(lvl) && fx_on(FX_DEPTH_FOG) {
        a8
    } else {
        0
    };
    let breathe = fx_on(FX_TORCH_BREATHING);

    // (level-pixel center x, y, pixel radius, is-flame) per emitter this frame.
    let mut emitters: Vec<(i32, i32, i32, bool)> = Vec::new();

    // Entity emitters: lanterns, glow worms, night wisps, the player (bigger radius
    // when holding a torch or lantern), plus furnace/oven ember glow.
    for eid in crate::level::get_entities_in_tiles(
        g,
        lvl,
        xo - MARGIN,
        yo - MARGIN,
        xo + w + MARGIN,
        yo + h + MARGIN,
    ) {
        let Some(e) = g.entities.get(eid) else {
            continue;
        };
        let mut r = crate::entity::behavior::get_light_radius(e);
        let mut flame = matches!(e.kind, EntityKind::Campfire(_));
        match &e.kind {
            EntityKind::Player(_) => {
                let mut holds_light = false;
                if let Some(item) = &e.player().active_item {
                    let held = item.get_name();
                    if matches!(item.kind, ItemKind::Torch { .. }) || held.contains("Torch") {
                        flame = true; // a held torch breathes like a planted one
                    }
                    if flame || held.contains("Lantern") {
                        holds_light = true;
                        r = r.max(8);
                    }
                }
                // The bare-handed self-glow (base radius 5) is a cave affordance; on
                // the surface it reads as an odd halo at dawn/dusk. Keep it only
                // underground, or when actually holding a light (torch item here,
                // held light-furniture via `player_behavior::get_light_radius`).
                if (3..=4).contains(&lvl) && !holds_light && r <= 5 {
                    r = 0;
                }
            }
            EntityKind::Crafter(c) => {
                if matches!(c.crafter_type, CrafterType::Furnace | CrafterType::Oven) {
                    r = r.max(4); // ember glow
                }
            }
            _ => {}
        }
        if r > 0 {
            emitters.push((e.c.x - 1, e.c.y - 4, r * PX_PER_RADIUS, flame));
        }
    }

    // Tile emitters: torches, lava, lit pumpkins, broken gravestones... — whatever
    // `dispatch::get_light_radius` reports. `tile_at` handles infinite/chunked levels
    // (negative coordinates included), unlike the legacy `level::render_light` scan.
    // Torch tiles are flames (they breathe); burning tiles already flicker through
    // `fire::light_radius`, and lava/pumpkins/gravestones hold steady.
    for yt in (yo - MARGIN)..=(yo + h + MARGIN) {
        for xt in (xo - MARGIN)..=(xo + w + MARGIN) {
            let tile = g.tile_at(lvl, xt, yt);
            let lr = crate::level::tile::dispatch::get_light_radius(g, &tile, lvl, xt, yt);
            if lr > 0 {
                let flame = matches!(tile.kind, TileKind::Torch { .. });
                emitters.push((xt * 16 + 8, yt * 16 + 8, lr * PX_PER_RADIUS, flame));
            }
        }
    }

    for (cx, cy, mut r, flame) in emitters {
        if flame && breathe {
            r = (r + flame_breath(g, cx, cy)).max(8);
        }
        // Strong emitters (a torch and up) get the night bloom ring.
        let halo_v = if halo_on && r >= 20 { half_band } else { 0 };
        // Tile reach of the widest write (falloff/halo/fog): px / 16, +1 for the
        // emitter's own off-center position within its tile.
        let reach = r + if fog_a8 > 0 {
            FOG_REACH
        } else if halo_v > 0 {
            HALO_W
        } else {
            0
        };
        let rt = (reach >> 4) + 1;
        let occ = occlusion_mask(g, lvl, cx >> 4, cy >> 4, rt);
        stamp_falloff(
            light,
            cx,
            cy,
            r,
            halo_v,
            fog_a8,
            occ.as_ref(),
            x_scroll,
            y_scroll,
        );
    }
}

/// Cheap integer scramble in 0..16 — an *unordered* (non-Bayer) dither for the depth
/// fog's edge, so the fog boundary reads as organic grain rather than the same
/// crosshatch the light bands use. World-anchored like everything else.
#[inline]
fn fog_noise(x: i32, y: i32) -> i32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B9) ^ (y as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    ((h >> 24) & 15) as i32
}

/// Final per-pixel mix: `grade(pixel) * max(ambient, light)`, with the light term
/// quantized into [`BANDS`] steps and Bayer-dithered so falloff edges read as ordered
/// pixel-art stipple. Dither coordinates are world-anchored (scroll added) so the
/// pattern doesn't crawl against the terrain when the camera moves.
///
/// With `fog` (underground), pixels at band 0 sort into three depths by their
/// sub-ambient light value (the [`FOG_REACH`] ramp [`stamp_falloff`] wrote): near a
/// light = the normal ambient floor, then a fringe band, then deep dark — each edge
/// broken up by [`fog_noise`] grain instead of a hard ring.
#[allow(clippy::too_many_arguments)]
fn compose(
    screen: &mut Screen,
    light: &Screen,
    luts: &[[[u8; 256]; 3]],
    a8: i32,
    stamp: bool,
    fog: bool,
    x_scroll: i32,
    y_scroll: i32,
) {
    // Fixed-point band scale: excess light 0..(255-a8) -> band*16 (4 fraction bits).
    let inv = ((BANDS as i32 * 16) << 8) / (255 - a8).max(1);
    // Fog thresholds on the sub-ambient ramp, in thirds of ambient.
    let (fog_hi, fog_lo) = (a8 * 2 / 3, a8 / 3);

    for y in 0..screen::H {
        let by = (((y + y_scroll) & 3) << 2) as usize;
        let row = (y * screen::W) as usize;
        for x in 0..screen::W {
            let i = row + x as usize;
            let mut band = 0usize;
            if stamp {
                let lv = light.pixels[i];
                let excess = lv - a8;
                if excess > 0 {
                    let q = (excess * inv) >> 8; // band index in 4.4 fixed point
                    let mut b = (q >> 4) as usize;
                    if b < BANDS && (q & 15) > BAYER[((x + x_scroll) & 3) as usize + by] {
                        b += 1;
                    }
                    band = b.min(BANDS);
                } else if fog {
                    // ±a8/8 grain on the two fog edges
                    let j = ((fog_noise(x + x_scroll, y + y_scroll) - 8) * a8) >> 6;
                    let lvj = lv + j;
                    if lvj < fog_lo {
                        band = BANDS + 2; // deep dark
                    } else if lvj < fog_hi {
                        band = BANDS + 1; // fringe
                    }
                }
            }
            let lut = &luts[band];
            let p = screen.pixels[i];
            screen.pixels[i] = ((lut[0][((p >> 16) & 0xFF) as usize] as i32) << 16)
                | ((lut[1][((p >> 8) & 0xFF) as usize] as i32) << 8)
                | lut[2][(p & 0xFF) as usize] as i32;
        }
    }
}

/// Aurora night sky: two interfering sine bands drifting slowly across the world,
/// added as a subtle green/teal wash, stronger toward the top of the screen.
fn aurora_bands(screen: &mut Screen, g: &Game, x_scroll: i32) {
    let t = g.game_time as f32;
    let mut cols = [(0i32, 0i32); screen::W as usize];
    for (x, col) in cols.iter_mut().enumerate() {
        let wx = (x as i32 + x_scroll / 3) as f32;
        let a = ((wx * 0.045 + t * 0.0045).sin()) * 0.5 + 0.5;
        let a = a * a; // sharpen the crests into distinct curtains
        let b = ((wx * 0.013 - t * 0.0017).sin()) * 0.5 + 0.5;
        let i = a * (0.35 + 0.65 * b); // 0..1 drifting interference bands
        *col = ((i * 40.0) as i32, (i * 25.0) as i32); // (green, blue) adds
    }
    for y in 0..screen::H {
        let k = 256 - (y * 150) / screen::H; // fade toward the bottom of the frame
        for (x, &(cg, cb)) in cols.iter().enumerate() {
            let dg = (cg * k) >> 8;
            let db = (cb * k) >> 8;
            if dg | db != 0 {
                screen.add_rgb(x as i32, y, 0, dg, db);
            }
        }
    }
}
