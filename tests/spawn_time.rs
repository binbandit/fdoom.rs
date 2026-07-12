//! New-world spawn time: seed-random (spawning at any readable point in the day is a
//! feature), but floored out of the dark band — night and the pre-dawn murk — so a
//! new player's first minutes are never spent in the dark (playtest: dawn tick 0 is
//! "nearly as dark as night").

use fdoom::core::updater::{DAY_LENGTH, SLEEP_END_TIME, Time, in_dark_band};
use fdoom::core::world::{self, new_world_spawn_time};
use fdoom::testutil::bare_game;

#[test]
fn spawn_time_is_never_in_the_dark_band_across_many_seeds() {
    let mut floored = 0;
    let mut spread = std::collections::HashSet::new();
    for seed in -2500..2500i64 {
        let t = new_world_spawn_time(seed);
        assert!(
            (0..DAY_LENGTH).contains(&t),
            "seed {seed}: spawn time {t} outside the day clock"
        );
        assert!(
            !in_dark_band(t),
            "seed {seed}: spawn time {t} is in the dark band"
        );
        assert!(
            t < Time::Night.tick_time(),
            "seed {seed}: spawn time {t} is in the night quarter"
        );
        if t == SLEEP_END_TIME {
            floored += 1;
        }
        spread.insert(t / (DAY_LENGTH / 8)); // eighth-of-day buckets
    }
    // The floor actually fires (the dark band is ~1/3 of the clock)...
    assert!(floored > 500, "floor never engaged: {floored}/5000 seeds");
    // ...but the spawn time stays seed-random, not a constant.
    assert!(
        spread.len() >= 4,
        "spawn times collapsed to too few day-eighths: {spread:?}"
    );
}

#[test]
fn spawn_time_is_deterministic_per_seed() {
    for seed in [0i64, 9, -42, 20260707] {
        assert_eq!(new_world_spawn_time(seed), new_world_spawn_time(seed));
    }
}

#[test]
fn freshly_initialized_worlds_boot_outside_the_dark_band() {
    // The full init path (what a new game actually runs), on a few seeds.
    for seed in [1i64, 9, 777] {
        let mut g = bare_game(&format!("spawn_time_{seed}"));
        world::reset_game(&mut g, true);
        g.settings.set("autosave", false);
        g.world_name = format!("st{seed}");
        g.world_seed = seed;
        world::init_world(&mut g);
        assert!(
            !in_dark_band(g.tick_count),
            "seed {seed}: booted at dark tick {}",
            g.tick_count
        );
        assert_eq!(g.tick_count, new_world_spawn_time(seed), "seed {seed}");
    }
}
