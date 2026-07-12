//! Regression coverage for the top visual-coherence oddities (docs/ODDITIES.md):
//!
//! - O1: emitter light pools must read continuously across ground seams.
//! - O2: boulder/crag boundary cells must sit on the real ground, not a flat backing.
//! - O3: the biome ground blend must never flip a tile's hue family.
//!
//! Each test stages the documented repro scene, dumps 1x + 6x screenshots under
//! `target/verify/oddities_fix/`, and asserts the pixel-level property that was
//! broken. FX toggles are process-global, so tests that touch them share a lock.

use std::sync::{Mutex, MutexGuard};

use fdoom::core::updater::DAY_LENGTH;
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
