//! Regression test: the in-game save hotkey (R) used to panic — the player's tick
//! called `save_world_named` directly while the player entity was taken out of the
//! arena for that very tick, and `write_player`'s `g.player()` blew up. The save is
//! now deferred to `Game::tick` (`g.pending_save`).

use fdoom::testutil::TestWorld;

#[test]
fn save_hotkey_saves_without_panicking() {
    let mut tw = TestWorld::infinite().debug().name("hotkey").build();

    // press R during ordinary gameplay (no menu open); the player tick defers the
    // save, the next Game::tick services it (this used to panic)
    tw.press("R");
    tw.tick_n(5); // a few more ticks: saving must unwind cleanly
    assert!(
        !tw.saving,
        "saving flag should clear once the save completes"
    );

    let save_dir = tw.game_dir.join("saves").join("hotkey");
    assert!(
        save_dir.join("Game.miniplussave").exists(),
        "Game save file missing at {save_dir:?}"
    );
    assert!(
        save_dir.join("Player.miniplussave").exists(),
        "Player save file missing at {save_dir:?}"
    );
}
