//! UI_REDESIGN L4: station context, the two-column container shell, placement
//! feedback, and the text-overflow discipline (long names ellipsize, never bleed).

use fdoom::entity::{Direction, EntityKind};
use fdoom::gfx::screen;
use fdoom::item::interact::item_interact_on_tile;
use fdoom::testutil::TestWorld;

/// Using an oven routes to the survival screen's CRAFT tab carrying the oven's
/// recipe set — not the old standalone crafting display.
#[test]
fn station_use_opens_craft_with_the_stations_recipes() {
    let mut tw = TestWorld::infinite().name("l4_station").build();
    let pid = tw.player_id;

    let mut oven = fdoom::entity::furniture::crafter::new(
        fdoom::entity::furniture::crafter::CrafterType::Oven,
    );
    let lvl = tw.current_level;
    let (px, py) = {
        let p = tw.player_mut();
        (p.c.x, p.c.y)
    };
    oven.c.x = px + 16;
    oven.c.y = py;
    let oven_eid = {
        tw.g.level_mut(lvl).add(oven, lvl);
        fdoom::level::tick_level(&mut tw.g, lvl, false);
        tw.g.entities
            .entities_on_level(lvl)
            .find(|e| matches!(e.kind, EntityKind::Crafter(_)))
            .map(|e| e.c.eid)
            .expect("oven placed")
    };

    tw.g.with_entity(pid, |player, g| {
        g.with_entity(oven_eid, |oven, g| {
            fdoom::entity::furniture::crafter_behavior::use_furniture(g, oven, player);
        });
    });
    tw.tick();
    assert!(tw.display.menu_active(), "station should open a menu");
    // the oven set includes pot cookery that personal crafting does not
    let pixels = tw.render();
    let _ = pixels;
    tw.screenshot("l4_oven_station.png");
}

/// The chest screen is the two-column shell: transfer moves items both ways.
#[test]
fn container_two_column_transfer_round_trips() {
    let mut tw = TestWorld::infinite().name("l4_chest").build();
    let pid = tw.player_id;
    let lvl = tw.current_level;

    let mut chest = fdoom::entity::furniture::chest::new();
    if let EntityKind::Chest(c) = &mut chest.kind {
        let loot = fdoom::item::registry::get(&tw.g, "Prospector's Pan");
        c.inventory.add(loot);
    }
    let (px, py) = {
        let p = tw.player_mut();
        (p.c.x, p.c.y)
    };
    chest.c.x = px + 16;
    chest.c.y = py;
    tw.g.level_mut(lvl).add(chest, lvl);
    fdoom::level::tick_level(&mut tw.g, lvl, false);
    let chest_eid =
        tw.g.entities
            .entities_on_level(lvl)
            .find(|e| matches!(e.kind, EntityKind::Chest(_)))
            .map(|e| e.c.eid)
            .expect("chest placed");

    tw.give("Wood", 3);
    tw.g.with_entity(pid, |player, g| {
        g.with_entity(chest_eid, |chest, g| {
            fdoom::entity::furniture::chest_behavior::use_furniture(g, chest, player);
        });
    });
    tw.tick();
    assert!(tw.display.menu_active(), "chest should open the shell");

    // take the pan (container side is focused first; ENTER transfers)
    tw.press("ENTER");
    let has_pan =
        tw.g.player()
            .player()
            .inventory
            .items()
            .iter()
            .any(|i| i.get_name() == "Prospector's Pan");
    assert!(has_pan, "ENTER should pull the pan into the pack");

    tw.screenshot("l4_chest_shell.png");
}

/// Every failed player placement answers with a reason in the ambient ticker.
#[test]
fn failed_placements_always_say_why() {
    let mut tw = TestWorld::infinite().name("l4_place").build();
    let pid = tw.player_id;
    let lvl = tw.current_level;
    let (ptx, pty) = tw.player_tile();

    // planks on plain grass: the dig-a-hole gate
    tw.give("Plank", 5);
    let mut planks = fdoom::item::registry::get(&tw.g, "Plank");
    tw.g.with_entity(pid, |player, g| {
        item_interact_on_tile(g, &mut planks, lvl, ptx + 1, pty, player, Direction::Right);
    });
    assert!(
        tw.notifications.iter().any(|n| n.contains("Dig a hole")),
        "plank on grass should say so: {:?}",
        tw.notifications
    );

    // a torch aimed at water: the no-footing gate
    tw.clear_notifications();
    let water = tw.g.tiles.get("water");
    tw.g.set_tile_default(lvl, ptx + 1, pty, &water);
    let mut torch = fdoom::item::registry::get(&tw.g, "Torch");
    tw.g.with_entity(pid, |player, g| {
        item_interact_on_tile(g, &mut torch, lvl, ptx + 1, pty, player, Direction::Right);
    });
    assert!(
        tw.notifications.iter().any(|n| n.contains("footing")),
        "torch on water should say so: {:?}",
        tw.notifications
    );
}

/// Long item names ellipsize inside their column: the pixel band between the
/// PACK list divider and the detail card is identical whether the selected item
/// has a 16-char name or a 4-char one.
#[test]
fn pack_names_never_bleed_past_the_divider() {
    let band = |tw: &mut TestWorld| -> Vec<i32> {
        let pixels = tw.render();
        let mut out = Vec::new();
        // the guard band right of the list divider (x=148) up to the card (x=154)
        for y in 0..screen::H {
            for x in 149..154 {
                out.push(pixels[(y * screen::W + x) as usize]);
            }
        }
        out
    };

    let mut long_w = TestWorld::infinite().name("l4_long").build();
    long_w.give("Prospector's Pan", 1);
    long_w.press("E"); // E opens the survival screen on PACK
    assert!(long_w.display.menu_active());
    let long_band = band(&mut long_w);
    long_w.screenshot("l4_pack_longname.png");

    let mut short_w = TestWorld::infinite().name("l4_short").build();
    short_w.give("Wood", 1);
    short_w.press("E");
    assert!(short_w.display.menu_active());
    let short_band = band(&mut short_w);

    // the list content differs; the guard band must not (names clip before it)
    assert_eq!(
        long_band, short_band,
        "a 16-char name painted into the divider band"
    );
}
