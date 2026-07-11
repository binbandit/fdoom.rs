//! Rare-world-events tests (`core::events`): schedule determinism, the
//! at-most-one-event-per-day guarantee and rate, Hollow Night grave-decay math (both the
//! accelerated night and the quiet week after), the Aurora spawn pause, Ember Rain
//! crater stamping/cooling, Whisper Fog marsh spawn pressure, Caravan supply drops,
//! and the real-calendar seasonal layer (mocked dates).

use fdoom::core::events::{self, Season, WorldEvent};
use fdoom::core::game::Game;
use fdoom::core::updater::{DAY_LENGTH, Time};
use fdoom::entity::EntityKind;
use fdoom::level::infinite_gen::Biome;
use fdoom::level::tile::{TileKind, dispatch};
use fdoom::testutil::{TestWorld, bare_game, find_biome};

/// First day >= `from` whose event matches `want`.
fn find_event_day(seed: i64, from: i32, want: Option<WorldEvent>) -> i32 {
    (from..from + 10_000)
        .find(|&d| events::event_for_day(seed, d) == want)
        .unwrap_or_else(|| panic!("no day with event {want:?} found for seed {seed:#x}"))
}

/// Pin the real calendar to a season-free date so tick-driven tests never pick up an
/// actual Halloween/Christmas window from the host clock.
fn pin_calm_date(g: &mut Game) {
    g.events.date_override = Some((6, 15));
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
    let mut counts = std::collections::HashMap::new();
    for d in 1..=days {
        // `event_for_day` yields at most one event per day by type; assert it is also
        // stable across repeated queries within a day.
        let e = events::event_for_day(seed, d);
        assert_eq!(e, events::event_for_day(seed, d), "day {d} not stable");
        if let Some(e) = e {
            event_days += 1;
            *counts.entry(format!("{e:?}")).or_insert(0) += 1;
        }
    }
    // ~1-in-6 days host an event: expect ~1000 out of 6000, allow a wide band.
    assert!(
        (700..=1300).contains(&event_days),
        "event rate off: {event_days} event days in {days}"
    );
    // The rebalanced roll splits evenly over all five kinds — each must show up.
    for kind in [
        "HollowNight",
        "Aurora",
        "EmberRain",
        "WhisperFog",
        "Caravan",
    ] {
        assert!(
            counts.get(kind).copied().unwrap_or(0) > 0,
            "no {kind} in {days} days (counts: {counts:?})"
        );
    }
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
    let mut g = bare_game("world_events_clock");
    let seed = 0x00C0FFEE_i64;
    g.world_seed = seed;
    pin_calm_date(&mut g);

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
    // in g.warnings (event cues are warning-tier, rendered as the centered band).
    g.events.day_number = find_event_day(seed, 1, Some(WorldEvent::HollowNight));
    g.clear_notifications();
    g.change_time_of_day(Time::Evening);
    events::tick(&mut g);
    assert_eq!(
        g.warnings,
        vec!["The evening is unnaturally still...".to_string()],
        "Hollow Night dusk cue missing"
    );

    // The dawn after: the clock dropping back to morning-0 is itself the day boundary
    // (events::tick counts it), and the quiet-week notification fires.
    g.clear_notifications();
    g.change_time_of_day(Time::Morning);
    events::tick(&mut g);
    assert_eq!(
        g.warnings,
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
    let seed = 0x00C0FFEE_i64;
    let mut g = TestWorld::infinite().seed(seed).build().g;

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
    let seed = 0x00C0FFEE_i64;
    let mut g = TestWorld::infinite().seed(seed).build().g;

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
    let seed = 0x5EED_5EED_i64;
    let mut g = TestWorld::infinite().seed(seed).build().g;
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

/* -------------------------------- seasonal layer ------------------------------------ */

#[test]
fn season_table_boundaries() {
    use events::season_for;
    // Halloween: Oct 24-31 inclusive.
    assert_eq!(season_for(10, 23), Season::None);
    assert_eq!(season_for(10, 24), Season::Halloween);
    assert_eq!(season_for(10, 31), Season::Halloween);
    assert_eq!(season_for(11, 1), Season::None);
    // Christmas: Dec 20-27 inclusive.
    assert_eq!(season_for(12, 19), Season::None);
    assert_eq!(season_for(12, 20), Season::Christmas);
    assert_eq!(season_for(12, 27), Season::Christmas);
    assert_eq!(season_for(12, 28), Season::None);
    // Plain days everywhere else.
    assert_eq!(season_for(1, 1), Season::None);
    assert_eq!(season_for(7, 7), Season::None);
}

#[test]
fn civil_date_conversion_anchors() {
    // Known days-since-1970-01-01 anchors, leap years included.
    assert_eq!(events::civil_month_day(0), (1, 1)); // 1970-01-01
    assert_eq!(events::civil_month_day(789), (2, 29)); // 1972-02-29
    assert_eq!(events::civil_month_day(11_016), (2, 29)); // 2000-02-29
    assert_eq!(events::civil_month_day(11_017), (3, 1)); // 2000-03-01
    assert_eq!(events::civil_month_day(20_750), (10, 24)); // 2026-10-24 (Halloween opens)
}

#[test]
fn mocked_date_drives_season_and_veil_cue() {
    let mut g = bare_game("world_events_season");
    let seed = 0x00C0FFEE_i64;
    g.world_seed = seed;
    g.events.date_override = Some((10, 25));

    // The first scheduler tick picks the season up from the mocked date.
    events::tick(&mut g);
    assert_eq!(g.events.season, Season::Halloween);

    // A day that is calm even through the veil: the dusk cue is the veil whisper alone.
    let calm = (1..10_000)
        .find(|&d| events::event_for_day_in_season(seed, d, Season::Halloween).is_none())
        .unwrap();
    g.events.day_number = calm;
    g.clear_notifications();
    g.change_time_of_day(Time::Evening);
    events::tick(&mut g);
    assert_eq!(
        g.warnings,
        vec!["The veil is thin tonight...".to_string()],
        "Halloween dusk cue missing"
    );

    // Christmas greets the dawn.
    g.events.season = Season::Christmas;
    g.events.date_override = Some((12, 25));
    g.clear_notifications();
    g.change_time_of_day(Time::Morning); // wraps the clock back: a new day begins
    events::tick(&mut g);
    assert_eq!(g.events.season, Season::Christmas, "wrap re-reads the date");
    assert!(
        g.warnings.contains(&"The air feels festive.".to_string()),
        "Christmas dawn cue missing: {:?}",
        g.warnings
    );
}

#[test]
fn halloween_doubles_night_events_only() {
    let seed = 0x00C0FFEE_i64;
    let days = 12_000;
    let (mut base_night, mut hall_night) = (0, 0);
    let (mut base_caravan, mut hall_caravan) = (0, 0);
    for d in 1..=days {
        let b = events::event_for_day_in_season(seed, d, Season::None);
        let h = events::event_for_day_in_season(seed, d, Season::Halloween);
        // The veil only *adds* nights: every base event survives unchanged.
        if let Some(e) = b {
            assert_eq!(h, Some(e), "day {d}: Halloween altered a base event");
        }
        match b {
            Some(e) if e.is_night() => base_night += 1,
            Some(WorldEvent::Caravan) => base_caravan += 1,
            _ => {}
        }
        match h {
            Some(e) if e.is_night() => hall_night += 1,
            Some(WorldEvent::Caravan) => hall_caravan += 1,
            _ => {}
        }
    }
    assert_eq!(
        base_caravan, hall_caravan,
        "the Caravan is a day event; the veil must not touch it"
    );
    let lo = base_night * 17 / 10;
    let hi = base_night * 23 / 10;
    assert!(
        (lo..=hi).contains(&hall_night),
        "night events should land ~twice as often through the veil: {base_night} base vs {hall_night} Halloween in {days} days"
    );
}

#[test]
fn christmas_suppresses_hollow_night() {
    let seed = 0x5EED_5EED_i64;
    let mut suppressed = 0;
    for d in 1..=12_000 {
        let b = events::event_for_day(seed, d);
        let c = events::event_for_day_in_season(seed, d, Season::Christmas);
        assert_ne!(
            c,
            Some(WorldEvent::HollowNight),
            "day {d}: Hollow Night must not land during Christmas"
        );
        if b == Some(WorldEvent::HollowNight) {
            assert_eq!(c, None, "day {d}: a suppressed Hollow Night is a calm day");
            suppressed += 1;
        } else {
            assert_eq!(c, b, "day {d}: Christmas must only touch Hollow Nights");
        }
    }
    assert!(suppressed > 0, "no Hollow Night found to suppress");
}

/* --------------------------------- ember rain --------------------------------------- */

/// Counts (lava, ore) tiles in the box around `(cx, cy)`.
fn count_lava_ore(g: &Game, lvl: usize, cx: i32, cy: i32, r: i32) -> (usize, usize) {
    let (mut lava, mut ore) = (0, 0);
    for y in cy - r..=cy + r {
        for x in cx - r..=cx + r {
            match g.tile_at(lvl, x, y).name.as_str() {
                "LAVA" => lava += 1,
                "IRON ORE" | "GOLD ORE" => ore += 1,
                _ => {}
            }
        }
    }
    (lava, ore)
}

#[test]
fn ember_rain_stamps_craters_that_cool_at_dawn() {
    let seed = 0x00C0FFEE_i64;
    let mut g = TestWorld::infinite().seed(seed).build().g;
    pin_calm_date(&mut g);

    g.events.day_number = find_event_day(seed, 1, Some(WorldEvent::EmberRain));
    g.change_time_of_day(Time::Night);
    assert!(events::ember_rain_active(&g));

    let p = g.try_player().expect("player exists");
    let (lvl, ptx, pty) = (p.c.level.unwrap(), p.c.x >> 4, p.c.y >> 4);
    assert_eq!(g.level(lvl).depth, 0, "player starts on the surface");
    let scan_r = 70; // covers the impact span (+-64 tiles) plus the crater ring

    let (lava0, ore0) = count_lava_ore(&g, lvl, ptx, pty, scan_r);
    let night0 = Time::Night.tick_time();
    for i in 1..=8000 {
        g.set_time(night0 + i);
        events::tick(&mut g);
    }
    let (lava1, ore1) = count_lava_ore(&g, lvl, ptx, pty, scan_r);
    assert!(
        lava1 > lava0,
        "no lava specks stamped in an ember-rain night ({lava0} -> {lava1})"
    );
    assert!(
        ore1 > ore0,
        "no ore stamped in an ember-rain night ({ore0} -> {ore1})"
    );

    // Dawn: the clock wraps, the event is over, every speck cools to rock.
    g.set_time(DAY_LENGTH);
    events::tick(&mut g);
    assert!(!events::ember_rain_active(&g));
    let (lava2, ore2) = count_lava_ore(&g, lvl, ptx, pty, scan_r);
    assert_eq!(lava2, lava0, "lava specks must cool to rock at dawn");
    assert_eq!(ore2, ore1, "the ore and rock stay after the night");
}

/* --------------------------------- the caravan -------------------------------------- */

fn queued_item_drops(g: &Game, lvl: usize) -> usize {
    g.level(lvl)
        .entities_to_add
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::ItemEntity(_)))
        .count()
}

#[test]
fn caravan_drops_supplies_near_a_trail() {
    let seed = 0x00C0FFEE_i64;
    let mut g = TestWorld::infinite().seed(seed).build().g;
    pin_calm_date(&mut g);

    g.events.day_number = find_event_day(seed, 1, Some(WorldEvent::Caravan));
    g.change_time_of_day(Time::Day);
    assert!(events::caravan_active(&g));

    // Fabricate a worn trail two tiles below the player.
    let p = g.try_player().expect("player exists");
    let (lvl, ptx, pty) = (p.c.level.unwrap(), p.c.x >> 4, p.c.y >> 4);
    let dirt = g.tiles.get("dirt");
    for dx in -3..=3 {
        g.set_tile_default(lvl, ptx + dx, pty + 2, &dirt);
    }

    let before = queued_item_drops(&g, lvl);
    g.clear_notifications();
    let day0 = Time::Day.tick_time();
    for i in 1..=1000 {
        g.set_time(day0 + i);
        events::tick(&mut g);
    }
    let dropped = queued_item_drops(&g, lvl) - before;
    assert!(
        dropped >= 1,
        "no supply drop in 1000 ticks beside a trail on a Caravan day"
    );
    assert!(
        g.notifications
            .contains(&"You find supplies dropped along the old trail.".to_string()),
        "supply notification missing: {:?}",
        g.notifications
    );
    // Every drop is one of the caravan's goods.
    for e in g.level(lvl).entities_to_add.iter().skip(before) {
        if let EntityKind::ItemEntity(d) = &e.kind {
            let name = d.item.get_name().to_uppercase();
            assert!(
                ["TORCH", "WOOD", "STONE", "BREAD"].contains(&name.as_str()),
                "unexpected caravan good: {name}"
            );
        }
    }

    // Control: same trail, same hours, but a calm day — nothing drops.
    g.events.day_number = find_event_day(seed, 1, None);
    assert!(!events::caravan_active(&g));
    g.clear_notifications();
    let before = queued_item_drops(&g, lvl);
    let from = g.tick_count;
    for i in 1..=1000 {
        g.set_time(from + i);
        events::tick(&mut g);
    }
    assert_eq!(
        queued_item_drops(&g, lvl),
        before,
        "supplies dropped on a non-Caravan day"
    );
}

/* --------------------------------- whisper fog -------------------------------------- */

#[test]
fn whisper_fog_doubles_marsh_spawn_pressure() {
    let seed = 0x5EED_5EED_i64;
    let mut g = TestWorld::infinite().seed(seed).build().g;
    pin_calm_date(&mut g);
    let lvl = g.current_level;

    g.events.day_number = find_event_day(seed, 1, Some(WorldEvent::WhisperFog));
    g.change_time_of_day(Time::Night);
    assert!(events::whisper_fog_active(&g));

    // In a marsh, the fog runs a second spawn pass; elsewhere it does not.
    let (mx, my) = find_biome(seed, Biome::Marsh);
    {
        let p = g.player_mut();
        p.c.x = mx * 16 + 8;
        p.c.y = my * 16 + 8;
    }
    assert_eq!(events::spawn_passes(&g, lvl), 2, "fog night in a marsh");
    let (px, py) = find_biome(seed, Biome::Plains);
    {
        let p = g.player_mut();
        p.c.x = px * 16 + 8;
        p.c.y = py * 16 + 8;
    }
    assert_eq!(events::spawn_passes(&g, lvl), 1, "fog night outside marsh");

    // The other gates through the same function.
    g.events.day_number = find_event_day(seed, 1, Some(WorldEvent::Aurora));
    assert_eq!(events::spawn_passes(&g, lvl), 0, "aurora pauses spawning");
    g.events.day_number = find_event_day(seed, 1, None);
    assert_eq!(events::spawn_passes(&g, lvl), 1, "calm night");

    // And the whispers reach the player through the night.
    g.events.day_number = find_event_day(seed, 1, Some(WorldEvent::WhisperFog));
    g.clear_notifications();
    let night0 = Time::Night.tick_time();
    for i in 1..=2100 {
        g.set_time(night0 + i);
        events::tick(&mut g);
    }
    assert!(
        g.warnings
            .contains(&"You hear whispers in the fog...".to_string()),
        "no whisper in {} fog-night ticks: {:?}",
        2100,
        g.warnings
    );
}
