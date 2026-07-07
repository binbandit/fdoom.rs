//! Headless tests for the verbose early-game crafting chain:
//! grass fibers -> cord, knapped stone -> sharp stone, stick + cord + sharp stone ->
//! crude tools — plus a registry-integrity sweep over every recipe in every station.

use std::path::{Path, PathBuf};

use fdoom::core::game::Game;
use fdoom::core::world;
use fdoom::entity::{Direction, EntityKind};
use fdoom::item::{ItemKind, Recipe, ToolType, registry};
use fdoom::level::tile::dispatch;

fn temp_game_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A headless game with the main player created (as `Game.main` does before world init).
fn new_game(dir: &Path) -> Game {
    let mut g = Game::new(false, false, dir.to_path_buf());
    let mut player = fdoom::entity::mob::player::new(&g, None);
    player.c.eid = 0;
    g.entities.put_back(player);
    g
}

fn make_world(g: &mut Game, seed: i64) {
    world::reset_game(g, false);
    g.settings.set("size", 128);
    g.settings.set("autosave", false);
    g.world_seed = seed;
    world::init_world(g);
}

/// True when a registry lookup for `name` resolves to a real item (not `UnknownItem`).
fn resolves(g: &Game, name: &str) -> bool {
    registry::get_opt(g, name, true)
        .is_some_and(|item| !matches!(item.kind, ItemKind::Unknown { .. }))
}

fn find_recipe<'a>(recipes: &'a [Recipe], product: &str) -> &'a Recipe {
    recipes
        .iter()
        .find(|r| r.product_name().eq_ignore_ascii_case(product))
        .unwrap_or_else(|| panic!("recipe for {product:?} not found"))
}

/// Every recipe's product and every cost must resolve in the item registry —
/// a typo'd name silently crafts an UnknownItem, so catch it here.
#[test]
fn all_recipe_names_resolve_in_registry() {
    let dir = temp_game_dir("crafting_registry");
    let g = new_game(&dir);
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
    let dir = temp_game_dir("crafting_personal");
    let g = new_game(&dir);
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
    let dir = temp_game_dir("crafting_loop");
    let mut g = new_game(&dir);
    make_world(&mut g, 0x5EED);

    // Gathered bare-handed: fibers + pebbles from tall grass, wood from a punched tree.
    let fibers = registry::get(&g, "Grass Fibers_3");
    let stone = registry::get(&g, "Stone_2");
    let wood = registry::get(&g, "Wood_1");

    let crafted = g
        .with_entity(0, |e, g| {
            let inv = &mut e.player_mut().inventory;
            inv.add(fibers);
            inv.add(stone);
            inv.add(wood);

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
    let dir = temp_game_dir("crafting_chop");
    let mut g = new_game(&dir);
    make_world(&mut g, 0x5EED);

    let lvl = g.current_level;
    let (px, py) = {
        let p = g.player();
        (p.c.x >> 4, p.c.y >> 4)
    };
    let (tx, ty) = (px + 1, py);

    // Fist: the bare-hand attack path hurts the tile with 1-3 damage.
    g.set_tile_named(lvl, tx, ty, "tree");
    let tree = g.tile_at(lvl, tx, ty);
    g.with_entity(0, |e, g| {
        let dmg = g.random.next_int_bound(3) + 1; // player_behavior's bare-hand roll
        dispatch::hurt_by(g, &tree, lvl, tx, ty, e, dmg, Direction::Down);
    });
    let fist_damage = g.level(lvl).get_data(tx, ty);
    assert!(
        (1..=3).contains(&fist_damage),
        "fist damage out of range: {fist_damage}"
    );

    // Crude axe: the tool interact path (pays stamina + durability).
    g.set_tile_named(lvl, tx, ty, "tree"); // fresh tree, damage 0
    let tree = g.tile_at(lvl, tx, ty);
    let mut axe = registry::get(&g, "Crude Axe");
    let used = g
        .with_entity(0, |e, g| {
            e.player_mut().stamina = 10;
            dispatch::interact(g, &tree, lvl, tx, ty, e, &mut axe, Direction::Down)
        })
        .expect("player entity missing");
    assert!(used, "crude axe did not interact with the tree");
    let axe_damage = g.level(lvl).get_data(tx, ty);
    assert!(
        axe_damage > fist_damage,
        "crude axe ({axe_damage}) should out-damage fists ({fist_damage})"
    );
    if let ItemKind::Tool { dur, .. } = axe.kind {
        assert!(dur < ToolType::Axe.durability(), "axe paid no durability");
    }

    // Tall grass breaks bare-handed into (at least) two fibers.
    g.set_tile_named(lvl, tx, ty, "Tall Grass");
    let grass = g.tile_at(lvl, tx, ty);
    g.with_entity(0, |e, g| {
        dispatch::hurt_by(g, &grass, lvl, tx, ty, e, 1, Direction::Down);
    });
    assert_eq!(g.tile_at(lvl, tx, ty).name, "GRASS");
    let fiber_drops = g
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
