//! Visual-excellence wave (`gfx::lighting` + `gfx::ambience`): true A/B pairs per
//! effect (same frame, effect off/on — dumped to target/verify at 3x), pixel-presence
//! smoke asserts, two hero shots, and the pass performance ceiling.
//!
//! The `FX_*` toggles are process-global, so every test here serializes on one lock.
//! (Other test binaries are separate processes — they always run with everything on.)

use std::sync::{Mutex, MutexGuard};

use fdoom::core::updater::DAY_LENGTH;
use fdoom::gfx::{lighting, screen};
use fdoom::testutil::{TestWorld, save_png, verify_path};

static FX_LOCK: Mutex<()> = Mutex::new(());

fn fx_lock() -> MutexGuard<'static, ()> {
    FX_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn world(name: &str) -> TestWorld {
    let mut tw = TestWorld::infinite()
        .seed(20260707)
        .name(&format!("visfx_{name}"))
        .build();
    tw.tick_n(8); // stream chunks around spawn
    tw
}

fn dump3x(name: &str, pixels: &[i32]) {
    save_png(
        verify_path(name),
        pixels,
        screen::W as usize,
        screen::H as usize,
        3,
    );
}

fn day_tick(frac: f32) -> i32 {
    (DAY_LENGTH as f32 * frac) as i32
}

/// Pin the clock, let chunks/entities settle, pin it back (settling advances it).
fn pin_time(tw: &mut TestWorld, tick: i32) {
    tw.g.set_time(tick);
    tw.tick_n(2);
    tw.g.set_time(tick);
}

/// Render the same frame with `bit` disabled then enabled, dump both at 3x, return
/// `(before, after)`.
fn ab_frames(tw: &mut TestWorld, bit: u32, name: &str) -> (Vec<i32>, Vec<i32>) {
    tw.g.notifications.clear(); // the HUD notification timer must not skew the diff
    lighting::set_disabled_fx(bit);
    let before = tw.render();
    lighting::set_disabled_fx(0);
    let after = tw.render();
    dump3x(&format!("visfx_{name}_before.png"), &before);
    dump3x(&format!("visfx_{name}_after.png"), &after);
    (before, after)
}

fn diff_count(a: &[i32], b: &[i32]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

fn mean_luma(pixels: &[i32], x0: i32, y0: i32, x1: i32, y1: i32) -> f64 {
    let mut sum = 0.0f64;
    let mut n = 0.0f64;
    for y in y0.max(0)..y1.min(screen::H) {
        for x in x0.max(0)..x1.min(screen::W) {
            let p = pixels[(x + y * screen::W) as usize];
            sum += 0.30 * ((p >> 16) & 0xFF) as f64
                + 0.59 * ((p >> 8) & 0xFF) as f64
                + 0.11 * (p & 0xFF) as f64;
            n += 1.0;
        }
    }
    sum / n.max(1.0)
}

/* ------------------------------ seam color-carry -------------------------------- */

#[test]
fn seam_carry_bridges_ground_families() {
    let _g = fx_lock();
    let mut tw = world("seam");

    // A controlled freckle field: grass everywhere, isolated snow tiles + a snow
    // patch + a sand strip — the exact scene the old multiplier blend failed on.
    for dy in -6..=6 {
        for dx in -9..=9 {
            tw.place("grass", dx, dy);
        }
    }
    for (dx, dy) in [(3, -3), (5, 1), (2, 3), (7, -1), (4, 4), (6, 3)] {
        tw.place("snow", dx, dy);
    }
    for dy in -1..=1 {
        for dx in 5..=6 {
            tw.place("snow", dx, dy);
        }
    }
    for dy in -4..=4 {
        for dx in -6..=-5 {
            tw.place("sand", dx, dy);
        }
    }
    pin_time(&mut tw, day_tick(0.375)); // noon: the blend must read with no grade help

    let (before, after) = ab_frames(&mut tw, lighting::FX_SEAM_BLEND, "seam");
    let d = diff_count(&before, &after);
    assert!(d > 400, "seam carry should repaint border strips, diff {d}");

    // Snow must bleed white speckle *outside* its own tile: sample the 5-px grass
    // band just east of the snow freckle at player+(3,-3).
    let (px, py) = tw.player_pos();
    let (ptx, pty) = tw.player_tile();
    let bx = (ptx + 3) * 16 + 16 - (px - screen::W / 2);
    let by = (pty - 3) * 16 - (py - (screen::H - 8) / 2);
    let whitened = |pxs: &[i32]| {
        let mut n = 0;
        for y in by..by + 16 {
            for x in bx..bx + 5 {
                let p = pxs[(x + y * screen::W) as usize];
                if ((p >> 16) & 0xFF) > 150 && (p & 0xFF) > 170 {
                    n += 1;
                }
            }
        }
        n
    };
    let (wb, wa) = (whitened(&before), whitened(&after));
    assert!(
        wa >= wb + 8,
        "expected white carry speckle on the grass side of the seam ({wb} -> {wa})"
    );
}

/* ---------------------------- golden-hour long shadows -------------------------- */

#[test]
fn golden_hour_windows_and_directions() {
    // Pure function checks: morning throws west, evening east, noon/night nothing.
    let m = fdoom::gfx::ambience::golden_hour(day_tick(0.085));
    assert_eq!(m.map(|(d, _)| d), Some(-1), "dawn shadows point west");
    let e = fdoom::gfx::ambience::golden_hour(day_tick(0.575));
    assert_eq!(e.map(|(d, _)| d), Some(1), "dusk shadows point east");
    assert_eq!(fdoom::gfx::ambience::golden_hour(day_tick(0.375)), None);
    assert_eq!(fdoom::gfx::ambience::golden_hour(day_tick(0.85)), None);
}

#[test]
fn long_shadows_stretch_at_sunset() {
    let _g = fx_lock();
    let mut tw = world("longshadow");
    for dy in -6..=6 {
        for dx in -9..=9 {
            tw.place("grass", dx, dy);
        }
    }
    tw.place("tree", -3, -2);
    tw.place("tree", -3, 2);
    let (wx, wy) = tw.place("Stone Wall", 3, 0);
    pin_time(&mut tw, day_tick(0.575)); // amber sunset peak — full-length shadows

    let (before, after) = ab_frames(&mut tw, lighting::FX_LONG_SHADOWS, "longshadow");
    let d = diff_count(&before, &after);
    assert!(d > 150, "sunset shadows should darken strips, diff {d}");

    // The wall's strip lies east of it (evening sun sits west).
    let (px, py) = tw.player_pos();
    let sx = wx * 16 + 16 - (px - screen::W / 2);
    let sy = wy * 16 - (py - (screen::H - 8) / 2);
    let lb = mean_luma(&before, sx, sy, sx + 16, sy + 16);
    let la = mean_luma(&after, sx, sy, sx + 16, sy + 16);
    assert!(
        la < lb - 2.0,
        "east of the wall should darken at sunset ({lb:.1} -> {la:.1})"
    );
    // ...and the tile west of the wall stays untouched by it.
    let lwb = mean_luma(&before, sx - 32, sy, sx - 16, sy + 16);
    let lwa = mean_luma(&after, sx - 32, sy, sx - 16, sy + 16);
    assert!(
        (lwb - lwa).abs() < 1.0,
        "west of the wall must not catch its evening shadow ({lwb:.1} -> {lwa:.1})"
    );
}

/* ------------------------------- contact shadows -------------------------------- */

#[test]
fn contact_shadow_grounds_the_player() {
    let _g = fx_lock();
    let mut tw = world("contact");
    for dy in -6..=6 {
        for dx in -9..=9 {
            tw.place("grass", dx, dy);
        }
    }
    pin_time(&mut tw, day_tick(0.375));

    let (before, after) = ab_frames(&mut tw, lighting::FX_CONTACT_SHADOWS, "contact");
    // The ellipse hides partly under the sprite; the visible part is small but real.
    let d = diff_count(&before, &after);
    assert!(
        d >= 6,
        "contact shadow should show under the player, diff {d}"
    );

    // Changed pixels cluster at the player's feet (screen center, rows +4/+5).
    let fx = screen::W / 2 - 1;
    let fy = (screen::H - 8) / 2;
    let mut near = 0;
    for y in (fy + 2)..(fy + 8) {
        for x in (fx - 6)..(fx + 6) {
            let i = (x + y * screen::W) as usize;
            if before[i] != after[i] {
                near += 1;
            }
        }
    }
    assert!(
        near >= 4,
        "expected shadow pixels by the feet, found {near} (total diff {d})"
    );
}

/* ------------------------------ night emitter halo ------------------------------ */

#[test]
fn night_halo_rings_strong_emitters() {
    let _g = fx_lock();
    let mut tw = world("halo");
    let (ptx, pty) = tw.player_tile();
    for (dx, dy) in [(-3, 0), (3, -1), (0, 3)] {
        let on = tw.tile_at(3, ptx + dx, pty + dy);
        let torch = tw.g.tiles.get(&format!("torch {}", on.name));
        tw.g.set_tile_default(3, ptx + dx, pty + dy, &torch);
    }
    pin_time(&mut tw, day_tick(0.85)); // deep night

    let (before, after) = ab_frames(&mut tw, lighting::FX_EMITTER_HALO, "halo");
    let d = diff_count(&before, &after);
    assert!(
        d > 120,
        "halo ring should add a dither band around torches, diff {d}"
    );
    // The halo only ever brightens (it lifts the light floor).
    let brighter = before
        .iter()
        .zip(&after)
        .all(|(&b, &a)| ((a >> 16) & 0xFF) >= ((b >> 16) & 0xFF));
    assert!(brighter, "a halo must never darken a pixel");
}

/* -------------------------------- torch breathing ------------------------------- */

#[test]
fn torch_breathing_moves_the_light_edge() {
    let _g = fx_lock();
    let mut tw = world("breath");
    let (ptx, pty) = tw.player_tile();
    let on = tw.tile_at(3, ptx + 2, pty);
    let torch = tw.g.tiles.get(&format!("torch {}", on.name));
    tw.g.set_tile_default(3, ptx + 2, pty, &torch);
    pin_time(&mut tw, day_tick(0.85));

    let (px, py) = tw.player_pos();
    let x_scroll = px - screen::W / 2;
    let y_scroll = py - (screen::H - 8) / 2;
    let mut r = fdoom::testutil::renderer();

    // One breath step apart (16 ticks), the stamped light buffer must move...
    lighting::set_disabled_fx(0);
    lighting::stamp_emitters(&mut r.light_screen, &tw.g, 3, x_scroll, y_scroll);
    let l0 = r.light_screen.pixels.clone();
    tw.g.game_time += 16;
    lighting::stamp_emitters(&mut r.light_screen, &tw.g, 3, x_scroll, y_scroll);
    let l1 = r.light_screen.pixels.clone();
    assert!(
        diff_count(&l0, &l1) > 50,
        "flame light should breathe across a wave step"
    );

    // ...and hold perfectly still with the effect off.
    lighting::set_disabled_fx(lighting::FX_TORCH_BREATHING);
    lighting::stamp_emitters(&mut r.light_screen, &tw.g, 3, x_scroll, y_scroll);
    let s0 = r.light_screen.pixels.clone();
    tw.g.game_time += 16;
    lighting::stamp_emitters(&mut r.light_screen, &tw.g, 3, x_scroll, y_scroll);
    let s1 = r.light_screen.pixels.clone();
    lighting::set_disabled_fx(0);
    assert_eq!(diff_count(&s0, &s1), 0, "steady light must not breathe");
}

/* -------------------------------- water glitter --------------------------------- */

#[test]
fn sun_and_moon_glitter_on_water() {
    let _g = fx_lock();
    let mut tw = world("glitter");
    for dy in -5..=4 {
        for dx in 1..=9 {
            tw.place("water", dx, dy);
        }
    }
    pin_time(&mut tw, day_tick(0.36)); // late morning sun

    let (before, after) = ab_frames(&mut tw, lighting::FX_WATER_GLITTER, "glitter_day");
    let d = diff_count(&before, &after);
    assert!(
        (6..2000).contains(&d),
        "day glitter should sprinkle a few dozen glints, diff {d}"
    );

    pin_time(&mut tw, day_tick(0.85)); // clear night: cool moon glitter, sparser
    let (nb, na) = ab_frames(&mut tw, lighting::FX_WATER_GLITTER, "glitter_night");
    let dn = diff_count(&nb, &na);
    assert!(
        (2..800).contains(&dn),
        "moon glitter should be present but sparse, diff {dn}"
    );
    assert!(dn < d, "moonlight must glitter less than sun ({dn} vs {d})");
}

/* -------------------------------- heat shimmer ---------------------------------- */

#[test]
fn lava_rows_shimmer() {
    let _g = fx_lock();
    let mut tw = world("shimmer");
    for dy in -2..=2 {
        for dx in 2..=7 {
            tw.place("lava", dx, dy);
        }
    }
    pin_time(&mut tw, day_tick(0.85)); // shimmer is time-of-day independent for lava

    let (before, after) = ab_frames(&mut tw, lighting::FX_HEAT_SHIMMER, "shimmer");
    let d = diff_count(&before, &after);
    assert!(d > 100, "lava rows should wobble, diff {d}");

    // Rows far from any lava (top HUD band aside, the far west column) stay put.
    let (px, py) = tw.player_pos();
    let lx0 = ((tw.player_tile().0 + 2) * 16) - (px - screen::W / 2);
    for y in 0..screen::H {
        for x in 0..lx0.min(screen::W) - 18 {
            let i = (x + y * screen::W) as usize;
            assert_eq!(
                before[i], after[i],
                "shimmer leaked west of the lava at ({x}, {y})"
            );
        }
    }
    let _ = py;
}

/* ------------------------------- drifting motes --------------------------------- */

#[test]
fn leaves_drift_over_forest() {
    let _g = fx_lock();
    let mut tw = world("motes");
    tw.goto_biome(fdoom::level::infinite_gen::Biome::Forest);
    pin_time(&mut tw, day_tick(0.375));

    // Accumulate a few spaced frames: single-frame density is deliberately tiny.
    lighting::set_disabled_fx(lighting::FX_MOTES);
    tw.g.notifications.clear();
    let before = tw.render();
    lighting::set_disabled_fx(0);
    let mut total = 0usize;
    let mut first: Option<Vec<i32>> = None;
    for i in 0..4 {
        let after = tw.render();
        total += diff_count(&before, &after);
        if i == 0 {
            first = Some(after);
        }
        tw.g.game_time += 12;
    }
    let after = first.unwrap();
    dump3x("visfx_motes_before.png", &before);
    dump3x("visfx_motes_after.png", &after);
    assert!(
        total >= 4,
        "expected at least a couple of leaf pixels across frames, got {total}"
    );
    assert!(
        diff_count(&before, &after) < 120,
        "mote density must stay tiny (a handful of 2-px leaves)"
    );
}

/* -------------------------------- mine depth fog -------------------------------- */

#[test]
fn depth_fog_deepens_beyond_the_lit_pool() {
    let _g = fx_lock();
    let mut tw = world("fog");
    tw.g.player_mut().c.level = Some(2);
    tw.g.current_level = 2;
    tw.tick_n(8); // stream underground chunks

    let (before, after) = ab_frames(&mut tw, lighting::FX_DEPTH_FOG, "fog");

    // Screen corners (far from the player's cave glow) sink into the deep band...
    let mut corner_b = 0.0;
    let mut corner_a = 0.0;
    for (x0, y0) in [
        (0, screen::H - 36),
        (screen::W - 36, screen::H - 36),
        (screen::W - 36, 40),
    ] {
        corner_b += mean_luma(&before, x0, y0, x0 + 36, y0 + 36);
        corner_a += mean_luma(&after, x0, y0, x0 + 36, y0 + 36);
    }
    assert!(
        corner_a < corner_b * 0.75,
        "cave corners should deepen under fog ({corner_b:.1} -> {corner_a:.1})"
    );

    // ...while the pool right around the player keeps its normal ambient.
    let cx = screen::W / 2;
    let cy = (screen::H - 8) / 2;
    let cb = mean_luma(&before, cx - 10, cy - 10, cx + 10, cy + 10);
    let ca = mean_luma(&after, cx - 10, cy - 10, cx + 10, cy + 10);
    assert!(
        (cb - ca).abs() < cb * 0.05 + 0.5,
        "the lit pool must keep its floor ({cb:.2} -> {ca:.2})"
    );
}

/* ---------------------------------- hero shots ----------------------------------- */

#[test]
fn hero_shots() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);

    // Sunset lakeshore: forest edge, water east — long shadows + glitter + leaves.
    let mut tw = world("hero_sunset");
    tw.goto_biome(fdoom::level::infinite_gen::Biome::Forest);
    for dy in -5..=5 {
        for dx in 3..=10 {
            tw.place("water", dx, dy);
        }
    }
    pin_time(&mut tw, day_tick(0.575));
    tw.g.notifications.clear();
    let px = tw.render();
    dump3x("visfx_hero_sunset.png", &px);

    // Torchlit night: three planted torches — halos, breathing, deep blue ambient.
    let mut tw = world("hero_night");
    let (ptx, pty) = tw.player_tile();
    for (dx, dy) in [(-3, 1), (2, -2), (1, 3)] {
        let on = tw.tile_at(3, ptx + dx, pty + dy);
        let torch = tw.g.tiles.get(&format!("torch {}", on.name));
        tw.g.set_tile_default(3, ptx + dx, pty + dy, &torch);
    }
    pin_time(&mut tw, day_tick(0.85));
    tw.g.notifications.clear();
    let px = tw.render();
    dump3x("visfx_hero_night.png", &px);
}

/* ---------------------------------- performance ---------------------------------- */

#[test]
fn visual_pass_stays_inside_budget() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);

    // Scenario worst cases: (night torches: halo+breath), (sunset shore: shadows +
    // glitter + seams + motes), (cave: fog), (noon desert lava shore: shimmer).
    let mut worst = std::time::Duration::ZERO;
    let mut worst_name = "";
    let scenarios: &[(&str, f32, bool)] = &[
        ("night_torches", 0.85, false),
        ("sunset_shore", 0.575, false),
        ("cave", 0.375, true),
        ("noon_mixed", 0.375, false),
    ];
    for &(name, frac, cave) in scenarios {
        let mut tw = world(&format!("perf_{name}"));
        let (ptx, pty) = tw.player_tile();
        match name {
            "night_torches" => {
                for (dx, dy) in [(-3, 0), (3, 0), (0, -3), (0, 3)] {
                    let on = tw.tile_at(3, ptx + dx, pty + dy);
                    let torch = tw.g.tiles.get(&format!("torch {}", on.name));
                    tw.g.set_tile_default(3, ptx + dx, pty + dy, &torch);
                }
            }
            "sunset_shore" | "noon_mixed" => {
                for dy in -5..=4 {
                    for dx in 2..=9 {
                        tw.place("water", dx, dy);
                    }
                }
                for dy in -3..=3 {
                    for dx in -6..=-4 {
                        tw.place(if name == "noon_mixed" { "lava" } else { "snow" }, dx, dy);
                    }
                }
            }
            _ => {}
        }
        if cave {
            tw.g.player_mut().c.level = Some(2);
            tw.g.current_level = 2;
            tw.tick_n(8);
        }
        pin_time(&mut tw, day_tick(frac));

        let mut r = fdoom::testutil::renderer();
        tw.g.has_gui = true;
        r.render(&mut tw.g); // fill the frame once
        let base = r.screen.pixels.clone();
        let (px, py) = tw.player_pos();
        let lvl = tw.g.current_level;
        let x_scroll = px - screen::W / 2;
        let y_scroll = py - (screen::H - 8) / 2;

        let iters = 60;
        let mut total = std::time::Duration::ZERO;
        for _ in 0..iters {
            r.screen.pixels.copy_from_slice(&base);
            let t0 = std::time::Instant::now();
            fdoom::gfx::ambience::contact_shadows(&mut r.screen, &tw.g, lvl, x_scroll, y_scroll);
            lighting::render_pass(
                &mut r.screen,
                &mut r.light_screen,
                &tw.g,
                lvl,
                x_scroll,
                y_scroll,
            );
            total += t0.elapsed();
        }
        let avg = total / iters;
        println!("visual pass [{name}]: {avg:?} avg over {iters} iters");
        if avg > worst {
            worst = avg;
            worst_name = name;
        }
    }
    println!("visual pass worst case: {worst_name} at {worst:?}");

    // Debug builds run far slower; this ceiling only catches accidental blow-ups.
    assert!(
        worst < std::time::Duration::from_millis(25),
        "visual pass too slow ({worst_name}: {worst:?})"
    );
    // Release profile: the real budget — the whole pass under 400µs worst case.
    #[cfg(not(debug_assertions))]
    assert!(
        worst < std::time::Duration::from_micros(400),
        "release visual pass over budget ({worst_name}: {worst:?})"
    );
}
