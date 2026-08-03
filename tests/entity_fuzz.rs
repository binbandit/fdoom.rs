//! Adversarial crash hunt for the entity/mob/combat lane: entity lifetime under the
//! take-out tick pattern, damage/distance math at extremes, and item/entity interaction
//! across every kind. Every test here is a regression guard for a panic that actually
//! happened on this code.

use fdoom::entity::{Entity, EntityKind, behavior};
use fdoom::level;
use fdoom::testutil::TestWorld;

/// Put `e` on the player's level at pixel `(x, y)`, drain the add-queue, and return its
/// eid. The eid is pinned before insertion (the arena only assigns one when it is < 0)
/// so the caller never picks up an unrelated entity the world queued on its own.
fn place_entity(tw: &mut TestWorld, mut e: Entity, x: i32, y: i32) -> i32 {
    static NEXT_EID: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(900_001);
    let lvl = tw.current_level;
    let eid = NEXT_EID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    e.c.eid = eid;
    e.c.x = x;
    e.c.y = y;
    tw.g.level_mut(lvl).add(e, lvl);
    level::tick_level(&mut tw.g, lvl, false);
    assert!(tw.g.entities.contains(eid), "entity placed");
    eid
}

/// Every mob kind the roster can build, as fresh entities.
fn all_mobs(g: &fdoom::core::game::Game) -> Vec<Entity> {
    use fdoom::entity::mob;
    vec![
        mob::cow::new(g),
        mob::deer::new(g),
        mob::pig::new(g),
        mob::sheep::new(g),
        mob::glow_worm::new(g),
        mob::zombie::new(g, 1),
        mob::snake::new(g, 1),
        mob::knight::new(g, 1),
        mob::marsh_lurker::new(g, 1),
        mob::feral_hound::new(g, 1),
        mob::stone_golem::new(g, 1),
        mob::night_wisp::new(g, 1),
        mob::ghost::new(g, 1),
    ]
}

/* ------------------------- distance math at world scale ------------------------- */

/// A mob left far behind (the player walked away) keeps ticking — nothing despawns it.
/// The chase/detect distance check squared the pixel delta in `i32`, so any mob more
/// than 46340px (~2896 tiles) away panicked with "attempt to multiply with overflow"
/// on EVERY tick.
#[test]
fn mob_far_from_player_ticks_without_overflow() {
    let mut tw = TestWorld::infinite().name("fuzz_far_mob").build();
    let (px, py) = tw.player_pos();
    let far = 60_000; // ~3750 tiles: a long walk, not an impossible one

    let mobs = all_mobs(&tw.g);
    for mob in mobs {
        let eid = place_entity(&mut tw, mob, px + far, py + far);
        let lvl = tw.current_level;
        // the entity tick is what the level loop does every frame
        tw.g.with_entity(eid, |e, g| behavior::entity_tick(g, e))
            .expect("mob present");
        // and hurting it goes down the text-particle/sound path with its own delta math
        tw.g.with_entity(eid, |e, g| {
            behavior::do_hurt(g, e, 1, fdoom::entity::Direction::Down)
        });
        tw.g.entities.delete(eid);
        let _ = lvl;
    }
}

/// The same squared-distance math, at the extreme end of the coordinate range: the
/// subtraction itself must not overflow either.
#[test]
fn mob_at_extreme_coordinates_ticks_without_overflow() {
    let mut tw = TestWorld::infinite().name("fuzz_extreme_coords").build();
    let mobs = all_mobs(&tw.g);
    for mob in mobs {
        let eid = place_entity(&mut tw, mob, i32::MAX / 2, i32::MIN / 2);
        tw.g.with_entity(eid, |e, g| behavior::entity_tick(g, e))
            .expect("mob present");
        tw.g.entities.delete(eid);
    }
}

/// Spawners run their own player-distance gate before spawning.
#[test]
fn distant_spawner_ticks_without_overflow() {
    let mut tw = TestWorld::infinite().name("fuzz_far_spawner").build();
    let (px, py) = tw.player_pos();
    let zombie = fdoom::entity::mob::zombie::new(&tw.g, 1);
    let spawner = fdoom::entity::furniture::spawner::new(zombie, &mut tw.g.random.clone());
    let eid = place_entity(&mut tw, spawner, px + 80_000, py);
    // the spawn interval is 200-500 ticks; arm it so the player-distance gate is
    // actually reached this tick
    if let Some(EntityKind::Spawner(s)) = tw.g.entities.get_mut(eid).map(|e| &mut e.kind) {
        s.spawn_tick = 1;
    }
    tw.g.with_entity(eid, |e, g| behavior::entity_tick(g, e))
        .expect("spawner present");
}

/// Firefly swarms spook when the player is near — same delta math, ambient entity.
#[test]
fn distant_fireflies_tick_without_overflow() {
    let mut tw = TestWorld::infinite().name("fuzz_far_fireflies").build();
    // the swarm disperses at dawn; it only reaches its spook check at night
    tw.g.change_time_of_day(fdoom::core::updater::Time::Night);
    let (px, py) = tw.player_pos();
    let swarm = fdoom::entity::fireflies::new(&mut tw.g.random.clone());
    let eid = place_entity(&mut tw, swarm, px + 70_000, py + 70_000);
    for _ in 0..4 {
        tw.g.with_entity(eid, |e, g| behavior::entity_tick(g, e))
            .expect("swarm present");
    }
    // a dispersed swarm never reaches its spook check — the test would be vacuous
    assert!(
        !tw.g.entities.get(eid).expect("swarm present").c.removed,
        "the swarm must survive to run its player-distance check"
    );
}

/// The snake family has its own strike/detect distance checks.
#[test]
fn distant_snakes_tick_without_overflow() {
    use fdoom::entity::mob::snake::{self, SnakeVariant};
    let mut tw = TestWorld::infinite().name("fuzz_far_snake").build();
    let (px, py) = tw.player_pos();
    for variant in [
        SnakeVariant::Grass,
        SnakeVariant::Adder,
        SnakeVariant::Rattler,
        SnakeVariant::Cave,
    ] {
        let s = snake::new_variant(&tw.g, variant, 1);
        let eid = place_entity(&mut tw, s, px + 90_000, py - 90_000);
        for _ in 0..4 {
            tw.g.with_entity(eid, |e, g| behavior::entity_tick(g, e))
                .expect("snake present");
        }
        tw.g.entities.delete(eid);
    }
}

/// TNT's blast radius walks every entity on the level; the distance is computed in
/// floating point, but the damage falloff and the entity loop still have to survive a
/// far-away target.
#[test]
fn tnt_blast_with_a_distant_entity_does_not_panic() {
    let mut tw = TestWorld::infinite().name("fuzz_tnt_far").build();
    let (px, py) = tw.player_pos();
    let cow = fdoom::entity::mob::cow::new(&tw.g);
    place_entity(&mut tw, cow, px + 100_000, py);

    let mut tnt = fdoom::entity::furniture::tnt::new();
    if let EntityKind::Tnt(t) = &mut tnt.kind {
        t.fuse_lit = true;
        t.ftik = fdoom::entity::furniture::tnt::FUSE_TIME - 1;
    }
    let eid = place_entity(&mut tw, tnt, px + 24, py);
    for _ in 0..6 {
        tw.g.with_entity(eid, |e, g| behavior::entity_tick(g, e));
    }
}
