//! Regression test: the in-game save hotkey (R) used to panic — the player's tick
//! called `save_world_named` directly while the player entity was taken out of the
//! arena for that very tick, and `write_player`'s `g.player()` blew up. The save is
//! now deferred to `Game::tick` (`g.pending_save`).

#[test]
fn save_hotkey_saves_without_panicking() {
    let tmp = std::env::temp_dir().join("fdoom_test_save_hotkey");
    let _ = std::fs::remove_dir_all(&tmp);
    let mut g = fdoom::core::game::Game::new(true, false, tmp.clone());
    fdoom::core::world::reset_game(&mut g, true);
    g.settings.set("size", 128);
    g.world_name = "hotkey".into();
    fdoom::core::world::init_world(&mut g);
    g.tick(); // drain add queues so the player is live in the arena

    // press R during ordinary gameplay (no menu open)
    g.input.key_toggled("R", true);
    g.tick(); // player tick sees the click and defers the save
    g.input.key_toggled("R", false);
    g.tick(); // Game::tick services the deferred save (this used to panic)
    for _ in 0..5 {
        g.tick(); // a few more ticks: saving must unwind cleanly
    }
    assert!(
        !g.saving,
        "saving flag should clear once the save completes"
    );

    let save_dir = tmp.join("saves").join("hotkey");
    assert!(
        save_dir.join("Game.miniplussave").exists(),
        "Game save file missing at {save_dir:?}"
    );
    assert!(
        save_dir.join("Player.miniplussave").exists(),
        "Player save file missing at {save_dir:?}"
    );
}
