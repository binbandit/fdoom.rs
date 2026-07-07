//! Rare-world-events foundation tests (`core::events`): schedule determinism, the
//! at-most-one-event-per-day guarantee and rate, Hollow Night grave-decay math (both the
//! accelerated night and the quiet week after), and the Aurora spawn pause.

use std::path::{Path, PathBuf};

use fdoom::core::events::{self, WorldEvent};
use fdoom::core::game::Game;
use fdoom::core::updater::{DAY_LENGTH, Time};
use fdoom::core::world;
use fdoom::level::tile::{TileKind, dispatch};

fn temp_game_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A headless game with the main player created (as `Game.main` does before world init).
fn new_game(dir: &Path) -> Game {
    let mut g = Game::new(false, false, dir.to_path_buf());
    let mut player = fdoom::entity::mob::player::new(&g, None);
    player.c.eid = 0;
    g.entities.put_back(player);
    g
}

/// Build a fresh 128x128 world for the given seed.
fn make_world(g: &mut Game, seed: i64) {
    world::reset_game(g, false);
    g.settings.set("size", 128);
    g.settings.set("autosave", false);
    g.world_seed = seed;
    world::init_world(g);
}

/// First day >= `from` whose event matches `want`.
fn find_event_day(seed: i64, from: i32, want: Option<WorldEvent>) -> i32 {
    (from..from + 10_000)
        .find(|&d| events::event_for_day(seed, d) == want)
        .unwrap_or_else(|| panic!("no day with event {want:?} found for seed {seed:#x}"))
}

/* ----------------------------------- pure schedule ---------------------------------- */

#[test]
fn event_schedule_is_deterministic_per_seed() {
    for seed in [0_i64, 0x00C0FFEE, -12345, i64::MAX] {
        let a: Vec<_> = (0..2000).map(|d| events::event_for_day(seed, d)).collect();
        let b: Vec<_> = (0..2000).map(|d| events::event_for_day(seed, d)).collect();
        assert_eq!(a, b, "schedule for seed {seed:#x} is not reproducible");
    }
    // Different seeds shuffle the calendar (astronomically unlikely to collide).
    let a: Vec<_> = (0..2000)
        .map(|d| events::event_for_day(0x00C0FFEE, d))
        .collect();
    let b: Vec<_> = (0..2000)
        .map(|d| events::event_for_day(0x5EED_5EED, d))
        .collect();
    assert_ne!(a, b, "two seeds produced an identical 2000-day calendar");
}

#[test]
fn at_most_one_event_per_day_with_sane_rate() {
    let seed = 0x00C0FFEE_i64;
    // Day 0 (a fresh world's first day) is always calm.
    assert_eq!(events::event_for_day(seed, 0), None);

    let days = 6000;
    let mut event_days = 0;
    let mut hollow = 0;
    let mut aurora = 0;
    for d in 1..=days {
        // `event_for_day` yields at most one event per day by type; assert it is also
        // stable across repeated queries within a day.
        let e = events::event_for_day(seed, d);
        assert_eq!(e, events::event_for_day(seed, d), "day {d} not stable");
        match e {
            Some(WorldEvent::HollowNight) => {
                event_days += 1;
                hollow += 1;
            }
            Some(WorldEvent::Aurora) => {
                event_days += 1;
                aurora += 1;
            }
            None => {}
        }
    }
    // ~1-in-6 days host an event: expect ~1000 out of 6000, allow a wide band.
    assert!(
        (700..=1300).contains(&event_days),
        "event rate off: {event_days} event days in {days}"
    );
    assert!(hollow > 0, "no Hollow Night in {days} days");
    assert!(aurora > 0, "no Aurora in {days} days");
}

#[test]
fn quiet_week_window_math() {
    let seed = 0x5EED_5EED_i64;
    // An isolated Hollow Night: none of the following QUIET_WEEK_DAYS + 1 days may host
    // another one, so the window edge is unambiguous.
    let h = (1..10_000)
        .find(|&d| {
            events::event_for_day(seed, d) == Some(WorldEvent::HollowNight)
                && (1..=events::QUIET_WEEK_DAYS + 1)
                    .all(|k| events::event_for_day(seed, d + k) != Some(WorldEvent::HollowNight))
        })
        .expect("no isolated Hollow Night found");

    assert!(
        !events::grave_decay_suppressed_on(seed, h),
        "the Hollow Night day itself must not be suppressed (graves crumble that night)"
    );
    for k in 1..=events::QUIET_WEEK_DAYS {
        assert!(
            events::grave_decay_suppressed_on(seed, h + k),
            "day {k} after a Hollow Night should be in the quiet week"
        );
    }
    assert!(
        !events::grave_decay_suppressed_on(seed, h + events::QUIET_WEEK_DAYS + 1),
        "quiet week must end after {} days",
        events::QUIET_WEEK_DAYS
    );
}

/* ------------------------------ day counter + cues ---------------------------------- */

#[test]
fn day_counter_wraps_and_dusk_cue_fires() {
    let dir = temp_game_dir("world_events_clock");
    let mut g = new_game(&dir);
    let seed = 0x00C0FFEE_i64;
    g.world_seed = seed;

    // Walk the day clock through a midnight wrap; events::tick counts the day.
    events::tick(&mut g); // snapshot tick 0
    assert_eq!(g.events.day_number, 0);
    g.set_time(DAY_LENGTH - 1);
    events::tick(&mut g);
    assert_eq!(g.events.day_number, 0, "no wrap yet");
    g.set_time(DAY_LENGTH); // wraps to morning, tick_count 0
    events::tick(&mut g);
    assert_eq!(g.events.day_number, 1, "midnight wrap must advance the day");

    // Jump the counter to a Hollow Night day and cross into evening: the dusk cue lands
    // in g.notifications.
    g.events.day_number = find_event_day(seed, 1, Some(WorldEvent::HollowNight));
    g.notifications.clear();
    g.change_time_of_day(Time::Evening);
    events::tick(&mut g);
    assert_eq!(
        g.notifications,
        vec!["The evening is unnaturally still...".to_string()],
        "Hollow Night dusk cue missing"
    );

    // The dawn after: the clock dropping back to morning-0 is itself the day boundary
    // (events::tick counts it), and the quiet-week notification fires.
    g.notifications.clear();
    g.change_time_of_day(Time::Morning);
    events::tick(&mut g);
    assert_eq!(
        g.notifications,
        vec!["Dawn breaks. The graves lie quiet.".to_string()],
        "Hollow Night dawn cue missing"
    );
}

/* ------------------------------- grave decay math ----------------------------------- */

/// Places an unbroken grave at (10,10) on the current level and returns its coords.
fn place_grave(g: &mut Game) -> (usize, i32, i32) {
    let lvl = g.current_level;
    let grave = g.tiles.get_id(43);
    g.set_tile_default(lvl, 10, 10, &grave);
    (lvl, 10, 10)
}

fn grave_broken(g: &Game, lvl: usize, x: i32, y: i32) -> bool {
    matches!(
        g.tile_at(lvl, x, y).kind,
        TileKind::GraveStone { broken: true }
    )
}

#[test]
fn hollow_night_greatly_accelerates_grave_decay() {
    let dir = temp_game_dir("world_events_hollow");
    let seed = 0x00C0FFEE_i64;
    let mut g = new_game(&dir);
    make_world(&mut g, seed);

    g.events.day_number = find_event_day(seed, 1, Some(WorldEvent::HollowNight));
    g.change_time_of_day(Time::Night);
    assert!(events::hollow_night_active(&g));

    let (lvl, x, y) = place_grave(&mut g);
    let mut ticks_to_crumble = None;
    for i in 0..300 {
        let tile = g.tile_at(lvl, x, y);
        dispatch::tick(&mut g, &tile, lvl, x, y);
        if grave_broken(&g, lvl, x, y) {
            ticks_to_crumble = Some(i + 1);
            break;
        }
    }
    // 1-in-3 per tick: P(surviving 300 ticks) ~ (2/3)^300 — effectively impossible.
    // A night gives every tile ~324 random ticks, so the cemetery collapses well
    // before dawn ("greatly accelerates" vs. the normal single 1-in-6 roll per night).
    assert!(
        ticks_to_crumble.is_some(),
        "grave survived 300 hollow-night ticks"
    );
}

#[test]
fn quiet_week_suppresses_grave_decay() {
    let dir = temp_game_dir("world_events_quiet");
    let seed = 0x00C0FFEE_i64;
    let mut g = new_game(&dir);
    make_world(&mut g, seed);

    // A day inside a quiet week that is not itself a Hollow Night (acceleration would
    // otherwise mask the suppression).
    let h = find_event_day(seed, 1, Some(WorldEvent::HollowNight));
    let day = (h + 1..=h + events::QUIET_WEEK_DAYS)
        .find(|&d| events::event_for_day(seed, d) != Some(WorldEvent::HollowNight))
        .expect("a quiet-week day exists");
    g.events.day_number = day;
    g.change_time_of_day(Time::Night);
    assert!(events::grave_decay_suppressed(&g));
    assert!(!events::hollow_night_active(&g));

    let (lvl, x, y) = place_grave(&mut g);
    for _ in 0..500 {
        let tile = g.tile_at(lvl, x, y);
        dispatch::tick(&mut g, &tile, lvl, x, y);
    }
    assert!(
        !grave_broken(&g, lvl, x, y),
        "grave crumbled during the quiet week"
    );
    assert_eq!(
        g.level(lvl).get_data(x, y),
        0,
        "suppressed nights must not even consume the once-per-night roll flag"
    );
}

/* -------------------------------- aurora spawn pause -------------------------------- */

#[test]
fn aurora_pauses_mob_spawning() {
    let dir = temp_game_dir("world_events_aurora");
    let seed = 0x5EED_5EED_i64;
    let mut g = new_game(&dir);
    make_world(&mut g, seed);
    let lvl = g.current_level;

    // Aurora night: try_spawn must add nothing, ever.
    g.events.day_number = find_event_day(seed, 1, Some(WorldEvent::Aurora));
    g.change_time_of_day(Time::Night);
    assert!(events::aurora_active(&g));
    assert!(events::mobs_docile(&g));
    let before = g.level(lvl).entities_to_add.len();
    for _ in 0..500 {
        fdoom::level::try_spawn(&mut g, lvl);
    }
    assert_eq!(
        g.level(lvl).entities_to_add.len(),
        before,
        "mobs spawned during an aurora night"
    );

    // Control: a calm night spawns as usual (surface zombies etc.).
    g.events.day_number = find_event_day(seed, 1, None);
    g.change_time_of_day(Time::Night);
    assert!(!events::aurora_active(&g));
    let before = g.level(lvl).entities_to_add.len();
    for _ in 0..500 {
        fdoom::level::try_spawn(&mut g, lvl);
    }
    assert!(
        g.level(lvl).entities_to_add.len() > before,
        "control night spawned nothing in 500 attempts"
    );
}
