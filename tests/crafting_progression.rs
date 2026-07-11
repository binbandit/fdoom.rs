//! One continuous survival progression from natural world drops to a crude axe.
//! No test-only inventory shortcuts are used.

use fdoom::entity::EntityKind;
use fdoom::entity::mob::player_behavior;
use fdoom::item::{ItemKind, registry};
use fdoom::level;
use fdoom::level::tile::TileKind;
use fdoom::rng::Rng;
use fdoom::testutil::{TestWorld, find_recipe};

const WORLD_SEEDS: &[i64] = &[
    0x5EED,
    0x00DD_5EED,
    20260707,
    4242,
    101,
    102,
    103,
    104,
    105,
    106,
];

fn reset_rngs(tw: &mut TestWorld, seed: i64) {
    tw.g.random = Rng::new(seed);
    for (idx, lvl) in tw.g.levels.iter_mut().enumerate() {
        if let Some(lvl) = lvl {
            lvl.random = Rng::new(seed ^ ((idx as i64 + 1) * 0x51F1_5EED));
        }
    }
}

fn inv_count(tw: &TestWorld, name: &str) -> i32 {
    let item = registry::get(&tw.g, name);
    tw.player().player().inventory.count(&item)
}

fn find_tiles(
    tw: &TestWorld,
    radius: i32,
    pred: impl Fn(&TileKind, &str) -> bool,
) -> Vec<(i32, i32)> {
    let (px, py) = tw.player_tile();
    let mut found = Vec::new();
    for r in 1..=radius {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let tile = tw.tile_at(tw.current_level, px + dx, py + dy);
                if pred(&tile.kind, &tile.name) {
                    found.push((px + dx, py + dy));
                }
            }
        }
    }
    found
}

fn pickup_matching(tw: &mut TestWorld, names: &[&str]) {
    let lvl = tw.current_level;
    level::tick_level(&mut tw.g, lvl, false);

    let eids: Vec<i32> = tw
        .entities
        .entities_on_level(lvl)
        .filter(|e| !e.c.removed)
        .filter_map(|e| match &e.kind {
            EntityKind::ItemEntity(d)
                if names
                    .iter()
                    .any(|name| d.item.get_name().eq_ignore_ascii_case(name)) =>
            {
                Some(e.c.eid)
            }
            _ => None,
        })
        .collect();

    for eid in eids {
        tw.g.with_entity(tw.g.player_id, |player, g| {
            let mut item_entity = g.entities.take(eid).expect("item entity missing");
            player_behavior::pickup_item(g, player, &mut item_entity);
            g.entities.put_back(item_entity);
        });
    }

    level::tick_level(&mut tw.g, lvl, false);
}

fn hit_tile(tw: &mut TestWorld, tx: i32, ty: i32, dmg: i32) -> bool {
    tw.teleport(tx - 1, ty);
    tw.hit(1, 0, dmg)
}

fn gather_fiber_and_stone(tw: &mut TestWorld) -> bool {
    let grasses = find_tiles(tw, 72, |kind, _| matches!(kind, TileKind::TallGrass { .. }));

    for (tx, ty) in grasses {
        if inv_count(tw, "Grass Fibers") >= 3 && inv_count(tw, "Stone") >= 2 {
            return true;
        }
        if matches!(
            tw.tile_at(tw.current_level, tx, ty).kind,
            TileKind::TallGrass { .. }
        ) {
            assert!(
                hit_tile(tw, tx, ty, 1),
                "failed to break grass at {tx},{ty}"
            );
            pickup_matching(tw, &["Grass Fibers", "Stone"]);
        }
    }

    inv_count(tw, "Grass Fibers") >= 3 && inv_count(tw, "Stone") >= 2
}

fn gather_wood(tw: &mut TestWorld) -> bool {
    let trees = find_tiles(tw, 72, |kind, _| {
        matches!(kind, TileKind::Tree | TileKind::TreeSpecies { .. })
    });

    for (tx, ty) in trees {
        if inv_count(tw, "Wood") >= 1 {
            return true;
        }

        for _ in 0..12 {
            let tile = tw.tile_at(tw.current_level, tx, ty);
            if !matches!(&tile.kind, TileKind::Tree | TileKind::TreeSpecies { .. }) {
                break;
            }
            assert!(hit_tile(tw, tx, ty, 3), "failed to punch tree at {tx},{ty}");
            pickup_matching(tw, &["Wood", "Stick"]);
        }
    }

    inv_count(tw, "Wood") >= 1
}

fn craft_personal(tw: &mut TestWorld, product: &str) {
    let recipe = find_recipe(&tw.g.recipes.craft, product).clone();
    tw.g.with_entity(tw.g.player_id, |player, g| {
        assert!(
            recipe.craft(g, &mut player.player_mut().inventory),
            "crafting {product} failed"
        );
    })
    .expect("player entity missing");
}

fn try_prepare_world(seed: i64) -> Option<TestWorld> {
    let mut tw = TestWorld::infinite().seed(seed).build();
    reset_rngs(&mut tw, 0xC0FF_EE00 | seed);

    {
        let player = tw.player().player();
        assert!(
            player.inventory.items().is_empty(),
            "player inventory is not empty"
        );
        assert!(
            player.active_item.is_none(),
            "player started with a held item"
        );
    }

    if !gather_fiber_and_stone(&mut tw) {
        return None;
    }
    if !gather_wood(&mut tw) {
        return None;
    }

    Some(tw)
}

#[test]
fn survival_gather_craft_chain_makes_a_crude_axe() {
    let mut tw = WORLD_SEEDS
        .iter()
        .find_map(|seed| try_prepare_world(*seed))
        .expect("no fixed candidate seed had nearby grass, stone, and wood");

    assert!(
        inv_count(&tw, "Grass Fibers") >= 3,
        "natural grass did not supply enough fiber"
    );
    assert!(
        inv_count(&tw, "Stone") >= 2,
        "natural gathering did not supply enough stone"
    );
    assert!(
        inv_count(&tw, "Wood") >= 1,
        "natural tree punching did not supply wood"
    );

    craft_personal(&mut tw, "Stick");
    craft_personal(&mut tw, "Cord");
    craft_personal(&mut tw, "Sharp Stone");
    craft_personal(&mut tw, "Crude Axe");

    let axe = registry::get(&tw.g, "Crude Axe");
    assert_eq!(
        tw.player().player().inventory.count(&axe),
        1,
        "crude axe missing from inventory after the real gather/craft chain"
    );

    let crude = tw
        .player()
        .player()
        .inventory
        .items()
        .iter()
        .find(|i| i.get_name() == "Crude Axe")
        .expect("crude axe item missing");
    assert!(
        matches!(crude.kind, ItemKind::Tool { .. }),
        "crafted Crude Axe is not a tool"
    );
}
