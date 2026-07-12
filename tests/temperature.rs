//! Temperature wave (`core::temperature` + the player-tick effects). Fast by
//! design: the band model and the mitigation pipeline are pure functions tested
//! with pinned inputs, the effects mechanism is driven directly with pinned scores
//! and timer fields (no wall-clock tick loops), and exactly one end-to-end test
//! boots real worlds at climate extremes for integration + screenshots.

use fdoom::core::temperature::{self, Band, Modifiers};
use fdoom::core::updater::DAY_LENGTH;
use fdoom::core::weather::Precip;
use fdoom::entity::furniture::campfire;
use fdoom::entity::mob::{self, player_behavior};
use fdoom::entity::{Entity, EntityKind};
use fdoom::item::{ItemKind, registry};
use fdoom::level::infinite_gen::{Biome, biome_at};
use fdoom::testutil::{TestWorld, find_recipe};

const SEED: i64 = 20260707;

/// Midday: the warmth wave's peak (the Day quarter's midpoint).
const NOON: i32 = DAY_LENGTH * 3 / 8;
/// Deep night: the warmth wave's trough.
const MIDNIGHT: i32 = DAY_LENGTH * 7 / 8;

/* --------------------------------- helpers --------------------------------- */

/// Nearest tile satisfying `pred` (outward ring search from the origin).
fn find_tile(pred: impl Fn(i32, i32) -> bool) -> (i32, i32) {
    for r in 0i32..800 {
        let ring = r * 8;
        for dy in (-ring..=ring).step_by(8) {
            for dx in (-ring..=ring).step_by(8) {
                if (dx.abs() == ring || dy.abs() == ring) && pred(dx, dy) {
                    return (dx, dy);
                }
            }
        }
    }
    panic!("no matching tile within range for seed {SEED:#x}");
}

/// Nearest tile whose climate lies in `[lo, hi)`.
fn find_climate(lo: f64, hi: f64) -> (i32, i32) {
    find_tile(|x, y| (lo..hi).contains(&temperature::climate(SEED, x, y)))
}

/// Pin the day clock (same shape as tests/weather.rs): jump to one tick before,
/// run one real tick through it, then set the day and drop stray cues. Day 0 keeps
/// the weather schedule dry, so band checks see pure climate + time of day. The
/// player's cue memory resets too: each observation is an entry "from comfort".
fn pin_clock(tw: &mut TestWorld, day: i32, tick: i32) {
    tw.set_time(tick - 1);
    tw.tick_n(1);
    assert_eq!(tw.tick_count, tick, "clock failed to pin");
    tw.events.day_number = day;
    tw.notifications.clear();
    tw.warnings.clear();
    let pd = tw.g.player_mut().player_mut();
    pd.temp_prev_band = 0;
    pd.temp_cue_cooldown = 0;
}

fn player_band(tw: &TestWorld) -> Band {
    temperature::band_for(&tw.g, tw.g.player())
}

fn player_score(tw: &TestWorld) -> f64 {
    temperature::score_for(&tw.g, tw.g.player())
}

fn wear(tw: &mut TestWorld, name: &str) {
    let item = registry::get(&tw.g, name);
    tw.g.player_mut().player_mut().cur_armor = Some(item);
}

/* ----------------------------- the climate field ----------------------------- */

#[test]
fn climate_copy_matches_biome_gates() {
    // temperature::climate is a local copy of infinite_gen's private climate_at;
    // the biome gates pin it: Tundra only forms below 0.30, Desert only above 0.70.
    let (mut tundra, mut desert) = (0, 0);
    for gy in -32..=32 {
        for gx in -32..=32 {
            let (x, y) = (gx * 64, gy * 64);
            match biome_at(SEED, x, y) {
                Biome::Tundra => {
                    tundra += 1;
                    assert!(temperature::climate(SEED, x, y) < 0.30, "at {x},{y}");
                }
                Biome::Desert => {
                    desert += 1;
                    assert!(temperature::climate(SEED, x, y) > 0.70, "at {x},{y}");
                }
                _ => {}
            }
        }
    }
    assert!(tundra > 0 && desert > 0, "sample must cover both extremes");
}

/* ------------------------------- ambient bands ------------------------------- */

#[test]
fn ambient_bands_across_biomes_and_times() {
    let score = |(x, y), t| temperature::ambient_score(SEED, x, y, t, Precip::None);

    // deep tundra midnight: freezing (pure ambient — no snow/coat modifiers)
    let tundra = find_climate(0.0, 0.10);
    assert_eq!(Band::from_score(score(tundra, MIDNIGHT)), Band::Freezing);

    // deep desert noon: scorching
    let desert = find_climate(0.90, 1.01);
    assert_eq!(Band::from_score(score(desert, NOON)), Band::Scorching);

    // temperate country at noon: comfort
    let mid = find_climate(0.48, 0.52);
    assert_eq!(Band::from_score(score(mid, NOON)), Band::Comfort);

    // the real-and-interesting one: desert nights go properly chilly
    assert!(
        score(desert, MIDNIGHT) <= -0.5,
        "desert midnight should be at least chilly, got {}",
        score(desert, MIDNIGHT)
    );

    // and nights are cooler than days everywhere
    for spot in [tundra, desert, mid] {
        assert!(score(spot, MIDNIGHT) < score(spot, NOON));
    }

    // plain deep-tundra midnight is extreme but NOT past the deadly line — the
    // 3-heart floor holds there (dying needs stacked weather on top)
    assert!(score(tundra, MIDNIGHT).abs() < temperature::DEADLY_SCORE);
}

#[test]
fn precipitation_chills() {
    let (fx, fy) = find_climate(0.48, 0.52);
    let base = temperature::ambient_score(SEED, fx, fy, NOON, Precip::None);
    let rain = temperature::ambient_score(SEED, fx, fy, NOON, Precip::Rain(1.0));
    let snow = temperature::ambient_score(SEED, fx, fy, NOON, Precip::Snow(1.0));
    assert!(rain < base && snow < rain, "snow chills harder than rain");
}

#[test]
fn band_thresholds_and_steps() {
    let cases = [
        (-3.0, Band::Freezing, -3),
        (-2.0, Band::Cold, -2),
        (-1.0, Band::Chilly, -1),
        (0.0, Band::Comfort, 0),
        (1.0, Band::Warm, 1),
        (2.0, Band::Hot, 2),
        (3.0, Band::Scorching, 3),
    ];
    for (s, band, steps) in cases {
        assert_eq!(Band::from_score(s), band);
        assert_eq!(band.steps(), steps);
    }
}

/* ------------------------- the mitigation pipeline (pure) ------------------------- */

#[test]
fn mitigation_pipeline() {
    let m = Modifiers::default;
    // f64 arithmetic: compare within epsilon
    let check = |got: f64, want: f64, what: &str| {
        assert!((got - want).abs() < 1e-9, "{what}: got {got}, want {want}");
    };

    // shade and the straw hat: one heat band each, clamped at comfort
    let shade = Modifiers {
        shaded: true,
        ..m()
    };
    check(temperature::apply_modifiers(2.8, &shade), 1.8, "shade");
    check(
        temperature::apply_modifiers(0.6, &shade),
        0.0,
        "shade clamp",
    );
    let hat = Modifiers {
        straw_hat: true,
        ..m()
    };
    check(temperature::apply_modifiers(2.8, &hat), 1.8, "hat");
    let both = Modifiers {
        shaded: true,
        straw_hat: true,
        ..m()
    };
    check(temperature::apply_modifiers(2.8, &both), 0.8, "hat+shade");

    // heat mitigations never touch the cold side
    check(temperature::apply_modifiers(-2.0, &both), -2.0, "cold side");

    // the fur coat: two cold bands, clamped at comfort; useless against heat
    let coat = Modifiers {
        fur_coat: true,
        ..m()
    };
    check(temperature::apply_modifiers(-2.8, &coat), -0.8, "coat");
    check(temperature::apply_modifiers(-1.0, &coat), 0.0, "coat clamp");
    check(
        temperature::apply_modifiers(2.0, &coat),
        2.0,
        "coat vs heat",
    );

    // fire overrides cold entirely, no matter how deep
    let fire = Modifiers {
        near_fire: true,
        ..m()
    };
    check(
        temperature::apply_modifiers(-3.7, &fire),
        0.0,
        "fire override",
    );
    check(
        temperature::apply_modifiers(1.2, &fire),
        1.2,
        "fire never cools",
    );

    // swimming breaks heat outright, and cold water is not modeled
    let swim = Modifiers {
        swimming: true,
        ..m()
    };
    check(
        temperature::apply_modifiers(3.4, &swim),
        0.0,
        "swim breaks heat",
    );
    check(
        temperature::apply_modifiers(-1.0, &swim),
        -1.0,
        "cold water unmodeled",
    );

    // snow underfoot chills half a band, and the coat still recovers it
    let snowy = Modifiers {
        snow_underfoot: true,
        ..m()
    };
    check(
        temperature::apply_modifiers(-2.5, &snowy),
        -3.0,
        "snow chill",
    );
    let snowy_coat = Modifiers {
        snow_underfoot: true,
        fur_coat: true,
        ..m()
    };
    check(
        temperature::apply_modifiers(-2.5, &snowy_coat),
        -1.0,
        "snow+coat",
    );
}

/* --------------------------- effects mechanism (pinned) --------------------------- */

/// Take the player out of the arena (the take-out tick shape) and hand it to `f`.
fn with_player(tw: &mut TestWorld, f: impl FnOnce(&mut fdoom::core::game::Game, &mut Entity)) {
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    f(&mut tw.g, &mut player);
    tw.g.entities.put_back(player);
}

/// Zero the cue memory: the boot tick ran the real temperature tick once at
/// spawn, which may have set the band memory and armed the cue cooldown.
fn fresh_cues(tw: &mut TestWorld) {
    let pd = tw.g.player_mut().player_mut();
    pd.temp_prev_band = 0;
    pd.temp_cue_cooldown = 0;
}

/// Drive one effects step with a pinned score on a pinned `game_time` tick.
fn effects_step(tw: &mut TestWorld, score: f64, game_time: i32) {
    tw.g.game_time = game_time;
    with_player(tw, |g, p| {
        p.player_mut().mob.hurt_time = 0; // no hurt i-frames between pinned steps
        player_behavior::apply_temperature_effects(g, p, score);
    });
}

fn health(tw: &TestWorld) -> i32 {
    tw.g.player().player().mob.health
}

#[test]
fn no_damage_inside_the_extreme_bands() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    // every band up to and including Cold/Hot, right at the edge, on a
    // damage-cadence tick: health is untouchable
    for score in [-2.49, -2.0, -1.0, 0.0, 1.0, 2.0, 2.49] {
        effects_step(&mut tw, score, 360);
        assert_eq!(health(&tw), 10, "score {score} must not damage");
    }
}

#[test]
fn extreme_damage_is_slow_and_floors_at_three_hearts() {
    let mut tw = TestWorld::infinite().seed(SEED).build();

    // damage fires only on the cadence tick...
    effects_step(&mut tw, -2.8, 361);
    assert_eq!(health(&tw), 10, "off-cadence ticks never damage");
    // ...and exactly one heart per cadence tick
    effects_step(&mut tw, -2.8, 360);
    assert_eq!(health(&tw), 9);

    // the mercy floor: merely-freezing damage stops at 3 hearts, however long
    tw.g.player_mut().player_mut().mob.health = 4;
    effects_step(&mut tw, -2.8, 720);
    assert_eq!(health(&tw), 3);
    for k in 3..8 {
        effects_step(&mut tw, -2.8, 360 * k);
    }
    assert_eq!(health(&tw), 3, "the floor holds");

    // the hot side floors identically
    effects_step(&mut tw, 2.8, 360 * 9);
    assert_eq!(health(&tw), 3);

    // only a truly extreme score pierces the floor (deep-deep climate + stacked
    // weather): death remains possible if every signal is ignored
    effects_step(&mut tw, -(temperature::DEADLY_SCORE + 0.2), 360 * 10);
    assert_eq!(health(&tw), 2);
}

#[test]
fn cues_fire_on_band_entry_with_cooldown() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    fresh_cues(&mut tw);
    tw.notifications.clear();
    tw.warnings.clear();

    // entering Cold: the ambient breath-fog cue
    effects_step(&mut tw, -2.0, 1);
    assert!(
        tw.notifications.iter().any(|n| n.contains("breath fogs")),
        "{:?}",
        tw.notifications
    );

    // crossing into Freezing while the cooldown runs: the warning waits...
    effects_step(&mut tw, -2.8, 2);
    assert!(tw.warnings.is_empty(), "cooldown gates the next cue");
    // ...and fires once the cooldown expires (the crossing retries)
    tw.g.player_mut().player_mut().temp_cue_cooldown = 0;
    effects_step(&mut tw, -2.8, 3);
    assert!(
        tw.warnings.iter().any(|w| w.contains("cold bites")),
        "{:?}",
        tw.warnings
    );

    // recovering to comfort: the ease-off cue
    tw.g.player_mut().player_mut().temp_cue_cooldown = 0;
    tw.notifications.clear();
    effects_step(&mut tw, 0.0, 4);
    assert!(
        tw.notifications.iter().any(|n| n.contains("chill eases")),
        "{:?}",
        tw.notifications
    );

    // the hot side's pair
    tw.g.player_mut().player_mut().temp_cue_cooldown = 0;
    effects_step(&mut tw, 2.0, 5);
    assert!(tw.notifications.iter().any(|n| n.contains("heat presses")));
    tw.g.player_mut().player_mut().temp_cue_cooldown = 0;
    effects_step(&mut tw, 3.0, 6);
    assert!(tw.warnings.iter().any(|w| w.contains("hammers down")));
}

#[test]
fn second_band_drags_stamina_and_shivers() {
    let mut tw = TestWorld::infinite().seed(SEED).build();

    // stamina recharge decays on the drag cadence in Cold/Hot...
    tw.g.player_mut().player_mut().stamina_recharge = 6;
    effects_step(&mut tw, -2.0, 3); // 3 % 3 == 0: a drag tick
    assert_eq!(tw.g.player().player().stamina_recharge, 5);
    // ...but never in the tint-only bands
    tw.g.player_mut().player_mut().stamina_recharge = 6;
    effects_step(&mut tw, -1.0, 6);
    assert_eq!(tw.g.player().player().stamina_recharge, 6);

    // a shiver puff lands on the puff cadence
    let lvl = tw.current_level;
    let queued_puffs = |tw: &TestWorld| {
        tw.g.level(lvl)
            .entities_to_add
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Particle(_)))
            .count()
    };
    let before = queued_puffs(&tw);
    effects_step(&mut tw, -2.0, 60);
    assert_eq!(queued_puffs(&tw), before + 1, "shiver puff on the cadence");
}

/* ------------------------ world-reading of the modifiers ------------------------ */

#[test]
fn modifiers_read_from_the_live_world() {
    // all near spawn: no far teleports, no tick loops
    let mut tw = TestWorld::infinite().seed(SEED).build();
    let (px, py) = tw.player_tile();

    let mods = |tw: &TestWorld| temperature::modifiers_for(&tw.g, tw.g.player());
    assert_eq!(
        mods(&tw),
        Modifiers::default(),
        "fresh spawn: nothing active"
    );

    wear(&mut tw, "Fur Coat");
    assert!(mods(&tw).fur_coat);
    wear(&mut tw, "Straw Hat");
    let m = mods(&tw);
    assert!(m.straw_hat && !m.fur_coat, "one worn slot");

    tw.place_at("tree", px + 1, py);
    assert!(mods(&tw).shaded, "tree canopy beside the player");
    tw.place_at("grass", px + 1, py);
    assert!(!mods(&tw).shaded);
    tw.place_at("Wood Planks", px, py);
    assert!(mods(&tw).shaded, "a built floor reads as a roofed interior");

    tw.place_at("snow", px, py);
    assert!(mods(&tw).snow_underfoot);

    let lvl = tw.current_level;
    tw.g.level_mut(lvl)
        .add_at(campfire::new(), px + 1, py, true, lvl);
    tw.tick_n(1); // drain into the arena (campfire::new starts lit)
    assert!(mods(&tw).near_fire);

    tw.place_at("water", px, py);
    assert!(mods(&tw).swimming);
}

#[test]
fn mines_read_constant_cave_cool() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    tw.g.player_mut().c.level = Some(2); // topmost mine layer (depth -1)
    assert_eq!(player_score(&tw), temperature::MINE_SCORE);
    assert_eq!(player_band(&tw), Band::Chilly); // a tint, never a mechanic
}

/* ----------------------------- items, drops, recipes ----------------------------- */

#[test]
fn fur_drops_from_cows_and_hounds() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    let lvl = tw.current_level;
    let (px, py) = tw.player_pos();

    let mut cow = mob::cow::new(&tw.g);
    cow.c.eid = 91001; // synthetic: not in the arena
    cow.c.level = Some(lvl);
    (cow.c.x, cow.c.y) = (px + 32, py);
    mob::cow::die(&mut tw.g, &mut cow);

    let mut hound = mob::feral_hound::new(&tw.g, 1);
    hound.c.eid = 91002;
    hound.c.level = Some(lvl);
    (hound.c.x, hound.c.y) = (px - 32, py);
    mob::feral_hound::die(&mut tw.g, &mut hound);

    let fur = tw
        .dropped_items()
        .iter()
        .filter(|n| n.as_str() == "Fur")
        .count();
    assert!(fur >= 2, "both kills guarantee at least one Fur each");
}

#[test]
fn coat_and_hat_items_and_recipes() {
    let tw = TestWorld::infinite().seed(SEED).build();

    for (name, level) in [("Fur Coat", 1), ("Straw Hat", 0)] {
        let item = registry::get(&tw.g, name);
        match item.kind {
            ItemKind::Armor { level: l, .. } => assert_eq!(l, level, "{name}"),
            ref k => panic!("{name} should be wearable armor, got {k:?}"),
        }
    }

    // personal crafting, no station — you're cold NOW
    let coat = find_recipe(&tw.recipes.craft, "Fur Coat");
    assert!(coat.get_costs().iter().any(|(n, c)| n == "FUR" && *c == 5));
    let hat = find_recipe(&tw.recipes.craft, "Straw Hat");
    assert!(
        hat.get_costs()
            .iter()
            .any(|(n, c)| n == "GRASS FIBERS" && *c == 6)
    );
}

/* --------------------------- the one end-to-end test --------------------------- */

/// The HUD dot's fill colors for a band-steps value, mirroring the renderer table
/// (both pulse phases for the extreme bands).
fn dot_colors(steps: i32) -> Vec<i32> {
    match steps {
        -1 => vec![0x5E8FD4],
        -2 => vec![0x3E6FE0],
        s if s <= -3 => vec![0x2B4FF0, 0x8FB4FF],
        1 => vec![0xD9A85A],
        2 => vec![0xE07E33],
        _ => vec![0xE0491F, 0xFF9A66],
    }
}

fn dot_pixel(pixels: &[i32]) -> i32 {
    pixels[16 * fdoom::gfx::screen::W as usize + 87] // center of the 5x5 dot
}

/// The single slow-ish test: two real worlds at (nearby) climate extremes verify
/// the whole chain — world -> score -> band -> HUD dot / cues / overlay — and
/// write the screenshots. Total simulated ticks stay in the dozens.
#[test]
fn end_to_end_worlds_hud_and_screenshots() {
    /* ---- tundra midnight: freezing, then a campfire makes it home ---- */
    let (tx, ty) = find_tile(|x, y| {
        temperature::climate(SEED, x, y) < 0.18 && biome_at(SEED, x, y) == Biome::Tundra
    });
    let mut tw = TestWorld::infinite().seed(SEED).build();
    tw.teleport(tx, ty);
    tw.tick_n(3); // stream the chunks in
    tw.place_at("snow", tx, ty); // controlled ground (the found tile could be anything)
    pin_clock(&mut tw, 0, MIDNIGHT);

    let steps = player_band(&tw).steps();
    assert!(
        steps <= -3,
        "tundra midnight on snow must freeze, got {steps}"
    );

    // shiver overlay: pin the puff cadence; two ticks catch it on either side
    tw.g.game_time = 59;
    tw.tick_n(2);
    assert!(
        tw.warnings.iter().any(|w| w.contains("cold bites")),
        "freezing entry warns: {:?}",
        tw.warnings
    );
    let px = dot_pixel(&tw.render());
    assert!(
        dot_colors(steps).contains(&px),
        "cold dot: got {px:#08x} for steps {steps}"
    );
    println!("shot: {}", tw.screenshot("temp_hud_cold.png").display());
    println!("shot: {}", tw.screenshot("temp_shiver.png").display());

    // campfire beside the player: cold overridden entirely, the dot vanishes
    let lvl = tw.current_level;
    tw.g.level_mut(lvl)
        .add_at(campfire::new(), tx + 1, ty, true, lvl);
    tw.tick_n(1);
    assert_eq!(player_band(&tw), Band::Comfort);
    let px = dot_pixel(&tw.render());
    for cols in [-3, -2, -1, 1, 2, 3].map(dot_colors) {
        assert!(!cols.contains(&px), "comfort must draw no dot");
    }
    println!(
        "shot: {}",
        tw.screenshot("temp_campfire_tundra_night.png").display()
    );

    /* ---- desert noon: scorching, then hat + shade + water walk it back ---- */
    let (dx, dy) = find_tile(|x, y| {
        temperature::climate(SEED, x, y) > 0.80 && biome_at(SEED, x, y) == Biome::Desert
    });
    let mut tw = TestWorld::infinite().seed(SEED).build();
    tw.teleport(dx, dy);
    tw.tick_n(3);
    tw.place_at("sand", dx, dy);
    pin_clock(&mut tw, 0, NOON);

    let steps = player_band(&tw).steps();
    assert!(steps >= 3, "deep desert noon must scorch, got {steps}");

    // sweat overlay + warning, then the shots
    tw.g.game_time = 59;
    tw.tick_n(2);
    assert!(tw.warnings.iter().any(|w| w.contains("hammers down")));
    let px = dot_pixel(&tw.render());
    assert!(dot_colors(steps).contains(&px), "hot dot: got {px:#08x}");
    println!("shot: {}", tw.screenshot("temp_hud_hot.png").display());
    println!("shot: {}", tw.screenshot("temp_sweat.png").display());

    // mitigation chain on the live world: hat one band, tree shade another,
    // water breaks heat outright
    let scorch = player_score(&tw);
    wear(&mut tw, "Straw Hat");
    assert_eq!(player_score(&tw), scorch - 1.0);
    tw.place_at("tree", dx + 1, dy);
    assert_eq!(player_score(&tw), scorch - 2.0);
    tw.place_at("water", dx, dy);
    assert_eq!(player_band(&tw), Band::Comfort, "water breaks heat");
}
