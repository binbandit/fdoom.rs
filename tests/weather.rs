//! Weather system (`core::weather` + the `gfx::lighting` rain/snow/bubble passes):
//! schedule determinism and rain fraction, smooth intensity ramps, desert/tundra
//! gating at the player, set-in/clear cues, fish presence, and a render smoke +
//! performance check.

use fdoom::core::events;
use fdoom::core::updater::DAY_LENGTH;
use fdoom::core::weather::{self, Precip};
use fdoom::gfx::{lighting, screen};
use fdoom::level::infinite_gen::{Biome, biome_at};
use fdoom::testutil::{TestWorld, renderer, save_png, verify_path};

const SEED: i64 = 20260707;

/// All (day, slice) pairs for days 1..=n.
fn slices(n: i32) -> impl Iterator<Item = (i32, i32)> {
    (1..=n).flat_map(|d| (0..weather::SLICES_PER_DAY).map(move |s| (d, s)))
}

/// Mid-slice tick (the plateau, where intensity == the slice peak).
fn mid(slice: i32) -> i32 {
    slice * weather::SLICE_LEN + weather::SLICE_LEN / 2
}

/// Pin the day clock to `tick` on `day` the way the lighting tests do: set, settle,
/// Pin the day clock to exactly `tick` on `day`: jump to one tick before, run a single
/// real tick through it (which also syncs the event scheduler's previous-clock snapshot
/// — a stale snapshot would read the jump as a midnight wrap and move the calendar on
/// the *next* tick), then set the day after all ticking and drop any stray cues.
fn pin_clock(tw: &mut TestWorld, day: i32, tick: i32) {
    tw.set_time(tick - 1);
    tw.tick_n(1);
    assert_eq!(tw.tick_count, tick, "clock failed to pin");
    tw.events.day_number = day;
    tw.notifications.clear();
}

/* ---------------------------------- schedule ---------------------------------- */

#[test]
fn schedule_is_deterministic_with_sane_rain_fraction() {
    // Pure and repeatable.
    assert_eq!(
        weather::schedule_intensity(SEED, 7, 1234),
        weather::schedule_intensity(SEED, 7, 1234)
    );

    // ~20% of slices rain; day 0 (fresh session) is always calm.
    let rainy = slices(300)
        .filter(|&(d, s)| weather::slice_raining(SEED, d, s))
        .count();
    let frac = rainy as f64 / (300 * weather::SLICES_PER_DAY) as f64;
    assert!(
        (0.12..=0.30).contains(&frac),
        "rain slice fraction out of bounds: {frac}"
    );
    for s in 0..weather::SLICES_PER_DAY {
        assert!(!weather::slice_raining(SEED, 0, s), "day 0 must stay calm");
    }

    // Different seeds get different calendars.
    let pattern = |seed| -> Vec<bool> {
        slices(50)
            .map(|(d, s)| weather::slice_raining(seed, d, s))
            .collect()
    };
    assert_ne!(pattern(SEED), pattern(SEED + 1));
}

#[test]
fn intensity_ramps_smoothly_between_slices() {
    let (day, _) = slices(500)
        .find(|&(d, s)| weather::slice_raining(SEED, d, s))
        .expect("no rain in 500 days?");

    // Walk two full days in 25-tick steps: bounded 0..1, no jumps, a real peak.
    let mut prev = weather::schedule_intensity(SEED, day, -25);
    let mut peak: f32 = 0.0;
    for t in (0..2 * DAY_LENGTH).step_by(25) {
        let cur = weather::schedule_intensity(SEED, day, t);
        assert!((0.0..=1.0).contains(&cur), "intensity out of range: {cur}");
        assert!(
            (cur - prev).abs() < 0.02,
            "intensity jump at day {day} tick {t}: {prev} -> {cur}"
        );
        peak = peak.max(cur);
        prev = cur;
    }
    assert!(peak > 0.5, "rainy slice never got going (peak {peak})");
}

/* ------------------------------- biome gating -------------------------------- */

#[test]
fn desert_multiplier_is_rare() {
    let mut rainy = 0;
    let mut wet = 0;
    for (d, s) in slices(2000) {
        if weather::slice_raining(SEED, d, s) {
            rainy += 1;
            if weather::desert_slice_wet(SEED, d, s) {
                wet += 1;
            }
        }
    }
    let frac = wet as f64 / rainy as f64;
    assert!(
        (0.08..=0.24).contains(&frac),
        "desert pass rate should sit near 0.15, got {frac} ({wet}/{rainy})"
    );
}

#[test]
fn biome_gating_at_the_player() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;

    // A rainy slice whose desert roll fails: bone dry in the desert.
    let (day, slice) = slices(2000)
        .find(|&(d, s)| {
            weather::slice_raining(seed, d, s) && !weather::desert_slice_wet(seed, d, s)
        })
        .expect("no desert-blocked rain slice found");
    tw.goto_biome(Biome::Desert);
    pin_clock(&mut tw, day, mid(slice));
    assert!(weather::schedule_intensity(seed, day, mid(slice)) > 0.5);
    assert_eq!(weather::precip(&tw), Precip::None);
    assert_eq!(weather::rain_intensity(&tw), 0.0);
    assert!(!weather::is_raining(&tw));

    // Any rainy slice: tundra presents it as snowfall — and snow is not rain.
    let (day, slice) = slices(2000)
        .find(|&(d, s)| weather::slice_raining(seed, d, s))
        .unwrap();
    tw.goto_biome(Biome::Tundra);
    pin_clock(&mut tw, day, mid(slice));
    assert!(
        matches!(weather::precip(&tw), Precip::Snow(i) if i > 0.5),
        "expected snow, got {:?}",
        weather::precip(&tw)
    );
    assert_eq!(weather::rain_intensity(&tw), 0.0);
    assert!(!weather::growth_boost(&tw));

    // Same slice in a forest: proper rain, with the effects API live.
    tw.goto_biome(Biome::Forest);
    pin_clock(&mut tw, day, mid(slice));
    assert!(weather::is_raining(&tw));
    assert!(weather::rain_intensity(&tw) > 0.5);
    assert!(weather::extinguishes_fire(&tw));
    assert!(weather::growth_boost(&tw));
    assert!(weather::fireflies_hidden(&tw));
}

/* ------------------------------------ cues ------------------------------------ */

/// A day hosting an isolated rain slice `s` (1..=4) with dry neighbors: the set-in
/// crossing sits in the dry run-up before `s`, the clear crossing right after it.
fn isolated_rain_slice(seed: i64) -> (i32, i32) {
    slices(2000)
        .find(|&(d, s)| {
            (1..=4).contains(&s)
                && weather::slice_raining(seed, d, s)
                && !weather::slice_raining(seed, d, s - 1)
                && !weather::slice_raining(seed, d, s + 1)
        })
        .expect("no isolated rain slice found")
}

/// First tick in `lo..hi` where the schedule crosses `CUE_THRESHOLD` (rising if `up`).
fn crossing(seed: i64, day: i32, lo: i32, hi: i32, up: bool) -> i32 {
    (lo..hi)
        .find(|&t| {
            let before = weather::schedule_intensity(seed, day, t - 1) >= weather::CUE_THRESHOLD;
            let after = weather::schedule_intensity(seed, day, t) >= weather::CUE_THRESHOLD;
            before != after && after == up
        })
        .expect("no threshold crossing in window")
}

#[test]
fn cues_fire_on_surface_at_thresholds() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;
    tw.goto_biome(Biome::Forest);

    let (day, s) = isolated_rain_slice(seed);
    let quarter = weather::SLICE_LEN / 4;

    // Rain sets in: the upcross lives in the run-up ramp just before slice s.
    let set_in = crossing(
        seed,
        day,
        s * weather::SLICE_LEN - quarter,
        s * weather::SLICE_LEN + 1,
        true,
    );
    pin_clock(&mut tw, day, set_in - 3);
    tw.tick_n(8);
    assert!(
        tw.notifications.iter().any(|n| n.contains("Rain patters")),
        "no set-in cue; got {:?}",
        tw.notifications
    );

    // And clears on the way out.
    let end = (s + 1) * weather::SLICE_LEN;
    let clears = crossing(seed, day, end - 1, end + quarter, false);
    pin_clock(&mut tw, day, clears - 3);
    tw.tick_n(8);
    assert!(
        tw.notifications.iter().any(|n| n.contains("rain clears")),
        "no clear cue; got {:?}",
        tw.notifications
    );

    // Underground the same crossing passes silently.
    tw.g.player_mut().c.level = Some(2);
    tw.g.current_level = 2;
    pin_clock(&mut tw, day, set_in - 3);
    tw.tick_n(8);
    assert!(
        !tw.notifications.iter().any(|n| n.contains("Rain")),
        "cue fired underground: {:?}",
        tw.notifications
    );
}

/* ------------------------------- fish presence -------------------------------- */

#[test]
fn fish_presence_field_is_deterministic_and_patchy() {
    let (mut above, mut below) = (0, 0);
    let mut diverges = false;
    for x in (-240..240).step_by(12) {
        for y in (-240..240).step_by(12) {
            let v = weather::fish_presence(SEED, x, y);
            assert!((0.0..=1.0).contains(&v), "presence out of range: {v}");
            assert_eq!(v, weather::fish_presence(SEED, x, y));
            if v > weather::FISH_PRESENCE_THRESHOLD {
                above += 1;
            } else {
                below += 1;
            }
            diverges |= v != weather::fish_presence(SEED + 1, x, y);
        }
    }
    assert!(above > 0, "no hotspots anywhere");
    assert!(below > 0, "everything a hotspot");
    assert!(diverges, "field ignores the seed");
}

/* ----------------------------- rendering + perf ------------------------------- */

/// A daytime (slice 1-2) heavy-rain slice on an event-free day, plus an event-free
/// all-dry comparison day — same clock, different calendar.
fn render_days(seed: i64) -> (i32, i32, i32) {
    let (rain_day, slice) = slices(2000)
        .find(|&(d, s)| {
            (1..=2).contains(&s)
                && weather::schedule_intensity(seed, d, mid(s)) > 0.9
                && events::event_for_day(seed, d).is_none()
        })
        .expect("no heavy daytime rain on a calm day");
    let dry_day = (1..2000)
        .find(|&d| {
            (0..weather::SLICES_PER_DAY).all(|s| !weather::slice_raining(seed, d, s))
                && events::event_for_day(seed, d).is_none()
        })
        .expect("no calm dry day");
    (rain_day, dry_day, mid(slice))
}

fn luma(pixels: &[i32]) -> f64 {
    let mut sum = 0.0;
    for &p in pixels {
        sum += 0.30 * ((p >> 16) & 0xff) as f64
            + 0.59 * ((p >> 8) & 0xff) as f64
            + 0.11 * (p & 0xff) as f64;
    }
    sum / pixels.len() as f64
}

#[test]
fn rain_renders_streaks_and_dims() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;
    tw.goto_biome(Biome::Forest);
    let (rain_day, dry_day, tick) = render_days(seed);

    let dump = |name: &str, px: &[i32]| {
        save_png(
            verify_path(name),
            px,
            screen::W as usize,
            screen::H as usize,
            3,
        );
    };

    pin_clock(&mut tw, rain_day, tick);
    let rain_px = tw.render();
    dump("weather_rain_day.png", &rain_px);
    pin_clock(&mut tw, dry_day, tick);
    let dry_px = tw.render();
    dump("weather_dry_day.png", &dry_px);

    // Dry day at intensity 0: re-render is streak-free and byte-identical.
    let dry_again = tw.render();
    assert_eq!(dry_px, dry_again, "dry frame should be deterministic");

    // Tundra shows the same slice as snowfall — dump a frame for eyeballing.
    tw.goto_biome(Biome::Tundra);
    pin_clock(&mut tw, rain_day, tick);
    assert!(matches!(weather::precip(&tw), Precip::Snow(_)));
    dump("weather_snow_tundra.png", &tw.render());

    // Streaks: pixels distinctly bluer than the same spot on the dry frame (the rain
    // dim pulls everything else *down*, so only additive streaks can beat this).
    let streaks = rain_px
        .iter()
        .zip(&dry_px)
        .filter(|&(&r, &d)| (r & 0xff) > (d & 0xff) + 18)
        .count();
    assert!(streaks > 300, "expected rain streak pixels, got {streaks}");

    // The cool dim reads on the whole frame.
    assert!(
        luma(&rain_px) < luma(&dry_px) - 1.5,
        "rain should dim the frame ({} vs {})",
        luma(&rain_px),
        luma(&dry_px)
    );
}

#[test]
fn fish_bubbles_show_on_hotspot_water() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;
    let (ox, oy) = tw.goto_biome(Biome::DeepOcean);

    // Park the camera over a presence hotspot that is still open ocean.
    let spot = (-60..60)
        .flat_map(|dy| (-60..60).map(move |dx| (ox + dx, oy + dy)))
        .find(|&(x, y)| {
            weather::fish_presence(seed, x, y) > weather::FISH_PRESENCE_THRESHOLD
                && biome_at(seed, x, y) == Biome::DeepOcean
        })
        .expect("no fish hotspot near the ocean");
    tw.teleport(spot.0, spot.1);
    tw.tick_n(8);

    let (px, py) = tw.player_pos();
    let (xs, ys) = (px - screen::W / 2, py - (screen::H - 8) / 2);

    // Drive the phase across a full cycle: some tile in view must bubble.
    let mut r = renderer();
    let flat = 0x101010;
    let mut bubbled = 0usize;
    for t in (0..192).step_by(8) {
        tw.g.tick_count = t;
        r.screen.pixels.fill(flat);
        lighting::fish_bubbles(&mut r.screen, &tw.g, 3, xs, ys, 1.0);
        bubbled += r.screen.pixels.iter().filter(|&&p| p != flat).count();
    }
    assert!(bubbled > 10, "no bubble specks over hotspot water");
}

#[test]
fn weather_render_pass_stays_fast() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;
    // Worst case for the pass: heavy rain over ocean (bubble scan + streaks + grade).
    tw.goto_biome(Biome::Ocean);
    let (rain_day, _, tick) = render_days(seed);
    pin_clock(&mut tw, rain_day, tick);
    assert!(weather::is_raining(&tw), "perf scenario should be raining");

    let base = tw.render();
    let (px, py) = tw.player_pos();
    let (xs, ys) = (px - screen::W / 2, py - (screen::H - 8) / 2);

    let mut r = renderer();
    let iters = 100u32;
    let mut total = std::time::Duration::ZERO;
    for _ in 0..iters {
        r.screen.pixels.copy_from_slice(&base);
        let t0 = std::time::Instant::now();
        lighting::render_pass(&mut r.screen, &mut r.light_screen, &tw.g, 3, xs, ys);
        total += t0.elapsed();
    }
    let avg = total / iters;
    println!("weather+lighting pass avg: {avg:?} over {iters} iters");
    // Same generous debug-build ceiling as tests/lighting.rs.
    assert!(
        avg < std::time::Duration::from_millis(25),
        "pass too slow: {avg:?}"
    );
}
