//! Adversarial crash hunt for the item/inventory/crafting lane: index invalidation,
//! stack-count arithmetic, loot-roll odds, and crafting every recipe at every station
//! with exact-cost / one-short / overflowing packs.

use fdoom::entity::EntityKind;
use fdoom::item::{Inventory, Item, Recipe, registry};
use fdoom::rng::Rng;
use fdoom::testutil::{TestWorld, bare_game};

/// A registry stackable with `count` in it.
fn stack(g: &fdoom::core::game::Game, name: &str, count: i32) -> Item {
    let mut item = registry::get(g, name);
    assert!(item.is_stackable(), "{name} must be stackable");
    item.set_count(count);
    item
}

/* --------------------------------- stack arithmetic --------------------------------- */

/// Merging two near-max stacks overflowed the count: "attempt to add with overflow"
/// in a debug build, a negative (instantly depleted) stack in release. Reachable from
/// hoarding with the dev console's `give`, and from any save whose item data carries a
/// big count — `registry::get` parses up to `i32::MAX` and `load` feeds it to `add`.
#[test]
fn merging_huge_stacks_saturates_instead_of_overflowing() {
    let g = bare_game("inv_stack_overflow");
    let mut inv = Inventory::new();
    inv.add(stack(&g, "Wood", 2_000_000_000));
    inv.add(stack(&g, "Wood", 2_000_000_000));
    assert_eq!(inv.inv_size(), 1, "the stacks merge into one");
    assert_eq!(
        inv.get(0).count(),
        i32::MAX,
        "and the count pins at the cap"
    );

    // counting across several near-max stacks must not overflow either
    let mut inv = Inventory::new();
    for _ in 0..3 {
        let mut item = stack(&g, "Stone", i32::MAX);
        // sidestep the merge so the stacks stay separate
        item = Item::new(
            &format!("Stone{}", inv.inv_size()),
            item.sprite.clone(),
            item.kind.clone(),
        );
        inv.add(item);
    }
    let probe = stack(&g, "Stone", 1);
    let _ = inv.count(&probe);
}

/// Picking a stack up while already holding one of the same kind merges into the held
/// item — the same addition, on the player's hand instead of the pack.
#[test]
fn picking_up_onto_a_huge_held_stack_does_not_overflow() {
    let mut tw = TestWorld::infinite().name("inv_pickup_overflow").build();
    let pid = tw.player_id;
    let held = stack(&tw.g, "Wood", 2_000_000_000);
    tw.g.with_entity(pid, |player, _g| {
        player.player_mut().active_item = Some(held);
    });

    let dropped = stack(&tw.g, "Wood", 2_000_000_000);
    let lvl = tw.current_level;
    let (px, py) = tw.player_pos();
    fdoom::level::drop_item(&mut tw.g, lvl, px, py, dropped);
    fdoom::level::tick_level(&mut tw.g, lvl, false);

    let item_eid =
        tw.g.entities
            .entities_on_level(lvl)
            .find(|e| matches!(e.kind, EntityKind::ItemEntity(_)))
            .map(|e| e.c.eid)
            .expect("the drop is live");
    tw.g.with_entity(pid, |player, g| {
        g.with_entity(item_eid, |drop, g| {
            fdoom::entity::mob::player_behavior::pickup_item(g, player, drop);
        });
    });
    let count =
        tw.g.with_entity(pid, |player, _g| {
            player.player().active_item.as_ref().map(|i| i.count())
        })
        .flatten();
    assert_eq!(count, Some(i32::MAX), "the held stack pins at the cap");
}

/* ------------------------------------ loot odds ------------------------------------ */

/// `try_add` takes a 1-in-`chance` roll. Callers compute the odds by dividing
/// (`9 / chance`, `3 / chance`), so integer division hands it a 0 — and the RNG
/// asserts `bound > 0`, panicking with "bound must be positive" while filling a
/// loot chest.
#[test]
fn try_add_with_zero_or_negative_chance_does_not_panic() {
    let g = bare_game("inv_try_add_zero");
    let mut rng = Rng::new(7);
    let mut inv = Inventory::new();
    let wood = registry::get(&g, "Wood");

    for chance in [0, -1, i32::MIN] {
        inv.try_add(&mut rng, chance, Some(wood.clone()));
        inv.try_add_num(&mut rng, chance, Some(wood.clone()), 3);
        inv.try_add_all_or_nothing(&mut rng, chance, &wood, 2, false);
        inv.try_add_all_or_nothing(&mut rng, chance, &wood, 2, true);
    }
    // a zero chance is a certainty, not a crash
    assert!(
        inv.count(&wood) > 0,
        "a 1-in-0 roll always hands the item over"
    );
}

/// The in-game caller of the above: the spawner-dungeon loot table divides its odds
/// by the layer depth, so at the dungeon layer three of its entries roll `3 / 4 == 0`.
#[test]
fn spawner_dungeon_chests_fill_without_panicking() {
    let mut tw = TestWorld::infinite().name("inv_spawner_loot").build();
    // the dungeon is the deepest layer of the 5-layer world
    let dungeon =
        tw.g.levels
            .iter()
            .enumerate()
            .find(|(_, l)| l.as_ref().is_some_and(|l| l.depth == -4))
            .map(|(i, _)| i)
            .expect("the dungeon layer exists");
    fdoom::core::world::generate_spawner_structures(&mut tw.g, dungeon);
}

/* ------------------------------- index invalidation ------------------------------- */

/// `add_at` inserts at a slot index. Every caller passes an index it just read off a
/// menu row, and menus outlive the collection they indexed — a slot past the end must
/// land at the end, not panic with "insertion index is out of bounds".
#[test]
fn add_at_a_stale_slot_lands_in_the_pack() {
    let g = bare_game("inv_add_at_stale");
    let mut inv = Inventory::new();
    inv.add(registry::get(&g, "Wood"));
    inv.add(registry::get(&g, "Stone"));
    // the row list said slot 7; the pack shrank to 2 under it
    inv.add_at(7, registry::get(&g, "Coal"));
    assert_eq!(inv.inv_size(), 3);
    inv.add_at(i32::MAX, registry::get(&g, "Sand"));
    assert_eq!(inv.inv_size(), 4);
}

/// The full drop/hold/equip menu loop with the selection parked on the LAST row,
/// driven until the pack empties: every one of those actions removes the row the
/// selection points at.
#[test]
fn emptying_the_pack_from_the_last_row_never_indexes_past_it() {
    for creative in [false, true] {
        let mut tw = {
            let b = TestWorld::infinite().name(&format!("inv_last_row_{creative}"));
            if creative { b.creative() } else { b }.build()
        };
        for name in ["Wood", "Stone", "Coal", "Leather Armor", "Cord"] {
            tw.give(name, 3);
        }
        tw.press("E");
        assert!(tw.g.display.menu_active(), "the survival screen opened");
        // park on the last row, then drop everything from there
        for _ in 0..8 {
            tw.press("UP");
        }
        for _ in 0..40 {
            tw.press("SHIFT-Q");
            tw.render();
        }
        // and again with ENTER (hold), which closes and reopens the screen
        for _ in 0..20 {
            if !tw.g.display.menu_active() {
                tw.press("E");
            }
            tw.press("UP");
            tw.press("ENTER");
            tw.render();
        }
    }
}

/// Transfers between a container and the pack, from both sides, with the selection at
/// the end of each list and rows disappearing under it.
#[test]
fn container_transfers_from_the_last_row_in_both_directions() {
    for creative in [false, true] {
        let mut tw = {
            let b = TestWorld::infinite().name(&format!("inv_container_{creative}"));
            if creative { b.creative() } else { b }.build()
        };
        for name in ["Wood", "Stone", "Coal"] {
            tw.give(name, 4);
        }
        let mut chest = fdoom::entity::furniture::chest::new();
        for name in ["Cord", "Sand", "Leather Armor"] {
            let item = registry::get(&tw.g, &format!("{name}_2"));
            chest.chest_mut().expect("chest").inventory.add(item);
        }
        let lvl = tw.current_level;
        let (px, py) = tw.player_pos();
        chest.c.eid = 800_001;
        chest.c.x = px + 16;
        chest.c.y = py;
        tw.g.level_mut(lvl).add(chest, lvl);
        fdoom::level::tick_level(&mut tw.g, lvl, false);

        let pid = tw.player_id;
        tw.g.with_entity(pid, |player, g| {
            g.with_entity(800_001, |c, g| {
                fdoom::entity::furniture::behavior::use_furniture(g, c, player);
            });
        });
        tw.tick();
        assert!(tw.g.display.menu_active(), "the container screen opened");

        // hammer both sides: move stacks and singles until both lists are empty
        for i in 0..60 {
            if i % 7 == 0 {
                tw.press("LEFT");
            }
            tw.press("DOWN");
            tw.press(if i % 3 == 0 { "Q" } else { "ENTER" });
            tw.render();
        }
        // then destroy the chest under the open screen and keep going
        tw.g.entities.delete(800_001);
        for _ in 0..6 {
            tw.press("ENTER");
            tw.render();
        }
    }
}

/* -------------------------------- crafting torture -------------------------------- */

fn all_station_lists(g: &fdoom::core::game::Game) -> Vec<(&'static str, Vec<Recipe>)> {
    vec![
        ("craft", g.recipes.craft.clone()),
        ("workbench", g.recipes.workbench.clone()),
        ("oven", g.recipes.oven.clone()),
        ("furnace", g.recipes.furnace.clone()),
        ("anvil", g.recipes.anvil.clone()),
        ("enchant", g.recipes.enchant.clone()),
        ("loom", g.recipes.loom.clone()),
        ("bench_modules", g.recipes.bench_modules.clone()),
    ]
}

/// Craft every recipe on every station list against an empty pack, an exact-cost pack,
/// a one-short pack, and a pack already holding `i32::MAX` of the product.
#[test]
fn every_recipe_crafts_at_every_stock_level() {
    let g = bare_game("inv_craft_matrix");
    for (station, recipes) in all_station_lists(&g) {
        for recipe in recipes {
            // empty pack: must decline, not panic
            let mut inv = Inventory::new();
            assert!(
                !recipe.craft(&g, &mut inv),
                "{station}/{} crafted from nothing",
                recipe.product_name()
            );

            // exact cost
            let mut inv = Inventory::new();
            for (cost, amt) in recipe.get_costs() {
                let mut item = registry::get(&g, cost);
                if item.is_stackable() {
                    item.set_count(*amt);
                    inv.add(item);
                } else {
                    for _ in 0..*amt {
                        inv.add(item.clone());
                    }
                }
            }
            assert!(
                recipe.craft(&g, &mut inv),
                "{station}/{} could not be crafted with its exact cost",
                recipe.product_name()
            );

            // one short of every cost
            let mut inv = Inventory::new();
            for (cost, amt) in recipe.get_costs() {
                let mut item = registry::get(&g, cost);
                if item.is_stackable() {
                    item.set_count((*amt - 1).max(0));
                    if item.count() > 0 {
                        inv.add(item);
                    }
                } else {
                    for _ in 0..(*amt - 1).max(0) {
                        inv.add(item.clone());
                    }
                }
            }
            assert!(
                !recipe.craft(&g, &mut inv),
                "{station}/{} crafted while a cost was short",
                recipe.product_name()
            );

            // a pack already holding the maximum of the product AND of every cost
            let mut inv = Inventory::new();
            let mut product = recipe.get_product(&g);
            if product.is_stackable() {
                product.set_count(i32::MAX);
                inv.add(product);
            }
            for (cost, _) in recipe.get_costs() {
                let mut item = registry::get(&g, cost);
                if item.is_stackable() {
                    item.set_count(i32::MAX);
                    inv.add(item);
                } else {
                    for _ in 0..64 {
                        inv.add(item.clone());
                    }
                }
            }
            recipe.craft(&g, &mut inv);
        }
    }
}

/// Crafting through the live CRAFT tab with the selection parked at the end of the
/// list, repeated until the ingredients run out (each craft rewrites the pack rows).
#[test]
fn crafting_from_the_live_screen_until_the_pack_runs_dry() {
    let mut tw = TestWorld::infinite().name("inv_craft_live").build();
    for (name, n) in [("Grass Fibers", 30), ("Stone", 30), ("Wood", 30)] {
        tw.give(name, n);
    }
    tw.press("Z");
    assert!(tw.g.display.menu_active(), "the craft screen opened");
    for i in 0..120 {
        if i % 5 == 0 {
            tw.press("DOWN");
        }
        tw.press("ENTER");
        tw.render();
    }
}
