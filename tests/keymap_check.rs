use fdoom::testutil::TestWorld;

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
    let mut tw = TestWorld::infinite().debug().build();
    tw.press("E");
    assert!(tw.display.menu_active(), "E should open the inventory");
}
