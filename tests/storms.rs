//! Severe weather (the storm tier on `core::weather`): schedule purity and
//! frequency, blizzard effects (extra cold band, fast settle, whiteout veil),
//! thunderstorm effects (sky flash, deterministic telegraphed lightning, the
//! self-limiting strike-fire-then-rain loop), the approachability floors (player
//! distance, town footprints, campfire sanctuary), and the render budget.

use fdoom::core::events;
use fdoom::core::temperature::{self, Modifiers};
use fdoom::core::updater::DAY_LENGTH;
use fdoom::core::weather::{self, Precip, Storm, Strike, StrikePhase};
use fdoom::entity::furniture::campfire;
use fdoom::gfx::{lighting, screen};
use fdoom::level::infinite_gen::Biome;
use fdoom::level::structures_gen::{self, StructureKind};
use fdoom::level::tile::{fire, snowfall};
use fdoom::testutil::{TestWorld, renderer, save_png, verify_path};

const SEED: i64 = 20260707;

/// All (day, slice) pairs for days 1..=n.
fn slices(n: i32) -> impl Iterator<Item = (i32, i32)> {
    (1..=n).flat_map(|d| (0..weather::SLICES_PER_DAY).map(move |s| (d, s)))
}

/// Mid-slice tick (the plateau).
fn mid(slice: i32) -> i32 {
    slice * weather::SLICE_LEN + weather::SLICE_LEN / 2
}

/// Pin the day clock (same recipe as tests/weather.rs): jump to one tick before,
/// run one real tick through it, then set the day and drop stray cues.
fn pin_clock(tw: &mut TestWorld, day: i32, tick: i32) {
    tw.set_time(tick - 1);
    tw.tick_n(1);
    assert_eq!(tw.tick_count, tick, "clock failed to pin");
    tw.events.day_number = day;
    tw.notifications.clear();
    tw.g.warnings.clear();
}

/// First severe slice on an event-free day (events own their nights' visuals).
fn severe_slice(seed: i64) -> (i32, i32) {
    slices(3000)
        .find(|&(d, s)| {
            weather::slice_severe(seed, d, s) && events::event_for_day(seed, d).is_none()
        })
        .expect("no severe slice in 3000 days")
}

/// A rainy-but-not-severe slice on an event-free day.
fn plain_rain_slice(seed: i64) -> (i32, i32) {
    slices(3000)
        .find(|&(d, s)| {
            weather::slice_raining(seed, d, s)
                && !weather::slice_severe(seed, d, s)
                && events::event_for_day(seed, d).is_none()
        })
        .expect("no plain rain slice")
}

/// The first scheduled strike within `r` tiles of (x, y), preferring daylight bolts
/// on forest ground when `staged` (for screenshots); event-free days only.
fn find_strike_near(seed: i64, x: i32, y: i32, r: i32, staged: bool) -> (i32, Strike) {
    let slots = DAY_LENGTH / weather::STRIKE_SLOT;
    for day in 1..1500 {
        if events::event_for_day(seed, day).is_some() {
            continue;
        }
        for slot in 0..slots {
            for s in weather::strikes_in_rect(
                seed,
                day,
                slot * weather::STRIKE_SLOT,
                x - r,
                y - r,
                x + r,
                y + r,
            ) {
                if staged {
                    // A readable stage: mid-day bolt, target and the player's
                    // vantage both on forest ground (not out on a lake).
                    let frac = s.tick as f32 / DAY_LENGTH as f32;
                    if !(0.20..0.42).contains(&frac)
                        || fdoom::level::infinite_gen::biome_at(seed, s.x, s.y) != Biome::Forest
                        || fdoom::level::infinite_gen::biome_at(seed, s.x - 8, s.y - 2)
                            != Biome::Forest
                    {
                        continue;
                    }
                }
                return (day, s);
            }
        }
    }
    panic!("no strike found near ({x}, {y})");
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

/* --------------------------------- the schedule --------------------------------- */

#[test]
fn severity_schedule_is_pure_with_sane_frequency() {
    // Pure and repeatable; distinct across seeds.
    assert_eq!(
        weather::storm_intensity(SEED, 7, 1234),
        weather::storm_intensity(SEED, 7, 1234)
    );

    for seed in SEED..SEED + 5 {
        let (mut rainy, mut severe) = (0u32, 0u32);
        for (d, s) in slices(600) {
            let sev = weather::slice_severe(seed, d, s);
            if weather::slice_raining(seed, d, s) {
                rainy += 1;
                severe += sev as u32;
            } else {
                assert!(!sev, "severe slice without rain (seed {seed}, {d}/{s})");
            }
        }
        let frac = severe as f64 / rainy as f64;
        assert!(
            (0.07..=0.25).contains(&frac),
            "severe fraction should sit near 0.15: {frac} (seed {seed})"
        );
    }

    // Day 0 (fresh session) stays calm, like the rain schedule.
    for s in 0..weather::SLICES_PER_DAY {
        assert!(!weather::slice_severe(SEED, 0, s));
    }

    // Different seeds get different storm calendars.
    let pattern = |seed| -> Vec<bool> {
        slices(80)
            .map(|(d, s)| weather::slice_severe(seed, d, s))
            .collect()
    };
    assert_ne!(pattern(SEED), pattern(SEED + 1));

    // A severe slice floors the *rain* schedule at the storm peak: storms are
    // heavy precipitation by definition (the effects API escalates for free).
    let (d, s) = severe_slice(SEED);
    assert!(weather::schedule_intensity(SEED, d, mid(s)) >= weather::STORM_PEAK_FLOOR);
    assert!(weather::storm_intensity(SEED, d, mid(s)) >= weather::STORM_PEAK_FLOOR);

    // Non-severe slices contribute nothing to the storm envelope.
    let (d, s) = plain_rain_slice(SEED);
    assert_eq!(weather::storm_intensity(SEED, d, mid(s)), 0.0);
}

/* ----------------------------- effects API under storm ----------------------------- */

#[test]
fn storms_still_read_as_rain_to_the_effects_api() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;
    tw.goto_biome(Biome::Forest);

    let (day, s) = severe_slice(seed);
    pin_clock(&mut tw, day, mid(s));

    // The severity tier presents as a thunderstorm here...
    assert!(
        matches!(weather::storm(&tw), Storm::Thunderstorm(sev) if sev > 0.99),
        "expected a full thunderstorm, got {:?}",
        weather::storm(&tw)
    );
    // ...and the whole rain effects API stays live underneath it.
    assert!(weather::is_raining(&tw));
    assert!(weather::rain_intensity(&tw) > 0.5);
    assert!(
        weather::extinguishes_fire(&tw),
        "storm rain must douse fires"
    );
    assert!(weather::growth_boost(&tw), "storms count as rain for crops");
    assert!(weather::fireflies_hidden(&tw));

    // A plain rain slice is never a storm.
    let (day, s) = plain_rain_slice(seed);
    pin_clock(&mut tw, day, mid(s));
    assert_eq!(weather::storm(&tw), Storm::None);
    assert!(weather::is_raining(&tw));
}

/* ----------------------------------- blizzard ----------------------------------- */

#[test]
fn blizzard_adds_one_cold_band_and_the_campfire_stays_home() {
    // Pure pipeline: one extra band, answered by the coat, overridden by the fire.
    let m = |blizzard, fur_coat, near_fire| Modifiers {
        blizzard,
        fur_coat,
        near_fire,
        ..Default::default()
    };
    let eq = |got: f64, want: f64, what: &str| {
        assert!((got - want).abs() < 1e-9, "{what}: got {got}, want {want}");
    };
    eq(
        temperature::apply_modifiers(-1.0, &m(true, false, false)),
        -2.0,
        "blizzard band",
    );
    eq(
        temperature::apply_modifiers(-1.0, &m(true, true, false)),
        0.0,
        "coat answers it",
    );
    // The sanctuary rule: a lit campfire overrides the whiteout's cold entirely —
    // fires keep their FULL warmth in a blizzard (campfire = home, by design).
    eq(
        temperature::apply_modifiers(-3.5, &m(true, false, true)),
        0.0,
        "fire sanctuary",
    );

    // Live world: the modifier reads the storm tier positionally.
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;
    tw.goto_biome(Biome::Tundra);
    let (day, s) = severe_slice(seed);
    pin_clock(&mut tw, day, mid(s));
    assert!(
        matches!(weather::storm(&tw), Storm::Blizzard(sev) if sev > 0.99),
        "expected a blizzard in tundra, got {:?}",
        weather::storm(&tw)
    );
    let (xt, yt) = tw.player_tile();
    assert!(weather::blizzard_at(&tw, xt, yt));
    assert!(temperature::modifiers_for(&tw, tw.player()).blizzard);

    // Same slice, warm country: rain, no blizzard flag.
    tw.goto_biome(Biome::Forest);
    pin_clock(&mut tw, day, mid(s));
    let (xt, yt) = tw.player_tile();
    assert!(!weather::blizzard_at(&tw, xt, yt));
    assert!(!temperature::modifiers_for(&tw, tw.player()).blizzard);

    // A plain snowy slice is cold, but not blizzard-cold.
    tw.goto_biome(Biome::Tundra);
    let (day, s) = plain_rain_slice(seed);
    pin_clock(&mut tw, day, mid(s));
    assert!(matches!(weather::precip(&tw), Precip::Snow(_)));
    assert!(!temperature::modifiers_for(&tw, tw.player()).blizzard);
}

#[test]
fn blizzard_settles_snow_three_times_faster() {
    // The odds are pinned, not statistical: the storm tier divides them by the
    // published factor for both ground and canopy.
    assert_eq!(
        snowfall::settle_odds(false, true),
        snowfall::settle_odds(false, false) / snowfall::BLIZZARD_SETTLE_FACTOR
    );
    assert_eq!(
        snowfall::settle_odds(true, true),
        snowfall::settle_odds(true, false) / snowfall::BLIZZARD_SETTLE_FACTOR
    );
    assert!(snowfall::settle_odds(false, true) >= 1);
    assert!(snowfall::settle_odds(true, true) >= 1);
}

/* ------------------------------ lightning strikes ------------------------------- */

#[test]
fn strikes_are_deterministic_telegraphed_and_respect_towns() {
    // Determinism: the same sweep twice, then a different seed.
    let sweep = |seed: i64| -> Vec<Strike> {
        let mut out = Vec::new();
        for day in 1..40 {
            for slot in 0..DAY_LENGTH / weather::STRIKE_SLOT {
                out.extend(weather::strikes_in_rect(
                    seed,
                    day,
                    slot * weather::STRIKE_SLOT,
                    -600,
                    -600,
                    600,
                    600,
                ));
            }
        }
        out
    };
    let strikes = sweep(SEED);
    assert!(!strikes.is_empty(), "no strikes scheduled in 40 days");
    assert_eq!(strikes, sweep(SEED), "strike schedule must be pure");
    assert_ne!(strikes, sweep(SEED + 1), "strike schedule ignores the seed");

    for s in &strikes {
        // Only warm (rain) country throws lightning.
        assert!(
            !weather::snow_climate(SEED, s.x, s.y),
            "a blizzard threw lightning at ({}, {})",
            s.x,
            s.y
        );
        // The telegraph always fits inside the slot before the bolt.
        let slot_start = (s.tick / weather::STRIKE_SLOT) * weather::STRIKE_SLOT;
        assert!(
            s.tick - slot_start >= weather::STRIKE_TELEGRAPH,
            "bolt at {} leaves no room for its telegraph (slot {})",
            s.tick,
            slot_start
        );
        // Never inside (or hugging) a town footprint.
        let r = structures_gen::MAX_RADIUS + 4;
        for p in structures_gen::placements_in_rect(SEED, s.x - r, s.y - r, s.x + r, s.y + r) {
            if matches!(p.kind, StructureKind::Hamlet | StructureKind::Village) {
                let d = (s.x - p.x).abs().max((s.y - p.y).abs());
                assert!(
                    d > structures_gen::kind_radius(p.kind),
                    "strike ({}, {}) inside {:?} at ({}, {})",
                    s.x,
                    s.y,
                    p.kind,
                    p.x,
                    p.y
                );
            }
        }
    }

    // Phase bookkeeping: telegraph ramps 0..1 across the 2 s lead, bolt for 3 ticks.
    let s = strikes[0];
    assert_eq!(
        weather::strike_phase(&s, s.tick - weather::STRIKE_TELEGRAPH - 1),
        StrikePhase::Idle
    );
    assert!(matches!(
        weather::strike_phase(&s, s.tick - weather::STRIKE_TELEGRAPH),
        StrikePhase::Telegraph(p) if p < 0.02
    ));
    assert!(matches!(
        weather::strike_phase(&s, s.tick - 1),
        StrikePhase::Telegraph(p) if p > 0.98
    ));
    assert_eq!(weather::strike_phase(&s, s.tick), StrikePhase::Bolt(0));
    assert_eq!(weather::strike_phase(&s, s.tick + 2), StrikePhase::Bolt(2));
    assert_eq!(weather::strike_phase(&s, s.tick + 3), StrikePhase::Idle);
}

#[test]
fn lightning_starts_a_fire_and_the_storm_rain_douses_it() {
    let mut tw = TestWorld::infinite().creative().name("storm_fire").build();
    let seed = tw.world_seed;
    let (fx, fy) = fdoom::testutil::find_biome(seed, Biome::Forest);
    let (day, s) = find_strike_near(seed, fx, fy, 300, false);

    // Stand 10-ish tiles off the target (outside the 8-tile floor, inside the
    // 40-tile action radius), stream the chunks, and plant the victim tree.
    tw.teleport(s.x + 9, s.y + 5);
    tw.tick_n(8);
    tw.place_at("tree", s.x, s.y);
    let lvl = tw.current_level;
    assert!(!fire::is_burning(&tw.g, lvl, s.x, s.y));

    // Tick across the bolt: the tree catches.
    pin_clock(&mut tw, day, s.tick - 4);
    tw.tick_n(8);
    assert!(
        fire::is_burning(&tw.g, lvl, s.x, s.y),
        "the bolt should ignite the tree (day {day}, tick {})",
        s.tick
    );

    // The storm's own rain fights the fire: within a few burn ticks the flame is
    // doused and the tree still stands (rain-douse runs before burn-out can).
    tw.tick_n(700);
    assert!(
        !fire::is_burning(&tw.g, lvl, s.x, s.y),
        "storm rain should have doused the strike fire"
    );
    assert!(
        tw.g.tile_at(lvl, s.x, s.y)
            .name
            .to_lowercase()
            .contains("tree"),
        "the doused tree should survive, got {}",
        tw.g.tile_at(lvl, s.x, s.y).name
    );
}

#[test]
fn strikes_never_land_beside_the_player() {
    let mut tw = TestWorld::infinite().creative().name("storm_floor").build();
    let seed = tw.world_seed;
    let (fx, fy) = fdoom::testutil::find_biome(seed, Biome::Forest);
    let (day, s) = find_strike_near(seed, fx, fy, 300, false);

    // Same bolt, but the player camps 3 tiles away: suppressed outright.
    tw.teleport(s.x + 3, s.y);
    tw.tick_n(8);
    tw.place_at("tree", s.x, s.y);
    let lvl = tw.current_level;
    pin_clock(&mut tw, day, s.tick - 4);
    tw.tick_n(8);
    assert!(
        !fire::is_burning(&tw.g, lvl, s.x, s.y),
        "a bolt landed within the 8-tile player floor"
    );
}

/* ------------------------------------- cues ------------------------------------- */

#[test]
fn escalation_cues_fire_on_the_storm_edge() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;
    let (day, s) = severe_slice(seed);

    // The storm-threshold crossings around slice s.
    let cross = |day: i32, lo: i32, hi: i32, up: bool| -> i32 {
        (lo..hi)
            .find(|&t| {
                let a = weather::storm_intensity(seed, day, t - 1) >= weather::STORM_THRESHOLD;
                let b = weather::storm_intensity(seed, day, t) >= weather::STORM_THRESHOLD;
                a != b && b == up
            })
            .expect("no storm crossing in window")
    };
    let quarter = weather::SLICE_LEN / 4;
    let set_in = cross(day, s * weather::SLICE_LEN - quarter, mid(s), true);

    // Blizzard: the centered warning band.
    tw.goto_biome(Biome::Tundra);
    pin_clock(&mut tw, day, set_in - 3);
    tw.tick_n(8);
    assert!(
        tw.g.warnings
            .iter()
            .any(|w| w.contains("wind turns to knives")),
        "no blizzard warning; got {:?} / {:?}",
        tw.g.warnings,
        tw.notifications
    );

    // ...and the calm-down cue on the way out.
    let end = (s + 1) * weather::SLICE_LEN;
    let clears = cross(day, end - quarter, end + quarter, false);
    pin_clock(&mut tw, day, clears - 3);
    tw.tick_n(8);
    assert!(
        tw.notifications.iter().any(|n| n.contains("wind eases")),
        "no blizzard calm-down; got {:?}",
        tw.notifications
    );

    // Thunderstorm: ambient-tier cue (the sky flashes are the show).
    tw.goto_biome(Biome::Forest);
    pin_clock(&mut tw, day, set_in - 3);
    tw.tick_n(8);
    assert!(
        tw.notifications
            .iter()
            .any(|n| n.contains("Thunder rolls in")),
        "no thunder cue; got {:?}",
        tw.notifications
    );
}

/* -------------------------------- render + budget -------------------------------- */

#[test]
fn storm_frames_render_and_stay_inside_budget() {
    let mut worst = std::time::Duration::ZERO;
    let mut worst_name = "";
    for (name, biome) in [("blizzard", Biome::Tundra), ("thunderstorm", Biome::Forest)] {
        let mut tw = TestWorld::infinite()
            .name(&format!("storm_perf_{name}"))
            .build();
        let seed = tw.world_seed;
        tw.goto_biome(biome);
        let (day, s) = severe_slice(seed);
        pin_clock(&mut tw, day, mid(s));
        assert_ne!(
            weather::storm(&tw),
            Storm::None,
            "perf frame must be storming"
        );

        let base = tw.render();
        let (px, py) = tw.player_pos();
        let (xs, ys) = (px - screen::W / 2, py - (screen::H - 8) / 2);
        let mut r = renderer();
        let iters = 60u32;
        let mut total = std::time::Duration::ZERO;
        for _ in 0..iters {
            r.screen.pixels.copy_from_slice(&base);
            let t0 = std::time::Instant::now();
            lighting::render_pass(&mut r.screen, &mut r.light_screen, &tw.g, 3, xs, ys);
            total += t0.elapsed();
        }
        let avg = total / iters;
        println!("storm render pass [{name}]: {avg:?} avg over {iters} iters");
        if avg > worst {
            worst = avg;
            worst_name = name;
        }
    }
    // Same debug ceiling as the other render-pass tests; release gets the visuals
    // budget class (the veil costs about what the morning mist does).
    assert!(
        worst < std::time::Duration::from_millis(25),
        "storm pass too slow ({worst_name}: {worst:?})"
    );
    // Release: the same 400µs class as tests/visuals.rs (classic 288x192 screen).
    // Measured on an M-series dev box: blizzard ~200µs, thunderstorm ~130µs.
    #[cfg(not(debug_assertions))]
    assert!(
        worst < std::time::Duration::from_micros(400),
        "release storm pass over budget ({worst_name}: {worst:?})"
    );
}

/* --------------------------------- money shots ---------------------------------- */

#[test]
fn storm_screenshots() {
    // Blizzard whiteout with the campfire sanctuary.
    let mut tw = TestWorld::infinite().name("storm_shot_bliz").build();
    let seed = tw.world_seed;
    tw.goto_biome(Biome::Tundra);
    let (day, s) = slices(3000)
        .find(|&(d, s)| {
            (1..=2).contains(&s)
                && weather::slice_severe(seed, d, s)
                && events::event_for_day(seed, d).is_none()
        })
        .expect("no daytime severe slice");
    let (ptx, pty) = tw.player_tile();
    let lvl = tw.current_level;
    let e = campfire::new();
    tw.g.level_mut(lvl).add_at(e, ptx + 3, pty, true, lvl);
    tw.tick_n(2);
    pin_clock(&mut tw, day, mid(s));
    assert!(matches!(weather::storm(&tw), Storm::Blizzard(_)));
    dump3x("storms_blizzard_campfire.png", &tw.render());

    // Thunderstorm at a sky-flash instant (and the same frame a beat later, dark).
    let mut tw = TestWorld::infinite().name("storm_shot_flash").build();
    let seed = tw.world_seed;
    tw.goto_biome(Biome::Forest);
    let flash_tick = (s * weather::SLICE_LEN..(s + 1) * weather::SLICE_LEN)
        .find(|&t| {
            weather::sky_flash(seed, day, t) >= 1.0
                && weather::storm_intensity(seed, day, t) >= weather::STORM_PEAK_FLOOR
        })
        .expect("no sky flash inside the storm plateau");
    pin_clock(&mut tw, day, flash_tick);
    dump3x("storms_thunder_flash.png", &tw.render());
    pin_clock(&mut tw, day, flash_tick + 8);
    dump3x("storms_thunder_between.png", &tw.render());

    // A telegraphed strike, then the burning tree in the rain.
    let mut tw = TestWorld::infinite()
        .creative()
        .name("storm_shot_strike")
        .build();
    let seed = tw.world_seed;
    let (fx, fy) = fdoom::testutil::find_biome(seed, Biome::Forest);
    let (day, s) = find_strike_near(seed, fx, fy, 300, true);
    tw.teleport(s.x - 8, s.y - 2); // 8.2 tiles out: past the floor, on screen
    tw.tick_n(8);
    tw.place_at("tree", s.x, s.y);
    pin_clock(&mut tw, day, s.tick - 30);
    dump3x("storms_strike_telegraph.png", &tw.render());
    pin_clock(&mut tw, day, s.tick - 4);
    tw.tick_n(6); // across the bolt: ignition
    let lvl = tw.current_level;
    assert!(fire::is_burning(&tw.g, lvl, s.x, s.y));
    pin_clock(&mut tw, day, s.tick + 1); // re-pin onto the bolt frame
    dump3x("storms_strike_fire.png", &tw.render());
}
