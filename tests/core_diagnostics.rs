//! Regression tests for `src/core` diagnostics: panics that were reachable through
//! public world-lifecycle entry points are now non-panicking recoveries, and the
//! recovery paths announce themselves through `core::log`.

use fdoom::core::log::{self, Level};
use fdoom::core::world;
use fdoom::testutil::TestWorld;

/// `reset_game`'s respawn path parks the player in the surface level's add-queue
/// until the next tick (`Level::add`). `change_level` then destroyed that player:
/// `Level::remove` clears the add-queue entry, after which no player exists anywhere.
///
/// Pre-fix panic observed here: `player entity missing` (src/entity/behavior.rs,
/// `Game::player_mut`), reached from src/core/world.rs `change_level`.
#[test]
fn change_level_tolerates_add_queue_player() {
    let mut tw = TestWorld::infinite().seed(0xC0FFEE).build();
    world::reset_game(&mut tw.g, true);
    let pid = tw.g.player_id;
    assert!(
        tw.g.entities.get(pid).is_none(),
        "precondition: respawned player is parked in a level add-queue, not the arena"
    );
    assert!(
        tw.g.try_player().is_some(),
        "precondition: the queue-aware lookup still sees the player"
    );

    world::change_level(&mut tw.g, -1);

    assert_eq!(tw.g.current_level, 2, "the level change went through");
    assert!(
        tw.g.try_player().is_some(),
        "the player survived the level change"
    );
}

/// With no player anywhere (arena or add-queues), a level change is skipped with a
/// warning instead of panicking.
///
/// Pre-fix panic observed here: `player entity missing` (src/entity/behavior.rs,
/// `Game::player_mut`), reached from src/core/world.rs `change_level`.
#[test]
fn change_level_without_player_warns_and_skips() {
    let mut tw = TestWorld::infinite().seed(7).build();
    let pid = tw.g.player_id;
    tw.g.entities.delete(pid);
    let before = tw.g.current_level;

    let (_, lines) = log::capture(Level::Warn, || world::change_level(&mut tw.g, -1));

    assert_eq!(tw.g.current_level, before, "the level change was skipped");
    assert!(
        tw.g.try_player().is_none(),
        "no player was conjured out of nowhere"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("WARN ") && l.contains("level change")),
        "the skip was announced as a warning, got: {lines:?}"
    );
}

/// `check_chest_count` places dungeon chests by retrying random tiles until one is
/// obsidian. A dungeon level holding no obsidian at all — reachable from a damaged
/// save whose level file parses but contains none — spun that search forever, hanging
/// the load with no error message and no crash: the worst possible failure mode.
///
/// If this regresses, the test hangs rather than fails; that is the nature of the bug.
#[test]
fn chest_placement_gives_up_on_a_dungeon_with_no_obsidian() {
    let mut tw = TestWorld::infinite().seed(31337).build();
    let dungeon = fdoom::level::lvl_idx(-4);

    // Scrub the floor: every tile becomes rock, so the obsidian search can never hit.
    let rock = tw.g.tiles.get("rock").id;
    let level = tw.g.levels[dungeon]
        .as_mut()
        .expect("the dungeon level is generated at world init");
    level.tiles.fill(rock);
    assert!(
        level.w >= 128,
        "precondition: the chest loop only runs for w >= 128, got {}",
        level.w
    );

    let (_, lines) = log::capture(Level::Warn, || {
        world::check_chest_count(&mut tw.g, dungeon, false);
    });

    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("WARN ") && l.contains("no obsidian")),
        "giving up on chest placement was announced as a warning, got: {lines:?}"
    );
}
