//! Headless tests for the survival weapons + food wave: registry/recipe integrity for
//! the new items, headless crafting of every new recipe, live-fire projectile checks
//! against a zombie, the spear throw/pickup roundtrip, and the bandage heal.

use fdoom::entity::mob::player_behavior;
use fdoom::entity::{Direction, EntityKind, behavior};
use fdoom::item::{Inventory, ItemKind, Recipe, ToolType, interact, registry};
use fdoom::testutil::{TestWorld, bare_game, find_recipe};

/// The new weapons/food recipes must sit at the intended stations, and the flora-wave
/// food items must exist with the agreed names and sensible heal values.
#[test]
fn new_items_and_recipes_sit_at_the_right_stations() {
    let g = bare_game("weapons_stations");

    for product in [
        "Crude Spear",
        "Throwing Knife",
        "Slingshot",
        "Bandage",
        "Jack-O-Lantern",
        "Fruit Medley",
    ] {
        find_recipe(&g.recipes.craft, product);
    }
    for product in ["Wood Spear", "Rock Spear", "Crossbow"] {
        find_recipe(&g.recipes.workbench, product);
    }
    for product in [
        "Crossbow Mechanism",
        "Iron Spear",
        "Gold Spear",
        "Gem Spear",
    ] {
        find_recipe(&g.recipes.anvil, product);
    }
    find_recipe(&g.recipes.oven, "Cooked Mushroom");
    find_recipe(&g.recipes.furnace, "Cooked Mushroom");

    // the crossbow is assembled, never conjured whole: its workbench recipe requires
    // the anvil-forged mechanism
    let crossbow = find_recipe(&g.recipes.workbench, "Crossbow");
    assert!(
        crossbow
            .get_costs()
            .iter()
            .any(|(c, _)| c == "CROSSBOW MECHANISM"),
        "Crossbow must require the anvil-forged mechanism"
    );

    // registered food items (the flora wave drops these by name) + expected heal values
    for (name, heal) in [
        ("Berry", 1),
        ("Mushroom", 1),
        ("Apple", 1),
        ("Cactus Fruit", 1),
        ("Coconut", 2),
        ("Cooked Mushroom", 3),
        ("Pumpkin", 2),
        ("Fruit Medley", 3),
    ] {
        let item = registry::get(&g, name);
        match item.kind {
            ItemKind::Food { heal: h, .. } => {
                assert_eq!(h, heal, "{name}: unexpected heal value")
            }
            other => panic!("{name} is not a Food item: {other:?}"),
        }
    }

    // Bandage is the new Medical kind (heals health, not hunger)
    let bandage = registry::get(&g, "Bandage");
    assert!(
        matches!(bandage.kind, ItemKind::Medical { heal: 3, .. }),
        "Bandage must be Medical with heal 3: {:?}",
        bandage.kind
    );

    // the new tools registered: tiered spears, flat crossbow/slingshot
    for name in [
        "Crude Spear",
        "Wood Spear",
        "Rock Spear",
        "Iron Spear",
        "Gold Spear",
        "Gem Spear",
    ] {
        let item = registry::get(&g, name);
        assert!(
            matches!(
                item.kind,
                ItemKind::Tool {
                    ttype: ToolType::Spear,
                    ..
                }
            ),
            "{name} is not a Spear tool"
        );
    }
    for (name, ttype) in [
        ("Crossbow", ToolType::Crossbow),
        ("Slingshot", ToolType::Slingshot),
    ] {
        let item = registry::get(&g, name);
        match item.kind {
            ItemKind::Tool {
                ttype: t, level, ..
            } => {
                assert_eq!(t, ttype, "{name}: wrong tool type");
                assert_eq!(level, 0, "{name} must be a single-tier (level 0) tool");
            }
            other => panic!("{name} is not a tool: {other:?}"),
        }
    }
}

/// Every new recipe crafts headlessly from exactly its listed costs.
#[test]
fn every_new_recipe_crafts_headlessly() {
    let g = bare_game("weapons_craft");
    let recipes = g.recipes.clone();

    let cases: &[(&str, &[Recipe])] = &[
        ("Crude Spear", &recipes.craft),
        ("Throwing Knife", &recipes.craft),
        ("Slingshot", &recipes.craft),
        ("Bandage", &recipes.craft),
        ("Jack-O-Lantern", &recipes.craft),
        ("Fruit Medley", &recipes.craft),
        ("Wood Spear", &recipes.workbench),
        ("Rock Spear", &recipes.workbench),
        ("Crossbow", &recipes.workbench),
        ("Crossbow Mechanism", &recipes.anvil),
        ("Iron Spear", &recipes.anvil),
        ("Gold Spear", &recipes.anvil),
        ("Gem Spear", &recipes.anvil),
        ("Cooked Mushroom", &recipes.oven),
    ];

    for (product, station) in cases {
        let recipe = find_recipe(station, product).clone();
        let mut inv = Inventory::new();
        for (cost, amt) in recipe.get_costs() {
            inv.add(registry::get(&g, &format!("{cost}_{amt}")));
        }
        assert!(
            recipe.craft(&g, &mut inv),
            "{product}: crafting failed with exactly the listed costs"
        );
        let crafted = registry::get(&g, product);
        assert!(
            inv.count(&crafted) >= 1,
            "{product}: product missing after craft"
        );
        // every listed cost was consumed
        for (cost, _) in recipe.get_costs() {
            assert_eq!(
                inv.count(&registry::get(&g, cost)),
                0,
                "{product}: cost {cost} not fully consumed"
            );
        }
    }
}

/// Position the player on a cleared grass strip facing right; returns (x, y) in px.
fn stage_player(tw: &mut TestWorld) -> (i32, i32) {
    for dx in -1..=7 {
        for dy in -1..=1 {
            tw.place("grass", dx, dy);
        }
    }
    let (xt, yt) = tw.player_tile();
    let (px, py) = (xt * 16 + 8, yt * 16 + 8);
    let lvl = tw.current_level;
    tw.g.with_entity(0, |e, g| {
        e.c.x = px;
        e.c.y = py;
        e.c.level = Some(lvl);
        let pd = e.player_mut();
        pd.mob.dir = Direction::Right;
        pd.mob.health = 10;
        pd.stamina = 10;
        pd.hunger = 10;
        pd.active_item = None;
        let _ = g;
    });
    (px, py)
}

/// Spawn a level-1 zombie at pixel coords and return its eid.
fn stage_zombie(tw: &mut TestWorld, x: i32, y: i32) -> i32 {
    let lvl = tw.current_level;
    let mut z = fdoom::entity::mob::zombie::new(tw, 1);
    z.c.x = x;
    z.c.y = y;
    tw.level_mut(lvl).add(z, lvl);
    tw.tick_recover(); // drain the add queue so it lands in the arena with a real eid
    tw.entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Zombie(_)) && (e.c.x - x).abs() < 40)
        .map(|e| e.c.eid)
        .expect("staged zombie not found in the arena")
}

fn zombie_health(tw: &TestWorld, zid: i32) -> Option<i32> {
    tw.entities
        .get(zid)
        .filter(|z| !z.c.removed)
        .and_then(|z| z.mob())
        .map(|m| m.health)
}

/// Fire each projectile weapon at a zombie ~3 tiles away and assert it got hurt.
#[test]
fn projectile_weapons_damage_a_zombie() {
    let mut tw = TestWorld::infinite().seed(0x5EED).build();

    // (weapon, ammo) — ammo None means the weapon itself is thrown/needs nothing extra
    let cases: &[(&str, Option<&str>)] = &[
        ("Crossbow", Some("arrow_3")),
        ("Slingshot", Some("Stone_3")),
        ("Throwing Knife_2", None),
    ];

    for (weapon, ammo) in cases {
        let (px, py) = stage_player(&mut tw);
        let zid = stage_zombie(&mut tw, px + 52, py);
        let h0 = zombie_health(&tw, zid).expect("zombie must start alive");

        tw.g.with_entity(0, |e, g| {
            let pd = e.player_mut();
            pd.active_item = Some(registry::get(g, weapon));
            pd.attack_time = 0;
            if let Some(ammo) = ammo {
                pd.inventory.add(registry::get(g, ammo));
            }
            player_behavior::attack(g, e);
        });

        // the projectile flies ~7-8 px/tick; give it time to connect
        let mut hurt = false;
        for _ in 0..30 {
            tw.tick_recover();
            match zombie_health(&tw, zid) {
                Some(h) if h < h0 => {
                    hurt = true;
                    break;
                }
                Some(_) => {}
                None => {
                    hurt = true; // killed outright (or removed) also counts as a hit
                    break;
                }
            }
        }
        assert!(hurt, "{weapon}: zombie was never damaged");

        // clean up the target for the next scenario
        tw.g.with_entity(zid, |z, g| behavior::remove_entity(g, z));
        tw.tick_recover();
    }
}

/// SHIFT-attack throws the held spear; it lands as a pickup that preserves durability,
/// and picking it back up returns the exact same tool.
#[test]
fn spear_throw_and_pickup_roundtrip() {
    let mut tw = TestWorld::infinite().seed(0x0DD5EED).build();
    stage_player(&mut tw);

    // a used spear: durability must survive the throw/pickup roundtrip
    let mut spear = registry::get(&tw, "Crude Spear");
    let ItemKind::Tool { dur, .. } = &mut spear.kind else {
        panic!("Crude Spear is not a tool");
    };
    *dur = 17;

    tw.input.press_key("shift", true);
    tw.g.with_entity(0, |e, g| {
        e.player_mut().active_item = Some(spear);
        player_behavior::attack(g, e);
        assert!(
            e.player().active_item.is_none(),
            "SHIFT-attack must throw the held spear"
        );
    });
    tw.input.press_key("shift", false);

    // let it fly its full range and land as an item entity
    let mut spear_entity_id = None;
    for _ in 0..40 {
        tw.tick_recover();
        spear_entity_id = tw
            .entities
            .iter()
            .find(|e| match &e.kind {
                EntityKind::ItemEntity(d) => d.item.get_name().eq_ignore_ascii_case("Crude Spear"),
                _ => false,
            })
            .map(|e| e.c.eid);
        if spear_entity_id.is_some() {
            break;
        }
    }
    let spear_entity_id = spear_entity_id.expect("thrown spear never landed as an item entity");

    // pick it back up (the touched_by path funnels into pickup_item)
    tw.g.with_entity(0, |e, g| {
        let mut item_entity = g
            .entities
            .take(spear_entity_id)
            .expect("spear item entity vanished before pickup");
        player_behavior::pickup_item(g, e, &mut item_entity);
        g.entities.put_back(item_entity);

        let recovered = e
            .player()
            .active_item
            .clone()
            .or_else(|| {
                e.player()
                    .inventory
                    .items()
                    .iter()
                    .find(|i| i.get_name().eq_ignore_ascii_case("Crude Spear"))
                    .cloned()
            })
            .expect("picked-up spear missing from hand and inventory");
        let ItemKind::Tool { ttype, dur, .. } = recovered.kind else {
            panic!("recovered spear is not a tool");
        };
        assert_eq!(ttype, ToolType::Spear);
        assert_eq!(dur, 17, "spear durability must survive the roundtrip");
    });
}

/// A bandage restores 3 health (not hunger) and is consumed.
#[test]
fn bandage_restores_health() {
    let mut tw = TestWorld::infinite().seed(0x5EED).build();
    let lvl = tw.current_level;

    tw.g.with_entity(0, |e, g| {
        {
            let pd = e.player_mut();
            pd.mob.health = 4;
            pd.mob.hurt_time = 0;
            pd.hunger = 7;
            pd.stamina = 10;
        }
        let mut bandage = registry::get(g, "Bandage_2");
        assert!(
            interact::item_interact_on_tile(g, &mut bandage, lvl, 0, 0, e, Direction::Down),
            "bandage use failed"
        );
        assert_eq!(e.player().mob.health, 7, "bandage must heal 3 health");
        assert_eq!(e.player().hunger, 7, "bandage must not touch hunger");
        assert_eq!(bandage.count(), 1, "one bandage must be consumed");
    });
}
