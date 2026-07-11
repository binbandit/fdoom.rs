//! Mob-life wave integration tests: the snake family, the Ghost, firefly swarms,
//! movement personalities, and tall-grass stealth. Headless, through `TestWorld`.

use fdoom::core::game::Game;
use fdoom::core::updater::Time;
use fdoom::entity::mob::snake::SnakeVariant;
use fdoom::entity::mob::{self, MovementStyle};
use fdoom::entity::{Direction, Entity, EntityKind, behavior};
use fdoom::gfx::screen;
use fdoom::level;
use fdoom::level::infinite_gen::Biome;
use fdoom::testutil::TestWorld;

/// Eid of the first live entity on `lvl` matching `pred`.
fn find_on_level(g: &Game, lvl: usize, pred: impl Fn(&Entity) -> bool) -> Option<i32> {
    g.entities
        .entities_on_level(lvl)
        .find(|e| !e.c.removed && pred(e))
        .map(|e| e.c.eid)
}

fn snake_variant(e: &Entity) -> Option<SnakeVariant> {
    match &e.kind {
        EntityKind::Snake(d) => Some(d.variant),
        _ => None,
    }
}

/* ------------------------------- roster params ------------------------------- */

#[test]
fn snake_family_params_and_save_names() {
    let tw = TestWorld::infinite().seed(101).build();
    let g = &tw.g;

    let grass = mob::snake::new_variant(g, SnakeVariant::Grass, 1);
    let adder = mob::snake::new_variant(g, SnakeVariant::Adder, 1);
    let rattler = mob::snake::new_variant(g, SnakeVariant::Rattler, 1);
    let cave = mob::snake::new(g, 1); // the classic constructor is the Cave Serpent

    // save names: "Snake" is kept by the Cave Serpent for save compatibility
    assert!(fdoom::saveload::save::write_entity(g, &grass, true).starts_with("GrassSnake"));
    assert!(fdoom::saveload::save::write_entity(g, &adder, true).starts_with("Adder"));
    assert!(fdoom::saveload::save::write_entity(g, &rattler, true).starts_with("Rattler"));
    assert!(fdoom::saveload::save::write_entity(g, &cave, true).starts_with("Snake"));

    // the rattler alone spawns coiled
    assert!(matches!(&rattler.kind, EntityKind::Snake(d) if d.coiled));
    assert!(matches!(&cave.kind, EntityKind::Snake(d) if !d.coiled));

    // danger scales by zone: harmless grass snake is the frailest, the cave serpent
    // the toughest
    let hp = |e: &Entity| e.mob().unwrap().max_health;
    assert!(hp(&grass) < hp(&adder));
    assert!(hp(&adder) <= hp(&rattler));
    assert!(hp(&rattler) < hp(&cave));

    // fireflies are ambience: never written to saves
    let mut rng = fdoom::rng::Rng::new(7);
    let swarm = fdoom::entity::fireflies::new(&mut rng);
    assert_eq!(fdoom::saveload::save::write_entity(g, &swarm, true), "");
    let n = match &swarm.kind {
        EntityKind::Fireflies(d) => d.count,
        _ => unreachable!(),
    };
    assert!((4..=8).contains(&n));
}

#[test]
fn movement_personalities_wired() {
    let tw = TestWorld::infinite().seed(102).build();
    let g = &tw.g;

    let style = |e: &Entity| e.mob_ai().unwrap().movement_style;
    assert_eq!(style(&mob::feral_hound::new(g, 1)), MovementStyle::Circle);
    assert_eq!(style(&mob::night_wisp::new(g, 1)), MovementStyle::Curve);
    assert_eq!(
        style(&mob::marsh_lurker::new(g, 1)),
        MovementStyle::FreezeBurst
    );
    assert_eq!(style(&mob::snake::new(g, 1)), MovementStyle::Slither);
    assert_eq!(style(&mob::ghost::new(g, 1)), MovementStyle::SineFloat);
    assert_eq!(style(&mob::zombie::new(g, 1)), MovementStyle::Classic);
    assert_eq!(style(&mob::cow::new(g)), MovementStyle::Classic);

    // style_step semantics: classic passthrough, freeze/burst phases, slither sway
    assert_eq!(
        behavior::style_step(MovementStyle::Classic, 11, 5, 7),
        (5, 7, None)
    );
    assert_eq!(
        behavior::style_step(MovementStyle::FreezeBurst, 10, 1, 1),
        (0, 0, None)
    );
    assert_eq!(
        behavior::style_step(MovementStyle::FreezeBurst, 130, 1, 1),
        (2, 2, None)
    );
    let (dx, dy, sway) = behavior::style_step(MovementStyle::Slither, 16, 1, 0);
    assert_eq!((dx, dy), (1, 0));
    assert!(matches!(sway, Some((0, s)) if s == 1 || s == -1));
}

/* ------------------------------- snake behavior ------------------------------- */

#[test]
fn rattler_warns_then_strikes() {
    let mut tw = TestWorld::infinite().seed(103).build();
    let (px, py) = tw.player_tile();
    let lvl = tw.g.current_level;

    let rattler = mob::snake::new_variant(&tw.g, SnakeVariant::Rattler, 1);
    tw.g.level_mut(lvl).add_at(rattler, px + 6, py, true, lvl);
    tw.tick_n(1); // drain into the arena (rattler ticks once, player 6 tiles away)

    let rid = find_on_level(&tw.g, lvl, |e| {
        snake_variant(e) == Some(SnakeVariant::Rattler)
    })
    .expect("rattler placed");

    // 6 tiles away: coiled, silent
    let coiled = |g: &Game| {
        matches!(&g.entities.get(rid).unwrap().kind,
        EntityKind::Snake(d) if d.coiled)
    };
    // the rattle telegraph is warning-tier (centered band), not ambient
    let rattled = |g: &Game| {
        g.warnings
            .iter()
            .any(|n| n.to_uppercase().contains("RATTLE"))
    };
    assert!(coiled(&tw.g));
    assert!(!rattled(&tw.g));

    // step to 3 tiles: the warning sounds, but it stays coiled
    tw.teleport(px + 3, py);
    tw.g.with_entity(rid, |r, g| mob::snake::tick(g, r));
    assert!(rattled(&tw.g), "no rattle warning within 4 tiles");
    assert!(coiled(&tw.g));

    // adjacent: uncoils with a primed strike
    tw.teleport(px + 5, py);
    tw.g.with_entity(rid, |r, g| mob::snake::tick(g, r));
    assert!(!coiled(&tw.g), "rattler should uncoil at strike range");

    // the primed touch hits for 2x snake damage (2 * (lvl + diff))
    let diff = tw.g.settings.get_idx("diff");
    let before = tw.g.player().mob().unwrap().health;
    tw.g.with_entity(rid, |r, g| {
        g.with_entity(g.player_id, |p, g| behavior::touched_by(g, r, p));
    });
    let after = tw.g.player().mob().unwrap().health;
    assert_eq!(before - after, 2 * (1 + diff));
}

#[test]
fn adder_drains_stamina_and_grass_snake_is_harmless() {
    let mut tw = TestWorld::infinite().seed(104).build();
    let (px, py) = tw.player_tile();
    let lvl = tw.g.current_level;

    let grass = mob::snake::new_variant(&tw.g, SnakeVariant::Grass, 1);
    let adder = mob::snake::new_variant(&tw.g, SnakeVariant::Adder, 1);
    tw.g.level_mut(lvl).add_at(grass, px + 5, py, true, lvl);
    tw.g.level_mut(lvl).add_at(adder, px + 5, py + 2, true, lvl);
    tw.tick_n(1);

    let gid = find_on_level(&tw.g, lvl, |e| {
        snake_variant(e) == Some(SnakeVariant::Grass)
    })
    .expect("grass snake placed");
    let aid = find_on_level(&tw.g, lvl, |e| {
        snake_variant(e) == Some(SnakeVariant::Adder)
    })
    .expect("adder placed");

    // grass snake touch: nothing happens
    let (h0, s0) = {
        let pd = tw.g.player().player();
        (pd.mob.health, pd.stamina)
    };
    tw.g.with_entity(gid, |snake, g| {
        g.with_entity(g.player_id, |p, g| behavior::touched_by(g, snake, p));
    });
    {
        let pd = tw.g.player().player();
        assert_eq!((pd.mob.health, pd.stamina), (h0, s0), "grass snake bit");
    }

    // adder bite: 1+diff damage plus 2 stamina
    let diff = tw.g.settings.get_idx("diff");
    tw.g.with_entity(aid, |snake, g| {
        g.with_entity(g.player_id, |p, g| behavior::touched_by(g, snake, p));
    });
    let pd = tw.g.player().player();
    assert_eq!(pd.mob.health, h0 - (1 + diff));
    assert_eq!(pd.stamina, s0 - 2);
}

/* ---------------------------------- the ghost ---------------------------------- */

#[test]
fn ghost_rises_at_broken_grave_and_fades_at_dawn() {
    let mut tw = TestWorld::infinite().seed(105).build();
    tw.g.change_time_of_day(Time::Night);
    let (px, py) = tw.player_tile();
    let lvl = tw.g.current_level;

    // a broken grave 8 tiles out; clear any wandering mobs so the clearance holds
    tw.place_at("broken grave stone", px + 8, py);
    let bystanders: Vec<i32> =
        tw.g.entities
            .entities_on_level(lvl)
            .filter(|e| !e.is_player())
            .map(|e| e.c.eid)
            .collect();
    for eid in bystanders {
        tw.g.entities.delete(eid);
    }

    let rose = mob::ghost::try_rise(&mut tw.g, lvl, (px + 8) * 16 + 8, py * 16 + 8, 1);
    assert!(rose, "ghost failed to rise at a broken grave");
    // no grave, no ghost
    assert!(!mob::ghost::try_rise(
        &mut tw.g,
        lvl,
        (px - 8) * 16 + 8,
        py * 16 + 8,
        1
    ));

    tw.tick_n(1);
    assert!(
        find_on_level(&tw.g, lvl, |e| matches!(e.kind, EntityKind::Ghost(_))).is_some(),
        "risen ghost not live at night"
    );

    // dawn banishes it
    tw.g.change_time_of_day(Time::Morning);
    tw.tick_n(2);
    assert!(
        find_on_level(&tw.g, lvl, |e| matches!(e.kind, EntityKind::Ghost(_))).is_none(),
        "ghost survived the dawn"
    );
}

#[test]
fn ghost_phases_through_rock_and_only_hurts_when_solid() {
    let mut tw = TestWorld::infinite().seed(106).build();
    tw.g.change_time_of_day(Time::Night);
    let (px, py) = tw.player_tile();
    let lvl = tw.g.current_level;

    let ghost = mob::ghost::new(&tw.g, 1);
    tw.g.level_mut(lvl).add_at(ghost, px + 6, py, true, lvl);
    tw.tick_n(1);
    let gid = find_on_level(&tw.g, lvl, |e| matches!(e.kind, EntityKind::Ghost(_)))
        .expect("ghost placed");

    // wall it in, then walk it straight through
    let gx = tw.g.entities.get(gid).unwrap().c.x;
    tw.place_at("rock", (gx >> 4) + 1, py);
    let moved =
        tw.g.with_entity(gid, |gh, g| {
            let x0 = gh.c.x;
            let ok = behavior::entity_move(g, gh, 16, 0);
            (ok, gh.c.x - x0)
        })
        .unwrap();
    assert_eq!(moved, (true, 16), "ghost blocked by rock");

    // damage passes through the phase form, lands on the solid pulse
    let hp = |g: &Game| g.entities.get(gid).unwrap().mob().unwrap().health;
    let before = hp(&tw.g);
    tw.g.with_entity(gid, |gh, g| {
        if let Some(m) = gh.mob_mut() {
            m.tick_time = 25; // phase half of the pulse
        }
        behavior::do_hurt(g, gh, 3, Direction::Down);
    });
    assert_eq!(hp(&tw.g), before, "phase-form ghost took damage");
    tw.g.with_entity(gid, |gh, g| {
        if let Some(m) = gh.mob_mut() {
            m.tick_time = 5; // solid half
        }
        behavior::do_hurt(g, gh, 3, Direction::Down);
    });
    assert_eq!(
        hp(&tw.g),
        before - 3,
        "solid-pulse ghost shrugged off damage"
    );
}

/* --------------------------------- fireflies --------------------------------- */

#[test]
fn fireflies_spawn_at_dusk_and_spook_into_scatter() {
    let mut tw = TestWorld::infinite().seed(107).build();
    tw.goto_biome(Biome::Forest);
    tw.g.change_time_of_day(Time::Evening);
    let lvl = tw.g.current_level;

    let mut found = false;
    for _ in 0..20_000 {
        level::try_spawn(&mut tw.g, lvl);
        if tw
            .g
            .level(lvl)
            .entities_to_add
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Fireflies(_)))
        {
            found = true;
            break;
        }
    }
    assert!(found, "no firefly swarm spawned at dusk in a forest");

    tw.tick_n(1);
    let fid = find_on_level(&tw.g, lvl, |e| matches!(e.kind, EntityKind::Fireflies(_)))
        .expect("swarm not live");

    // walk into the swarm: it scatters
    let (fx, fy) = {
        let e = tw.g.entities.get(fid).unwrap();
        (e.c.x, e.c.y)
    };
    let p = tw.g.player_mut();
    p.c.x = fx + 8;
    p.c.y = fy;
    tw.tick_n(1);
    let scattered = matches!(
        &tw.g.entities.get(fid).expect("swarm vanished").kind,
        EntityKind::Fireflies(d)
            if matches!(d.state, fdoom::entity::fireflies::FireflyState::Scatter { .. })
    );
    assert!(scattered, "spooked swarm did not scatter");

    // dawn disperses the swarm entirely
    tw.g.change_time_of_day(Time::Morning);
    tw.tick_n(2);
    assert!(
        find_on_level(&tw.g, lvl, |e| matches!(e.kind, EntityKind::Fireflies(_))).is_none(),
        "fireflies survived the dawn"
    );
}

/* ------------------------------ spawn tables ------------------------------ */

/// Loop `try_spawn` until a queued entity matches, or give up.
fn spawn_until(tw: &mut TestWorld, pred: impl Fn(&Entity) -> bool, tries: usize) -> bool {
    let lvl = tw.g.current_level;
    for _ in 0..tries {
        level::try_spawn(&mut tw.g, lvl);
        if tw.g.level(lvl).entities_to_add.iter().any(&pred) {
            return true;
        }
    }
    false
}

#[test]
fn desert_nights_spawn_coiled_rattlers() {
    let mut tw = TestWorld::infinite().seed(108).build();
    tw.g.past_day1 = true; // fresh worlds get a safe day 1; skip it
    tw.goto_biome(Biome::Desert);
    tw.g.change_time_of_day(Time::Night);
    assert!(
        spawn_until(
            &mut tw,
            |e| matches!(&e.kind, EntityKind::Snake(d)
                if d.variant == SnakeVariant::Rattler && d.coiled),
            20_000
        ),
        "no coiled rattler spawned in the desert"
    );
}

#[test]
fn marsh_spawns_adders_and_plains_spawn_grass_snakes() {
    let mut tw = TestWorld::infinite().seed(109).build();
    tw.g.past_day1 = true;
    tw.goto_biome(Biome::Marsh);
    tw.g.change_time_of_day(Time::Night);
    assert!(
        spawn_until(
            &mut tw,
            |e| snake_variant(e) == Some(SnakeVariant::Adder),
            20_000
        ),
        "no adder spawned in the marsh"
    );

    let mut tw = TestWorld::infinite().seed(110).build();
    tw.g.past_day1 = true;
    tw.goto_biome(Biome::Plains);
    assert!(
        spawn_until(
            &mut tw,
            |e| snake_variant(e) == Some(SnakeVariant::Grass),
            20_000
        ),
        "no grass snake spawned on the plains"
    );
}

/* ------------------------------ grass stealth ------------------------------ */

#[test]
fn hostile_in_tall_grass_at_night_shows_only_eye_glints() {
    let mut tw = TestWorld::infinite().seed(111).build();
    let (px, py) = tw.player_tile();
    let lvl = tw.g.current_level;

    tw.place_at("tall grass", px + 3, py);
    let zombie = mob::zombie::new(&tw.g, 1);
    tw.g.level_mut(lvl).add_at(zombie, px + 3, py, true, lvl);
    tw.tick_n(1);
    let zid = find_on_level(&tw.g, lvl, |e| matches!(e.kind, EntityKind::Zombie(_)))
        .expect("zombie placed");

    // pin it to the grass tile and make it night
    tw.place_at("tall grass", px + 3, py); // re-place in case anything trampled it
    {
        let z = tw.g.entities.get_mut(zid).unwrap();
        z.c.x = (px + 3) * 16 + 8;
        z.c.y = py * 16 + 8;
    }
    tw.g.change_time_of_day(Time::Night);

    let pixels = tw.render();
    let (plx, ply) = tw.player_pos();
    let (zx, zy) = {
        let z = tw.g.entities.get(zid).unwrap();
        (z.c.x, z.c.y)
    };
    let cx = screen::W / 2 + (zx - plx);
    let cy = (screen::H - 8) / 2 + (zy - ply);

    // somewhere in the zombie's box there must be a warm (yellow-ish) eye pixel:
    // red high, blue crushed — tall grass alone grades green/blue at night
    let mut warm = 0;
    for y in (cy - 12).max(0)..(cy + 6).min(screen::H) {
        for x in (cx - 10).max(0)..(cx + 10).min(screen::W) {
            let p = pixels[(x + y * screen::W) as usize];
            let (r, g, b) = ((p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff);
            if r >= 25 && g >= 12 && b <= r / 2 {
                warm += 1;
            }
        }
    }
    assert!(
        warm >= 1,
        "no warm eye glints found near hidden zombie at ({cx},{cy})"
    );
    // Hostiles now carry a faint radius-1 eye-gleam emitter (night threat
    // legibility, playtest #3), which warm-tints a speckle of grass around the
    // hiding spot — so the bound allows the pool but still catches a full body
    // render leaking through the grass clip (~100+ px).
    assert!(
        warm <= 90,
        "too many warm pixels ({warm}) — body not clipped to eyes?"
    );
}

/* ------------------------------ visual smoke ------------------------------ */

/// Stages the new roster in one night scene and writes
/// `target/verify/mob_life_gallery.png` (`just shots` upscales it for eyeballing):
/// coiled rattler, ghost, firefly swarm, and a grass-hidden zombie.
#[test]
fn mob_life_gallery_renders() {
    let mut tw = TestWorld::infinite().seed(112).build();
    let (px, py) = tw.player_tile();
    let lvl = tw.g.current_level;

    tw.place_at("sand", px - 4, py - 2);
    let rattler = mob::snake::new_variant(&tw.g, SnakeVariant::Rattler, 1);
    tw.g.level_mut(lvl)
        .add_at(rattler, px - 4, py - 2, true, lvl);

    let ghost = mob::ghost::new(&tw.g, 1);
    tw.g.level_mut(lvl).add_at(ghost, px + 4, py - 2, true, lvl);

    let mut rng = fdoom::rng::Rng::new(9);
    let swarm = fdoom::entity::fireflies::new(&mut rng);
    tw.g.level_mut(lvl).add_at(swarm, px - 4, py + 2, true, lvl);

    tw.place_at("tall grass", px + 4, py + 2);
    let zombie = mob::zombie::new(&tw.g, 1);
    tw.g.level_mut(lvl)
        .add_at(zombie, px + 4, py + 2, true, lvl);

    // night first: the ghost and the swarm despawn if they ever tick in daylight
    tw.g.change_time_of_day(Time::Night);
    tw.tick_n(1);
    // pin the zombie back onto its grass after the settling tick
    if let Some(zid) = find_on_level(&tw.g, lvl, |e| matches!(e.kind, EntityKind::Zombie(_))) {
        let z = tw.g.entities.get_mut(zid).unwrap();
        z.c.x = (px + 4) * 16 + 8;
        z.c.y = (py + 2) * 16 + 8;
    }
    tw.screenshot("mob_life_gallery.png");
}
