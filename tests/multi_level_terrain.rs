//! Multi-level terrain: dig-down descent (Dug Pit -> Chasm -> Ladder) and deep water.

use fdoom::core::{game::Game, world};
use fdoom::entity::Direction;
use fdoom::level::tile::dispatch;

fn new_infinite(dir_name: &str) -> Game {
    let tmp = std::env::temp_dir().join(dir_name);
    let _ = std::fs::remove_dir_all(&tmp);
    let mut g = Game::new(false, false, tmp);
    world::reset_game(&mut g, true);
    g.settings.set("worldtype", "Infinite");
    g.world_name = "digtest".into();
    g.world_seed = 4242;
    world::init_world(&mut g);
    g.tick();
    g
}

fn interact(g: &mut Game, lvl: usize, xt: i32, yt: i32, item_name: &str) -> bool {
    let mut item = fdoom::item::registry::get(g, item_name);
    let mut player = g.entities.take(g.player_id).expect("player");
    let def = g.tile_at(lvl, xt, yt);
    let used = dispatch::interact(
        g,
        &def,
        lvl,
        xt,
        yt,
        &mut player,
        &mut item,
        Direction::Down,
    );
    g.entities.put_back(player);
    used
}

#[test]
fn dig_down_through_the_world() {
    let mut g = new_infinite("fdoom_dig_test");
    let lvl = g.current_level; // surface (3)
    let (px, py) = {
        let p = g.player();
        (p.c.x >> 4, p.c.y >> 4)
    };

    // find a dirt-convertible tile next to the player: shovel the grass tile under them
    let (tx, ty) = (px + 1, py);
    let grass = g.tiles.get("grass");
    g.set_tile_default(lvl, tx, ty, &grass);

    // grass -> dirt (shovel turf), dirt -> pit
    assert!(interact(&mut g, lvl, tx, ty, "Gem Shovel"), "shovel grass");
    let dirt_id = g.tiles.get("dirt").id;
    if g.tile_at(lvl, tx, ty).id == dirt_id {
        assert!(interact(&mut g, lvl, tx, ty, "Gem Shovel"), "shovel dirt");
    }
    assert!(
        g.tile_at(lvl, tx, ty).name.eq_ignore_ascii_case("Dug Pit"),
        "shoveling opens a pit"
    );

    // keep digging to bedrock
    for _ in 0..fdoom::level::tile::depth::MAX_STAGE {
        interact(&mut g, lvl, tx, ty, "Gem Shovel");
    }
    assert_eq!(
        g.level(lvl).get_data(tx, ty),
        fdoom::level::tile::depth::MAX_STAGE,
        "pit bottoms out at max stage"
    );
    // shovel can't go deeper
    assert!(!interact(&mut g, lvl, tx, ty, "Gem Shovel"));

    // pickaxe breaks through into a chasm...
    assert!(
        interact(&mut g, lvl, tx, ty, "Gem Pickaxe"),
        "pickaxe breakthrough"
    );
    assert!(g.tile_at(lvl, tx, ty).name.eq_ignore_ascii_case("Chasm"));

    // ...and the mine below has the ladder back up at the same coordinates
    let below = lvl - 1;
    assert!(
        g.tile_at(below, tx, ty).name.eq_ignore_ascii_case("Ladder"),
        "ladder stamped below"
    );

    // standing on the chasm drops the player a level, onto the ladder
    {
        let p = g.player_mut();
        p.c.x = tx * 16 + 8;
        p.c.y = ty * 16 + 8;
        p.player_mut().on_stair_delay = 0;
    }
    for _ in 0..200 {
        g.tick();
        if g.current_level == below {
            break;
        }
    }
    assert_eq!(g.current_level, below, "chasm descends one layer");
    // and riding the ladder goes back up
    {
        let p = g.player_mut();
        p.c.x = tx * 16 + 8;
        p.c.y = ty * 16 + 8;
        p.player_mut().on_stair_delay = 0;
    }
    for _ in 0..200 {
        g.tick();
        if g.current_level == lvl {
            break;
        }
    }
    assert_eq!(g.current_level, lvl, "ladder climbs back up");
}

#[test]
fn deep_water_needs_a_raft() {
    let mut g = new_infinite("fdoom_raft_test");
    let lvl = g.current_level;
    let deep = g.tiles.get("Deep Water");
    let (px, py) = {
        let p = g.player();
        (p.c.x >> 4, p.c.y >> 4)
    };
    g.set_tile_default(lvl, px + 2, py, &deep);
    let def = g.tile_at(lvl, px + 2, py);

    let blocked = {
        let p = g.player();
        !dispatch::may_pass(&g, &def, lvl, px + 2, py, p)
    };
    assert!(blocked, "deep water blocks a raftless player");

    let raft = fdoom::item::registry::get(&g, "Raft");
    g.player_mut().player_mut().inventory.add(raft);
    let allowed = {
        let p = g.player();
        dispatch::may_pass(&g, &def, lvl, px + 2, py, p)
    };
    assert!(allowed, "a raft in the inventory lets the player cross");
}
