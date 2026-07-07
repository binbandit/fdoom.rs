//! Lighting/atmosphere pass (`gfx::lighting`) — visual verification frames at fixed
//! day-clock times (dumped to target/verify at 3x for inspection), plus continuity,
//! darkness, radiance, aurora, and performance checks.

use fdoom::core::renderer::Renderer;
use fdoom::core::updater::DAY_LENGTH;
use fdoom::core::{events, game::Game};
use fdoom::gfx::{lighting, screen};
use fdoom::testutil::{TestWorld, find_biome, renderer, save_png, verify_path};

fn new_world(name: &str, seed: i64) -> Game {
    let mut g = TestWorld::infinite()
        .seed(seed)
        .name(&format!("lighting_{name}"))
        .build()
        .g;
    g.has_gui = true; // let the renderer draw in headless mode
    g
}

/// Dump a frame at 3x nearest-neighbor so the pixel-art dither is easy to inspect.
fn dump_png(name: &str, pixels: &[i32]) {
    save_png(
        verify_path(name),
        pixels,
        screen::W as usize,
        screen::H as usize,
        3,
    );
}

fn mean_channels(pixels: &[i32]) -> (f64, f64, f64) {
    let mut r = 0u64;
    let mut g = 0u64;
    let mut b = 0u64;
    for &p in pixels {
        r += ((p >> 16) & 0xff) as u64;
        g += ((p >> 8) & 0xff) as u64;
        b += (p & 0xff) as u64;
    }
    let n = pixels.len() as f64;
    (r as f64 / n, g as f64 / n, b as f64 / n)
}

fn luma(pixels: &[i32]) -> f64 {
    let (r, g, b) = mean_channels(pixels);
    0.30 * r + 0.59 * g + 0.11 * b
}

fn settle(g: &mut Game, ticks: usize) {
    for _ in 0..ticks {
        g.tick();
    }
}

fn render_at(g: &mut Game, r: &mut Renderer, tick: i32) -> Vec<i32> {
    g.set_time(tick);
    settle(g, 2);
    g.set_time(tick); // settle ticks advance the clock; pin it back
    r.render(g);
    r.screen.pixels.clone()
}

/* --------------------------- time-of-day grading frames --------------------------- */

#[test]
fn day_cycle_frames_and_ordering() {
    let mut g = new_world("cycle", 20260707);
    let mut r = renderer();
    settle(&mut g, 8); // stream chunks around spawn

    let day = DAY_LENGTH as f32;
    let shots: &[(&str, f32)] = &[
        ("dawn", 0.085),
        ("noon", 0.375),
        ("sunset_amber", 0.575),
        ("sunset_violet", 0.615),
        ("dusk", 0.68),
        ("night", 0.85),
    ];
    let mut lumas = std::collections::HashMap::new();
    let mut frames = std::collections::HashMap::new();
    for &(name, t) in shots {
        let px = render_at(&mut g, &mut r, (day * t) as i32);
        dump_png(&format!("light_{name}.png"), &px);
        lumas.insert(name, luma(&px));
        frames.insert(name, px);
    }

    // Brightness ordering across the day.
    assert!(lumas["noon"] > lumas["dawn"], "noon should outshine dawn");
    assert!(
        lumas["dawn"] > lumas["sunset_violet"],
        "dawn brighter than late sunset"
    );
    assert!(
        lumas["sunset_violet"] > lumas["night"],
        "sunset brighter than night"
    );
    assert!(
        lumas["night"] < lumas["noon"] * 0.55,
        "night must be substantially darker than day (got {} vs {})",
        lumas["night"],
        lumas["noon"]
    );

    // Hue checks: amber sunset is red-heavy, night is blue-heavy.
    let (ar, _, ab) = mean_channels(&frames["sunset_amber"]);
    assert!(
        ar > ab * 1.2,
        "amber sunset should read warm (r {ar}, b {ab})"
    );
    let (nr, _, nb) = mean_channels(&frames["night"]);
    assert!(nb > nr, "night should read cool/blue (r {nr}, b {nb})");
}

#[test]
fn ambient_is_continuous_no_pops() {
    // Walk the whole day in 30-tick steps; per-step gain deltas must stay tiny.
    let mut prev = lighting::surface_ambient(0);
    for tick in (30..=DAY_LENGTH).step_by(30) {
        let cur = lighting::surface_ambient(tick % DAY_LENGTH);
        for c in 0..3 {
            let d = (cur.gain[c] - prev.gain[c]).abs();
            assert!(
                d < 0.01,
                "ambient pop at tick {tick}: channel {c} jumped {d:.4}"
            );
        }
        prev = cur;
    }
    // The midnight wrap itself is seamless.
    let end = lighting::surface_ambient(DAY_LENGTH - 1);
    let start = lighting::surface_ambient(0);
    for c in 0..3 {
        assert!((end.gain[c] - start.gain[c]).abs() < 0.01, "midnight pop");
    }
}

/* --------------------------------- radiance pass ---------------------------------- */

#[test]
fn torch_lights_the_night() {
    let mut g = new_world("torch", 20260707);
    let mut r = renderer();
    settle(&mut g, 8);

    // Plain night first.
    let night = render_at(&mut g, &mut r, (DAY_LENGTH as f32 * 0.85) as i32);

    // Plant torches next to the player, then re-render the same night.
    let (ptx, pty) = {
        let p = g.player();
        (p.c.x >> 4, p.c.y >> 4)
    };
    for (dx, dy) in [(-2, 0), (2, -1), (0, 2)] {
        let on = g.tile_at(3, ptx + dx, pty + dy);
        let torch = g.tiles.get(&format!("torch {}", on.name));
        g.set_tile_default(3, ptx + dx, pty + dy, &torch);
    }
    let lit = render_at(&mut g, &mut r, (DAY_LENGTH as f32 * 0.85) as i32);
    dump_png("light_torch_night.png", &lit);

    assert!(
        luma(&lit) > luma(&night) + 1.0,
        "torches should brighten the night frame ({} vs {})",
        luma(&lit),
        luma(&night)
    );

    // The screen center (player + torches) must hold genuinely bright, warm pixels.
    let cx = screen::W / 2;
    let cy = screen::H / 2;
    let mut bright_warm = 0;
    for y in (cy - 40).max(0)..(cy + 40).min(screen::H) {
        for x in (cx - 40).max(0)..(cx + 40).min(screen::W) {
            let p = lit[(x + y * screen::W) as usize];
            let (pr, pb) = ((p >> 16) & 0xff, p & 0xff);
            if pr > 90 && pr > pb {
                bright_warm += 1;
            }
        }
    }
    assert!(
        bright_warm > 300,
        "expected a warm pool of torchlight at screen center, got {bright_warm} px"
    );
}

#[test]
fn caves_are_near_black_until_lit() {
    let mut g = new_world("cave", 20260707);
    for lvl in 0..=2 {
        let a = lighting::ambient_for(&g, lvl);
        assert!(
            a.brightness < 0.10,
            "cave level {lvl} ambient should be near-black, got {}",
            a.brightness
        );
    }
    // Surface at noon is identity-bright by comparison.
    g.set_time(DAY_LENGTH / 4 + DAY_LENGTH / 8);
    let a = lighting::ambient_for(&g, 3);
    assert!(
        a.brightness > 0.95,
        "noon surface should be full brightness"
    );

    // Render an actual cave frame: drop the player into the top cave layer, unlit,
    // then plant a torch beside them and see it push the darkness back.
    let mut r = renderer();
    g.player_mut().c.level = Some(2);
    g.current_level = 2;
    settle(&mut g, 8); // stream underground chunks
    r.render(&mut g);
    let dark = r.screen.pixels.clone();
    dump_png("light_cave_dark.png", &dark);

    let (ptx, pty) = {
        let p = g.player();
        (p.c.x >> 4, p.c.y >> 4)
    };
    let on = g.tile_at(2, ptx + 1, pty);
    let torch = g.tiles.get(&format!("torch {}", on.name));
    g.set_tile_default(2, ptx + 1, pty, &torch);
    r.render(&mut g);
    let lit = r.screen.pixels.clone();
    dump_png("light_cave_torch.png", &lit);

    // Ignore the HUD region (top-left frames + hearts row): sample the lower half.
    let lower = |px: &[i32]| luma(&px[(screen::W * screen::H / 2) as usize..]);
    assert!(
        lower(&dark) < 14.0,
        "unlit cave should be near-black, luma {}",
        lower(&dark)
    );
    assert!(
        lower(&lit) > lower(&dark),
        "a torch must lift cave darkness"
    );
}

/* ------------------------------- biome ground tint -------------------------------- */

#[test]
fn biome_ground_tint_shifts_the_palette() {
    use fdoom::level::infinite_gen::Biome;
    let mut g = new_world("biometint", 20260707);
    let mut r = renderer();
    let noon = DAY_LENGTH / 4 + DAY_LENGTH / 8;

    let mut shot = |g: &mut Game, r: &mut Renderer, biome, name: &str| {
        let (tx, ty) = find_biome(g.world_seed, biome);
        {
            let p = g.player_mut();
            p.c.x = tx * 16 + 8;
            p.c.y = ty * 16 + 8;
        }
        settle(g, 8); // stream chunks
        let px = render_at(g, r, noon);
        dump_png(&format!("light_biome_{name}.png"), &px);
        mean_channels(&px)
    };

    let (dr, _, db) = shot(&mut g, &mut r, Biome::Desert, "desert_noon");
    let (fr, _, fb) = shot(&mut g, &mut r, Biome::Forest, "forest_noon");

    // Desert ground leans warm (r over b boosted); forest leans cool (r pulled down).
    assert!(
        dr / db > fr / fb,
        "desert should read warmer than forest (desert r/b {:.3}, forest r/b {:.3})",
        dr / db,
        fr / fb
    );
}

/* ---------------------------------- event skies ----------------------------------- */

#[test]
fn aurora_tints_the_night_green() {
    let mut g = new_world("aurora", 20260707);
    let mut r = renderer();
    settle(&mut g, 8);

    let night = (DAY_LENGTH as f32 * 0.85) as i32;
    let plain = render_at(&mut g, &mut r, night);

    // Jump the session calendar to the next Aurora day (schedule is pure per seed).
    // Set day_number *after* all ticking: `events::tick` counts a backwards day-clock
    // jump as a day wrap, which would move the calendar off the aurora day.
    let day = (1..10_000)
        .find(|&d| events::event_for_day(g.world_seed, d) == Some(events::WorldEvent::Aurora))
        .expect("no aurora day in range");
    g.set_time(night);
    settle(&mut g, 2);
    g.set_time(night);
    g.events.day_number = day;
    assert!(events::aurora_active(&g), "aurora should be active");
    r.render(&mut g);
    let aurora = r.screen.pixels.clone();
    dump_png("light_aurora_night.png", &aurora);

    let (_, pg, _) = mean_channels(&plain);
    let (_, ag, _) = mean_channels(&aurora);
    assert!(
        ag > pg + 1.0,
        "aurora night should carry a green wash ({ag} vs {pg})"
    );
    assert!(ag < pg + 20.0, "aurora must stay subtle ({ag} vs {pg})");
}

/* ---------------------------------- performance ----------------------------------- */

#[test]
fn lighting_pass_is_fast() {
    let mut g = new_world("perf", 20260707);
    let mut r = renderer();
    settle(&mut g, 8);

    // Worst-ish case: night (full grade + stamp), torches near the player.
    let (ptx, pty) = {
        let p = g.player();
        (p.c.x >> 4, p.c.y >> 4)
    };
    for (dx, dy) in [(-3, 0), (3, 0), (0, -3), (0, 3)] {
        let on = g.tile_at(3, ptx + dx, pty + dy);
        let torch = g.tiles.get(&format!("torch {}", on.name));
        g.set_tile_default(3, ptx + dx, pty + dy, &torch);
    }
    g.set_time((DAY_LENGTH as f32 * 0.85) as i32);
    r.render(&mut g); // fill the frame once

    let base = r.screen.pixels.clone();
    let (px, py) = {
        let p = g.player();
        (p.c.x, p.c.y)
    };
    let x_scroll = px - screen::W / 2;
    let y_scroll = py - (screen::H - 8) / 2;

    let iters = 100;
    let mut total = std::time::Duration::ZERO;
    for _ in 0..iters {
        r.screen.pixels.copy_from_slice(&base);
        let t0 = std::time::Instant::now();
        lighting::render_pass(
            &mut r.screen,
            &mut r.light_screen,
            &g,
            3,
            x_scroll,
            y_scroll,
        );
        total += t0.elapsed();
    }
    let avg = total / iters;
    println!("lighting pass avg: {avg:?} over {iters} iters");
    // Budget: ~2ms release. Debug builds are far slower; keep a generous ceiling so
    // the test still catches an accidental O(n^2) regression.
    assert!(
        avg < std::time::Duration::from_millis(25),
        "lighting pass too slow: {avg:?}"
    );
}
