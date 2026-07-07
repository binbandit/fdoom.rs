//! Headless tests for the verbose early-game crafting chain:
//! grass fibers -> cord, knapped stone -> sharp stone, stick + cord + sharp stone ->
//! crude tools — plus a registry-integrity sweep over every recipe in every station.

use fdoom::entity::EntityKind;
use fdoom::item::{ItemKind, Recipe, ToolType, registry};
use fdoom::testutil::{TestWorld, bare_game, find_recipe};

/// True when a registry lookup for `name` resolves to a real item (not `UnknownItem`).
fn resolves(g: &fdoom::core::game::Game, name: &str) -> bool {
    registry::get_opt(g, name, true)
        .is_some_and(|item| !matches!(item.kind, ItemKind::Unknown { .. }))
}

/// Every recipe's product and every cost must resolve in the item registry —
/// a typo'd name silently crafts an UnknownItem, so catch it here.
#[test]
fn all_recipe_names_resolve_in_registry() {
    let g = bare_game("crafting_registry");
    let recipes = g.recipes.clone();
    let stations: [(&str, &[Recipe]); 7] = [
        ("craft", &recipes.craft),
        ("workbench", &recipes.workbench),
        ("loom", &recipes.loom),
        ("oven", &recipes.oven),
        ("furnace", &recipes.furnace),
        ("anvil", &recipes.anvil),
        ("enchant", &recipes.enchant),
    ];
    for (station, list) in stations {
        assert!(!list.is_empty(), "{station}: no recipes registered");
        for r in list {
            assert!(
                resolves(&g, r.product_name()),
                "{station}: product {:?} is not a registry item",
                r.product_name()
            );
            for (cost, amt) in r.get_costs() {
                assert!(*amt > 0, "{station}: {:?} costs 0 {cost}", r.product_name());
                assert!(
                    resolves(&g, cost),
                    "{station}: recipe {:?} cost {cost:?} is not a registry item",
                    r.product_name()
                );
            }
        }
    }
}

/// The survival chain must live in *personal* crafting (no station, no tools).
#[test]
fn personal_crafting_offers_the_survival_chain() {
    let g = bare_game("crafting_personal");
    for product in [
        "Cord",
        "Sharp Stone",
        "Stick",
        "Crude Axe",
        "Crude Pickaxe",
        "Fishing Rod",
        "Workbench",
    ] {
        find_recipe(&g.recipes.craft, product);
    }
    // The old "5 wood -> full wood tool, bare-handed" magic must be gone: wood-tier
    // tools live at the workbench and all require cord.
    for product in [
        "Wood Sword",
        "Wood Axe",
        "Wood Hoe",
        "Wood Pickaxe",
        "Wood Shovel",
        "Wood Bow",
    ] {
        assert!(
            !g.recipes
                .craft
                .iter()
                .any(|r| r.product_name().eq_ignore_ascii_case(product)),
            "{product} must not be personally craftable"
        );
        let r = find_recipe(&g.recipes.workbench, product);
        assert!(
            r.get_costs().iter().any(|(c, _)| c == "CORD"),
            "{product} workbench recipe must require Cord"
        );
    }
}

/// Simulates the first-hour loop from gathered materials: 1 wood -> 2 sticks,
/// 3 fibers -> cord, 2 stone -> sharp stone, then lash them into a crude axe.
#[test]
fn early_loop_crafts_a_crude_axe() {
    let mut tw = TestWorld::infinite().seed(0x5EED).build();

    // Gathered bare-handed: fibers + pebbles from tall grass, wood from a punched tree.
    tw.give("Grass Fibers", 3);
    tw.give("Stone", 2);
    tw.give("Wood", 1);

    let crafted =
        tw.g.with_entity(0, |e, g| {
            for product in ["Stick", "Cord", "Sharp Stone", "Crude Axe"] {
                let recipe = find_recipe(&g.recipes.craft, product).clone();
                assert!(
                    recipe.craft(g, &mut e.player_mut().inventory),
                    "crafting {product} failed (missing ingredients?)"
                );
            }

            let inv = &e.player().inventory;
            // 1 Wood -> 2 Sticks; the axe consumed one, so one stick remains.
            assert_eq!(inv.count(&registry::get(g, "Stick")), 1);
            assert_eq!(inv.count(&registry::get(g, "Grass Fibers")), 0);
            assert_eq!(inv.count(&registry::get(g, "Stone")), 0);
            inv.items()
                .iter()
                .find(|i| i.get_name() == "Crude Axe")
                .cloned()
                .expect("crude axe missing from inventory")
        })
        .expect("player entity missing");

    let ItemKind::Tool { ttype, level, dur } = crafted.kind else {
        panic!("crude axe is not a tool: {:?}", crafted.kind);
    };
    assert_eq!(ttype, ToolType::Axe);
    assert_eq!(level, 0, "crude tier must be tool level 0");
    assert!(dur > 0);
}

/// A crude axe chop hurts a tree more than a bare-handed punch, and tall grass
/// yields fibers when broken with no tool at all.
#[test]
fn crude_axe_outchops_fists_and_grass_yields_fibers() {
    let mut tw = TestWorld::infinite().seed(0x5EED).build();
    let lvl = tw.current_level;

    // Fist: the bare-hand attack path hurts the tile with 1-3 damage.
    let (tx, ty) = tw.place("tree", 1, 0);
    let dmg = tw.random.next_int_bound(3) + 1; // player_behavior's bare-hand roll
    tw.hit(1, 0, dmg);
    let fist_damage = tw.level(lvl).get_data(tx, ty);
    assert!(
        (1..=3).contains(&fist_damage),
        "fist damage out of range: {fist_damage}"
    );

    // Crude axe: the tool interact path (pays stamina + durability).
    tw.place("tree", 1, 0); // fresh tree, damage 0
    tw.player_mut().player_mut().stamina = 10;
    let mut axe = registry::get(&tw, "Crude Axe");
    assert!(
        tw.interact_item(&mut axe, 1, 0),
        "crude axe did not interact with the tree"
    );
    let axe_damage = tw.level(lvl).get_data(tx, ty);
    assert!(
        axe_damage > fist_damage,
        "crude axe ({axe_damage}) should out-damage fists ({fist_damage})"
    );
    if let ItemKind::Tool { dur, .. } = axe.kind {
        assert!(dur < ToolType::Axe.durability(), "axe paid no durability");
    }

    // Tall grass breaks bare-handed into (at least) two fibers.
    tw.place("Tall Grass", 1, 0);
    tw.hit(1, 0, 1);
    assert_eq!(tw.tile_at(lvl, tx, ty).name, "GRASS");
    let fiber_drops = tw
        .level(lvl)
        .entities_to_add
        .iter()
        .filter(|e| match &e.kind {
            EntityKind::ItemEntity(d) => d.item.get_name() == "Grass Fibers",
            _ => false,
        })
        .count();
    assert!(
        fiber_drops >= 2,
        "tall grass dropped {fiber_drops} fibers, expected at least 2"
    );
}
