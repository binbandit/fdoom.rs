//! Snow accumulation wave (`level::tile::snowfall` + the `weather` cold-reach gate):
//! during snow slices the cold fringe outside Tundra whitens one tile at a time
//! (grass/tufts → snow, broadleaf → snow tree), and thaws back once the sky clears —
//! while tundra-interior snow never melts and worked tiles never convert.

use fdoom::core::weather::{self, Precip};
use fdoom::level::infinite_gen::{self, Biome};
use fdoom::level::tile::{TileKind, dispatch, snowfall};
use fdoom::rng::Rng;
use fdoom::testutil::TestWorld;

const SEED: i64 = 20260707;

/// Pin the day clock the way the weather tests do: jump to one tick before, run a
/// single real tick (syncs the event scheduler's previous-clock snapshot), then set
/// the day and drop any stray cues.
fn pin_clock(tw: &mut TestWorld, day: i32, tick: i32) {
    tw.set_time(tick - 1);
    tw.tick_n(1);
    assert_eq!(tw.tick_count, tick, "clock failed to pin");
    tw.events.day_number = day;
    tw.notifications.clear();
}

/// Mid-slice tick (the plateau, where intensity == the slice peak).
fn mid(slice: i32) -> i32 {
    slice * weather::SLICE_LEN + weather::SLICE_LEN / 2
}

/// First (day, slice) from day 1 whose rain roll matches `raining`. Rain slices are
/// required to be *isolated* (dry neighbors), so the cue test's threshold crossings
/// exist on both edges and mid-slice plateaus are exact.
fn find_slice(seed: i64, raining: bool) -> (i32, i32) {
    let wet = |d: i32, s: i32| weather::slice_raining(seed, d, s);
    (1..1000)
        .flat_map(|d| (0..weather::SLICES_PER_DAY).map(move |s| (d, s)))
        .find(|&(d, s)| {
            if raining {
                // s >= 1 keeps the cue test's pre-slice crossing scan inside the day
                s >= 1 && wet(d, s) && !wet(d, s - 1) && !wet(d, s + 1)
            } else {
                !wet(d, s)
            }
        })
        .expect("no matching slice in 1000 days?")
}

/// A cold-fringe tile: sub-[`weather::COLD_REACH`] climate but *not* Tundra (nor any
/// native-snow position) — ordinary green country that snow only visits. Scans the
/// safely-interior 0.315..0.345 band so blend jitter can't flip the classification.
fn find_cold_fringe(seed: i64) -> (i32, i32) {
    let fringe = |x: i32, y: i32| {
        let c = infinite_gen::climate_at(seed, x, y);
        (0.315..0.345).contains(&c)
            && matches!(
                infinite_gen::biome_at(seed, x, y),
                Biome::Plains | Biome::Forest
            )
            && !snowfall::snow_native(seed, x, y)
    };
    let r = 2000;
    (-r..r)
        .step_by(8)
        .flat_map(|y| (-r..r).step_by(8).map(move |x| (x, y)))
        .find(|&(x, y)| fringe(x, y))
        .expect("no cold-fringe tile within scan range")
}

/// A warm green tile (climate comfortably above the cold reach): rain country.
fn find_warm_green(seed: i64) -> (i32, i32) {
    let warm = |x: i32, y: i32| {
        infinite_gen::climate_at(seed, x, y) > 0.42
            && matches!(
                infinite_gen::biome_at(seed, x, y),
                Biome::Plains | Biome::Forest
            )
    };
    let r = 2000;
    (-r..r)
        .step_by(8)
        .flat_map(|y| (-r..r).step_by(8).map(move |x| (x, y)))
        .find(|&(x, y)| warm(x, y))
        .expect("no warm green tile within scan range")
}

/// A tundra-core tile: Tundra by the authoritative lookup, deep inside the cold.
fn find_tundra_core(seed: i64) -> (i32, i32) {
    let core = |x: i32, y: i32| {
        infinite_gen::climate_at(seed, x, y) < 0.28
            && infinite_gen::biome_at(seed, x, y) == Biome::Tundra
    };
    let r = 3000;
    (-r..r)
        .step_by(8)
        .flat_map(|y| (-r..r).step_by(8).map(move |x| (x, y)))
        .find(|&(x, y)| core(x, y))
        .expect("no tundra core tile within scan range")
}

/// Run the tile's random tick `n` times at a frozen clock (direct `dispatch::tick`
/// calls — the same entry the level's random tile pass uses).
fn tick_tile_n(tw: &mut TestWorld, x: i32, y: i32, n: usize) {
    let lvl = tw.current_level;
    for _ in 0..n {
        let def = tw.g.tile_at(lvl, x, y);
        dispatch::tick(&mut tw.g, &def, lvl, x, y);
    }
}

/* --------------------------------- presentation --------------------------------- */

#[test]
fn cold_fringe_presents_snow_and_warm_country_rain() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    let (day, s) = find_slice(SEED, true);

    let (fx, fy) = find_cold_fringe(SEED);
    tw.teleport(fx, fy);
    tw.tick_n(2);
    pin_clock(&mut tw, day, mid(s));
    assert_ne!(
        infinite_gen::biome_at(SEED, fx, fy),
        Biome::Tundra,
        "fringe scan returned tundra"
    );
    assert!(
        matches!(weather::precip(&tw), Precip::Snow(i) if i > 0.5),
        "cold fringe should present snow, got {:?}",
        weather::precip(&tw)
    );
    assert!(weather::snowing_at(&tw, fx, fy));

    // The same slice reads as rain in warm country.
    let (wx, wy) = find_warm_green(SEED);
    tw.teleport(wx, wy);
    tw.tick_n(2);
    tw.events.day_number = day; // ticking may have wrapped nothing; re-pin the day
    assert!(
        matches!(weather::precip(&tw), Precip::Rain(_)),
        "warm country should present rain, got {:?}",
        weather::precip(&tw)
    );
    assert!(!weather::snowing_at(&tw, wx, wy));
}

/* --------------------------------- accumulation --------------------------------- */

#[test]
fn snow_settles_on_fringe_grass_gradually_and_flips_trees() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    let (fx, fy) = find_cold_fringe(SEED);
    tw.teleport(fx, fy);
    tw.tick_n(4); // stream the chunks in
    let (day, s) = find_slice(SEED, true);
    pin_clock(&mut tw, day, mid(s));
    tw.g.random = Rng::new(0xC0FFEE); // fixed stream: the counts below are stable

    // Stage a 6x6 grass clearing and one broadleaf.
    for dy in 0..6 {
        for dx in 0..6 {
            tw.place_at("Grass", fx + dx, fy + dy);
        }
    }
    let (tx, ty) = (fx + 7, fy);
    tw.place_at("Tree", tx, ty);

    let lvl = tw.current_level;
    let count_snow = |tw: &TestWorld| {
        (0..36)
            .filter(|i| {
                let def = tw.g.tile_at(lvl, fx + i % 6, fy + i / 6);
                matches!(def.kind, TileKind::Snow)
            })
            .count()
    };

    // ~100 random ticks per tile is under half a slice: some snow, not a whiteout.
    let pass = |tw: &mut TestWorld, n: usize| {
        for _ in 0..n {
            for i in 0..36 {
                let (x, y) = (fx + i % 6, fy + i / 6);
                let def = tw.g.tile_at(lvl, x, y);
                dispatch::tick(&mut tw.g, &def, lvl, x, y);
            }
        }
    };
    pass(&mut tw, 100);
    let early = count_snow(&tw);
    pass(&mut tw, 300);
    let late = count_snow(&tw);
    assert!(early > 0, "no snow settled after 100 passes");
    assert!(
        early < 36,
        "instant whiteout — accumulation should be gradual"
    );
    assert!(
        late > early,
        "snow count must keep growing ({early} -> {late})"
    );

    // The broadleaf flips to its snow-covered form.
    tick_tile_n(&mut tw, tx, ty, 20_000);
    assert!(
        matches!(tw.g.tile_at(lvl, tx, ty).kind, TileKind::SnowTree),
        "tree never flipped to snow tree"
    );
}

/* ------------------------------------- thaw ------------------------------------- */

#[test]
fn fringe_snow_thaws_in_clear_weather_but_tundra_never_melts() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    let (fx, fy) = find_cold_fringe(SEED);
    tw.teleport(fx, fy);
    tw.tick_n(4);
    let (day, s) = find_slice(SEED, false);
    pin_clock(&mut tw, day, mid(s));
    tw.g.random = Rng::new(0x7EA);

    // Visiting snow melts back to green.
    tw.place_at("Snow", fx, fy);
    tw.place_at("Snow Tree", fx + 1, fy);
    let lvl = tw.current_level;
    tick_tile_n(&mut tw, fx, fy, 30_000);
    tick_tile_n(&mut tw, fx + 1, fy, 30_000);
    // (once green, ordinary grass ticks may sprout it onward into tufts)
    assert!(
        matches!(
            tw.g.tile_at(lvl, fx, fy).kind,
            TileKind::Grass | TileKind::TallGrass { .. }
        ),
        "fringe snow never thawed"
    );
    assert!(
        matches!(tw.g.tile_at(lvl, fx + 1, fy).kind, TileKind::Tree),
        "fringe snow tree never thawed"
    );

    // Tundra interior: snow is home — the same clear slice melts nothing.
    let (ux, uy) = find_tundra_core(SEED);
    tw.teleport(ux, uy);
    tw.tick_n(4);
    tw.events.day_number = day;
    tw.place_at("Snow", ux, uy);
    tw.place_at("Snow Tree", ux + 1, uy);
    tick_tile_n(&mut tw, ux, uy, 30_000);
    tick_tile_n(&mut tw, ux + 1, uy, 30_000);
    assert!(
        matches!(tw.g.tile_at(lvl, ux, uy).kind, TileKind::Snow),
        "tundra snow melted"
    );
    assert!(
        matches!(tw.g.tile_at(lvl, ux + 1, uy).kind, TileKind::SnowTree),
        "tundra snow tree melted"
    );
}

/* --------------------------------- correctness ---------------------------------- */

#[test]
fn worked_tiles_and_warm_ground_never_convert() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    let (fx, fy) = find_cold_fringe(SEED);
    tw.teleport(fx, fy);
    tw.tick_n(4);
    let (day, s) = find_slice(SEED, true);
    pin_clock(&mut tw, day, mid(s));
    tw.g.random = Rng::new(0x5AFE);
    let lvl = tw.current_level;

    // Player-worked and non-natural tiles sit through a blizzard untouched.
    for (i, name) in ["Wood Planks", "farmland", "sand", "dirt"]
        .iter()
        .enumerate()
    {
        let x = fx + i as i32;
        tw.place_at(name, x, fy);
        let before = tw.g.tile_at(lvl, x, fy).name.clone();
        tick_tile_n(&mut tw, x, fy, 20_000);
        let after = tw.g.tile_at(lvl, x, fy).name.clone();
        assert_eq!(before, after, "{name} changed under snowfall");
    }

    // And the climate gate holds: warm-country grass ignores the same snowy slice.
    let (wx, wy) = find_warm_green(SEED);
    tw.teleport(wx, wy);
    tw.tick_n(4);
    tw.events.day_number = day;
    tw.place_at("Grass", wx, wy);
    // (grass ticks sprout tufts naturally — snow-family is what must not appear)
    tick_tile_n(&mut tw, wx, wy, 20_000);
    assert!(
        !matches!(
            tw.g.tile_at(lvl, wx, wy).kind,
            TileKind::Snow | TileKind::SnowTree
        ),
        "warm-country grass converted to snow"
    );
}

/* ------------------------------------- cues ------------------------------------- */

/// First tick in `from..to` where the intensity crosses the cue threshold in the
/// given direction (mirrors the weather test helper).
fn crossing(seed: i64, day: i32, from: i32, to: i32, up: bool) -> i32 {
    let level = |t: i32| weather::schedule_intensity(seed, day, t) >= weather::CUE_THRESHOLD;
    (from..to)
        .find(|&t| level(t - 1) != level(t) && level(t) == up)
        .expect("no threshold crossing in range")
}

#[test]
fn cold_reach_cues_use_the_visiting_snow_flavor() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    let (fx, fy) = find_cold_fringe(SEED);
    tw.teleport(fx, fy);
    tw.tick_n(2);

    let (day, s) = find_slice(SEED, true);
    let quarter = weather::SLICE_LEN / 4;
    let set_in = crossing(
        SEED,
        day,
        s * weather::SLICE_LEN - quarter,
        s * weather::SLICE_LEN + 1,
        true,
    );
    pin_clock(&mut tw, day, set_in - 3);
    tw.tick_n(8);
    assert!(
        tw.notifications
            .iter()
            .any(|n| n.contains("cold creeps in")),
        "no cold-reach set-in cue; got {:?}",
        tw.notifications
    );

    let end = (s + 1) * weather::SLICE_LEN;
    let clears = crossing(SEED, day, end - 1, end + quarter, false);
    pin_clock(&mut tw, day, clears - 3);
    tw.tick_n(8);
    assert!(
        tw.notifications
            .iter()
            .any(|n| n.contains("begins to thaw")),
        "no thaw cue; got {:?}",
        tw.notifications
    );
}

/* --------------------------------- screenshots ---------------------------------- */

/// Play a snowy spell organically on a cold-fringe clearing and screenshot the
/// stages (target/verify/): before, mid-spell (the half-converted frame), end of the
/// spell, and the thaw. Eyeball shots — the asserts only guard "something settled".
#[test]
fn snow_spell_screens() {
    // A day whose daylight slice 1 snows and slices 2..3 are dry (clear thaw run).
    let wet = |d: i32, s: i32| weather::slice_raining(SEED, d, s);
    let day = (1..2000)
        .find(|&d| wet(d, 1) && !wet(d, 0) && !wet(d, 2) && !wet(d, 3))
        .expect("no lone daylight snow slice in 2000 days?");

    let mut tw = TestWorld::infinite().seed(SEED).creative().build();
    let (fx, fy) = find_cold_fringe(SEED);
    tw.teleport(fx, fy);
    tw.tick_n(8);
    let lvl = tw.current_level;

    let count_snow = |tw: &TestWorld| {
        let mut n = 0;
        for y in fy - 8..fy + 8 {
            for x in fx - 12..fx + 12 {
                if matches!(
                    tw.g.tile_at(lvl, x, y).kind,
                    TileKind::Snow | TileKind::SnowTree
                ) {
                    n += 1;
                }
            }
        }
        n
    };
    let run = |tw: &mut TestWorld, n: usize| {
        for _ in 0..n {
            tw.tick_recover();
        }
    };

    // Late slice 0: dry, full daylight.
    pin_clock(&mut tw, day, 9_500);
    let base = count_snow(&tw);
    tw.screenshot("snow_spell_1_before.png");

    // Ride the snow slice: half-way and near the end.
    pin_clock(&mut tw, day, weather::SLICE_LEN + 600);
    run(&mut tw, 4_500);
    let mid_spell = count_snow(&tw);
    tw.screenshot("snow_spell_2_mid.png");
    run(&mut tw, 5_000);
    let end_spell = count_snow(&tw);
    tw.screenshot("snow_spell_3_after.png");

    // Clear weather: the dusting recedes.
    pin_clock(&mut tw, day, 2 * weather::SLICE_LEN + 600);
    run(&mut tw, 12_000);
    let thawed = count_snow(&tw);
    tw.screenshot("snow_spell_4_thaw.png");

    assert!(
        mid_spell > base,
        "no snow settled by mid-spell ({base} -> {mid_spell})"
    );
    assert!(
        end_spell > mid_spell,
        "dusting stopped growing ({mid_spell} -> {end_spell})"
    );
    assert!(
        thawed < end_spell,
        "no thaw in clear weather ({end_spell} -> {thawed})"
    );
}
