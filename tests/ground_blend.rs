//! Ground-blend seam regressions (QA lane: blending / seam carry / corner rounding).
//!
//! The bug these pin: the seam carry used a 4x4 Bayer mask indexed by world
//! coordinates. The tile pitch (16) is a multiple of 4, so a carry strip at depth
//! `d` from a seam always landed on the same `x & 3` — every seam in the world
//! sampled the same four thresholds. Coverage quantized to 0/25/50/75/100% and was
//! locked to the tile grid: depth 0 came out a *solid* 1 px line of the neighbour's
//! colour down every tile edge, and the ramp rose again at depth 3. Instead of
//! dissolving a boundary the carry outlined it, and because both sides did it the
//! colours swapped across the seam (the column just inside grass read 78% toward
//! sand while the column just inside sand read 31%).
//!
//! Set `BLEND_SHOT_DIR=/some/dir` to dump the staged frames as PNGs.

use std::sync::{Mutex, MutexGuard};

use fdoom::gfx::lighting;
use fdoom::level::infinite_gen::Biome;
use fdoom::testutil::{TestWorld, find_biome, save_png};

const W: i32 = 288;
const H: i32 = 192;

/// The `FX_*` toggles are process-global and cargo runs a binary's tests in
/// parallel, so every test here serializes on one lock (same idiom as
/// `tests/visuals.rs`) — otherwise a rendering test can be caught mid-frame by
/// another test's `set_disabled_fx`.
static FX_LOCK: Mutex<()> = Mutex::new(());

fn fx_lock() -> MutexGuard<'static, ()> {
    FX_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn noon(tw: &mut TestWorld) {
    tw.g.tick_count = fdoom::core::updater::DAY_LENGTH / 4;
}

/// A flat field on plains, left half `a` and right half `b`, seam on the tile
/// boundary at the player's tile x. Returns the player's tile coords.
fn stage_seam(tw: &mut TestWorld, a: &str, b: &str) -> (i32, i32) {
    let (px, py) = tw.player_tile();
    for j in -6..7 {
        for i in -9..10 {
            tw.place_at(if i < 0 { a } else { b }, px + i, py + j);
        }
    }
    tw.tick_n(2);
    (px, py)
}

/// Rows well clear of the player sprite, in tile-relative pixel offsets.
fn probe_rows() -> Vec<i32> {
    (-5..-2)
        .flat_map(|j: i32| (0..16).map(move |k| j * 16 + k))
        .collect()
}

fn shot(name: &str, px: &[i32], w: usize, h: usize) {
    if let Ok(dir) = std::env::var("BLEND_SHOT_DIR") {
        save_png(std::path::Path::new(&dir).join(name), px, w, h, 6);
    }
}

/// Mean colour of each pixel column across the seam, projected onto the
/// `a` -> `b` colour axis as a 0..100 mix percentage.
fn mix_profile(tw: &mut TestWorld, px: i32, py: i32, span: i32) -> Vec<i32> {
    let frame = tw.render_at(W, H);
    let (plx, ply) = tw.player_pos();
    let (xs, ys) = (plx - W / 2, ply - (H - 8) / 2);
    let rows = probe_rows();
    let mut cols = Vec::new();
    for d in -span..span {
        let sx = px * 16 + d - xs;
        let (mut r, mut g, mut b) = (0i64, 0i64, 0i64);
        for &rr in &rows {
            let sy = py * 16 + rr - ys;
            let p = frame[(sy * W + sx) as usize];
            r += ((p >> 16) & 0xff) as i64;
            g += ((p >> 8) & 0xff) as i64;
            b += (p & 0xff) as i64;
        }
        let n = rows.len() as i64;
        cols.push(((r / n) as i32, (g / n) as i32, (b / n) as i32));
    }
    let c0 = cols[0];
    let c1 = cols[cols.len() - 1];
    let ax = (
        (c1.0 - c0.0) as f32,
        (c1.1 - c0.1) as f32,
        (c1.2 - c0.2) as f32,
    );
    let len2 = (ax.0 * ax.0 + ax.1 * ax.1 + ax.2 * ax.2).max(1.0);
    cols.iter()
        .map(|&(r, g, b)| {
            (((r - c0.0) as f32 * ax.0 + (g - c0.1) as f32 * ax.1 + (b - c0.2) as f32 * ax.2)
                / len2
                * 100.0)
                .round() as i32
        })
        .collect()
}

/// The blended transition must be a *ramp*: reading across a seam, the ground must
/// grade one way only. It must never invert at the tile boundary, which is what
/// draws a two-tone pinstripe along the 16 px grid.
#[test]
fn seam_transition_is_monotonic_and_never_inverts_at_the_boundary() {
    let _g = fx_lock();
    let seed = 20260707;
    let mut tw = TestWorld::infinite().seed(seed).name("gb_mono").build();
    let (bx, by) = find_biome(seed, Biome::Plains);
    tw.teleport(bx, by);
    tw.tick_n(4);
    noon(&mut tw);

    for (a, b) in [("Grass", "Sand"), ("Grass", "Snow"), ("Mud", "Sand")] {
        let (px, py) = stage_seam(&mut tw, a, b);
        let span = 10;
        let prof = mix_profile(&mut tw, px, py, span);
        // index of the seam column (d = 0) inside the profile
        let seam = span as usize;
        // The carry band is depth 5 each side; check the ramp across it. Column
        // means carry the art's own texture noise, hence the tolerance.
        let mut worst = 0;
        for k in (seam - 5)..(seam + 4) {
            let drop = prof[k] - prof[k + 1];
            worst = worst.max(drop);
        }
        assert!(
            worst <= 20,
            "{a}|{b}: transition is not a ramp — biggest reversal {worst} pts across the \
             carry band. profile(-10..+10) = {prof:?}"
        );
        // the headline regression: the column just inside `a` must not read as
        // *more* `b`-coloured than the column just inside `b`
        let (inside_a, inside_b) = (prof[seam - 1], prof[seam]);
        assert!(
            inside_a <= inside_b + 10,
            "{a}|{b}: the carry inverted the seam — the last {a} column reads {inside_a}% \
             toward {b} while the first {b} column reads only {inside_b}%. profile = {prof:?}"
        );
    }
}

/// The carry mask must not be phase-locked to the tile grid: no pixel column may
/// come out fully covered (that is a drawn line, not a dissolve), and past the
/// deliberate one-pixel hump the ramp must fall away monotonically.
#[test]
fn carry_coverage_is_not_locked_to_the_tile_grid() {
    let _g = fx_lock();
    let seed = 20260707;
    let mut tw = TestWorld::infinite().seed(seed).name("gb_lock").build();
    let (bx, by) = find_biome(seed, Biome::Plains);
    tw.teleport(bx, by);
    tw.tick_n(4);
    noon(&mut tw);
    let (px, py) = stage_seam(&mut tw, "Grass", "Sand");

    lighting::set_disabled_fx(lighting::FX_SEAM_BLEND);
    let off = tw.render_at(W, H);
    lighting::set_disabled_fx(0);
    let on = tw.render_at(W, H);
    let (plx, ply) = tw.player_pos();
    let (xs, ys) = (plx - W / 2, ply - (H - 8) / 2);
    let rows = probe_rows();

    // coverage per column, as a percentage of the probed rows
    let mut cov = Vec::new();
    for d in -6..6i32 {
        let sx = px * 16 + d - xs;
        let n = rows
            .iter()
            .filter(|&&rr| {
                let sy = py * 16 + rr - ys;
                on[(sy * W + sx) as usize] != off[(sy * W + sx) as usize]
            })
            .count();
        cov.push(100 * n / rows.len());
    }
    assert!(
        cov.iter().all(|&c| c < 95),
        "a carry strip is a solid line, not a dither: coverage per column (-6..+6) = {cov:?}"
    );
    // Both sides, walking away from the seam past the hump at depth 1: coverage
    // must decay. (Left side columns run -2, -3, -4, -5; right side +2..+5.)
    let left: Vec<usize> = (2..6).map(|d| cov[6 - 1 - d]).collect();
    let right: Vec<usize> = (2..6).map(|d| cov[6 + d]).collect();
    for (side, v) in [("left", &left), ("right", &right)] {
        assert!(
            v.windows(2).all(|w| w[0] >= w[1]),
            "{side} carry ramp rises again away from the seam (a detached comb tooth): {v:?} \
             (full profile {cov:?})"
        );
    }
}

/// The whole ground pass is world-anchored, so the same world pixel must render
/// identically at any logical screen size. This is the guard for the dynamic
/// 288x192..640x400 resolution work: a grid sized or strided for the classic
/// screen would read the wrong neighbour tiles on a bigger window.
#[test]
fn ground_pass_is_identical_at_every_logical_size() {
    let _g = fx_lock();
    let seed = 20260707;
    for (label, biome) in [
        ("desert", Biome::Desert),
        ("tundra", Biome::Tundra),
        ("marsh", Biome::Marsh),
    ] {
        let mut tw = TestWorld::infinite()
            .seed(seed)
            .name(&format!("gb_size_{label}"))
            .build();
        let (bx, by) = find_biome(seed, biome);
        tw.teleport(bx, by);
        tw.tick_n(4);
        noon(&mut tw);
        let (plx, ply) = tw.player_pos();
        for &(w, h) in &[(608i32, 400i32), (400, 300), (300, 200), (289, 193)] {
            let a = tw.render_at(W, H);
            let b = tw.render_at(w, h);
            let (ax, ay) = (plx - W / 2, ply - (H - 8) / 2);
            let (bx2, by2) = (plx - w / 2, ply - (h - 8) / 2);
            let mut diff = 0;
            let mut first = None;
            for y in 8..H - 50 {
                for x in 8..W - 8 {
                    let (wx, wy) = (x + ax, y + ay);
                    let (xb, yb) = (wx - bx2, wy - by2);
                    if xb < 8 || yb < 8 || xb >= w - 8 || yb >= h - 50 {
                        continue;
                    }
                    if a[(y * W + x) as usize] != b[(yb * w + xb) as usize] {
                        diff += 1;
                        first.get_or_insert((wx, wy));
                    }
                }
            }
            assert_eq!(
                diff, 0,
                "{label}: {w}x{h} renders the shared world rect differently from 288x192 \
                 ({diff} px, first at world {first:?})"
            );
        }
    }
}

/// A dry bush standing on grass must stand on *grass*. It used to hard-render a
/// sand patch under itself everywhere, which the sand-family blend factor then lit
/// and the seam carry ringed: a glowing neon-yellow ball on the meadow (O23).
#[test]
fn a_dry_bush_on_grass_paints_no_sand() {
    let _g = fx_lock();
    let seed = 20260707;
    let mut tw = TestWorld::infinite().seed(seed).name("gb_bush").build();
    let (bx, by) = find_biome(seed, Biome::Plains);
    tw.teleport(bx, by);
    tw.tick_n(4);
    noon(&mut tw);
    let (px, py) = tw.player_tile();
    for j in -6..7 {
        for i in -9..10 {
            tw.place_at("Grass", px + i, py + j);
        }
    }
    let (tx, ty) = (px - 3, py - 1);
    tw.place_at("Dry Bush", tx, ty);
    tw.tick_n(2);

    let frame = tw.render_at(W, H);
    let (plx, ply) = tw.player_pos();
    let (xs, ys) = (plx - W / 2, ply - (H - 8) / 2);
    // the bush tile plus the 8 px ring the carry would have haloed
    let (mut sandy, mut total) = (0, 0);
    let mut crop = Vec::new();
    for dy in -8..24i32 {
        for dx in -8..24i32 {
            let (x, y) = (tx * 16 + dx - xs, ty * 16 + dy - ys);
            let p = frame[(y * W + x) as usize];
            crop.push(p);
            let (r, g, b) = ((p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff);
            total += 1;
            // dune yellow: bright, warm, and clearly not the meadow's green
            if r > 150 && g > 150 && b < 130 && r - b > 60 {
                sandy += 1;
            }
        }
    }
    shot("blend_drybush_on_grass.png", &crop, 32, 32);
    assert_eq!(
        sandy, 0,
        "a dry bush on grass paints {sandy}/{total} dune-yellow pixels — the sand patch \
         and its carry halo are back"
    );
}
