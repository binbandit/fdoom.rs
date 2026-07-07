//! Multi-level terrain: dig-down descent (Dug Pit -> Chasm -> Ladder) and deep water.

use fdoom::level::tile::dispatch;
use fdoom::testutil::TestWorld;

#[test]
fn dig_down_through_the_world() {
    let mut tw = TestWorld::infinite().seed(4242).build();
    let lvl = tw.current_level; // surface (3)

    // shovel a grass tile next to the player: grass -> dirt (turf), dirt -> pit
    let (tx, ty) = tw.place("grass", 1, 0);
    assert!(tw.interact_with("Gem Shovel", 1, 0), "shovel grass");
    if tw.tile_at(lvl, tx, ty).id == tw.tiles.get("dirt").id {
        assert!(tw.interact_with("Gem Shovel", 1, 0), "shovel dirt");
    }
    assert!(
        tw.tile_at(lvl, tx, ty).name.eq_ignore_ascii_case("Dug Pit"),
        "shoveling opens a pit"
    );

    // keep digging to bedrock
    for _ in 0..fdoom::level::tile::depth::MAX_STAGE {
        tw.interact_with("Gem Shovel", 1, 0);
    }
    assert_eq!(
        tw.level(lvl).get_data(tx, ty),
        fdoom::level::tile::depth::MAX_STAGE,
        "pit bottoms out at max stage"
    );
    // shovel can't go deeper
    assert!(!tw.interact_with("Gem Shovel", 1, 0));

    // pickaxe breaks through into a chasm...
    assert!(
        tw.interact_with("Gem Pickaxe", 1, 0),
        "pickaxe breakthrough"
    );
    assert!(tw.tile_at(lvl, tx, ty).name.eq_ignore_ascii_case("Chasm"));

    // ...and the mine below has the ladder back up at the same coordinates
    let below = lvl - 1;
    assert!(
        tw.tile_at(below, tx, ty)
            .name
            .eq_ignore_ascii_case("Ladder"),
        "ladder stamped below"
    );

    // standing on the chasm drops the player a level, onto the ladder
    tw.teleport(tx, ty);
    tw.player_mut().player_mut().on_stair_delay = 0;
    for _ in 0..200 {
        tw.tick();
        if tw.current_level == below {
            break;
        }
    }
    assert_eq!(tw.current_level, below, "chasm descends one layer");
    // and riding the ladder goes back up
    tw.teleport(tx, ty);
    tw.player_mut().player_mut().on_stair_delay = 0;
    for _ in 0..200 {
        tw.tick();
        if tw.current_level == lvl {
            break;
        }
    }
    assert_eq!(tw.current_level, lvl, "ladder climbs back up");
}

#[test]
fn deep_water_needs_a_raft() {
    let mut tw = TestWorld::infinite().seed(4242).build();
    let lvl = tw.current_level;
    let (tx, ty) = tw.place("Deep Water", 2, 0);
    let def = tw.tile_at(lvl, tx, ty);

    let blocked = !dispatch::may_pass(&tw, &def, lvl, tx, ty, tw.player());
    assert!(blocked, "deep water blocks a raftless player");

    tw.give("Raft", 1);
    let allowed = dispatch::may_pass(&tw, &def, lvl, tx, ty, tw.player());
    assert!(allowed, "a raft in the inventory lets the player cross");
}
