//! Regression coverage for the water-family visual oddities (docs/ODDITIES.md):
//!
//! - O4:  the swimming ring must never render as an opaque black box (it used to on
//!   every water-family tile whose id wasn't exactly "water"), and its color must
//!   follow the liquid kind.
//! - O5:  inland ponds must have ragged (non-rectangular) but deterministic
//!   outlines, and their bank rims must follow the climate — never warm mud around
//!   a snowfield pond.
//! - O10/O11 ("O13" in the fix lane): every walkable-ground|water edge gets the
//!   same waterline treatment on all four axes; mud shores included.
//! - O14 ("O15"): the shallow side of the ocean→deep boundary feathers raggedly
//!   toward Deep Water instead of ending in a hard tile seam.
//!
//! Each test stages its scene, dumps 1x + 6x screenshots under
//! `target/verify/water_oddities/`, and asserts the pixel- or tile-level property.
//! FX toggles are process-global, so tests that touch them share a lock.

use std::sync::{Mutex, MutexGuard};

use fdoom::core::updater::DAY_LENGTH;
use fdoom::gfx::{lighting, screen};
use fdoom::level::infinite_gen::{climate_at, land_at};
use fdoom::level::tile::{TileKind, tidal};
use fdoom::testutil::{TestWorld, save_png, verify_path};

static FX_LOCK: Mutex<()> = Mutex::new(());

fn fx_lock() -> MutexGuard<'static, ()> {
    FX_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn day_tick(frac: f32) -> i32 {
    (DAY_LENGTH as f32 * frac) as i32
}

fn pin_time(tw: &mut TestWorld, tick: i32) {
    tw.g.set_time(tick);
    tw.tick_n(2);
    tw.g.set_time(tick);
}

/// Dump a frame at 1x and 6x under `target/verify/water_oddities/`.
fn shot(name: &str, pixels: &[i32]) {
    let dir = verify_path("water_oddities");
    std::fs::create_dir_all(&dir).ok();
    save_png(
        dir.join(format!("{name}.png")),
        pixels,
        screen::W as usize,
        screen::H as usize,
        1,
    );
    save_png(
        dir.join(format!("{name}_big.png")),
        pixels,
        screen::W as usize,
        screen::H as usize,
        6,
    );
}

/// Screen x/y of the north-west pixel corner of world tile `(tx, ty)`.
fn tile_screen_origin(tw: &TestWorld, tx: i32, ty: i32) -> (i32, i32) {
    let (px, py) = tw.player_pos();
    (
        tx * 16 - (px - screen::W / 2),
        ty * 16 - (py - (screen::H - 8) / 2),
    )
}

fn pixel(frame: &[i32], x: i32, y: i32) -> (i32, i32, i32) {
    let p = frame[(x + y * screen::W) as usize];
    ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF)
}

fn luma(rgb: (i32, i32, i32)) -> i32 {
    (30 * rgb.0 + 59 * rgb.1 + 11 * rgb.2) / 100
}

/* ------------------------- O4: swim ring, per-liquid color ------------------------ */

/// Count near-black pixels and collect the dominant blue-ish ring color inside the
/// swim-ring area around the player.
fn ring_stats(tw: &TestWorld, frame: &[i32]) -> (usize, usize) {
    let (px, py) = tw.player_pos();
    let (cx, cy) = (
        px - (px - screen::W / 2) - 8,
        py - (py - (screen::H - 8) / 2) - 8,
    );
    // the ring renders at (xo, yo+3) as two 8x8 halves; scan generously around it
    let mut black = 0;
    let mut blue = 0;
    for y in cy - 2..cy + 14 {
        for x in cx - 2..cx + 18 {
            let rgb = pixel(frame, x, y);
            if luma(rgb) < 8 {
                black += 1;
            }
            if rgb.2 > rgb.0 && rgb.2 > 90 {
                blue += 1;
            }
        }
    }
    (black, blue)
}

/// O4: a player swimming on a submerged tidal flat must sit in a water ring, not an
/// opaque black rectangle — and the flat's ring must differ from open water's.
#[test]
fn o4_swim_ring_on_tidal_flat_not_black() {
    let _g = fx_lock();
    lighting::set_disabled_fx(lighting::FX_WATER_GLITTER);
    let mut tw = TestWorld::infinite().seed(9).name("wo_o4").build();
    tw.tick_n(4);

    // noon = mid tide, falling: flats below the mid level are submerged
    let noon = day_tick(0.375);
    tw.g.set_time(noon);
    let seed = tw.g.world_seed;
    let lvl = tw.current_level;

    // ODDITIES O4 repro shore (seed 9, tile (-8,36)): find a flat that is submerged
    // at noon's mid tide
    tw.teleport(-8, 36);
    tw.tick_n(4);
    let mut spot = None;
    'search: for r in 0..48i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (-8 + dx, 36 + dy);
                if matches!(tw.g.tile_at(lvl, x, y).kind, TileKind::TidalFlat)
                    && land_at(seed, x, y) < 0.417
                {
                    spot = Some((x, y));
                    break 'search;
                }
            }
        }
    }
    let (fx, fy) = spot.expect("a submerged tidal flat near the O4 shore");
    tw.teleport(fx, fy);
    pin_time(&mut tw, noon);
    assert!(
        tidal::is_submerged(&tw.g, fx, fy),
        "test flat should be under the noon tide"
    );
    tw.g.notifications.clear();
    let tidal_frame = tw.render();
    shot("o4_swim_tidal_flat", &tidal_frame);
    let (black, blue) = ring_stats(&tw, &tidal_frame);
    println!("O4 tidal ring: {black} near-black px, {blue} blue px");
    assert!(
        black < 20,
        "swim ring on a tidal flat reads as a black box ({black} near-black px)"
    );
    assert!(
        blue > 30,
        "swim ring on a tidal flat missing ({blue} blue px)"
    );

    // open water for comparison: the ring colors must differ per liquid kind
    for dy in -2..=2 {
        for dx in -2..=2 {
            tw.place("water", dx, dy);
        }
    }
    pin_time(&mut tw, noon);
    tw.g.notifications.clear();
    let water_frame = tw.render();
    shot("o4_swim_open_water", &water_frame);
    let (black_w, blue_w) = ring_stats(&tw, &water_frame);
    assert!(black_w < 20 && blue_w > 30, "open-water ring sanity");

    // compare the ring areas pixel-for-pixel: the tidal ring is a duller shade
    let (px, py) = tw.player_pos();
    let (cx, cy) = (
        px - (px - screen::W / 2) - 8,
        py - (py - (screen::H - 8) / 2) - 8,
    );
    let mut differing = 0;
    for y in cy + 3..cy + 11 {
        for x in cx..cx + 16 {
            let a = pixel(&tidal_frame, x, y);
            let b = pixel(&water_frame, x, y);
            if a != b {
                differing += 1;
            }
        }
    }
    println!("O4 ring pixels differing between tidal flat and open water: {differing}");
    assert!(
        differing > 12,
        "tidal-flat ring should be a different shade than open water ({differing} px differ)"
    );
}

/* -------------------- O5: pond shape raggedness + rim material -------------------- */

/// Water tiles of the inland pond inside a window around `(cx, cy)`.
fn pond_tiles(tw: &mut TestWorld, cx: i32, cy: i32, r: i32) -> Vec<(i32, i32)> {
    let lvl = tw.current_level;
    let seed = tw.g.world_seed;
    fdoom::level::ensure_chunks_at(&mut tw.g, lvl, cx, cy, true);
    fdoom::level::ensure_chunks_at(&mut tw.g, lvl, cx - r, cy - r, true);
    fdoom::level::ensure_chunks_at(&mut tw.g, lvl, cx + r, cy + r, true);
    let mut out = Vec::new();
    for y in cy - r..=cy + r {
        for x in cx - r..=cx + r {
            if matches!(tw.g.tile_at(lvl, x, y).kind, TileKind::Water) && land_at(seed, x, y) > 0.48
            {
                out.push((x, y));
            }
        }
    }
    out
}

/// O5a: the seed-42 plains pond at (-115,-244) must not be a filled rectangle, and
/// must generate identically in two separately-built worlds (determinism).
#[test]
fn o5_pond_outline_ragged_and_deterministic() {
    let mut tw = TestWorld::infinite().seed(42).name("wo_o5a").build();
    let tiles = pond_tiles(&mut tw, -115, -244, 10);
    assert!(tiles.len() >= 6, "pond expected at seed 42 (-115,-244)");
    let (x0, x1) = (
        tiles.iter().map(|t| t.0).min().unwrap(),
        tiles.iter().map(|t| t.0).max().unwrap(),
    );
    let (y0, y1) = (
        tiles.iter().map(|t| t.1).min().unwrap(),
        tiles.iter().map(|t| t.1).max().unwrap(),
    );
    let bbox = ((x1 - x0 + 1) * (y1 - y0 + 1)) as usize;
    println!(
        "O5 pond: {} water tiles in a {}x{} bbox",
        tiles.len(),
        x1 - x0 + 1,
        y1 - y0 + 1
    );
    assert!(
        bbox >= tiles.len() + 3,
        "pond outline is a filled rectangle: {} tiles fill their {bbox}-tile bbox",
        tiles.len()
    );

    // determinism: a second world from the same seed generates the same pond
    let mut tw2 = TestWorld::infinite().seed(42).name("wo_o5a2").build();
    let tiles2 = pond_tiles(&mut tw2, -115, -244, 10);
    assert_eq!(tiles, tiles2, "pond shape must be deterministic per seed");

    // and show it
    tw.teleport(-115, -240);
    tw.tick_n(8);
    pin_time(&mut tw, day_tick(0.375));
    tw.g.notifications.clear();
    let frame = tw.render();
    shot("o5_pond_plains", &frame);
}

/// O5b: the cold-country pond (seed 9, near (835,-725), climate < 0.33) must not be
/// ringed with warm mud — its margins follow the snowfield.
#[test]
fn o5_snow_pond_rim_is_not_mud() {
    let mut tw = TestWorld::infinite().seed(9).name("wo_o5b").build();
    let seed = tw.g.world_seed;
    let lvl = tw.current_level;
    let tiles = pond_tiles(&mut tw, 835, -725, 8);
    assert!(!tiles.is_empty(), "cold pond expected at seed 9 (835,-725)");
    let mut snow_rim = 0;
    for &(x, y) in &tiles {
        assert!(
            climate_at(seed, x, y) < 0.34,
            "scene drifted: pond not cold"
        );
        for (nx, ny) in [(x, y - 1), (x, y + 1), (x - 1, y), (x + 1, y)] {
            let t = tw.g.tile_at(lvl, nx, ny);
            assert!(
                !matches!(t.kind, TileKind::Mud),
                "warm mud rim on a snow-country pond at ({nx},{ny})"
            );
            if matches!(t.kind, TileKind::Snow) {
                snow_rim += 1;
            }
        }
    }
    assert!(snow_rim > 0, "cold pond should sit in snowy margins");

    tw.teleport(835, -722);
    tw.tick_n(8);
    pin_time(&mut tw, day_tick(0.375));
    tw.g.notifications.clear();
    let frame = tw.render();
    shot("o5_pond_snow_rim", &frame);
}

/* --------------------- O10/O11: waterline on all four axes ---------------------- */

/// Stage a single water tile in a field of `ground` and count "lap" pixels (warmer
/// than water: r > b) in the 2px band inside each of the tile's four edges.
/// Returns [north, south, west, east].
fn waterline_bands(ground: &str, name: &str) -> [usize; 4] {
    let mut tw = TestWorld::infinite().seed(9).name(name).build();
    tw.tick_n(4);
    for dy in -6..=6 {
        for dx in -8..=8 {
            tw.place(ground, dx, dy);
        }
    }
    let (wx, wy) = tw.place("water", 3, 0);
    pin_time(&mut tw, day_tick(0.375));
    tw.g.notifications.clear();
    let frame = tw.render();
    shot(&format!("o10_waterline_{ground}"), &frame);

    let (sx, sy) = tile_screen_origin(&tw, wx, wy);
    let warm = |x: i32, y: i32| {
        let rgb = pixel(&frame, x, y);
        rgb.0 > rgb.2
    };
    let mut bands = [0usize; 4];
    for i in 0..16 {
        for d in 0..2 {
            bands[0] += warm(sx + i, sy + d) as usize; // north
            bands[1] += warm(sx + i, sy + 15 - d) as usize; // south
            bands[2] += warm(sx + d, sy + i) as usize; // west
            bands[3] += warm(sx + 15 - d, sy + i) as usize; // east
        }
    }
    bands
}

/// O10/O11: sand AND mud shores get waterline pixels on all four edges of an
/// isolated water tile, with no axis favored.
#[test]
fn o10_waterline_present_on_all_axes_for_sand_and_mud() {
    let _g = fx_lock();
    lighting::set_disabled_fx(lighting::FX_WATER_GLITTER);
    for ground in ["sand", "Mud"] {
        let bands = waterline_bands(ground, &format!("wo_o10_{ground}"));
        println!("O10 {ground} waterline bands (N,S,W,E): {bands:?}");
        for (i, n) in bands.iter().enumerate() {
            assert!(
                *n >= 8,
                "{ground}|water edge {i} has too little waterline ({n} of 32 px)"
            );
        }
        let (min, max) = (
            *bands.iter().min().unwrap() as f64,
            *bands.iter().max().unwrap() as f64,
        );
        assert!(
            max / min <= 2.5,
            "{ground}|water waterline is axis-inconsistent: bands {bands:?}"
        );
    }
}

/* -------------------- O14: shallow-side feather toward deep water ----------------- */

/// O14: a shallow-water tile bordering Deep Water darkens raggedly toward the deep
/// edge, so the boundary no longer traces the tile grid. The feather is deliberately
/// gentle (it continues the deep side's first darken band), so the test diffs two
/// renders — with and without the deep neighbor — instead of thresholding luma.
#[test]
fn o14_shallow_side_feathers_toward_deep() {
    let _g = fx_lock();
    lighting::set_disabled_fx(lighting::FX_WATER_GLITTER);
    let mut tw = TestWorld::infinite().seed(9).name("wo_o14").build();
    tw.tick_n(4);
    for dy in -6..=6 {
        for dx in -8..=8 {
            tw.place("water", dx, dy);
        }
    }
    let noon = day_tick(0.375);
    pin_time(&mut tw, noon);
    tw.g.notifications.clear();
    let before = tw.render();

    for dy in -6..=6 {
        for dx in 1..=8 {
            tw.place("Deep Water", dx, dy);
        }
    }
    tw.g.set_time(noon); // same tick: identical water shimmer in both frames
    tw.g.notifications.clear();
    let frame = tw.render();
    shot("o14_ocean_deep_seam", &frame);

    // the shallow tile just west of the boundary, away from the player sprite
    let (ptx, pty) = tw.player_tile();
    let (sx, sy) = tile_screen_origin(&tw, ptx, pty + 3);
    let mut reach = Vec::new(); // per-row horizontal extent of the feather
    let mut total = 0;
    for y in sy..sy + 16 {
        let mut r = 0;
        for d in 0..16 {
            let x = sx + 15 - d;
            let i = (x + y * screen::W) as usize;
            if before[i] != frame[i] {
                r = r.max(d + 1);
                total += 1;
            }
        }
        reach.push(r);
    }
    println!("O14 feather: {total} px changed, per-row reach {reach:?}");
    assert!(
        total >= 24,
        "shallow side of the deep boundary shows no feather ({total} px changed)"
    );
    let (rmin, rmax) = (*reach.iter().min().unwrap(), *reach.iter().max().unwrap());
    assert!(
        rmax > rmin,
        "shallow-side feather is a ruler-straight column ({reach:?})"
    );
}

/* ------------------------- visual evidence: natural scenes ------------------------ */

/// Screenshot-only sweep of the ODDITIES natural repro coordinates, so the fix
/// evidence under `target/verify/water_oddities/` stays current. Assertions are
/// deliberately minimal — the eyeball pass happens on the PNGs.
#[test]
fn natural_scene_shots() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);

    // O14 natural: ocean→deep boundary, seed 9 (0,-126)
    let mut tw = TestWorld::infinite().seed(9).name("wo_nat_deep").build();
    tw.teleport(0, -126);
    tw.tick_n(8);
    pin_time(&mut tw, day_tick(0.375));
    tw.g.notifications.clear();
    shot("nat_ocean_deep", &tw.render());

    // O5/O11 natural: marsh pool interior, seed 42 (112,128) — center the shot on
    // the nearest pool water so the mud|water treatment is actually in frame
    let mut tw = TestWorld::infinite().seed(42).name("wo_nat_marsh").build();
    tw.teleport(112, 128);
    tw.tick_n(8);
    let lvl = tw.current_level;
    'marsh: for r in 0..24i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if (dx.abs() == r || dy.abs() == r)
                    && matches!(tw.g.tile_at(lvl, 112 + dx, 128 + dy).kind, TileKind::Water)
                {
                    tw.teleport(112 + dx, 128 + dy - 2);
                    break 'marsh;
                }
            }
        }
    }
    tw.tick_n(8);
    pin_time(&mut tw, day_tick(0.375));
    tw.g.notifications.clear();
    shot("nat_marsh_pool", &tw.render());

    // O13 natural: beach shore at mid tide, seed 9 (-8,36)
    let mut tw = TestWorld::infinite().seed(9).name("wo_nat_shore").build();
    tw.teleport(-8, 36);
    tw.tick_n(8);
    pin_time(&mut tw, day_tick(0.375));
    tw.g.notifications.clear();
    shot("nat_beach_tide", &tw.render());

    // staged grass|water and snow|water pairs, both axes in one frame
    let mut tw = TestWorld::infinite().seed(9).name("wo_pairs").build();
    tw.tick_n(4);
    for ground in ["grass", "snow"] {
        for dy in -6..=6 {
            for dx in -8..=8 {
                tw.place(ground, dx, dy);
            }
        }
        for dy in -6..=6 {
            for dx in 3..=8 {
                tw.place("water", dx, dy);
            }
        }
        for dy in 3..=6 {
            for dx in -8..=8 {
                tw.place("water", dx, dy);
            }
        }
        pin_time(&mut tw, day_tick(0.375));
        tw.g.notifications.clear();
        shot(&format!("pair_{ground}_water"), &tw.render());
    }
}
