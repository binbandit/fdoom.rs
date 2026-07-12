//! First-day onboarding thread: the one-time ambient cues, the empty-inventory
//! hint, and craft-menu affordability dimming (PLAYTEST item 4).

use fdoom::entity::EntityKind;
use fdoom::entity::mob::player_behavior;
use fdoom::gfx::color;
use fdoom::testutil::TestWorld;

/// The hint/dim text tier renders in DARK_GRAY (shade 222); count its lit pixels.
fn dark_gray_pixels(pixels: &[i32]) -> usize {
    let dim = color::upgrade(color::get_byte(222));
    pixels.iter().filter(|&&p| p == dim).count()
}

#[test]
fn tall_grass_cue_fires_once_on_new_worlds_only() {
    let mut tw = TestWorld::infinite().name("fd_cue1").build();
    assert!(
        tw.player_mut().player_mut().grass_cue_delay > 0,
        "a brand-new world should arm the tall-grass cue"
    );

    // shorten the minute-long fuse so the test doesn't tick 3600 times
    tw.player_mut().player_mut().grass_cue_delay = 3;
    tw.tick_n(5);
    assert!(
        tw.notifications.iter().any(|n| n.contains("tall grass")),
        "cue 1 should fire when the delay runs out: {:?}",
        tw.notifications
    );

    // one-shot: the fuse is spent, it never fires again
    tw.clear_notifications();
    tw.tick_n(10);
    assert!(
        !tw.notifications.iter().any(|n| n.contains("tall grass")),
        "cue 1 must not re-fire"
    );
}

#[test]
fn tall_grass_cue_skipped_if_player_already_has_fibers() {
    let mut tw = TestWorld::infinite().name("fd_cue1b").build();
    tw.give("Grass Fibers", 2);
    tw.player_mut().player_mut().grass_cue_delay = 3;
    tw.tick_n(5);
    assert!(
        !tw.notifications.iter().any(|n| n.contains("tall grass")),
        "no tall-grass hint for a player who already found fibers"
    );
}

#[test]
fn first_fiber_pickup_points_at_the_craft_key() {
    let mut tw = TestWorld::infinite().name("fd_cue2").build();
    let lvl = tw.current_level;
    let (px, py) = {
        let p = tw.player_mut();
        (p.c.x, p.c.y)
    };

    // drop fibers at the player's feet and pick them up through the real path, twice
    for round in 0..2 {
        let fibers = fdoom::item::registry::get(&tw, "Grass Fibers");
        fdoom::level::drop_item(&mut tw.g, lvl, px, py, fibers);
        // drops sit in the level's pending list until it ticks
        fdoom::level::tick_level(&mut tw.g, lvl, false);
        let eid = tw
            .g
            .entities
            .entities_on_level(lvl)
            .find_map(|e| match &e.kind {
                EntityKind::ItemEntity(d) if d.item.get_name() == "Grass Fibers" => Some(e.c.eid),
                _ => None,
            })
            .expect("dropped fibers should exist");
        tw.g.with_entity(tw.g.player_id, |player, g| {
            let mut item_entity = g.entities.take(eid).expect("item entity");
            player_behavior::pickup_item(g, player, &mut item_entity);
            g.entities.put_back(item_entity);
        });

        let fired = tw
            .notifications
            .iter()
            .any(|n| n.contains("twist into cord"));
        if round == 0 {
            assert!(fired, "first fibers should cue the craft key");
            assert!(
                tw.notifications
                    .iter()
                    .any(|n| n.contains("[Z]") || n.contains('[')),
                "the cue should name the craft binding: {:?}",
                tw.notifications
            );
            tw.clear_notifications();
        } else {
            assert!(!fired, "the fiber cue is one-shot");
        }
    }
}

#[test]
fn empty_inventory_shows_a_hint_line() {
    let mut tw = TestWorld::infinite().name("fd_inv").build();

    let pid = tw.player_id;
    let player = tw.entities.take(pid).unwrap();
    let inv = fdoom::screen::survival_display::SurvivalDisplay::new(&tw, &player);
    tw.entities.put_back(player);
    tw.set_menu(inv);
    tw.tick();
    let empty_dim = dark_gray_pixels(&tw.render());
    tw.screenshot("first_day_inv_empty.png");
    tw.press("ESCAPE");

    tw.give("Wood", 5);
    let player = tw.entities.take(pid).unwrap();
    let inv = fdoom::screen::survival_display::SurvivalDisplay::new(&tw, &player);
    tw.entities.put_back(player);
    tw.set_menu(inv);
    tw.tick();
    let filled_dim = dark_gray_pixels(&tw.render());

    assert!(
        empty_dim > filled_dim + 20,
        "the empty panel should carry a dim hint line (empty {empty_dim} vs filled {filled_dim})"
    );
}

#[test]
fn unaffordable_recipes_render_dimmer_than_affordable_ones() {
    let mut tw = TestWorld::infinite().name("fd_craft").build();

    // bare hands: everything unaffordable — unselected rows drop to the dim tier
    tw.press("Z");
    assert!(tw.display.menu_active(), "craft menu should open on Z");
    let broke_dim = dark_gray_pixels(&tw.render());
    tw.screenshot("first_day_craft_broke.png");
    tw.press("ESCAPE");

    // flush with materials: the same rows brighten back to the normal tier
    for (item, n) in [("Wood", 99), ("Grass Fibers", 99), ("Stone", 99)] {
        tw.give(item, n);
    }
    tw.press("Z");
    assert!(tw.display.menu_active());
    let flush_dim = dark_gray_pixels(&tw.render());
    tw.screenshot("first_day_craft_flush.png");

    assert!(
        broke_dim > flush_dim + 20,
        "unaffordable rows should read dimmer (broke {broke_dim} vs flush {flush_dim})"
    );
}
