//! Gentle thirst (UI_REDESIGN L6): the slow companion stat. Pins the approachability
//! floors — drain meaningfully slower than hunger, the worst-of drag composition,
//! chip damage that stops at 4 hearts — plus the drinking deltas, the HUD row in its
//! reserved slot, the SELF pane's WATER row, and the tolerant save marker.

use fdoom::core::temperature;
use fdoom::core::updater::DAY_LENGTH;
use fdoom::entity::Entity;
use fdoom::entity::mob::player::{MAX_HUNGER_STAMS, MAX_HUNGER_TICKS, MAX_STAT, MAX_THIRST};
use fdoom::entity::mob::player_behavior::{
    self, BOTTLE_THIRST, HAND_DRINK_QUEASY_IN, HAND_DRINK_THIRST, SPRING_DRINK_THIRST,
    THIRST_DAMAGE_FLOOR, THIRST_DAMAGE_PERIOD, THIRST_HOT_MULT, THIRST_LOW, THIRST_UNIT_TICKS,
};
use fdoom::gfx::screen;
use fdoom::item::{PotionType, interact, registry};
use fdoom::saveload::{load::Load, save};
use fdoom::testutil::{TestWorld, bare_game};

const SEED: i64 = 20260707;

/// Take the player out of the arena (the take-out tick shape) and hand it to `f`.
fn with_player(tw: &mut TestWorld, f: impl FnOnce(&mut fdoom::core::game::Game, &mut Entity)) {
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    f(&mut tw.g, &mut player);
    tw.g.entities.put_back(player);
}

/// Drive one thirst step with pinned band steps / swimming on a pinned `game_time`.
fn thirst_step(tw: &mut TestWorld, steps: i32, swimming: bool, game_time: i32) {
    tw.g.game_time = game_time;
    with_player(tw, |g, p| {
        p.player_mut().mob.hurt_time = 0; // no hurt i-frames between pinned steps
        player_behavior::apply_thirst_effects(g, p, steps, swimming);
    });
}

/// Run `n` consecutive drain steps (game_time advances so cadence gates behave).
fn drain_n(tw: &mut TestWorld, steps: i32, swimming: bool, n: i32) {
    for t in 0..n {
        thirst_step(tw, steps, swimming, t + 1);
    }
}

fn thirst(tw: &TestWorld) -> i32 {
    tw.g.player().player().thirst
}

fn set_thirst(tw: &mut TestWorld, v: i32) {
    let pd = tw.g.player_mut().player_mut();
    pd.thirst = v;
    pd.thirst_tick = 0;
    pd.thirst_cued = false;
}

/* ----------------------------------- drain rates ----------------------------------- */

#[test]
fn drain_is_meaningfully_slower_than_hunger_and_spares_day_one() {
    // Pinned ratio: an exerting player (stamina below max, normal difficulty)
    // loses a hunger unit every MAX_HUNGER_TICKS * MAX_HUNGER_STAMS[1] ticks;
    // thirst must take at least 3x longer per unit.
    let hunger_exert_unit = MAX_HUNGER_TICKS * MAX_HUNGER_STAMS[1];
    assert!(
        THIRST_UNIT_TICKS >= 3 * hunger_exert_unit,
        "thirst unit ({THIRST_UNIT_TICKS}) must be >= 3x hunger's exertion unit ({hunger_exert_unit})"
    );

    // A kid bumbles through day one: full-to-low (where effects begin) spans about
    // one full day of the day clock — never less than 90% of it.
    let full_to_low = (MAX_THIRST - THIRST_LOW) * THIRST_UNIT_TICKS;
    assert!(
        full_to_low * 10 >= DAY_LENGTH * 9,
        "thirst must not matter before ~a day: {full_to_low} < 0.9 * {DAY_LENGTH}"
    );
    assert!(
        full_to_low <= DAY_LENGTH * 2,
        "but it should matter eventually: {full_to_low} > 2 days"
    );

    // Behavior over simulated hours: exactly one unit per THIRST_UNIT_TICKS of
    // comfortable play, no drift.
    let mut tw = TestWorld::infinite().seed(SEED).build();
    set_thirst(&mut tw, MAX_THIRST);
    drain_n(&mut tw, 0, false, THIRST_UNIT_TICKS);
    assert_eq!(thirst(&tw), MAX_THIRST - 1, "one unit per drain window");
    drain_n(&mut tw, 0, false, THIRST_UNIT_TICKS);
    assert_eq!(thirst(&tw), MAX_THIRST - 2);
}

#[test]
fn hot_bands_accelerate_cold_bands_and_swimming_pause() {
    let mut tw = TestWorld::infinite().seed(SEED).build();

    // Hot/Scorching: the accumulator runs THIRST_HOT_MULT-fast — the desert asks
    // for water.
    set_thirst(&mut tw, MAX_THIRST);
    drain_n(&mut tw, 2, false, THIRST_UNIT_TICKS / THIRST_HOT_MULT);
    assert_eq!(thirst(&tw), MAX_THIRST - 1, "hot band drains 2x");

    // Chilly (one band out, e.g. cave-cool) still drains at the normal rate...
    set_thirst(&mut tw, MAX_THIRST);
    drain_n(&mut tw, -1, false, THIRST_UNIT_TICKS);
    assert_eq!(thirst(&tw), MAX_THIRST - 1, "chilly still drains");

    // ...but the Cold/Freezing bands never drain, however long.
    set_thirst(&mut tw, MAX_THIRST);
    drain_n(&mut tw, -2, false, 2 * THIRST_UNIT_TICKS);
    assert_eq!(thirst(&tw), MAX_THIRST, "cold bands pause the drain");
    drain_n(&mut tw, -3, false, 2 * THIRST_UNIT_TICKS);
    assert_eq!(thirst(&tw), MAX_THIRST, "freezing pauses the drain");

    // Swimming pauses it too, even under a hot sun.
    set_thirst(&mut tw, MAX_THIRST);
    drain_n(&mut tw, 2, true, 2 * THIRST_UNIT_TICKS);
    assert_eq!(thirst(&tw), MAX_THIRST, "swimming pauses the drain");
}

/* ---------------------------------- effects ladder ---------------------------------- */

#[test]
fn low_band_drags_recharge_worst_of_composed() {
    let mut tw = TestWorld::infinite().seed(SEED).build();

    // 10..=4: nothing — recharge untouched even on a drag-cadence tick.
    set_thirst(&mut tw, THIRST_LOW + 1);
    tw.g.player_mut().player_mut().stamina_recharge = 6;
    thirst_step(&mut tw, 0, false, 3);
    assert_eq!(
        tw.g.player().player().stamina_recharge,
        6,
        "no drag above low"
    );

    // 3..=1: the ~2/3 drag on the cadence tick.
    set_thirst(&mut tw, THIRST_LOW);
    thirst_step(&mut tw, 0, false, 6);
    assert_eq!(
        tw.g.player().player().stamina_recharge,
        5,
        "low thirst drags"
    );
    thirst_step(&mut tw, 0, false, 7); // off-cadence: nothing
    assert_eq!(tw.g.player().player().stamina_recharge, 5);

    // Worst-of with Queasy: while a Queasy stomach is already halving recharge
    // (in the main player tick), the thirst drag stands down entirely.
    tw.g.player_mut()
        .player_mut()
        .potioneffects
        .insert(PotionType::Queasy, 600);
    thirst_step(&mut tw, 0, false, 9);
    assert_eq!(
        tw.g.player().player().stamina_recharge,
        5,
        "queasy wins the worst-of; thirst never stacks on it"
    );
    tw.g.player_mut().player_mut().potioneffects.clear();

    // Worst-of with the temperature drag: on a tick where the Hot-band drag runs,
    // running BOTH mechanisms costs exactly one point, not two.
    tw.g.player_mut().player_mut().stamina_recharge = 6;
    tw.g.game_time = 12;
    with_player(&mut tw, |g, p| {
        p.player_mut().mob.hurt_time = 0;
        player_behavior::apply_temperature_effects(g, p, 2.0); // Hot: temp drag fires
        player_behavior::apply_thirst_effects(g, p, 2, false); // thirst stands down
    });
    assert_eq!(
        tw.g.player().player().stamina_recharge,
        5,
        "hot + parched must cost one drag, never two"
    );
}

#[test]
fn dry_throat_cue_fires_on_band_entry_only() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    tw.notifications.clear();

    let cue_count = |tw: &TestWorld| {
        tw.notifications
            .iter()
            .filter(|n| n.contains("throat is dry"))
            .count()
    };

    set_thirst(&mut tw, THIRST_LOW + 1);
    thirst_step(&mut tw, 0, false, 1);
    assert_eq!(cue_count(&tw), 0, "no cue above the low band");

    // enter the band: one cue
    tw.g.player_mut().player_mut().thirst = THIRST_LOW;
    thirst_step(&mut tw, 0, false, 2);
    assert_eq!(cue_count(&tw), 1, "one cue on entry");

    // deeper and repeated ticks: still one
    tw.g.player_mut().player_mut().thirst = 1;
    for t in 3..40 {
        thirst_step(&mut tw, 0, false, t);
    }
    assert_eq!(cue_count(&tw), 1, "the cue never repeats inside the band");

    // recover, then re-enter: it may speak once more
    tw.g.player_mut().player_mut().thirst = THIRST_LOW + 2;
    thirst_step(&mut tw, 0, false, 41);
    tw.g.player_mut().player_mut().thirst = THIRST_LOW;
    thirst_step(&mut tw, 0, false, 43);
    assert_eq!(cue_count(&tw), 2, "re-entry after recovery cues again");
}

#[test]
fn parched_damage_floors_at_four_hearts_and_stops() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    set_thirst(&mut tw, 0);

    let health = |tw: &TestWorld| tw.g.player().player().mob.health;

    // off-cadence ticks never damage
    thirst_step(&mut tw, 0, false, THIRST_DAMAGE_PERIOD + 1);
    assert_eq!(health(&tw), 10);
    // one heart per cadence tick
    thirst_step(&mut tw, 0, false, THIRST_DAMAGE_PERIOD);
    assert_eq!(health(&tw), 9, "slow chip damage while parched");

    // the floor: damage walks health down to 4 hearts and then stops for good —
    // no DEADLY_SCORE-style override exists; thirst never kills.
    for k in 2..20 {
        tw.g.player_mut().player_mut().thirst = 0;
        thirst_step(&mut tw, 0, false, THIRST_DAMAGE_PERIOD * k);
    }
    assert_eq!(
        health(&tw),
        THIRST_DAMAGE_FLOOR,
        "parched damage must floor at {THIRST_DAMAGE_FLOOR} hearts"
    );
    assert_eq!(THIRST_DAMAGE_FLOOR, 4, "the documented floor");

    // starting at (or under) the floor: untouchable
    tw.g.player_mut().player_mut().mob.health = 3;
    for k in 20..30 {
        thirst_step(&mut tw, 0, false, THIRST_DAMAGE_PERIOD * k);
    }
    assert_eq!(health(&tw), 3, "below the floor nothing chips");
}

/* ------------------------------------- drinking ------------------------------------- */

#[test]
fn water_bottle_restores_thirst_and_stamina() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    set_thirst(&mut tw, 2);
    tw.g.player_mut().player_mut().stamina = 5;

    let mut bottle = registry::get(&tw.g, "Water Bottle");
    with_player(&mut tw, |g, p| {
        assert!(interact::item_interact_on_tile(
            g,
            &mut bottle,
            0,
            0,
            0,
            p,
            fdoom::entity::Direction::Down
        ));
    });
    assert_eq!(thirst(&tw), 2 + BOTTLE_THIRST, "+6 thirst per bottle");
    assert_eq!(
        tw.g.player().player().stamina,
        9,
        "the +4 stamina sip stays"
    );
    assert_eq!(bottle.get_name(), "Empty Bottle", "the bottle empties");

    // a fully fresh player refuses to waste the water
    set_thirst(&mut tw, MAX_THIRST);
    tw.g.player_mut().player_mut().stamina = 10;
    let mut bottle = registry::get(&tw.g, "Water Bottle");
    with_player(&mut tw, |g, p| {
        assert!(!interact::item_interact_on_tile(
            g,
            &mut bottle,
            0,
            0,
            0,
            p,
            fdoom::entity::Direction::Down
        ));
    });
    assert_eq!(bottle.get_name(), "Water Bottle", "not consumed when fresh");

    // thirst alone (full stamina) is reason enough to drink
    set_thirst(&mut tw, 6);
    let mut bottle = registry::get(&tw.g, "Water Bottle");
    with_player(&mut tw, |g, p| {
        assert!(interact::item_interact_on_tile(
            g,
            &mut bottle,
            0,
            0,
            0,
            p,
            fdoom::entity::Direction::Down
        ));
    });
    assert_eq!(thirst(&tw), MAX_THIRST, "capped at full");
}

#[test]
fn handless_drinking_deltas_and_queasy_odds() {
    let mut tw = TestWorld::infinite().seed(SEED).build();

    // spring water: always safe, always +3
    let trials = 200;
    for i in 0..trials {
        set_thirst(&mut tw, 5);
        with_player(&mut tw, |g, p| {
            player_behavior::drink_from_water(g, p, true)
        });
        assert_eq!(thirst(&tw), 5 + SPRING_DRINK_THIRST, "spring drink is +3");
        assert!(
            !tw.g
                .player()
                .player()
                .potioneffects
                .contains_key(&PotionType::Queasy),
            "spring water is always safe (trial {i})"
        );
        tw.notifications.clear();
    }

    // ordinary water: +2 with a 1-in-6 mild Queasy gamble
    assert_eq!(HAND_DRINK_QUEASY_IN, 6, "the documented odds");
    let trials = 600;
    let mut queasy_hits = 0;
    for _ in 0..trials {
        set_thirst(&mut tw, 5);
        tw.g.player_mut().player_mut().potioneffects.clear();
        with_player(&mut tw, |g, p| {
            player_behavior::drink_from_water(g, p, false)
        });
        assert_eq!(thirst(&tw), 5 + HAND_DRINK_THIRST, "hand drink is +2");
        if tw
            .g
            .player()
            .player()
            .potioneffects
            .contains_key(&PotionType::Queasy)
        {
            queasy_hits += 1;
        }
        tw.notifications.clear();
    }
    // deterministic world rng; a wide band around trials/6 = 100 catches a broken gate
    assert!(
        (60..=140).contains(&queasy_hits),
        "queasy odds off 1-in-6: {queasy_hits}/{trials}"
    );
}

#[test]
fn empty_hands_at_water_drink_through_the_attack_key() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    // face a spring tile and swing with empty hands
    tw.place("Spring Water", 1, 0);
    {
        let pd = tw.g.player_mut().player_mut();
        pd.mob.dir = fdoom::entity::Direction::Right;
        pd.active_item = None;
    }
    set_thirst(&mut tw, 4);
    with_player(&mut tw, player_behavior::attack);
    assert_eq!(
        thirst(&tw),
        4 + SPRING_DRINK_THIRST,
        "attack at a spring drinks"
    );

    // at full thirst the same swing passes through (no drink, no consumption)
    set_thirst(&mut tw, MAX_THIRST);
    with_player(&mut tw, player_behavior::attack);
    assert_eq!(thirst(&tw), MAX_THIRST);
}

#[test]
fn broths_and_honey_count_as_drinks() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    for (dish, bonus) in [("Hearty Stew", 2), ("Fish Chowder", 2), ("Honey Jar", 1)] {
        set_thirst(&mut tw, 4);
        {
            let pd = tw.g.player_mut().player_mut();
            pd.hunger = 2; // room to eat
            pd.stamina = 10; // stamina to pay
            pd.potioneffects.clear();
        }
        let mut item = registry::get(&tw.g, dish);
        with_player(&mut tw, |g, p| {
            assert!(
                interact::item_interact_on_tile(
                    g,
                    &mut item,
                    0,
                    0,
                    0,
                    p,
                    fdoom::entity::Direction::Down
                ),
                "{dish} should be eatable"
            );
        });
        assert_eq!(thirst(&tw), 4 + bonus, "{dish} thirst bonus");
    }
}

/* -------------------------------------- the HUD -------------------------------------- */

fn px(pixels: &[i32], (x, y): (i32, i32)) -> i32 {
    pixels[(y * screen::W + x) as usize]
}

/// The reserved slot's probe (hud_qol geometry): inside the thirst strip at y=182,
/// clear of the food strip's 2px overlap rows.
const THIRST_PROBE: (i32, i32) = (2, 186);

#[test]
fn hud_row_hides_at_full_lingers_and_pulses_low() {
    let mut tw = TestWorld::infinite().name("thirst_hud").build();
    tw.g.tick_count = DAY_LENGTH * 3 / 8; // midday: pulse-on phase (24300/15 even), bright backdrop
    {
        let pd = tw.g.player_mut().player_mut();
        pd.mob.health = 10;
        pd.stamina = 10;
        pd.stamina_recharge_delay = 0;
        pd.hunger = 10;
        pd.thirst = MAX_THIRST;
    }
    let base = tw.render(); // primes the HUD memory: full + settled = no rows

    // spend some water: the row appears in the reserved slot
    tw.g.player_mut().player_mut().thirst = 4;
    let spent = tw.render();
    tw.screenshot("thirst_hud_spent.png");
    assert_ne!(
        px(&spent, THIRST_PROBE),
        px(&base, THIRST_PROBE),
        "thirst row should appear once below full"
    );

    // at/below 30% (3 droplets) the 1px white pulse underline shows
    tw.g.player_mut().player_mut().thirst = 3;
    let low = tw.render();
    assert_eq!(
        px(&low, (10, 190)),
        0xF0F0F0,
        "low thirst should draw the pulse underline"
    );

    // refill: the row lingers ~90 frames, then tucks away
    tw.g.player_mut().player_mut().thirst = MAX_THIRST;
    let lingering = tw.render();
    assert_ne!(
        px(&lingering, THIRST_PROBE),
        px(&base, THIRST_PROBE),
        "a just-changed meter lingers briefly even at full"
    );
    for _ in 0..91 {
        tw.render();
    }
    let settled = tw.render();
    assert_eq!(
        px(&settled, THIRST_PROBE),
        px(&base, THIRST_PROBE),
        "refilled thirst row must hide after the linger window"
    );
}

#[test]
fn self_pane_shows_the_water_row() {
    let mut tw = TestWorld::infinite().name("thirst_self").build();
    tw.g.tick_count = 30;
    tw.g.player_mut().player_mut().thirst = 6;
    tw.press("E");
    for _ in 0..3 {
        tw.press("RIGHT"); // PACK -> WEAR -> CRAFT -> SELF
    }
    let at_six = tw.render();
    tw.screenshot("thirst_self_pane.png");

    // the WATER row is the 4th meter row: icon at x=24..32, y=84..92. The droplet
    // stand-in renders in water blues — some pixel there must be clearly blue.
    let mut found_blue = false;
    for y in 84..92 {
        for x in 24..32 {
            let p = px(&at_six, (x, y));
            let (r, b) = ((p >> 16) & 0xFF, p & 0xFF);
            if b > r + 40 {
                found_blue = true;
            }
        }
    }
    assert!(
        found_blue,
        "WATER row droplet icon missing from the SELF pane"
    );

    // the numeric readout tells the number: 6/10 vs 9/10 renders differently
    tw.g.player_mut().player_mut().thirst = 9;
    let at_nine = tw.render();
    let row_changed =
        (84..92).any(|y| (24..140).any(|x| px(&at_six, (x, y)) != px(&at_nine, (x, y))));
    assert!(row_changed, "the WATER row must show the live number");
}

/* ------------------------------------ persistence ------------------------------------ */

#[test]
fn save_marker_roundtrip_and_old_saves_load_full() {
    // below full: the tolerant trailing marker is written and read back
    let mut g1 = bare_game("thirst_save");
    let mut p = g1.entities.take(0).unwrap();
    p.player_mut().thirst = 5;
    let mut data = Vec::new();
    save::write_player(&g1, &p, &mut data);
    g1.entities.put_back(p);
    assert_eq!(data.last().map(String::as_str), Some("Thirst:5"));

    let mut g2 = bare_game("thirst_load");
    let loader = Load::with_version(&g2, fdoom::core::game::version());
    loader.load_player(&mut g2, &data);
    assert_eq!(
        g2.player().player().thirst,
        5,
        "thirst survives the roundtrip"
    );

    // at full the entry is omitted — untouched saves stay format-identical
    let mut g3 = bare_game("thirst_save_full");
    let mut p = g3.entities.take(0).unwrap();
    p.player_mut().thirst = MAX_THIRST;
    let mut data_full = Vec::new();
    save::write_player(&g3, &p, &mut data_full);
    g3.entities.put_back(p);
    assert!(
        !data_full.iter().any(|d| d.starts_with("Thirst:")),
        "full thirst writes no marker: {data_full:?}"
    );

    // a classic save (no marker at all) loads at full thirst
    let legacy: Vec<String> = [
        "264",
        "152",
        "16",
        "9",
        "7",
        "5",
        "0",
        "1234",
        "0",
        "PotionEffects[]",
        "520",
        "true",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut g4 = bare_game("thirst_old");
    let loader = Load::with_version(&g4, fdoom::core::game::version());
    loader.load_player(&mut g4, &legacy);
    assert_eq!(
        g4.player().player().thirst,
        MAX_THIRST,
        "old saves must load at full thirst"
    );
}

/* -------------------------- screenshots (looked at, per repo rule) -------------------------- */

#[test]
fn desert_hot_scene_screenshot() {
    let mut tw = TestWorld::infinite()
        .seed(SEED)
        .name("thirst_desert")
        .build();
    // hot-climate country at midday: the HOT badge accompanies the droplet row
    use fdoom::level::infinite_gen::Biome;
    tw.goto_biome(Biome::Desert);
    tw.set_time(DAY_LENGTH * 3 / 8); // midday
    {
        let pd = tw.g.player_mut().player_mut();
        pd.thirst = MAX_THIRST;
        pd.hunger = 7;
    }
    let base = tw.render(); // primes: thirst full = row absent
    tw.g.player_mut().player_mut().thirst = 5;
    let steps = temperature::band_for(&tw.g, tw.g.player()).steps();
    let frame = tw.render();
    tw.screenshot("thirst_desert_hot.png");
    assert!(
        steps >= 2,
        "expected a Hot+ band for the scene, got {steps}"
    );
    assert_ne!(
        px(&frame, THIRST_PROBE),
        px(&base, THIRST_PROBE),
        "the droplet row must be up in the desert scene"
    );
}

#[test]
fn spring_drink_scene_screenshot() {
    let mut tw = TestWorld::infinite()
        .seed(SEED)
        .name("thirst_spring")
        .build();
    tw.set_time(DAY_LENGTH * 3 / 8); // midday, so the scene reads
    for dy in -1..=1 {
        tw.place("Spring Water", 1, dy);
        tw.place("Spring Water", 2, dy);
    }
    {
        let pd = tw.g.player_mut().player_mut();
        pd.mob.dir = fdoom::entity::Direction::Right;
        pd.active_item = None;
    }
    set_thirst(&mut tw, 4);
    with_player(&mut tw, player_behavior::attack);
    assert_eq!(thirst(&tw), 4 + SPRING_DRINK_THIRST);
    tw.render();
    tw.screenshot("thirst_spring_drink.png");
}

/* --------------------------------- shared invariants --------------------------------- */

#[test]
fn thirst_shares_the_stat_scale() {
    assert_eq!(MAX_THIRST, MAX_STAT, "thirst rides the 0..=10 stat scale");
    assert_eq!(THIRST_DAMAGE_PERIOD, 360, "documented chip-damage cadence");
}
