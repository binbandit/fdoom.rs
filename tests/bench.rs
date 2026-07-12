//! THE BENCH (UI_REDESIGN L5): the modular prospector's station. Craft it cheap,
//! bolt on found-or-crafted modules, and its recipe list grows; legacy bench-shaped
//! stations still work and break down into their module.

use fdoom::entity::EntityKind;
use fdoom::entity::furniture::crafter::{CrafterType, Module};
use fdoom::entity::furniture::crafter_behavior::{bench_recipes, fitted_mask};
use fdoom::testutil::TestWorld;

fn spawn_crafter(tw: &mut TestWorld, ctype: CrafterType) -> i32 {
    let lvl = tw.current_level;
    let (px, py) = {
        let p = tw.player_mut();
        (p.c.x, p.c.y)
    };
    let mut e = fdoom::entity::furniture::crafter::new(ctype);
    e.c.x = px + 16;
    e.c.y = py;
    tw.g.level_mut(lvl).add(e, lvl);
    fdoom::level::tick_level(&mut tw.g, lvl, false);
    tw.g.entities
        .entities_on_level(lvl)
        .find(|e| matches!(&e.kind, EntityKind::Crafter(c) if c.crafter_type == ctype))
        .map(|e| e.c.eid)
        .expect("crafter placed")
}

fn use_crafter(tw: &mut TestWorld, eid: i32) {
    let pid = tw.player_id;
    tw.g.with_entity(pid, |player, g| {
        g.with_entity(eid, |e, g| {
            fdoom::entity::furniture::crafter_behavior::use_furniture(g, e, player);
        });
    });
    tw.tick();
}

/// The bench is in the personal chain — a fresh survivor can reach every recipe
/// family with zero loot finds (bench recipe + module recipes at the bench).
#[test]
fn every_family_reachable_without_loot() {
    let tw = TestWorld::infinite().name("bench_paths").build();
    assert!(
        tw.g.recipes
            .craft
            .iter()
            .any(|r| r.product_name().eq_ignore_ascii_case("bench")),
        "the bench must be personally craftable"
    );
    for m in Module::VALUES {
        assert!(
            tw.g.recipes
                .bench_modules
                .iter()
                .any(|r| r.product_name().eq_ignore_ascii_case(m.item_name())),
            "{} must be craftable at the bench (loot is a shortcut, not a gate)",
            m.item_name()
        );
    }
    // the retired standalone stations are no longer craftable anywhere
    for retired in ["Anvil", "Loom", "Enchanter", "Workbench"] {
        let anywhere =
            tw.g.recipes
                .craft
                .iter()
                .chain(&tw.g.recipes.workbench)
                .any(|r| r.product_name().eq_ignore_ascii_case(retired));
        assert!(!anywhere, "{retired} recipe should be retired");
    }
}

/// Holding a module and using the bench bolts it on: the recipe list grows by
/// that family, and the module is consumed.
#[test]
fn holding_a_module_fits_it_and_grows_the_list() {
    let mut tw = TestWorld::infinite().name("bench_fit").build();
    let bench = spawn_crafter(&mut tw, CrafterType::Bench);

    let before = bench_recipes(&tw.g, &[]).len();
    let vice = fdoom::item::registry::get(&tw.g, "Vice");
    tw.player_mut().player_mut().active_item = Some(vice);
    use_crafter(&mut tw, bench);

    let modules = {
        let e = tw.g.entities.get(bench).unwrap();
        match &e.kind {
            EntityKind::Crafter(c) => c.modules.clone(),
            _ => unreachable!(),
        }
    };
    assert_eq!(modules, vec![Module::Vice], "the vice should be fitted");
    assert!(
        tw.g.player().player().active_item.is_none(),
        "the module is consumed"
    );
    let after = bench_recipes(&tw.g, &modules).len();
    assert!(
        after > before,
        "the anvil family should join the list ({before} -> {after})"
    );
    assert_eq!(fitted_mask(&modules), [true, false, false]);
    tw.screenshot("bench_rack_vice.png");
}

/// Fitted modules survive save/load (trailing-field scheme, old-save tolerant).
#[test]
fn fitted_modules_survive_save_and_load() {
    let mut tw = TestWorld::infinite().name("bench_save").build();
    let bench = spawn_crafter(&mut tw, CrafterType::Bench);
    for name in ["Vice", "Assay Kit"] {
        let m = fdoom::item::registry::get(&tw.g, name);
        tw.player_mut().player_mut().active_item = Some(m);
        use_crafter(&mut tw, bench);
        tw.press("ESCAPE");
    }

    let name = tw.g.world_name.clone();
    fdoom::saveload::save::save_world_named(&mut tw.g, &name);

    let mut g2 = fdoom::core::game::Game::new(false, false, tw.g.game_dir.clone());
    let mut player = fdoom::entity::mob::player::new(&g2, None);
    player.c.eid = 0;
    g2.entities.put_back(player);
    fdoom::saveload::load::load_world_named(&mut g2, &name);

    // loaded entities sit in the level's add-queue until the first tick drains it
    let lvl = g2.current_level;
    let modules = g2
        .entities
        .iter()
        .chain(g2.level(lvl).entities_to_add.iter())
        .find_map(|e| match &e.kind {
            EntityKind::Crafter(c) if c.crafter_type == CrafterType::Bench => {
                Some(c.modules.clone())
            }
            _ => None,
        })
        .expect("bench should survive the roundtrip");
    assert!(modules.contains(&Module::Vice));
    assert!(modules.contains(&Module::AssayKit));
    assert!(!modules.contains(&Module::Spindle));
}

/// A grandfathered anvil still opens its own list, and ENTER on it in the pack
/// breaks it down into the VICE.
#[test]
fn legacy_anvil_works_and_breaks_down_into_its_module() {
    let mut tw = TestWorld::infinite().name("bench_legacy").build();
    let anvil = spawn_crafter(&mut tw, CrafterType::Anvil);
    use_crafter(&mut tw, anvil);
    assert!(tw.display.menu_active(), "legacy anvil still opens");
    tw.press("ESCAPE");

    // glove-style: put the anvil furniture item straight into the pack
    let anvil_item = fdoom::item::registry::get(&tw.g, "Anvil");
    assert!(
        matches!(anvil_item.kind, fdoom::item::ItemKind::Furniture { .. }),
        "anvil furniture item must exist for old saves"
    );
    tw.player_mut().player_mut().inventory.add(anvil_item);

    tw.press("E");
    assert!(tw.display.menu_active());
    tw.press("ENTER"); // the only pack item is the anvil -> breaks down
    let inv_names: Vec<String> =
        tw.g.player()
            .player()
            .inventory
            .items()
            .iter()
            .map(|i| i.get_name().to_string())
            .collect();
    assert!(
        inv_names.iter().any(|n| n == "Vice"),
        "breakdown should yield the Vice, got {inv_names:?}"
    );
    assert!(
        !inv_names.iter().any(|n| n == "Anvil"),
        "the anvil is consumed"
    );
}

/// Heat stations are untouched: the oven still opens with its own recipe list.
#[test]
fn oven_and_furnace_stay_their_own_stations() {
    let mut tw = TestWorld::infinite().name("bench_heat").build();
    let oven = spawn_crafter(&mut tw, CrafterType::Oven);
    use_crafter(&mut tw, oven);
    assert!(tw.display.menu_active(), "oven still opens its own screen");
    assert!(
        tw.g.recipes
            .workbench
            .iter()
            .any(|r| r.product_name().eq_ignore_ascii_case("oven")),
        "the oven stays buildable"
    );
}
