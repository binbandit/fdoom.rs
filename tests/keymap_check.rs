#[test]
fn e_key_opens_inventory_mapping() {
    let mut input = fdoom::core::io::input_handler::InputHandler::new();
    input.key_toggled("E", true);
    input.tick();
    assert!(
        input.get_key("inventory").clicked,
        "E should click inventory"
    );
    assert!(
        !input.get_key("menu").clicked,
        "E must not click menu anymore"
    );
}

#[test]
fn e_opens_inventory_in_game() {
    let tmp = std::env::temp_dir().join("fdoom_dbg_ekey");
    let _ = std::fs::remove_dir_all(&tmp);
    let mut g = fdoom::core::game::Game::new(true, false, tmp);
    fdoom::core::world::reset_game(&mut g, true);
    g.settings.set("size", 128);
    g.world_name = "dbg".into();
    fdoom::core::world::init_world(&mut g);
    g.tick(); // drain add queues

    g.input.key_toggled("E", true);
    g.tick();
    g.input.key_toggled("E", false);
    g.tick();
    assert!(g.display.menu_active(), "E should open the inventory");
}
