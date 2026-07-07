//! Headless gameplay soak tests: generate real worlds and drive thousands of game ticks
//! with pseudo-random input, day/night flips, level changes and a TNT explosion, asserting
//! the game never panics, the player survives (or respawns), and the entity arena does not
//! grow without bound.

use fdoom::core::updater::Time;
use fdoom::core::world;
use fdoom::entity::EntityKind;
use fdoom::testutil::TestWorld;

/// Sane ceiling for the arena on a 128x128 world: mob caps are per level (~150-300) and
/// six levels exist, plus particles/items; anything past this indicates a leak.
const MAX_ENTITIES: usize = 5000;

/// Tiny xorshift for driving inputs; deliberately independent of the game's RNG.
fn next_rand(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn assert_world_sane(tw: &mut TestWorld, context: &str) {
    assert!(
        tw.try_player().is_some(),
        "{context}: player entity missing from the arena"
    );
    let count = tw.entities.len();
    assert!(
        count < MAX_ENTITIES,
        "{context}: entity arena grew to {count} (>= {MAX_ENTITIES})"
    );
}

/// ~5000 ticks of pseudo-random movement/attack with periodic day/night flips so night
/// mobs spawn and fight, on two different world seeds.
#[test]
fn soak_random_play_two_seeds() {
    for seed in [0x00C0FFEE_i64, 0x5EED_5EED_i64] {
        let mut tw = TestWorld::infinite().seed(seed).build();
        assert_world_sane(&mut tw, "after init_world");

        let keys = ["W", "A", "S", "D", "SPACE", "SPACE"];
        let mut rng = seed as u64 | 1;
        for tick in 0..5000 {
            let r = next_rand(&mut rng);
            let key = keys[(r % keys.len() as u64) as usize];
            tw.input.key_toggled(key, (r >> 8) & 1 == 0);

            // Flip day/night every ~10 in-game seconds so night mobs spawn and despawn.
            if tick % 1200 == 0 {
                tw.change_time_of_day(Time::Night);
            } else if tick % 1200 == 600 {
                tw.change_time_of_day(Time::Morning);
            }

            tw.tick_recover();
        }
        assert_world_sane(&mut tw, &format!("seed {seed:#x} after 5000 ticks"));
    }
}

/// Walk all six levels (surface -> underground x3 -> dungeon wrap -> sky -> surface),
/// ticking 200 times on each, so stairs placement, per-level spawning and the air wizard
/// all get exercised.
#[test]
fn soak_walk_all_levels() {
    let mut tw = TestWorld::infinite().seed(0x1E5E15).build();

    let start_level = tw.current_level;
    for step in 0..5 {
        world::change_level(&mut tw, -1);
        for _ in 0..200 {
            tw.tick_recover();
        }
        assert_world_sane(&mut tw, &format!("level walk step {step}"));
    }
    assert_eq!(
        tw.current_level, start_level,
        "five -1 level changes should wrap back to the start level"
    );
}

/// Light a TNT right next to the player and tick through the fuse, the blast (which hurts
/// the player and smashes tiles) and the exploded-tile restore countdown.
#[test]
fn soak_tnt_explosion_near_player() {
    let mut tw = TestWorld::infinite().seed(0x7477 /* "tnt" */).build();

    let lvl = tw.current_level;
    let (px, py) = tw.player_pos();

    let mut tnt = fdoom::entity::furniture::tnt::new();
    tnt.c.x = px + 12;
    tnt.c.y = py;
    let EntityKind::Tnt(data) = &mut tnt.kind else {
        panic!("tnt::new() did not build a Tnt entity");
    };
    data.fuse_lit = true;
    tw.level_mut(lvl).add(tnt, lvl);

    // Fuse (90 ticks) + explosion + tile-restore countdown (18 ticks), with margin.
    for _ in 0..200 {
        tw.tick_recover();
    }
    assert_world_sane(&mut tw, "after TNT explosion");

    // The TNT entity must be gone once the explosion resolved.
    let tnt_left = tw
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Tnt(_)))
        .count();
    assert_eq!(tnt_left, 0, "exploded TNT entity was not removed");
}
