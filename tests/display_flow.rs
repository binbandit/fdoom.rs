//! Display-stack regression tests: the take-out tick pattern must keep Java's
//! `menu != null` semantics (exiting the only-open display used to fail).
use fdoom::core::game::Game;

#[test]
fn inventory_esc_closes() {
    let tmp = std::env::temp_dir().join("fdoom_dbg_esc");
    let _ = std::fs::remove_dir_all(&tmp);
    let mut g = Game::new(true, false, tmp);
    fdoom::core::world::reset_game(&mut g, true);
    g.settings.set("size", 128);
    g.world_name = "dbg".into();
    g.loaded_world = false;
    fdoom::core::world::init_world(&mut g);
    assert!(g.ready_to_render_gameplay);

    g.tick(); // drain the entitiesToAdd queue so the player is in the arena

    // open the inventory like the I key does
    let pid = g.player_id;
    let player = g.entities.take(pid).unwrap();
    let inv_display = fdoom::screen::player_inv_display::PlayerInvDisplay::new(&g, &player);
    g.entities.put_back(player);
    g.set_menu(inv_display);
    g.tick();
    assert!(g.display.menu_active(), "inventory should be open");
    eprintln!("stack len after open: {}", g.display.stack.len());

    // press ESC for one tick
    g.input.key_toggled("ESCAPE", true);
    g.tick();
    g.input.key_toggled("ESCAPE", false);
    eprintln!("stack len after esc tick: {}", g.display.stack.len());
    g.tick();
    eprintln!("stack len after next tick: {}", g.display.stack.len());
    assert!(
        !g.display.menu_active(),
        "inventory should be closed after ESC"
    );
}
