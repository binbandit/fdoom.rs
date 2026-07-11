//! End-to-end coverage for mobs that must be reachable through the production
//! natural spawner, not debug-spawn commands or direct constructors.

use fdoom::core::updater::Time;
use fdoom::entity::{Entity, EntityKind};
use fdoom::level::infinite_gen::Biome;
use fdoom::level::{self, lvl_idx};
use fdoom::rng::Rng;
use fdoom::testutil::TestWorld;

#[derive(Clone, Copy)]
enum WantedMob {
    MarshLurker,
    FeralHound,
    StoneGolem,
}

impl WantedMob {
    fn name(self) -> &'static str {
        match self {
            WantedMob::MarshLurker => "marsh_lurker",
            WantedMob::FeralHound => "feral_hound",
            WantedMob::StoneGolem => "stone_golem",
        }
    }

    fn matches(self, e: &Entity) -> bool {
        matches!(
            (self, &e.kind),
            (WantedMob::MarshLurker, EntityKind::MarshLurker(_))
                | (WantedMob::FeralHound, EntityKind::FeralHound(_))
                | (WantedMob::StoneGolem, EntityKind::StoneGolem(_))
        )
    }
}

fn clear_non_player(tw: &mut TestWorld, lvl: usize) {
    tw.level_mut(lvl).entities_to_add.clear();
    tw.level_mut(lvl).entities_to_remove.clear();
    let player_id = tw.player_id;
    for eid in tw.entities.ids_on_level(lvl) {
        if eid != player_id {
            tw.entities.delete(eid);
        }
    }
    tw.level_mut(lvl).mob_count = 0;
}

fn has_mob(tw: &TestWorld, lvl: usize, wanted: WantedMob) -> bool {
    tw.entities
        .entities_on_level(lvl)
        .any(|e| !e.c.removed && wanted.matches(e))
        || tw
            .level(lvl)
            .entities_to_add
            .iter()
            .any(|e| wanted.matches(e))
}

fn assert_natural_spawn(
    tw: &mut TestWorld,
    lvl: usize,
    wanted: WantedMob,
    level_rng_seed: i64,
    game_rng_seed: i64,
) {
    clear_non_player(tw, lvl);
    tw.g.random = Rng::new(game_rng_seed);
    tw.level_mut(lvl).random = Rng::new(level_rng_seed);

    for _ in 0..512 {
        level::try_spawn(&mut tw.g, lvl);
        if has_mob(tw, lvl, wanted) {
            level::tick_level(&mut tw.g, lvl, false);
            assert!(
                has_mob(tw, lvl, wanted),
                "{} was queued but did not become live",
                wanted.name()
            );
            return;
        }
        clear_non_player(tw, lvl);
    }

    panic!(
        "{} did not spawn naturally on level {lvl} with level RNG {level_rng_seed:#x}",
        wanted.name()
    );
}

fn stage_surface(tw: &mut TestWorld, biome: Biome, time: Time) -> usize {
    let lvl = tw.current_level;
    tw.goto_biome(biome);
    tw.g.past_day1 = true;
    tw.g.change_time_of_day(time);
    clear_non_player(tw, lvl);
    lvl
}

fn stage_mine(tw: &mut TestWorld, depth: i32) -> usize {
    let lvl = lvl_idx(depth);
    tw.g.current_level = lvl;
    tw.g.past_day1 = true;
    tw.g.change_time_of_day(Time::Day);
    tw.g.with_entity(tw.g.player_id, |p, _g| {
        p.c.level = Some(lvl);
    });
    let (px, py) = tw.player_tile();
    level::ensure_chunks_at(&mut tw.g, lvl, px, py, true);
    clear_non_player(tw, lvl);
    lvl
}

#[test]
fn marsh_lurker_spawns_naturally_from_marsh_water_or_mud() {
    // Conditions from level::try_spawn: surface depth 0, past day 1, any time of day,
    // rnd <= 25, and lurker_check_start_pos: unlit WATER or MUD with spawn clearance.
    // There is no explicit biome check for the mob, but staging in Marsh gives the
    // natural pool/mud terrain the rule is designed for.
    let mut tw = TestWorld::infinite().seed(0x5EED).build();
    let lvl = stage_surface(&mut tw, Biome::Marsh, Time::Day);

    assert_natural_spawn(
        &mut tw,
        lvl,
        WantedMob::MarshLurker,
        0x1A22_0001,
        0x1A22_1001,
    );
}

#[test]
fn feral_hound_spawns_naturally_on_open_plains() {
    // Conditions from level::try_spawn: surface depth 0, past day 1, Plains or Savanna
    // by biome_at, enemy start position, and rnd <= 12 by day or 41..=60 by night.
    let mut tw = TestWorld::infinite().seed(0x5EED).build();
    let lvl = stage_surface(&mut tw, Biome::Plains, Time::Day);

    assert_natural_spawn(
        &mut tw,
        lvl,
        WantedMob::FeralHound,
        0x2B33_0001,
        0x2B33_1001,
    );
}

#[test]
fn stone_golem_spawns_naturally_in_the_mines() {
    // Conditions from level::try_spawn: a non-surface mine depth (-1..=-3), any time
    // of day, enemy start position, and rnd 71..=85. The finite dungeon depth -4 uses
    // a separate table and does not select stone golems.
    let mut tw = TestWorld::infinite().seed(0x5EED).build();
    let lvl = stage_mine(&mut tw, -1);

    assert_natural_spawn(
        &mut tw,
        lvl,
        WantedMob::StoneGolem,
        0x3C44_0001,
        0x3C44_1001,
    );
}
