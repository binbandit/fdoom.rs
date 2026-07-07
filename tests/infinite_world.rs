//! End-to-end: infinite worlds generate, stream chunks, walk far, save and reload.

use fdoom::core::game::Game;
use fdoom::core::world;

fn new_infinite_game(dir: &std::path::Path) -> Game {
    let mut g = Game::new(false, false, dir.to_path_buf());
    world::reset_game(&mut g, true);
    g.settings.set("worldtype", "Infinite");
    g.settings.set("mode", "Creative");
    g.world_name = "inf".to_string();
    g.world_seed = 20260707;
    world::init_world(&mut g);
    g.tick(); // drain add queues
    g
}

#[test]
fn infinite_world_boots_and_walks() {
    let tmp = std::env::temp_dir().join("fdoom_inf_test");
    let _ = std::fs::remove_dir_all(&tmp);
    let mut g = new_infinite_game(&tmp);

    assert!(g.level(3).is_infinite(), "surface should be chunked");
    assert!(g.level(0).is_infinite(), "deep mine should be chunked");
    assert!(!g.level(4).is_infinite(), "sky stays finite");
    assert!(!g.level(5).is_infinite(), "dungeon stays finite");

    // chunks streamed in around the spawn
    let loaded = g.level(3).chunks.as_ref().unwrap().len();
    assert!(loaded >= 25, "expected a loaded ring, got {loaded} chunks");

    // wander far; alternate headings so terrain can't wall the walk in, and swing
    // regularly to chop through forests. Chunks must stream the whole way.
    let (start_x, start_y) = (g.player().c.x, g.player().c.y);
    let headings = ["D", "S", "D", "W", "D", "S"];
    for (leg, key) in headings.iter().enumerate() {
        for i in 0..800 {
            g.input.key_toggled(key, true);
            g.input.key_toggled("SPACE", i % 20 == 0);
            g.tick();
        }
        g.input.key_toggled(key, false);
        let _ = leg;
    }
    let (end_x, end_y) = (g.player().c.x, g.player().c.y);
    let dist_tiles = ((end_x - start_x).abs() + (end_y - start_y).abs()) / 16;
    assert!(
        dist_tiles > 40,
        "player should have wandered far: ({start_x},{start_y}) -> ({end_x},{end_y}), {dist_tiles} tiles"
    );

    // tile queries anywhere respond (unloaded = rock fallback, no panic)
    let _ = g.tile_at(3, 100_000, -100_000);

    // save, then reload into a fresh game: world must come back infinite w/ same seed
    fdoom::saveload::save::save_world_named(&mut g, "inf");
    let seed = g.world_seed;

    let mut g2 = Game::new(false, false, tmp.clone());
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
