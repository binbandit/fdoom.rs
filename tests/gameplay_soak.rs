//! Headless gameplay soak tests: generate real worlds and drive thousands of game ticks
//! with pseudo-random input, day/night flips, level changes and a TNT explosion, asserting
//! the game never panics, the player survives (or respawns), and the entity arena does not
//! grow without bound.

use std::path::{Path, PathBuf};

use fdoom::core::game::Game;
use fdoom::core::updater::Time;
use fdoom::core::world;
use fdoom::entity::EntityKind;

/// Sane ceiling for the arena on a 128x128 world: mob caps are per level (~150-300) and
/// six levels exist, plus particles/items; anything past this indicates a leak.
const MAX_ENTITIES: usize = 5000;

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
    player.c.eid = 0; // Java main() gives the main player eid 0
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

/// Tiny xorshift for driving inputs; deliberately independent of the game's RNG.
fn next_rand(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Tick once, then deal with whatever the tick produced: close any menu that opened
/// (pause/death/level transition) and respawn the player if it died — exactly what the
/// death display's "respawn" entry does.
fn tick_and_recover(g: &mut Game) {
    g.tick();
    if g.display.menu_active() {
        g.clear_menu();
    }
    let player_gone = g.try_player().map(|p| p.c.removed).unwrap_or(true);
    if player_gone {
        world::reset_game(g, true);
    }
}

fn assert_world_sane(g: &mut Game, context: &str) {
    assert!(
        g.try_player().is_some(),
        "{context}: player entity missing from the arena"
    );
    let count = g.entities.len();
    assert!(
        count < MAX_ENTITIES,
        "{context}: entity arena grew to {count} (>= {MAX_ENTITIES})"
    );
}

/// ~5000 ticks of pseudo-random movement/attack with periodic day/night flips so night
/// mobs spawn and fight, on two different world seeds.
#[test]
fn soak_random_play_two_seeds() {
    for (i, seed) in [0x00C0FFEE_i64, 0x5EED_5EED_i64].into_iter().enumerate() {
        let dir = temp_game_dir(&format!("gameplay_soak_{i}"));
        let mut g = new_game(&dir);
        make_world(&mut g, seed);
        assert_world_sane(&mut g, "after init_world");

        let keys = ["W", "A", "S", "D", "SPACE", "SPACE"];
        let mut rng = seed as u64 | 1;
        for tick in 0..5000 {
            let r = next_rand(&mut rng);
            let key = keys[(r % keys.len() as u64) as usize];
            g.input.key_toggled(key, (r >> 8) & 1 == 0);

            // Flip day/night every ~10 in-game seconds so night mobs spawn and despawn.
            if tick % 1200 == 0 {
                g.change_time_of_day(Time::Night);
            } else if tick % 1200 == 600 {
                g.change_time_of_day(Time::Morning);
            }

            tick_and_recover(&mut g);
        }
        assert_world_sane(&mut g, &format!("seed {seed:#x} after 5000 ticks"));
    }
}

/// Walk all six levels (surface -> underground x3 -> dungeon wrap -> sky -> surface),
/// ticking 200 times on each, so stairs placement, per-level spawning and the air wizard
/// all get exercised.
#[test]
fn soak_walk_all_levels() {
    let dir = temp_game_dir("gameplay_soak_levels");
    let mut g = new_game(&dir);
    make_world(&mut g, 0x1E5E15);

    // Tick once so the freshly added player is committed from the level's add-queue into
    // the arena. (`world::change_level` right after `init_world` would otherwise drop the
    // player: `level.remove(pid)` deletes it from the add-queue before `player_mut()`.)
    tick_and_recover(&mut g);

    let start_level = g.current_level;
    for step in 0..6 {
        world::change_level(&mut g, -1);
        for _ in 0..200 {
            tick_and_recover(&mut g);
        }
        assert_world_sane(&mut g, &format!("level walk step {step}"));
    }
    assert_eq!(
        g.current_level, start_level,
        "six -1 level changes should wrap back to the start level"
    );
}

/// Light a TNT right next to the player and tick through the fuse, the blast (which hurts
/// the player and smashes tiles) and the exploded-tile restore countdown.
#[test]
fn soak_tnt_explosion_near_player() {
    let dir = temp_game_dir("gameplay_soak_tnt");
    let mut g = new_game(&dir);
    make_world(&mut g, 0x7477 /* "tnt" */);

    let lvl = g.current_level;
    let (px, py) = {
        let p = g.try_player().expect("player exists after init");
        (p.c.x, p.c.y)
    };

    let mut tnt = fdoom::entity::furniture::tnt::new();
    tnt.c.x = px + 12;
    tnt.c.y = py;
    let EntityKind::Tnt(data) = &mut tnt.kind else {
        panic!("tnt::new() did not build a Tnt entity");
    };
    data.fuse_lit = true;
    g.level_mut(lvl).add(tnt, lvl);

    // Fuse (90 ticks) + explosion + tile-restore countdown (18 ticks), with margin.
    for _ in 0..200 {
        tick_and_recover(&mut g);
    }
    assert_world_sane(&mut g, "after TNT explosion");

    // The TNT entity must be gone once the explosion resolved.
    let tnt_left = g
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Tnt(_)))
        .count();
    assert_eq!(tnt_left, 0, "exploded TNT entity was not removed");
}
