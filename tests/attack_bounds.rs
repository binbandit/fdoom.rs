//! Regression: attacking tiles must work at NEGATIVE world coordinates (infinite
//! worlds have no edges; a finite-bounds guard once ate every attack west/north of 0,0).

use fdoom::testutil::TestWorld;

#[test]
fn punching_breaks_things() {
    let mut tw = TestWorld::infinite().seed(99).build();

    // put a tall grass tile right of the player and face right
    let (tx, ty) = tw.place("tall grass", 1, 0);
    let tg_id = tw.tile_at(tw.current_level, tx, ty).id;
    tw.player_mut().player_mut().stamina = 10;

    // walk right briefly so the player faces the grass
    tw.press("D");
    // press attack (SPACE) for a few ticks
    for i in 0..30 {
        tw.input.key_toggled("SPACE", i % 4 == 0);
        tw.tick();
        if tw.tile_at(tw.current_level, tx, ty).id != tg_id {
            break;
        }
    }
    assert_ne!(
        tw.tile_at(tw.current_level, tx, ty).id,
        tg_id,
        "punching tall grass should break it"
    );
}
