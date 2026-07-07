//! Regression: attacking tiles must work at NEGATIVE world coordinates (infinite
//! worlds have no edges; a finite-bounds guard once ate every attack west/north of 0,0).

use fdoom::core::{game::Game, world};

#[test]
fn punching_breaks_things() {
    let tmp = std::env::temp_dir().join("fdoom_attack_repro");
    let _ = std::fs::remove_dir_all(&tmp);
    let mut g = Game::new(false, false, tmp);
    world::reset_game(&mut g, true);
    g.world_name = "atk".into();
    g.world_seed = 99;
    world::init_world(&mut g);
    g.tick();

    let lvl = g.current_level;
    let (px, py) = {
        let p = g.player();
        (p.c.x >> 4, p.c.y >> 4)
    };
    // put a tall grass tile right of the player and face right
    let tg = g.tiles.get("tall grass");
    g.set_tile_default(lvl, px + 1, py, &tg);
    {
        let p = g.player_mut();
        p.player_mut().stamina = 10;
    }
    // walk right briefly so the player faces the grass
    g.input.key_toggled("D", true);
    g.tick();
    g.input.key_toggled("D", false);
    // press attack (SPACE) for a few ticks
    for i in 0..30 {
        g.input.key_toggled("SPACE", i % 4 == 0);
        g.tick();
        if g.tile_at(lvl, px + 1, py).id != tg.id {
            break;
        }
    }
    assert_ne!(
        g.tile_at(lvl, px + 1, py).id,
        tg.id,
        "punching tall grass should break it"
    );
}
