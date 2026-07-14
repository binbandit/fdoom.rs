//! Regression coverage for the top visual-coherence oddities (docs/ODDITIES.md):
//!
//! - O1: emitter light pools must read continuously across ground seams.
//! - O2: boulder/crag boundary cells must sit on the real ground, not a flat backing.
//! - O3: the biome ground blend must never flip a tile's hue family.
//! - O6/O7: prop/flora tiles stand on the ground that surrounds them.
//! - O8: drops float on liquids in a ripple ring — no black sprite-copy shadow.
//! - O9: precipitation identity is per-column, not player-global.
//! - O16/O17: rock faces and dune ripples vary per tile (no quilt, no ruled lines).
//! - O18: day water reads as daytime water; the night palette is untouched.
//!
//! Each test stages the documented repro scene, dumps 1x + 6x screenshots under
//! `target/verify/oddities_fix/`, and asserts the pixel-level property that was
//! broken. FX toggles are process-global, so tests that touch them share a lock.

use std::sync::{Mutex, MutexGuard};

use fdoom::core::updater::DAY_LENGTH;
use fdoom::core::weather::{self, Precip};
use fdoom::entity::EntityKind;
use fdoom::entity::furniture::campfire;
use fdoom::gfx::{lighting, screen};
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

/// Dump a frame at 1x and 6x under `target/verify/oddities_fix/`.
fn shot(name: &str, pixels: &[i32]) {
    let dir = verify_path("oddities_fix");
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

fn mean_rgb(pixels: &[i32], x0: i32, y0: i32, x1: i32, y1: i32) -> [f64; 3] {
    let mut sum = [0.0f64; 3];
    let mut n = 0.0f64;
    for y in y0.max(0)..y1.min(screen::H) {
        for x in x0.max(0)..x1.min(screen::W) {
            let p = pixels[(x + y * screen::W) as usize];
            sum[0] += ((p >> 16) & 0xFF) as f64;
            sum[1] += ((p >> 8) & 0xFF) as f64;
            sum[2] += (p & 0xFF) as f64;
            n += 1.0;
        }
    }
    [
        sum[0] / n.max(1.0),
        sum[1] / n.max(1.0),
        sum[2] / n.max(1.0),
    ]
}

fn luma(rgb: [f64; 3]) -> f64 {
    0.30 * rgb[0] + 0.59 * rgb[1] + 0.11 * rgb[2]
}

/// Screen x/y of the north-west pixel corner of world tile `(tx, ty)`.
fn tile_screen_origin(tw: &TestWorld, tx: i32, ty: i32) -> (i32, i32) {
    let (px, py) = tw.player_pos();
    (
        tx * 16 - (px - screen::W / 2),
        ty * 16 - (py - (screen::H - 8) / 2),
    )
}

/* --------------------------- O1: light-pool continuity --------------------------- */

/// ODDITIES O1 repro: seed 9, staged grass|sand seam, campfire *on* the seam, night.
/// The warm pool must read continuously: mirrored patches at equal distance from the
/// emitter on the grass and sand side may differ in albedo, but not by the old
/// half-moon split (bright orange vs near-dark green).
#[test]
fn o1_light_pool_survives_ground_seam() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);
    let mut tw = TestWorld::infinite().seed(9).name("odd_o1").build();
    tw.tick_n(8);

    let (ptx, pty) = tw.player_tile();
    for dy in -8..=8 {
        for dx in -11..=0 {
            tw.place("grass", dx, dy);
        }
        for dx in 1..=11 {
            tw.place("sand", dx, dy);
        }
    }

    // Campfire two tiles north of the player, its light center pinned exactly on the
    // grass|sand seam line (world x = (ptx + 1) * 16).
    let lvl = tw.current_level;
    let e = campfire::new();
    tw.g.level_mut(lvl).add_at(e, ptx + 1, pty - 2, true, lvl);
    tw.tick_n(1);
    let seam_x = (ptx + 1) * 16;
    let eid =
        tw.g.entities
            .entities_on_level(lvl)
            .find(|e| matches!(e.kind, EntityKind::Campfire(_)))
            .map(|e| e.c.eid)
            .expect("campfire placed");
    let fire_y = {
        let e = tw.g.entities.get_mut(eid).unwrap();
        e.c.x = seam_x + 1; // emitter x = c.x - 1
        e.c.y - 4 // emitter y
    };
    pin_time(&mut tw, day_tick(0.85)); // deep night
    tw.g.notifications.clear();
    let frame = tw.render();
    shot("o1_seam_campfire_night", &frame);

    let (sx, _) = tile_screen_origin(&tw, ptx + 1, pty - 2);
    let (_, sy) = {
        let (px, py) = tw.player_pos();
        (px, fire_y - (py - (screen::H - 8) / 2))
    };
    // Mirrored 8x16 patches, 6..14 px each side of the seam, vertically centered on
    // the emitter — both sit well inside the campfire's lit pool.
    let grass = mean_rgb(&frame, sx - 14, sy - 8, sx - 6, sy + 8);
    let sand = mean_rgb(&frame, sx + 6, sy - 8, sx + 14, sy + 8);
    let (lg, ls) = (luma(grass), luma(sand));
    let asym = (ls - lg).abs() / ls.max(lg);
    println!("O1 pool luma: grass {lg:.1} vs sand {ls:.1} (asym {asym:.3})");
    assert!(
        asym < 0.30,
        "emitter pool splits at the seam: grass {lg:.1} vs sand {ls:.1} (asym {asym:.3})"
    );
    // And the pool must actually be lit vs the far dark grass.
    let dark = mean_rgb(&frame, sx - 120, sy - 8, sx - 104, sy + 8);
    assert!(
        lg > luma(dark) * 2.0,
        "grass side of the pool should read lit ({lg:.1} vs dark {:.1})",
        luma(dark)
    );
}

/* ----------------------------- O2: rock ground backing --------------------------- */

/// ODDITIES O2 repro (staged): a lone boulder on desert sand. The cell area outside
/// the boulder blob must show the *actual sand art* (identical pixels to the same
/// frame without the boulder), not a flat approximation fill.
#[test]
fn o2_rock_backing_shows_real_ground() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);
    let mut tw = TestWorld::infinite().seed(9).name("odd_o2").build();
    tw.tick_n(8);
    tw.goto_biome(fdoom::level::infinite_gen::Biome::Desert);
    tw.tick_n(8);

    for dy in -6..=6 {
        for dx in -8..=8 {
            tw.place("sand", dx, dy);
        }
    }
    pin_time(&mut tw, day_tick(0.375)); // noon
    tw.g.notifications.clear();
    let bare = tw.render();

    let (rtx, rty) = tw.place("rock", 2, 0);
    tw.g.notifications.clear();
    let with_rock = tw.render();
    shot("o2_boulder_on_sand", &with_rock);

    // Count pixels of the boulder tile identical to the bare-sand frame, split into
    // base-color and texture pixels. The old flat backing happened to match the sand
    // art's *base* color, so plain equality can't tell it from real ground — the
    // ripple/speck texture showing through is what proves the sand art is really
    // rendered beneath the blob.
    let (sx, sy) = tile_screen_origin(&tw, rtx, rty);
    let mut counts = std::collections::HashMap::new();
    for y in sy..sy + 16 {
        for x in sx..sx + 16 {
            *counts
                .entry(bare[(x + y * screen::W) as usize])
                .or_insert(0) += 1;
        }
    }
    let base = *counts.iter().max_by_key(|(_, n)| **n).unwrap().0;
    let mut same = 0;
    let mut textured = 0;
    for y in sy..sy + 16 {
        for x in sx..sx + 16 {
            let i = (x + y * screen::W) as usize;
            if bare[i] == with_rock[i] {
                same += 1;
                if bare[i] != base {
                    textured += 1;
                }
            }
        }
    }
    println!("O2 boulder tile: {same}/256 pixels show the sand beneath ({textured} textured)");
    assert!(
        same >= 25 && textured >= 5,
        "boulder cell should show real textured sand outside the blob, got {same} matching \
         ({textured} textured) of 256"
    );
}

/* ------------------------------ O3: tint identity -------------------------------- */

/// ODDITIES O3 repro (staged): a 3x3 grass patch in open desert sand and an isolated
/// sand freckle in grass. Identity must survive the blend: grass stays green-dominant
/// everywhere (no near-white bleach, no sand-yellow flip), and the freckle must not
/// project a "glow square" onto its neighbors.
#[test]
fn o3_blend_keeps_hue_family() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);
    let mut tw = TestWorld::infinite().seed(9).name("odd_o3").build();
    tw.tick_n(8);

    for dy in -8..=8 {
        for dx in -11..=11 {
            tw.place("sand", dx, dy);
        }
    }
    for dy in -1..=1 {
        for dx in -1..=1 {
            tw.place("grass", dx, dy);
        }
    }
    // Isolated single-tile islands well clear of the patch.
    tw.place("grass", 6, -3);
    tw.place("snow", 6, 3);
    pin_time(&mut tw, day_tick(0.375));
    tw.g.notifications.clear();
    let frame = tw.render();
    shot("o3_islands_in_sand", &frame);

    let (ptx, pty) = tw.player_tile();

    // (a) The grass patch's center tile keeps its green hue family.
    let (cx, cy) = tile_screen_origin(&tw, ptx, pty);
    let center = mean_rgb(&frame, cx + 2, cy + 2, cx + 14, cy + 14);
    println!(
        "O3 grass patch center rgb: {:.0}/{:.0}/{:.0}",
        center[0], center[1], center[2]
    );
    assert!(
        center[1] > center[0] * 1.08,
        "grass beside desert must stay green-dominant, got r {:.0} g {:.0}",
        center[0],
        center[1]
    );

    // (b) A lone grass island keeps its identity too (whole tile, not just center).
    let (gx, gy) = tile_screen_origin(&tw, ptx + 6, pty - 3);
    let island = mean_rgb(&frame, gx, gy, gx + 16, gy + 16);
    println!(
        "O3 grass island rgb: {:.0}/{:.0}/{:.0}",
        island[0], island[1], island[2]
    );
    assert!(
        island[1] > island[0] * 1.05,
        "an isolated grass tile must still read green, got r {:.0} g {:.0}",
        island[0],
        island[1]
    );

    // (c) No glow square: the sand ring around the snow island must not end up
    // brighter than open sand (the old carry overshot the sand's own brightness).
    let (nx, ny) = tile_screen_origin(&tw, ptx + 6, pty + 3);
    let ring = mean_rgb(&frame, nx - 16, ny - 16, nx + 32, ny + 32);
    let open = mean_rgb(&frame, nx - 96, ny - 16, nx - 48, ny + 32);
    println!(
        "O3 ring luma {:.1} vs open sand {:.1}",
        luma(ring),
        luma(open)
    );
    assert!(
        luma(ring) < luma(open) * 1.06,
        "the carry ring must not glow brighter than open sand ({:.1} vs {:.1})",
        luma(ring),
        luma(open)
    );
}

/* ------------------- O6/O7: props stand on the surrounding ground ---------------- */

/// Fraction of a tile's pixels that read green-dominant (the old hardcoded grass
/// base under props).
fn green_frac(frame: &[i32], sx: i32, sy: i32) -> f64 {
    let mut green = 0;
    for y in sy..sy + 16 {
        for x in sx..sx + 16 {
            let p = frame[(x + y * screen::W) as usize];
            let (r, g, b) = ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF);
            if g > r + 12 && g > b + 12 {
                green += 1;
            }
        }
    }
    green as f64 / 256.0
}

/// ODDITIES O6 repro (staged): a gravestone and a tall-grass tuft on a dirt plot.
/// Neither may punch a grass-green square into the dirt: green pixels stay below
/// the tuft's own sprite coverage, nowhere near a full green base.
#[test]
fn o6_props_stand_on_dirt_plot() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);
    let mut tw = TestWorld::infinite().seed(9).name("odd_o6").build();
    tw.tick_n(8);
    for dy in -5..=5 {
        for dx in -8..=8 {
            tw.place("dirt", dx, dy);
        }
    }
    let (gx_t, gy_t) = tw.place("Grave stone", 3, 2);
    let (tx_t, ty_t) = tw.place("tall grass", -3, 2);
    pin_time(&mut tw, day_tick(0.375));
    tw.g.notifications.clear();
    let frame = tw.render();
    shot("o6_props_on_dirt", &frame);

    let (gx, gy) = tile_screen_origin(&tw, gx_t, gy_t);
    let (tx, ty) = tile_screen_origin(&tw, tx_t, ty_t);
    let grave_green = green_frac(&frame, gx, gy);
    let tuft_green = green_frac(&frame, tx, ty);
    println!("O6 green fraction: grave {grave_green:.2}, tuft {tuft_green:.2}");
    assert!(
        grave_green < 0.10,
        "gravestone stamps a green base onto the dirt plot ({grave_green:.2})"
    );
    // the tuft sprite itself is green; only a full green BASE square is a bug
    assert!(
        tuft_green < 0.45,
        "tall grass stamps a green base onto the dirt plot ({tuft_green:.2})"
    );
}

/// ODDITIES O7 repro (staged): a pine standing in open grass country must not
/// stamp its species-default snow square; a cactus stranded on grass must not
/// stamp a sand square.
#[test]
fn o7_flora_base_follows_local_ground() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);
    let mut tw = TestWorld::infinite().seed(9).name("odd_o7").build();
    tw.tick_n(8);
    for dy in -5..=5 {
        for dx in -8..=8 {
            tw.place("grass", dx, dy);
        }
    }
    let (px_t, py_t) = tw.place("Pine Tree", 3, 2);
    let (cx_t, cy_t) = tw.place("cactus", -3, 2);
    pin_time(&mut tw, day_tick(0.375));
    tw.g.notifications.clear();
    let frame = tw.render();
    shot("o7_flora_on_grass", &frame);

    // pine: no near-white (snow-base) pixels in its tile
    let (px, py) = tile_screen_origin(&tw, px_t, py_t);
    let mut snowy = 0;
    // cactus: no sand-yellow pixels in its tile
    let (cx, cy) = tile_screen_origin(&tw, cx_t, cy_t);
    let mut sandy = 0;
    for dy in 0..16 {
        for dx in 0..16 {
            let p = frame[(px + dx + (py + dy) * screen::W) as usize];
            let (r, g, b) = ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF);
            if r > 195 && g > 195 && b > 195 {
                snowy += 1;
            }
            let p = frame[(cx + dx + (cy + dy) * screen::W) as usize];
            let (r, g, b) = ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF);
            if r > 170 && g > 150 && b < 110 && r > b + 90 {
                sandy += 1;
            }
        }
    }
    println!("O7 pine snow-base pixels {snowy}, cactus sand-base pixels {sandy}");
    assert!(
        snowy < 12,
        "pine on grass country still stamps a snow square ({snowy} white pixels)"
    );
    assert!(
        sandy < 12,
        "cactus on grass still stamps a sand square ({sandy} yellow pixels)"
    );
}

/* --------------------- O8: drops float on water, no black box -------------------- */

/// ODDITIES O8 repro (staged): a fresh drop bouncing over open water. The old
/// render painted a full black sprite-copy under it; now it floats in a ripple
/// ring with the shadow suppressed. Near-black pixels in the drop's neighborhood
/// must stay at sprite-outline counts.
#[test]
fn o8_no_black_box_under_floating_drops() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);
    let mut tw = TestWorld::infinite().seed(9).name("odd_o8").build();
    tw.tick_n(8);
    for dy in -6..=6 {
        for dx in -9..=9 {
            tw.place("water", dx, dy);
        }
    }
    pin_time(&mut tw, day_tick(0.375));
    let lvl = tw.current_level;
    let (px, py) = tw.player_pos();
    let apple = fdoom::item::registry::get(&tw.g, "apple");
    fdoom::level::drop_item(&mut tw.g, lvl, px + 4 * 16, py, apple);
    tw.tick_n(3); // still mid-bounce: the old shadow copy sat fully exposed
    tw.g.notifications.clear();
    let frame = tw.render();
    shot("o8_drop_on_water", &frame);

    // locate the drop entity for an exact screen box
    let (ex, ey) =
        tw.g.entities
            .entities_on_level(lvl)
            .find(|e| matches!(e.kind, EntityKind::ItemEntity(_)))
            .map(|e| (e.c.x, e.c.y))
            .expect("drop exists");
    let (ptx, pty) = tw.player_pos();
    let sx = ex - (ptx - screen::W / 2);
    let sy = ey - (pty - (screen::H - 8) / 2);
    let mut dark = 0;
    for y in (sy - 10).max(0)..(sy + 10).min(screen::H) {
        for x in (sx - 10).max(0)..(sx + 10).min(screen::W) {
            let p = frame[(x + y * screen::W) as usize];
            let lum = 0.30 * ((p >> 16) & 0xFF) as f64
                + 0.59 * ((p >> 8) & 0xFF) as f64
                + 0.11 * (p & 0xFF) as f64;
            if lum < 22.0 {
                dark += 1;
            }
        }
    }
    println!("O8 near-black pixels around floating drop: {dark}");
    assert!(
        dark <= 12,
        "floating drop still casts a black sprite-copy shadow ({dark} near-black pixels)"
    );
}

/* ------------------- O9: precipitation identity is per-column -------------------- */

/// ODDITIES O9 repro: stand exactly on a snow-climate boundary during a precip
/// slice. With the fix, ONE screen shows rain streaks over the warm columns and
/// snow flecks over the cold columns; the A/B against the FX_PRECIP-disabled
/// frame isolates exactly the precipitation pixels.
#[test]
fn o9_two_precip_kinds_on_one_screen() {
    let _g = fx_lock();
    let mut tw = TestWorld::infinite().seed(9).name("odd_o9").build();
    tw.tick_n(4);
    let seed = tw.g.world_seed;
    // climate flip along the documented plains|tundra border row
    let y = 768;
    let flip = (240..500)
        .find(|&x| weather::snow_climate(seed, x, y) != weather::snow_climate(seed, x + 1, y))
        .expect("no climate flip on the scan row");
    tw.teleport(flip, y);
    tw.tick_n(8);
    // pin the clock onto a daytime precip slice (day 0 is always dry)
    let mut found = false;
    'outer: for day in 1..20 {
        for step in 0..60 {
            let t = day_tick(0.25) + step * 300;
            if t >= day_tick(0.70) {
                break;
            }
            if weather::schedule_intensity(seed, day, t) > 0.3 {
                tw.g.events.day_number = day;
                tw.g.set_time(t);
                found = true;
                break 'outer;
            }
        }
    }
    assert!(found, "no precip slice found in 20 days");
    assert!(!matches!(weather::precip(&tw.g), Precip::None));
    tw.g.notifications.clear();

    lighting::set_disabled_fx(lighting::FX_PRECIP);
    let base = tw.render();
    lighting::set_disabled_fx(0);
    let on = tw.render();
    shot("o9_precip_split", &on);

    let (ppx, _) = tw.player_pos();
    let x_scroll = ppx - screen::W / 2;
    let (mut warm, mut warm_blue, mut cold, mut cold_blue) = (0, 0, 0, 0);
    for y in 0..screen::H {
        for x in 0..screen::W {
            let i = (x + y * screen::W) as usize;
            if base[i] == on[i] {
                continue;
            }
            let dr = ((on[i] >> 16) & 0xFF) - ((base[i] >> 16) & 0xFF);
            let dg = ((on[i] >> 8) & 0xFF) - ((base[i] >> 8) & 0xFF);
            // rain adds (52,64,86)*k — green rises over red; snow flecks add
            // (a,a,a+10), so their green NEVER leads red
            let blue = dg > dr + 2;
            let wtx = (x + x_scroll) >> 4;
            if weather::snow_climate(seed, wtx, 768) {
                cold += 1;
                cold_blue += i32::from(blue);
            } else {
                warm += 1;
                warm_blue += i32::from(blue);
            }
        }
    }
    println!("O9 diff pixels: warm {warm} ({warm_blue} blue), cold {cold} ({cold_blue} blue)");
    assert!(
        warm >= 10 && cold >= 10,
        "both sides of the climate border must receive precipitation (warm {warm}, cold {cold})"
    );
    assert!(
        warm_blue * 2 > warm,
        "warm side should be dominated by blue rain streaks ({warm_blue}/{warm})"
    );
    assert!(
        cold_blue * 2 < cold,
        "cold side should be dominated by neutral snow flecks ({cold_blue}/{cold})"
    );
}

/* ------------- O16/O17: interior faces vary per tile (no quilt/ruling) ----------- */

/// Distinct 16x16 pixel patterns among the sampled staged-field interior tiles.
fn distinct_tiles(tw: &TestWorld, frame: &[i32], coords: &[(i32, i32)]) -> usize {
    let mut set = std::collections::HashSet::new();
    for &(tx, ty) in coords {
        let (sx, sy) = tile_screen_origin(tw, tx, ty);
        let mut tile = Vec::with_capacity(256);
        for y in sy..sy + 16 {
            for x in sx..sx + 16 {
                tile.push(frame[(x + y * screen::W) as usize]);
            }
        }
        set.insert(tile);
    }
    set.len()
}

/// ODDITIES O16: interior mountain rock rendered the same quilt block on every
/// tile. Sampled interior faces must now vary.
#[test]
fn o16_rock_interior_faces_vary() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);
    let mut tw = TestWorld::infinite().seed(9).name("odd_o16").build();
    tw.tick_n(8);
    for dy in -6..=6 {
        for dx in -9..=9 {
            if (dx, dy) != (0, 0) {
                tw.place("rock", dx, dy);
            }
        }
    }
    pin_time(&mut tw, day_tick(0.375));
    tw.g.notifications.clear();
    let frame = tw.render();
    shot("o16_rock_field", &frame);

    let (ptx, pty) = tw.player_tile();
    let coords: Vec<(i32, i32)> = (0..12)
        .map(|i| (ptx - 5 + i % 6 * 2, pty + 2 + i / 6))
        .collect();
    let distinct = distinct_tiles(&tw, &frame, &coords);
    println!("O16 distinct rock faces: {distinct}/12");
    assert!(
        distinct >= 6,
        "rock interior repeats as a quilt: only {distinct} distinct faces of 12"
    );
}

/// ODDITIES O17: desert sand rendered identical ripple rows per tile, ruling the
/// screen with unbroken lines. Sampled interior tiles must now vary.
#[test]
fn o17_sand_ripples_vary_per_tile() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);
    let mut tw = TestWorld::infinite().seed(9).name("odd_o17").build();
    tw.tick_n(8);
    tw.goto_biome(fdoom::level::infinite_gen::Biome::Desert);
    tw.tick_n(8);
    for dy in -6..=6 {
        for dx in -9..=9 {
            tw.place("sand", dx, dy);
        }
    }
    pin_time(&mut tw, day_tick(0.375));
    tw.g.notifications.clear();
    let frame = tw.render();
    shot("o17_sand_field", &frame);

    let (ptx, pty) = tw.player_tile();
    let coords: Vec<(i32, i32)> = (0..12)
        .map(|i| (ptx - 5 + i % 6 * 2, pty + 2 + i / 6))
        .collect();
    let distinct = distinct_tiles(&tw, &frame, &coords);
    println!("O17 distinct sand tiles: {distinct}/12");
    assert!(
        distinct >= 6,
        "sand ripples repeat in lockstep: only {distinct} distinct tiles of 12"
    );
}

/* --------------------- O18: day water reads as daytime water --------------------- */

/// ODDITIES O18: open water at noon read as a night sky (deep indigo). The body
/// palette now rides the ambient brightness — green channel comes up at noon —
/// while the night read keeps the classic dark indigo.
#[test]
fn o18_day_water_is_not_night_sky() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);
    let mut tw = TestWorld::infinite().seed(9).name("odd_o18").build();
    tw.tick_n(8);
    for dy in -6..=6 {
        for dx in -9..=9 {
            let t = if dx >= -2 { "water" } else { "grass" };
            tw.place(t, dx, dy);
        }
    }
    let (ptx, pty) = tw.player_tile();
    let (sx, sy) = tile_screen_origin(&tw, ptx + 2, pty + 2);

    pin_time(&mut tw, day_tick(0.375));
    tw.g.notifications.clear();
    let noon = tw.render();
    shot("o18_noon_pool", &noon);
    let day = mean_rgb(&noon, sx, sy, sx + 96, sy + 48);

    pin_time(&mut tw, day_tick(0.85));
    tw.g.notifications.clear();
    let night = tw.render();
    let dark = mean_rgb(&night, sx, sy, sx + 96, sy + 48);

    println!(
        "O18 water rgb noon {:.0}/{:.0}/{:.0}, night {:.0}/{:.0}/{:.0}",
        day[0], day[1], day[2], dark[0], dark[1], dark[2]
    );
    // noon: a real daytime blue (green channel well off the floor), clearly blue
    assert!(
        day[1] > day[0] * 1.2 && day[2] > day[1],
        "noon water still reads night-sky indigo (rgb {:.0}/{:.0}/{:.0})",
        day[0],
        day[1],
        day[2]
    );
    // night: stays dark and blue-dominant, far below the noon read
    assert!(
        luma(dark) < luma(day) * 0.5 && dark[2] > dark[1],
        "night water lost its dark read (rgb {:.0}/{:.0}/{:.0})",
        dark[0],
        dark[1],
        dark[2]
    );
}

/* --------------------------- natural-scene captures ------------------------------ */

/// Recapture the ODDITIES natural scenes (O2 mountain borders, O3 biome corners) so
/// before/after can be judged by eye. Assertion-light on purpose: natural terrain
/// shifts with worldgen; the staged tests above carry the hard guarantees.
#[test]
fn oddity_natural_scene_shots() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);
    let scenes: &[(&str, i64, i32, i32)] = &[
        ("o2_nb9_mountains_plains", 9, 49, 72),
        ("o2_nb9_mountains_desert", 9, 37, -48),
        ("o3_nb42_forest_tundra", 42, 487, 336),
        ("o3_nb9_savanna_desert", 9, 359, -168),
        // batch 2 (O6/O7/O16/O17): cemetery, plains|tundra border, field scenes
        ("o6_st9_cemetery", 9, 183, 153),
        ("o7_nb9_plains_tundra", 9, 363, 768),
        ("o16_field_mountains", 9, -96, 120),
        ("o17_field_desert", 9, 200, -192),
    ];
    for &(name, seed, tx, ty) in scenes {
        let mut tw = TestWorld::infinite()
            .seed(seed)
            .name(&format!("odd_{name}"))
            .build();
        tw.tick_n(4);
        tw.teleport(tx, ty);
        tw.tick_n(8);
        pin_time(&mut tw, day_tick(0.375));
        tw.g.notifications.clear();
        let frame = tw.render();
        shot(name, &frame);
    }
}
