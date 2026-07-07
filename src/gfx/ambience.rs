//! Scene ambience for the visual-excellence wave — original post-port work, no Java
//! counterpart. Companions to `gfx::lighting`'s grade/radiance pass:
//!
//! - **Golden-hour long shadows** — at dawn and dusk a low, clear sun stretches
//!   1-tile dithered shadow strips off trees and light-blocking tiles, west in the
//!   morning and east in the evening.
//! - **Contact shadows** — a 2-px dithered ellipse under every grounded mob (drawn
//!   *before* the sprite pass, from `Renderer::render_level`, so feet sit on it).
//! - **Sun/moon glitter** — a drifting, world-anchored sparkle band on open water:
//!   warm and lively by day, cool and sparse on clear nights.
//! - **Heat shimmer** — 1-px per-row horizontal wobble over lava (always) and over
//!   desert sand at a clear high noon.
//! - **Drifting motes** — a handful of tumbling leaves over forest and bright pollen
//!   specks over plains on clear daylit frames.
//!
//! House rules apply throughout: quantized steps and ordered dither (no smooth
//! alpha), world-anchored patterns (nothing crawls when the camera pans), and
//! everything is gated/driven from `lighting::render_pass` (except the contact
//! shadows, which must run under the sprite pass).

use crate::core::game::Game;
use crate::core::updater::DAY_LENGTH;
use crate::entity::EntityKind;
use crate::gfx::lighting::{BAYER, FX_CONTACT_SHADOWS, fx_on};
use crate::gfx::screen::{self, Screen};
use crate::level::infinite_gen::{self, Biome};
use crate::level::tile::TileKind;

/// Multiplicative darken of one pixel: `keep` of 256 survives.
#[inline]
fn darken_px(p: &mut i32, keep: i32) {
    let r = ((((*p >> 16) & 0xFF) * keep) >> 8) << 16;
    let g = ((((*p >> 8) & 0xFF) * keep) >> 8) << 8;
    let b = ((*p & 0xFF) * keep) >> 8;
    *p = r | g | b;
}

/// Bounds-checked opaque pixel write, scaled by `k256` (ambient brightness in 0-256)
/// so drawn-on-top motes still sit inside the frame's grade.
#[inline]
fn put_px(screen: &mut Screen, x: i32, y: i32, rgb: i32, k256: i32) {
    if !(0..screen::W).contains(&x) || !(0..screen::H).contains(&y) {
        return;
    }
    let r = (((rgb >> 16) & 0xFF) * k256) >> 8;
    let g = (((rgb >> 8) & 0xFF) * k256) >> 8;
    let b = ((rgb & 0xFF) * k256) >> 8;
    screen.pixels[(x + y * screen::W) as usize] = (r << 16) | (g << 8) | b;
}

/* ----------------------------- golden-hour shadows ------------------------------ */

/// Golden-hour sun state: `Some((dir, q))` inside the dawn/dusk windows, where `dir`
/// is the direction shadows extend (-1 = west while the morning sun sits east,
/// +1 = east while the evening sun sinks west) and `q` (1-4) is the quantized
/// strength envelope — length and dither coverage both scale with it, so shadows
/// grow in visible steps instead of fading continuously.
pub fn golden_hour(tick_count: i32) -> Option<(i32, i32)> {
    let t = tick_count.rem_euclid(DAY_LENGTH) as f32 / DAY_LENGTH as f32;
    // (window start, peak, window end, shadow direction) — the windows track the
    // rose-dawn and amber-sunset keyframes in `lighting::SURFACE_KEYS`.
    for (a, p, b, dir) in [(0.030, 0.085, 0.170, -1), (0.515, 0.575, 0.640, 1)] {
        if t > a && t < b {
            let env = if t <= p {
                (t - a) / (p - a)
            } else {
                (b - t) / (b - p)
            };
            return Some((dir, ((env * 4.0).ceil() as i32).clamp(1, 4)));
        }
    }
    None
}

/// Does this tile throw a golden-hour shadow? Trees (which deliberately don't block
/// emitter light) plus everything that does.
fn casts_sun_shadow(g: &Game, lvl: usize, tx: i32, ty: i32) -> bool {
    let def = g.tile_at(lvl, tx, ty);
    match def.kind {
        TileKind::Tree
        | TileKind::TreeSpecies { .. }
        | TileKind::SnowTree
        | TileKind::Cactus
        | TileKind::FruitingCactus => true,
        _ => crate::level::tile::dispatch::blocks_light(g, &def, lvl, tx, ty),
    }
}

/// Stamp the golden-hour shadow strips: each caster tile darkens up to one tile of
/// ground on its shadow side, in two dither steps (denser near the caster, half
/// coverage beyond). Casters never shadow other casters — forest interiors stay
/// clean and only the sun-away edge throws.
pub fn long_shadows(
    screen: &mut Screen,
    g: &Game,
    lvl: usize,
    x_scroll: i32,
    y_scroll: i32,
    dir: i32,
    q: i32,
) {
    let tx0 = x_scroll >> 4;
    let ty0 = y_scroll >> 4;
    let nx = (((x_scroll + screen::W - 1) >> 4) - tx0 + 1) as usize;
    let ny = (((y_scroll + screen::H - 1) >> 4) - ty0 + 1) as usize;

    // Caster grid with a one-tile horizontal margin: an off-screen tree on the sun
    // side still throws onto the visible edge.
    let cw = nx + 2;
    let mut cast = vec![false; cw * ny];
    for (j, row) in cast.chunks_mut(cw).enumerate() {
        for (i, c) in row.iter_mut().enumerate() {
            *c = casts_sun_shadow(g, lvl, tx0 + i as i32 - 1, ty0 + j as i32);
        }
    }

    const KEEP: i32 = 190; // one quantized darken level (~0.74), the dither does the rest
    let len = 4 * q; // 4..16 px — a full tile only at the peak of the window
    for tj in 0..ny {
        let ty = ty0 + tj as i32;
        let y0 = (ty * 16 - y_scroll).max(0);
        let y1 = (ty * 16 + 16 - y_scroll).min(screen::H);
        for ti in 0..nx {
            let ci = tj * cw + ti + 1;
            // The strip lands on the neighbor in `dir`; another caster swallows it.
            if !cast[ci] || cast[(ci as i32 + dir) as usize] {
                continue;
            }
            let tx = tx0 + ti as i32;
            for k in 0..len {
                let wx = if dir > 0 {
                    tx * 16 + 16 + k
                } else {
                    tx * 16 - 1 - k
                };
                let x = wx - x_scroll;
                if !(0..screen::W).contains(&x) {
                    continue;
                }
                // Two steps: full coverage for the near half, half beyond.
                let cov = if 2 * k < len { q * 2 } else { q };
                let bx = (wx & 3) as usize;
                for y in y0..y1 {
                    if BAYER[bx + (((y + y_scroll) & 3) << 2) as usize] < cov {
                        darken_px(&mut screen.pixels[(y * screen::W + x) as usize], KEEP);
                    }
                }
            }
        }
    }
}

/* ------------------------------- contact shadows -------------------------------- */

/// A 2-px dithered ellipse under every grounded mob, drawn between the tile pass and
/// the sprite pass (`Renderer::render_level` hook) so the mob's feet land on top of
/// it. Swimmers make ripples instead, and floaters (ghosts, wisps) cast nothing.
pub fn contact_shadows(screen: &mut Screen, g: &Game, lvl: usize, x_scroll: i32, y_scroll: i32) {
    if !fx_on(FX_CONTACT_SHADOWS) {
        return;
    }
    let xo = x_scroll >> 4;
    let yo = y_scroll >> 4;
    let w = (screen::W + 15) >> 4;
    let h = (screen::H + 15) >> 4;
    for eid in crate::level::get_entities_in_tiles(g, lvl, xo - 1, yo - 1, xo + w + 1, yo + h + 1) {
        let Some(e) = g.entities.get(eid) else {
            continue;
        };
        if !e.is_mob()
            || matches!(e.kind, EntityKind::Ghost(_) | EntityKind::NightWisp(_))
            || crate::entity::behavior::is_swimming(g, e)
        {
            continue;
        }
        // Deep water means riding the raft — no ground to shadow.
        if matches!(
            g.tile_at(lvl, e.c.x >> 4, e.c.y >> 4).kind,
            TileKind::DeepWater
        ) {
            continue;
        }
        // Half-widths of the two ellipse rows; low-slung mobs get a smaller pool.
        let (hw0, hw1) = match e.kind {
            EntityKind::GlowWorm(_) => (2, 1),
            EntityKind::Snake(_) => (3, 2),
            _ => (4, 3),
        };
        let cx = e.c.x - 1; // sprite center (see `stamp_emitters`)
        const KEEP: i32 = 176; // ~0.69 on the checkered half — reads as soft shade
        for (dy, hw) in [(4, hw0), (5, hw1)] {
            let wy = e.c.y + dy;
            let y = wy - y_scroll;
            if !(0..screen::H).contains(&y) {
                continue;
            }
            for wx in (cx - hw)..(cx + hw) {
                let x = wx - x_scroll;
                // 50% checker, world-anchored
                if (0..screen::W).contains(&x) && (wx ^ wy) & 1 == 0 {
                    darken_px(&mut screen.pixels[(y * screen::W + x) as usize], KEEP);
                }
            }
        }
    }
}

/* -------------------------------- water glitter --------------------------------- */

/// Sun/moon glitter on open water: a world-anchored band (wavelength ~96 px,
/// drifting slowly westward with the sun) inside which hashed cells flash short
/// glints. Day glints are warm and doubled with a 1-px dash leaning toward the sun
/// (east before noon, west after); clear-night moon glints are cool, sparser, and
/// dimmer. Twinkle phase is per-cell, so the band shimmers instead of blinking.
pub fn water_glitter(
    screen: &mut Screen,
    g: &Game,
    lvl: usize,
    x_scroll: i32,
    y_scroll: i32,
    brightness: f32,
) {
    let day = brightness >= 0.55;
    let night = brightness <= 0.40;
    if !day && !night {
        return; // dusk/dawn transition: no glitter, the sky show owns those minutes
    }

    // Water mask over the visible tile grid — one lookup per tile, not per glint.
    let tx0 = x_scroll >> 4;
    let ty0 = y_scroll >> 4;
    let nx = (((x_scroll + screen::W - 1) >> 4) - tx0 + 1) as usize;
    let ny = (((y_scroll + screen::H - 1) >> 4) - ty0 + 1) as usize;
    let mut water = vec![false; nx * ny];
    let mut any = false;
    for (j, row) in water.chunks_mut(nx).enumerate() {
        for (i, w) in row.iter_mut().enumerate() {
            *w = matches!(
                g.tile_at(lvl, tx0 + i as i32, ty0 + j as i32).kind,
                TileKind::Water | TileKind::DeepWater
            );
            any |= *w;
        }
    }
    if !any {
        return;
    }

    const GLINT_SALT: u64 = 0x5011A; // "solar"
    let seed = g.world_seed;
    let t = g.tick_count;
    let day_frac = t.rem_euclid(DAY_LENGTH) as f32 / DAY_LENGTH as f32;
    let az: i32 = if day_frac < 0.375 { 1 } else { -1 }; // dash toward the sun
    let (cell, base_cov, cr, cg, cb) = if day {
        (6, 6, 88, 78, 48) // warm sun sparkle
    } else {
        (9, 3, 30, 38, 58) // cool, sparse moonlight
    };

    let i0 = x_scroll.div_euclid(cell) - 1;
    let i1 = (x_scroll + screen::W).div_euclid(cell) + 1;
    let j0 = y_scroll.div_euclid(cell) - 1;
    let j1 = (y_scroll + screen::H).div_euclid(cell) + 1;
    for j in j0..=j1 {
        for i in i0..=i1 {
            let h = infinite_gen::hash(seed, GLINT_SALT, i, j);
            let wx = i * cell + (h % cell as u64) as i32;
            let wy = j * cell + ((h >> 8) % cell as u64) as i32;
            // The glitter band: diagonal, drifting west across the day like the
            // sun's reflection path. Quantized to full/half/none.
            let m = (wx + wy / 2 + t / 24).rem_euclid(96);
            let cov = match m {
                0..=13 => base_cov,
                14..=27 => base_cov / 2,
                _ => continue,
            };
            if BAYER[((i & 3) + ((j & 3) << 2)) as usize] >= cov {
                continue;
            }
            let (ti, tj) = ((wx >> 4) - tx0, (wy >> 4) - ty0);
            if ti < 0 || tj < 0 || ti >= nx as i32 || tj >= ny as i32 {
                continue;
            }
            if !water[tj as usize * nx + ti as usize] {
                continue;
            }
            // Twinkle: visible ~9 of 24 phase steps, peaking in the middle.
            let ph = (t / 3 + ((h >> 16) & 31) as i32).rem_euclid(24);
            if ph >= 9 {
                continue;
            }
            let k = if (2..7).contains(&ph) { 2 } else { 1 };
            let (sx, sy) = (wx - x_scroll, wy - y_scroll);
            screen.add_rgb(sx, sy, cr * k, cg * k, cb * k);
            screen.add_rgb(sx + az, sy, cr * k / 2, cg * k / 2, cb * k / 2);
        }
    }
}

/* -------------------------------- heat shimmer ---------------------------------- */

/// Is the day clock inside the "punishing sun" window that makes desert sand
/// shimmer? (Lava shimmers regardless.)
pub fn high_noon(tick_count: i32) -> bool {
    let t = tick_count.rem_euclid(DAY_LENGTH) as f32 / DAY_LENGTH as f32;
    (0.30..0.45).contains(&t)
}

/// Heat shimmer: rows over hot tiles slide 1 px left/right on a slow, world-anchored
/// wave (amplitude 1, wavelength 16 rows, one step every ~20 ticks). Runs inside the
/// lighting pass — before the HUD — so UI rows never wobble. Hot = lava anywhere;
/// with `desert_noon`, desert-biome sand too.
pub fn heat_shimmer(
    screen: &mut Screen,
    g: &Game,
    lvl: usize,
    x_scroll: i32,
    y_scroll: i32,
    desert_noon: bool,
) {
    let tx0 = x_scroll >> 4;
    let ty0 = y_scroll >> 4;
    let nx = (((x_scroll + screen::W - 1) >> 4) - tx0 + 1) as usize;
    let ny = (((y_scroll + screen::H - 1) >> 4) - ty0 + 1) as usize;
    let seed = g.world_seed;
    let mut hot = vec![false; nx * ny];
    let mut any = false;
    for (j, row) in hot.chunks_mut(nx).enumerate() {
        for (i, hcell) in row.iter_mut().enumerate() {
            let (tx, ty) = (tx0 + i as i32, ty0 + j as i32);
            *hcell = match g.tile_at(lvl, tx, ty).kind {
                TileKind::Lava => true,
                TileKind::Sand | TileKind::QuickSand if desert_noon => {
                    infinite_gen::biome_at_blended(seed, tx, ty) == Biome::Desert
                }
                _ => false,
            };
            any |= *hcell;
        }
    }
    if !any {
        return;
    }

    for y in 0..screen::H {
        let wy = y + y_scroll;
        // amplitude-1 square-ish wave: right, rest, left, rest — 4-row bands
        let o = [0, 1, 0, -1][(((wy >> 2) + g.game_time / 20) & 3) as usize];
        if o == 0 {
            continue;
        }
        let tj = ((wy >> 4) - ty0) as usize;
        let row = &mut hot[tj * nx..(tj + 1) * nx];
        let base = (y * screen::W) as usize;
        let mut ti = 0usize;
        while ti < nx {
            if !row[ti] {
                ti += 1;
                continue;
            }
            let start = ti;
            while ti < nx && row[ti] {
                ti += 1;
            }
            // pixel span of this hot run, clamped to the screen
            let x0 = ((tx0 + start as i32) * 16 - x_scroll).max(0) as usize;
            let x1 = (((tx0 + ti as i32) * 16) - x_scroll).min(screen::W) as usize;
            if x1 <= x0 + 1 {
                continue;
            }
            if o > 0 {
                screen
                    .pixels
                    .copy_within(base + x0..base + x1 - 1, base + x0 + 1);
            } else {
                screen
                    .pixels
                    .copy_within(base + x0 + 1..base + x1, base + x0);
            }
        }
    }
}

/* -------------------------------- drifting motes -------------------------------- */

/// Leaf/pollen palettes: (body, highlight) pairs — two greens and one early-autumn
/// amber, muted to sit inside the terrain palette.
const LEAF_COLORS: [(i32, i32); 3] = [
    (0x27481C, 0x7FB558),
    (0x2F5D25, 0x8FBF5A),
    (0x6E5320, 0xC9A84C),
];

/// Daylight ambience motes on clear surface frames: over forest, a handful of
/// 2-px tumbling leaves sink lazily past; over plains, rare pollen specks glint.
/// Same world-anchored falling-lattice trick as the snow, but on 64-px cells with
/// only one mote in five cells — 3-6 on screen, by design, never a particle storm.
pub fn drift_motes(screen: &mut Screen, g: &Game, x_scroll: i32, y_scroll: i32, brightness: f32) {
    if brightness < 0.60 {
        return; // daylight ambience only
    }
    const MOTE_SALT: u64 = 0x1EAF;
    const CELL: i32 = 64;
    let seed = g.world_seed;
    let t = g.game_time as i64;
    let fall = t / 8; // ~0.125 px/tick — leaves sink, they don't rain
    let k256 = (brightness.clamp(0.0, 1.0) * 256.0) as i32;

    let i0 = x_scroll.div_euclid(CELL) - 1;
    let i1 = (x_scroll + screen::W).div_euclid(CELL) + 1;
    let j0 = (y_scroll as i64 - fall).div_euclid(CELL as i64) as i32 - 1;
    let j1 = ((y_scroll + screen::H) as i64 - fall).div_euclid(CELL as i64) as i32 + 1;
    for j in j0..=j1 {
        for i in i0..=i1 {
            let h = infinite_gen::hash(seed, MOTE_SALT, i, j);
            if h % 5 != 0 {
                continue;
            }
            let sway =
                [0, 1, 2, 1, 0, -1, -2, -1][((t / 18 + ((h >> 32) & 7) as i64) & 7) as usize];
            let wx = i * CELL + ((h >> 8) % CELL as u64) as i32 + sway;
            let wy = (j as i64 * CELL as i64 + fall) as i32 + ((h >> 16) % CELL as u64) as i32;
            let (sx, sy) = (wx - x_scroll, wy - y_scroll);
            match infinite_gen::biome_at(seed, wx >> 4, wy >> 4) {
                Biome::Forest => {
                    // a 2-px leaf that tumbles: the highlight pixel orbits the body
                    let (body, hi) = LEAF_COLORS[((h >> 44) % 3) as usize];
                    let (dx, dy) = [(1, 0), (0, 1), (-1, 0), (0, -1)]
                        [((t / 10 + ((h >> 40) & 3) as i64) & 3) as usize];
                    put_px(screen, sx, sy, body, k256);
                    put_px(screen, sx + dx, sy + dy, hi, k256);
                }
                Biome::Plains => {
                    // pollen: a single bright warm speck, winking on and off
                    let ph = (t / 6 + ((h >> 24) & 15) as i64) & 15;
                    if ph < 8 {
                        screen.add_rgb(sx, sy, 70, 62, 26);
                    }
                }
                _ => {}
            }
        }
    }
}
