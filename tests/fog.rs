//! Ambient fog (`core::weather` fog schedule + `gfx::lighting::fog_grade` +
//! `gfx::ambience::mist_patches`): schedule purity and frequency bands, regional
//! moisture shaping (marsh vs desert), mist/haze tint separation, the density cap
//! vs the Whisper Fog event floor, the foggy-dawn cue, render smoke, budget, and
//! the hero screenshots.

use std::sync::{Mutex, MutexGuard};

use fdoom::core::events::{self, WorldEvent};
use fdoom::core::updater::DAY_LENGTH;
use fdoom::core::weather::{self, FogSample};
use fdoom::gfx::{lighting, screen};
use fdoom::level::infinite_gen::Biome;
use fdoom::testutil::{TestWorld, find_biome, renderer, save_png, verify_path};

const SEED: i64 = 20260707;

/// Render-touching tests serialize on one lock (the `FX_*` toggles are
/// process-global, and parallel renders would skew the budget measurement).
static FX_LOCK: Mutex<()> = Mutex::new(());

fn fx_lock() -> MutexGuard<'static, ()> {
    FX_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn day_tick(frac: f32) -> i32 {
    (DAY_LENGTH as f32 * frac) as i32
}

/// First day >= `from` matching the predicate.
fn find_day(from: i32, pred: impl Fn(i32) -> bool) -> i32 {
    (from..from + 10_000)
        .find(|&d| pred(d))
        .expect("no matching day in 10k days")
}

/// Pin the day clock (weather.rs test idiom): jump, run one real tick through it,
/// then set the day and drop stray cues.
fn pin_clock(tw: &mut TestWorld, day: i32, tick: i32) {
    tw.g.set_time(tick - 1);
    tw.tick_n(1);
    assert_eq!(tw.g.tick_count, tick, "clock failed to pin");
    tw.g.events.day_number = day;
    tw.g.notifications.clear();
}

fn world(name: &str) -> TestWorld {
    let mut tw = TestWorld::infinite()
        .seed(SEED)
        .name(&format!("fog_{name}"))
        .build();
    tw.tick_n(8);
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

/* ----------------------------------- schedule ----------------------------------- */

#[test]
fn schedule_is_pure_and_deterministic() {
    let (mx, my) = find_biome(SEED, Biome::Marsh);
    for day in 1..=8 {
        for step in 0..24 {
            let t = step * (DAY_LENGTH / 24);
            let a = weather::fog_sample(SEED, day, t, mx, my);
            let b = weather::fog_sample(SEED, day, t, mx, my);
            assert_eq!(a, b, "fog sample must be pure (day {day}, tick {t})");
        }
    }
    // the split bases/spatial fast path must agree with the one-shot read
    let day = find_day(1, |d| weather::mist_day(SEED, d));
    let t = day_tick(0.06);
    let bases = weather::mist_bases(SEED, day, t);
    assert_eq!(
        weather::mist_from(&bases, SEED, mx, my),
        weather::mist_at(SEED, day, t, mx, my)
    );
    // different seeds diverge somewhere in a short window
    assert!(
        (1..40).any(|d| weather::mist_day(SEED, d) != weather::mist_day(SEED ^ 0x5A5A, d)),
        "schedule ignores the seed"
    );
}

#[test]
fn fog_day_frequencies_hold_plausible_bands() {
    let seeds = [SEED, 42, -987654321];
    let days_per_seed = 400;
    let (mut mist, mut haze, mut bank) = (0, 0, 0);
    for &s in &seeds {
        for d in 1..=days_per_seed {
            mist += weather::mist_day(s, d) as i32;
            haze += weather::haze_day(s, d) as i32;
            bank += weather::bank_day(s, d) as i32;
        }
        // fresh sessions start clear, same convention as rain/events
        assert!(!weather::mist_day(s, 0) && !weather::haze_day(s, 0) && !weather::bank_day(s, 0));
    }
    let n = (seeds.len() as i32 * days_per_seed) as f64;
    let (fm, fh, fb) = (mist as f64 / n, haze as f64 / n, bank as f64 / n);
    assert!((0.34..=0.46).contains(&fm), "mist-day rate {fm}");
    assert!((0.10..=0.20).contains(&fh), "haze-day rate {fh}");
    assert!((0.29..=0.41).contains(&fb), "bank-day rate {fb}");
}

#[test]
fn marsh_mist_denser_than_desert() {
    let (mx, my) = find_biome(SEED, Biome::Marsh);
    let (dx, dy) = find_biome(SEED, Biome::Desert);
    let day = find_day(1, |d| weather::mist_day(SEED, d));
    let t = day_tick(0.06); // hold plateau of the mist window

    let marsh = weather::mist_at(SEED, day, t, mx, my);
    let desert = weather::mist_at(SEED, day, t, dx, dy);
    assert!(
        marsh > 0.15,
        "marsh must carry real mist on a mist day, got {marsh}"
    );
    assert!(
        desert < marsh / 2.0,
        "desert must stay far drier than marsh ({desert} vs {marsh})"
    );
    // desert interior (away from any shoreline) reads bone dry
    assert!(
        weather::fog_moisture(SEED, dx, dy) < 0.4,
        "desert moisture unexpectedly high"
    );

    // burn-off: gone by mid-morning, silently
    assert_eq!(weather::mist_at(SEED, day, day_tick(0.20), mx, my), 0.0);
    // and the pre-dawn instant of the window is still clear
    assert_eq!(weather::mist_at(SEED, day, 0, mx, my), 0.0);
}

#[test]
fn haze_is_warm_and_mist_is_cool_and_windows_are_disjoint() {
    let mist_day = find_day(1, |d| weather::mist_day(SEED, d));
    let haze_day = find_day(1, |d| weather::haze_day(SEED, d));
    let (mx, my) = find_biome(SEED, Biome::Marsh);

    // disjoint windows: no mist at golden hour, no haze at dawn
    assert_eq!(
        weather::mist_at(SEED, mist_day, day_tick(0.55), mx, my),
        0.0
    );
    assert_eq!(
        weather::haze_at(SEED, haze_day, day_tick(0.06), mx, my),
        0.0
    );
    let h = weather::haze_at(SEED, haze_day, day_tick(0.52), mx, my);
    assert!(h > 0.05, "haze day must haze at late afternoon, got {h}");
    assert!(h <= weather::HAZE_MAX + 1e-4);

    // tint separation through the grade: mist washes cool, haze washes warm
    let neutral = fdoom::gfx::lighting::surface_ambient(day_tick(0.30));
    let misty = lighting::fog_grade(
        neutral,
        &FogSample {
            mist: 0.4,
            haze: 0.0,
        },
    );
    let hazy = lighting::fog_grade(
        neutral,
        &FogSample {
            mist: 0.0,
            haze: 0.25,
        },
    );
    assert!(
        misty.wash[2] > misty.wash[0],
        "mist wash must lean blue: {:?}",
        misty.wash
    );
    assert!(
        hazy.wash[0] > hazy.wash[2] * 3.0,
        "haze wash must lean strongly warm: {:?}",
        hazy.wash
    );
    assert!(
        hazy.tint[0] > hazy.tint[2],
        "haze tint must warm the frame: {:?}",
        hazy.tint
    );
}

/* ------------------------------ cap vs Whisper Fog ------------------------------- */

#[test]
fn ambient_density_capped_well_below_whisper_floor() {
    // The constant relation the whole design hangs on: the rare event owns the top
    // of the scale, with clear air between.
    const {
        assert!(weather::AMBIENT_FOG_MAX + 0.25 <= weather::WHISPER_FOG_FLOOR);
        assert!(weather::HAZE_MAX < weather::AMBIENT_FOG_MAX);
    }

    let spots = [
        find_biome(SEED, Biome::Marsh),
        find_biome(SEED, Biome::Desert),
        find_biome(SEED, Biome::Plains),
        find_biome(SEED, Biome::Beach),
    ];
    for day in 1..=60 {
        for step in 0..36 {
            let t = step * (DAY_LENGTH / 36);
            for &(x, y) in &spots {
                let s = weather::fog_sample(SEED, day, t, x, y);
                assert!(
                    s.mist <= weather::AMBIENT_FOG_MAX + 1e-4,
                    "mist over cap: {} (day {day} t {t})",
                    s.mist
                );
                assert!(
                    s.haze <= weather::HAZE_MAX + 1e-4,
                    "haze over cap: {} (day {day} t {t})",
                    s.haze
                );
            }
        }
    }
}

#[test]
fn whisper_fog_event_owns_the_top_of_the_scale() {
    let mut tw = world("whisper");
    tw.g.events.date_override = Some((6, 15)); // season-free calendar
    let day = find_day(1, |d| {
        events::event_for_day(SEED, d) == Some(WorldEvent::WhisperFog)
    });
    pin_clock(&mut tw, day, day_tick(0.80)); // deep night of the event

    let (mx, my) = find_biome(SEED, Biome::Marsh);
    let (px, py) = find_biome(SEED, Biome::Plains);
    let marsh = weather::fog_density(&tw.g, mx, my);
    let plains = weather::fog_density(&tw.g, px, py);
    assert!(
        marsh >= weather::WHISPER_FOG_FLOOR,
        "whisper night marsh must report the event floor, got {marsh}"
    );
    assert!(
        plains <= weather::AMBIENT_FOG_MAX + 1e-4,
        "outside the marsh the event adds nothing, got {plains}"
    );
}

/* ------------------------------------- cue --------------------------------------- */

#[test]
fn foggy_dawn_cue_fires_once_and_burnoff_is_silent() {
    let mut tw = world("cue");
    let (mx, my) = find_biome(SEED, Biome::Marsh);
    tw.teleport(mx, my);
    tw.tick_n(4); // stream the marsh chunks
    let day = find_day(1, |d| {
        weather::mist_day(SEED, d) && !weather::haze_day(SEED, d)
    });

    // the pure schedule tells us exactly where the ramp crosses the cue threshold
    let cross = (1..DAY_LENGTH / 4)
        .find(|&t| {
            weather::mist_at(SEED, day, t - 1, mx, my) < weather::FOG_CUE_THRESHOLD
                && weather::mist_at(SEED, day, t, mx, my) >= weather::FOG_CUE_THRESHOLD
        })
        .expect("mist day must cross the cue threshold at a marsh dawn");

    pin_clock(&mut tw, day, cross - 5);
    tw.tick_n(10);
    let hits =
        tw.g.notifications
            .iter()
            .filter(|n| n.contains("Mist hangs"))
            .count();
    assert_eq!(hits, 1, "dawn cue must fire exactly once: {hits}");

    // burn-off: crossing back down through the threshold stays silent
    let fade = (cross..DAY_LENGTH / 4)
        .find(|&t| weather::mist_at(SEED, day, t, mx, my) < weather::FOG_CUE_THRESHOLD)
        .expect("mist must burn off before mid-morning");
    pin_clock(&mut tw, day, fade - 5);
    tw.tick_n(10);
    assert!(
        !tw.g.notifications.iter().any(|n| n.contains("Mist")),
        "burn-off must be silent: {:?}",
        tw.g.notifications
    );
}

/* -------------------------------- render + budget -------------------------------- */

/// A marsh dawn on a mist day, campfire crackling near the player — the fog system's
/// worst frame (per-tile moisture reads + full-screen patches + emitter).
fn misty_marsh(name: &str) -> (TestWorld, i32) {
    let mut tw = world(name);
    let (mx, my) = find_biome(SEED, Biome::Marsh);
    tw.teleport(mx, my);
    tw.tick_n(8);
    // a misty morning with dry dawn slices, so rain streaks don't muddy the frame
    let day = find_day(1, |d| {
        weather::mist_day(SEED, d)
            && weather::bank_day(SEED, d)
            && !weather::slice_raining(SEED, d, 0)
            && !weather::slice_raining(SEED, d, 1)
    });

    // campfire two tiles east (fire.rs idiom)
    let (ptx, pty) = tw.player_tile();
    let lvl = tw.g.current_level;
    let e = fdoom::entity::furniture::campfire::new();
    tw.g.level_mut(lvl).add_at(e, ptx + 2, pty - 1, true, lvl);
    tw.tick_n(1);
    (tw, day)
}

#[test]
fn mist_renders_and_stays_inside_budget() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);
    let (mut tw, day) = misty_marsh("budget");
    pin_clock(&mut tw, day, day_tick(0.06));

    // A/B smoke: the effect must actually draw
    lighting::set_disabled_fx(lighting::FX_AMBIENT_FOG);
    let before = tw.render();
    lighting::set_disabled_fx(0);
    let after = tw.render();
    let diff = before.iter().zip(&after).filter(|(a, b)| a != b).count();
    assert!(
        diff > 3000,
        "ambient fog barely draws on a misty marsh dawn: {diff} px"
    );

    // Budget: the whole visual pass on the misty frame, same harness and ceilings
    // as tests/visuals.rs (400us release / 25ms debug safety net).
    let mut r = renderer();
    tw.g.has_gui = true;
    r.render(&mut tw.g);
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
    println!("visual pass [misty_marsh_dawn]: {avg:?} avg over {iters} iters");
    assert!(
        avg < std::time::Duration::from_millis(25),
        "misty visual pass too slow: {avg:?}"
    );
    #[cfg(not(debug_assertions))]
    assert!(
        avg < std::time::Duration::from_micros(400),
        "release misty visual pass over budget: {avg:?}"
    );
}

/* --------------------------------- hero shots ------------------------------------ */

#[test]
fn fog_screenshots() {
    let _g = fx_lock();
    lighting::set_disabled_fx(0);
    let (mut tw, day) = misty_marsh("shots");

    // fireflies wander at night; park a swarm by the campfire, then wind the clock
    // back to dawn without ticking so it survives for the frame
    let (ptx, pty) = tw.player_tile();
    let lvl = tw.g.current_level;
    pin_clock(&mut tw, day, day_tick(0.80));
    let swarm = fdoom::entity::fireflies::new(&mut tw.g.random);
    tw.g.level_mut(lvl)
        .add_at(swarm, ptx + 3, pty + 1, true, lvl);
    tw.tick_n(1);
    tw.g.events.day_number = day;

    // 1. money shot: misty marsh dawn, campfire + fireflies glowing through
    tw.g.set_time(day_tick(0.06));
    tw.g.notifications.clear();
    dump3x("fog_dawn_marsh.png", &tw.render());

    // 2. burn-off: same coords, mid-morning clock
    tw.g.set_time(day_tick(0.20));
    tw.g.notifications.clear();
    dump3x("fog_burnoff.png", &tw.render());

    // 3. clear-day dawn for contrast: same coords/time, fog-free AND rain-free day
    let clear = find_day(1, |d| {
        !weather::mist_day(SEED, d)
            && !weather::bank_day(SEED, d)
            && !weather::haze_day(SEED, d)
            && (0..weather::SLICES_PER_DAY).all(|s| !weather::slice_raining(SEED, d, s))
    });
    tw.g.events.day_number = clear;
    tw.g.set_time(day_tick(0.06));
    tw.g.notifications.clear();
    dump3x("fog_clear_dawn.png", &tw.render());

    // 4. afternoon haze at golden hour (plains, where the light show reads best)
    let mut hz = world("haze");
    let haze = find_day(1, |d| {
        weather::haze_day(SEED, d)
            && !weather::slice_raining(SEED, d, 2)
            && !weather::slice_raining(SEED, d, 3)
    });
    pin_clock(&mut hz, haze, day_tick(0.55));
    dump3x("fog_haze_golden.png", &hz.render());
    hz.g.events.day_number = clear;
    hz.g.set_time(day_tick(0.55));
    hz.g.notifications.clear();
    dump3x("fog_haze_off_golden.png", &hz.render());
}
