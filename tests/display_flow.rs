//! Display-stack regression tests: the take-out tick pattern must keep Java's
//! `menu != null` semantics (exiting the only-open display used to fail).

use fdoom::testutil::TestWorld;

#[test]
fn inventory_esc_closes() {
    let mut tw = TestWorld::infinite().debug().build();
    assert!(tw.ready_to_render_gameplay);

    // open the inventory like the I key does
    let pid = tw.player_id;
    let player = tw.entities.take(pid).unwrap();
    let inv_display = fdoom::screen::player_inv_display::PlayerInvDisplay::new(&tw, &player);
    tw.entities.put_back(player);
    tw.set_menu(inv_display);
    tw.tick();
    assert!(tw.display.menu_active(), "inventory should be open");

    tw.press("ESCAPE");
    assert!(
        !tw.display.menu_active(),
        "inventory should be closed after ESC"
    );
}
