//! Software lighting/atmosphere post-process — original post-port work, no Java
//! counterpart. (The Java fork's disabled `Screen::overlay` darkness pass is preserved
//! untouched; this module supersedes it visually without reusing it.)
//!
//! Runs at the end of `Renderer::render_level`, after the world and entities are drawn
//! but *before* the HUD and menus, so UI text stays crisp. Four stages:
//!
//! 0. **Biome ground tint** — on the infinite surface layer, each visible tile's pixels
//!    are multiplied by a mild per-biome factor (`biome_at_blended`, domain-warped so
//!    boundaries are patchy): Minecraft-style ground palette shifts per biome.
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

use crate::core::game::Game;
use crate::core::updater::DAY_LENGTH;
use crate::core::weather::{self, Precip};
use crate::entity::EntityKind;
use crate::entity::furniture::crafter::CrafterType;
use crate::gfx::screen::{self, Screen};
use crate::item::ItemKind;

/// Number of light bands above ambient (band 0 = pure ambient, band `BANDS` = full
/// warm light). Few enough that the dithered steps read as deliberate pixel-art.
const BANDS: usize = 10;

/// Bayer 4x4 ordered-dither thresholds (0-15), same matrix as `Screen::DITHER`.
const BAYER: [i32; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];

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

/// The whole pipeline: grade + radiance + event sky. `screen` is the freshly drawn
/// world frame; `light` is the scratch light buffer (raw 0-255 brightness).
pub fn render_pass(
    screen: &mut Screen,
    light: &mut Screen,
    g: &Game,
    lvl: usize,
    x_scroll: i32,
    y_scroll: i32,
) {
    // Per-biome ground tint first: it shifts the *base* image the grade then operates
    // on, and must apply even at neutral noon (it is how biomes read at midday).
    if lvl == 3 && g.levels[lvl].as_ref().is_some_and(|l| l.is_infinite()) {
        biome_tint_pass(screen, g, x_scroll, y_scroll);
    }

    // Weather is surface-only presentation, and event skies own the night: an Ember
    // Rain's warm glow and falling embers would clash with cool rain streaks, and an
    // aurora shimmers over a *clear* sky by definition. The schedule itself coexists
    // (weather::is_raining stays live); only the visuals yield.
    let aurora = crate::core::events::aurora_active(g) && lvl >= 3;
    let precip = if lvl == 3 && !aurora && !crate::core::events::ember_rain_active(g) {
        weather::precip(g)
    } else {
        Precip::None
    };

    let amb = weather_grade(ambient_for(g, lvl), precip);

    if !amb.is_identity() || aurora {
        let a8 = ((amb.brightness * 255.0).round() as i32).clamp(0, 254);
        // Near full daylight the light term can never exceed ambient — skip the stamp.
        let stamp = a8 < 240;
        if stamp {
            stamp_emitters(light, g, lvl, x_scroll, y_scroll);
        }

        let luts = build_luts(&amb);
        compose(screen, light, &luts, a8, stamp, x_scroll, y_scroll);

        if aurora {
            aurora_bands(screen, g, x_scroll);
        }
    }

    // Ambience on top of the graded frame (surface only): fish bubbles on open water,
    // then whatever falls from the sky.
    if lvl == 3 {
        fish_bubbles(screen, g, lvl, x_scroll, y_scroll, amb.brightness);
        match precip {
            Precip::Rain(i) => rain_streaks(screen, g, i, amb.brightness, x_scroll, y_scroll),
            Precip::Snow(i) => snow_flecks(screen, g, i, amb.brightness, x_scroll, y_scroll),
            Precip::None => {}
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

/// Per-biome ground tint factors (8.8 fixed point, 256 = 1.0), Minecraft-style: a
/// mild palette shift so biome transitions read on the ground itself, not just the
/// flora. Deliberately subtle (~0.86-1.08). `None` = neutral, skip the multiply.
fn biome_tint(b: crate::level::infinite_gen::Biome) -> Option<[i32; 3]> {
    use crate::level::infinite_gen::Biome;
    let f =
        |r: f32, g: f32, b: f32| Some([(r * 256.0) as i32, (g * 256.0) as i32, (b * 256.0) as i32]);
    match b {
        Biome::Forest => f(0.88, 1.00, 1.02),    // deeper, cooler green
        Biome::Savanna => f(1.08, 1.02, 0.86),   // warmer, yellower
        Biome::Marsh => f(0.90, 0.94, 0.86),     // darker sage
        Biome::Tundra => f(0.94, 0.98, 1.08),    // cooler
        Biome::Desert => f(1.06, 1.00, 0.90),    // warmer sand
        Biome::Mountains => f(0.97, 0.98, 1.03), // faintly cool stone
        // plains stay the neutral reference; water/beach keep their painted colors
        Biome::Plains | Biome::Beach | Biome::Ocean | Biome::DeepOcean => None,
    }
}

/// Multiply each visible tile's 16x16 pixel block by its biome tint. One
/// `biome_at_blended` lookup per tile (~250/frame), not per pixel; the domain warp in
/// that lookup is what makes boundaries patchy rather than ruled lines.
fn biome_tint_pass(screen: &mut Screen, g: &Game, x_scroll: i32, y_scroll: i32) {
    let seed = g.world_seed;
    for ty in (y_scroll >> 4)..=((y_scroll + screen::H - 1) >> 4) {
        let y0 = (ty * 16 - y_scroll).max(0);
        let y1 = (ty * 16 + 16 - y_scroll).min(screen::H);
        for tx in (x_scroll >> 4)..=((x_scroll + screen::W - 1) >> 4) {
            let biome = crate::level::infinite_gen::biome_at_blended(seed, tx, ty);
            let Some([fr, fg, fb]) = biome_tint(biome) else {
                continue;
            };
            let x0 = (tx * 16 - x_scroll).max(0);
            let x1 = (tx * 16 + 16 - x_scroll).min(screen::W);
            for y in y0..y1 {
                let row = (y * screen::W) as usize;
                for p in screen.pixels[row + x0 as usize..row + x1 as usize].iter_mut() {
                    let r = ((((*p >> 16) & 0xFF) * fr) >> 8).min(255);
                    let g2 = ((((*p >> 8) & 0xFF) * fg) >> 8).min(255);
                    let b = (((*p & 0xFF) * fb) >> 8).min(255);
                    *p = (r << 16) | (g2 << 8) | b;
                }
            }
        }
    }
}

/// Build the per-frame grading tables: for each light band, a 256-entry map per
/// channel. Band 0 is the pure ambient grade; higher bands ramp brightness toward
/// full while the tint converges on [`WARM_TINT`] *faster* than the brightness
/// (sqrt bias) — so even the dim outer falloff already reads as firelight, not as a
/// neutral "hole" in the darkness that lets the terrain's own hue glow green.
fn build_luts(amb: &Ambient) -> Vec<[[u8; 256]; 3]> {
    let mut luts = vec![[[0u8; 256]; 3]; BANDS + 1];
    for (k, lut) in luts.iter_mut().enumerate() {
        let w = k as f32 / BANDS as f32;
        let ws = w * w * (3.0 - 2.0 * w); // smoothstep: soft shoulder at both ends
        let wt = ws.powf(0.4); // tint leads brightness: even dim falloff reads warm
        let brightness = amb.brightness + (1.0 - amb.brightness) * ws;
        for c in 0..3 {
            let tint = amb.tint[c] + (WARM_TINT[c] - amb.tint[c]) * wt;
            let gain_fp = (tint * brightness * 256.0).round() as i32;
            // the atmosphere wash fades out where real light takes over
            let wash = (amb.wash[c] * (1.0 - ws)).round() as i32;
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

/// Stamp one emitter's radial falloff (the `Screen::render_light` curve) into the
/// light buffer, masked per tile by the emitter's line-of-sight when it has one.
fn stamp_falloff(
    light: &mut Screen,
    cx: i32,
    cy: i32,
    r: i32,
    occ: Option<&Occlusion>,
    x_scroll: i32,
    y_scroll: i32,
) {
    let x = cx - x_scroll;
    let y = cy - y_scroll;
    let x0 = (x - r).max(0);
    let x1 = (x + r).min(screen::W);
    let y0 = (y - r).max(0);
    let y1 = (y + r).min(screen::H);
    let rr = r * r;
    for yy in y0..y1 {
        let ty = (yy + y_scroll) >> 4;
        let yd = (yy - y) * (yy - y);
        let row = (yy * screen::W) as usize;
        for xx in x0..x1 {
            if let Some(o) = occ {
                if !o.visible((xx + x_scroll) >> 4, ty) {
                    continue;
                }
            }
            let xd = xx - x;
            let dist = xd * xd + yd;
            if dist <= rr {
                let br = 255 - dist * 255 / rr;
                let px = &mut light.pixels[row + xx as usize];
                if *px < br {
                    *px = br;
                }
            }
        }
    }
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

    // (level-pixel center x, y, pixel radius) per emitter this frame.
    let mut emitters: Vec<(i32, i32, i32)> = Vec::new();

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
        match &e.kind {
            EntityKind::Player(_) => {
                let mut holds_light = false;
                if let Some(item) = &e.player().active_item {
                    let held = item.get_name();
                    if matches!(item.kind, ItemKind::Torch { .. })
                        || held.contains("Torch")
                        || held.contains("Lantern")
                    {
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
            emitters.push((e.c.x - 1, e.c.y - 4, r * PX_PER_RADIUS));
        }
    }

    // Tile emitters: torches, lava, lit pumpkins, broken gravestones... — whatever
    // `dispatch::get_light_radius` reports. `tile_at` handles infinite/chunked levels
    // (negative coordinates included), unlike the legacy `level::render_light` scan.
    for yt in (yo - MARGIN)..=(yo + h + MARGIN) {
        for xt in (xo - MARGIN)..=(xo + w + MARGIN) {
            let tile = g.tile_at(lvl, xt, yt);
            let lr = crate::level::tile::dispatch::get_light_radius(g, &tile, lvl, xt, yt);
            if lr > 0 {
                emitters.push((xt * 16 + 8, yt * 16 + 8, lr * PX_PER_RADIUS));
            }
        }
    }

    for (cx, cy, r) in emitters {
        // Tile reach of the falloff: r px / 16, +1 for the emitter's own off-center
        // position within its tile.
        let rt = (r >> 4) + 1;
        let occ = occlusion_mask(g, lvl, cx >> 4, cy >> 4, rt);
        stamp_falloff(light, cx, cy, r, occ.as_ref(), x_scroll, y_scroll);
    }
}

/// Final per-pixel mix: `grade(pixel) * max(ambient, light)`, with the light term
/// quantized into [`BANDS`] steps and Bayer-dithered so falloff edges read as ordered
/// pixel-art stipple. Dither coordinates are world-anchored (scroll added) so the
/// pattern doesn't crawl against the terrain when the camera moves.
fn compose(
    screen: &mut Screen,
    light: &Screen,
    luts: &[[[u8; 256]; 3]],
    a8: i32,
    stamp: bool,
    x_scroll: i32,
    y_scroll: i32,
) {
    // Fixed-point band scale: excess light 0..(255-a8) -> band*16 (4 fraction bits).
    let inv = ((BANDS as i32 * 16) << 8) / (255 - a8).max(1);

    for y in 0..screen::H {
        let by = (((y + y_scroll) & 3) << 2) as usize;
        let row = (y * screen::W) as usize;
        for x in 0..screen::W {
            let i = row + x as usize;
            let mut band = 0usize;
            if stamp {
                let excess = light.pixels[i] - a8;
                if excess > 0 {
                    let q = (excess * inv) >> 8; // band index in 4.4 fixed point
                    let mut b = (q >> 4) as usize;
                    if b < BANDS && (q & 15) > BAYER[((x + x_scroll) & 3) as usize + by] {
                        b += 1;
                    }
                    band = b.min(BANDS);
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
