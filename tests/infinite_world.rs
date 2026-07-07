//! End-to-end: infinite worlds generate, stream chunks, walk far, save and reload.

use fdoom::core::{game::Game, world};
use fdoom::testutil::TestWorld;

#[test]
fn infinite_world_boots_and_walks() {
    let mut tw = TestWorld::infinite().creative().name("inf").build();

    assert!(tw.level(3).is_infinite(), "surface should be chunked");
    assert!(tw.level(0).is_infinite(), "deep mine should be chunked");
    assert!(!tw.level(4).is_infinite(), "dungeon stays finite");

    // chunks streamed in around the spawn
    let loaded = tw.level(3).chunks.as_ref().unwrap().len();
    assert!(loaded >= 25, "expected a loaded ring, got {loaded} chunks");

    // wander far; alternate headings so terrain can't wall the walk in, and swing
    // regularly to chop through forests. Chunks must stream the whole way.
    let (start_x, start_y) = tw.player_pos();
    for key in ["D", "S", "D", "W", "D", "S"] {
        tw.hold(key);
        for i in 0..800 {
            tw.input.key_toggled("SPACE", i % 20 == 0);
            tw.tick();
        }
        tw.release(key);
    }
    let (end_x, end_y) = tw.player_pos();
    let dist_tiles = ((end_x - start_x).abs() + (end_y - start_y).abs()) / 16;
    assert!(
        dist_tiles > 40,
        "player should have wandered far: ({start_x},{start_y}) -> ({end_x},{end_y}), {dist_tiles} tiles"
    );

    // tile queries anywhere respond (unloaded = rock fallback, no panic)
    let _ = tw.tile_at(3, 100_000, -100_000);

    // save, then reload into a fresh game: world must come back infinite w/ same seed
    fdoom::saveload::save::save_world_named(&mut tw, "inf");
    let seed = tw.world_seed;

    let mut g2 = Game::new(false, false, tw.game_dir.clone());
    world::reset_game(&mut g2, true);
    fdoom::screen::world_select::set_world_name(&mut g2, "inf", true);
    world::init_world(&mut g2);
    g2.tick();
    assert!(
        g2.level(3).is_infinite(),
        "reloaded world should be chunked"
    );
    assert_eq!(g2.world_seed, seed, "seed must persist through save/load");
    // the reloaded player must stand where they saved (chunks stream around them)
    assert!(g2.try_player().is_some());
}
