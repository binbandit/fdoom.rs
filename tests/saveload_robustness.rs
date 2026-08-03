//! Damaged-save regression tests.
//!
//! Every test here feeds the loader input that used to PANIC (truncated records,
//! non-numeric fields, out-of-range indexes) and asserts the loader now recovers:
//! a warning is emitted and the game stays playable. Warnings are observed through
//! `fdoom::core::log::capture`; assertions use `contains`/`any` rather than exact
//! line counts, because capture buffers are process-global and a concurrent test
//! may emit unrelated lines.

use fdoom::core::game::Game;
use fdoom::core::log::{self, Level};
use fdoom::entity::EntityKind;
use fdoom::saveload::load::{self, Load, load_entity};
use fdoom::saveload::{Version, save};
use fdoom::testutil::bare_game;

fn v3() -> Version {
    Version::new("3.0.0")
}

/// A headless game with all five level slots instantiated (16x16), so loaded
/// entities have somewhere to land.
fn game_with_levels(name: &str) -> Game {
    let mut g = bare_game(name);
    let diff = g.settings.get_idx("diff");
    for (i, &depth) in fdoom::level::IDX_TO_DEPTH.iter().enumerate() {
        g.levels[i] = Some(fdoom::level::Level::empty(16, 16, depth, diff));
    }
    g
}

/// Entities queued onto level `lvl` by `load_entity`.
fn queued(g: &Game, lvl: usize) -> &[fdoom::entity::Entity] {
    &g.level(lvl).entities_to_add
}

/* ------------------------------- entity records ------------------------------- */

#[test]
fn entity_record_without_brackets_is_skipped() {
    let mut g = game_with_levels("robust_nobracket");
    let (res, warns) = log::capture(Level::Warn, || {
        let a = load_entity(&mut g, "Zombie", &v3(), true);
        let b = load_entity(&mut g, "Zom]bie[8:16:3]", &v3(), true);
        (a, b)
    });
    assert_eq!(res, (None, None));
    assert!(
        warns.iter().any(|l| l.contains("Zombie")),
        "warns: {warns:?}"
    );
    for lvl in 0..5 {
        assert!(queued(&g, lvl).is_empty());
    }
}

#[test]
fn entity_record_too_short_recovers() {
    let mut g = game_with_levels("robust_short");
    let (_, warns) = log::capture(Level::Warn, || {
        load_entity(&mut g, "Zombie[5]", &v3(), true);
        load_entity(&mut g, "Bed[]", &v3(), true);
    });
    assert!(!warns.is_empty(), "expected warnings for truncated records");
}

#[test]
fn mob_health_field_recovers() {
    let mut g = game_with_levels("robust_health");
    let (_, warns) = log::capture(Level::Warn, || {
        load_entity(&mut g, "Zombie[8:16:banana:2:3]", &v3(), true);
    });
    let z = queued(&g, 3)
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Zombie(_)))
        .expect("zombie should still load");
    assert_eq!(z.enemy_mob().unwrap().lvl, 2);
    assert!(
        z.mob().unwrap().health > 0,
        "health fell back to a sane default"
    );
    assert!(
        warns.iter().any(|l| l.contains("banana")),
        "warns: {warns:?}"
    );
}

#[test]
fn mob_level_field_recovers() {
    let mut g = game_with_levels("robust_moblvl");
    let (_, warns) = log::capture(Level::Warn, || {
        load_entity(&mut g, "Zombie[8:16:5:banana:3]", &v3(), true);
    });
    let z = queued(&g, 3)
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Zombie(_)))
        .expect("zombie should still load");
    assert_eq!(z.enemy_mob().unwrap().lvl, 1, "mob level fell back to 1");
    assert_eq!(z.mob().unwrap().health, 5);
    assert!(
        warns.iter().any(|l| l.contains("banana")),
        "warns: {warns:?}"
    );
}

#[test]
fn chest_record_too_short_recovers() {
    let mut g = game_with_levels("robust_chest_short");
    let (_, warns) = log::capture(Level::Warn, || {
        load_entity(&mut g, "Chest[8:16]", &v3(), true);
    });
    assert!(
        !warns.is_empty(),
        "expected a warning for the truncated chest"
    );
}

#[test]
fn death_chest_missing_time_recovers() {
    let mut g = game_with_levels("robust_deathchest");
    let (_, warns) = log::capture(Level::Warn, || {
        load_entity(&mut g, "DeathChest[8:16:3]", &v3(), true);
    });
    assert!(
        queued(&g, 3)
            .iter()
            .any(|e| matches!(e.kind, EntityKind::DeathChest(_))),
        "death chest should load with a default despawn time"
    );
    assert!(
        warns.iter().any(|l| l.contains("DeathChest")),
        "warns: {warns:?}"
    );
}

#[test]
fn scav_container_trailer_recovers() {
    let mut g = game_with_levels("robust_scav");
    let (_, warns) = log::capture(Level::Warn, || {
        load_entity(&mut g, "ScavContainer[8:16:3]", &v3(), true);
    });
    let sc = queued(&g, 3)
        .iter()
        .find_map(|e| match &e.kind {
            EntityKind::ScavContainer(sc) => Some(sc),
            _ => None,
        })
        .expect("scav container should load with defaults");
    assert_eq!(
        sc.kind,
        fdoom::entity::furniture::scav_container::ScavKind::Crate
    );
    assert!(!sc.searched);
    assert!(!warns.is_empty(), "warns: {warns:?}");
}

#[test]
fn spawner_missing_mob_level_recovers() {
    let mut g = game_with_levels("robust_spawner");
    let (_, warns) = log::capture(Level::Warn, || {
        load_entity(&mut g, "Spawner[8:16:Zombie]", &v3(), true);
    });
    assert!(
        !warns.is_empty(),
        "expected warnings for the short spawner record"
    );
}

#[test]
fn lantern_bad_ordinal_recovers() {
    let mut g = game_with_levels("robust_lantern");
    let (_, warns) = log::capture(Level::Warn, || {
        load_entity(&mut g, "Lantern[8:16:99:3]", &v3(), true);
    });
    let lt = queued(&g, 3)
        .iter()
        .find_map(|e| match &e.kind {
            EntityKind::Lantern(l) => Some(l.lantern_type),
            _ => None,
        })
        .expect("lantern should load");
    assert_eq!(lt, fdoom::entity::furniture::lantern::LanternType::Norm);
    assert!(warns.iter().any(|l| l.contains("99")), "warns: {warns:?}");
}

#[test]
fn entity_level_field_recovers() {
    let mut g = game_with_levels("robust_entlevel");
    let (_, warns) = log::capture(Level::Warn, || {
        load_entity(&mut g, "Bed[8:16:99]", &v3(), true);
        load_entity(&mut g, "Bed[8:16:banana]", &v3(), true);
    });
    for lvl in 0..5 {
        assert!(
            queued(&g, lvl).is_empty(),
            "bad-level entities must be dropped"
        );
    }
    assert!(warns.iter().any(|l| l.contains("Bed")), "warns: {warns:?}");
}

#[test]
fn non_local_record_without_eid_is_skipped() {
    let mut g = game_with_levels("robust_noeid");
    let (res, warns) = log::capture(Level::Warn, || {
        load_entity(&mut g, "Cow[8:16]", &v3(), false)
    });
    assert_eq!(res, None);
    assert!(warns.iter().any(|l| l.contains("Cow")), "warns: {warns:?}");
}

#[test]
fn non_local_transient_payloads_recover() {
    let mut g = game_with_levels("robust_transient");
    let (_, warns) = log::capture(Level::Warn, || {
        load_entity(&mut g, "ItemEntity[8:16:7:apple]", &v3(), false);
        load_entity(&mut g, "TextParticle[8:16:7:msg:red:3]", &v3(), false);
    });
    assert!(
        queued(&g, 3)
            .iter()
            .any(|e| matches!(e.kind, EntityKind::TextParticle(_))),
        "text particle should load with a default color"
    );
    assert!(!warns.is_empty(), "warns: {warns:?}");
}

/* ------------------------------- player / inventory ------------------------------- */

#[test]
fn player_file_truncated_recovers() {
    let mut g = bare_game("robust_player_trunc");
    let l = Load::with_version(&g, v3());
    let (_, warns) = log::capture(Level::Warn, || l.load_player(&mut g, &[]));
    let p = g
        .entities
        .get(g.player_id)
        .expect("player entity must survive a truncated Player file");
    assert_eq!(p.player().shirt_color, 110, "boot default kept");
    assert_eq!(
        g.current_level,
        fdoom::level::lvl_idx(0),
        "fell back to the surface"
    );
    assert!(!warns.is_empty(), "warns: {warns:?}");
}

fn player_record(potions: &str, level: &str) -> Vec<String> {
    format!("264,152,16,9,7,5,0,1234,{level},{potions},520,true")
        .split(',')
        .map(String::from)
        .collect()
}

#[test]
fn unknown_potion_effect_skipped() {
    let mut g = bare_game("robust_potion");
    let l = Load::with_version(&g, v3());
    let data = player_record("PotionEffects[Bogus;100]", "3");
    let (_, warns) = log::capture(Level::Warn, || l.load_player(&mut g, &data));
    let p = g.entities.get(g.player_id).expect("player survived");
    assert_eq!(p.player().get_score(), 1234);
    assert!(p.player().potioneffects.is_empty());
    assert!(
        warns.iter().any(|l| l.contains("Bogus")),
        "warns: {warns:?}"
    );
}

#[test]
fn potion_effect_missing_duration_recovers() {
    let mut g = bare_game("robust_potion_dur");
    let l = Load::with_version(&g, v3());
    let data = player_record("PotionEffects[Regen]", "3");
    let (_, warns) = log::capture(Level::Warn, || l.load_player(&mut g, &data));
    let p = g.entities.get(g.player_id).expect("player survived");
    assert_eq!(p.player().get_score(), 1234);
    assert!(
        warns.iter().any(|l| l.contains("Regen")),
        "warns: {warns:?}"
    );
}

#[test]
fn player_level_out_of_range_recovers() {
    let mut g = bare_game("robust_player_lvl");
    let l = Load::with_version(&g, v3());
    let data = player_record("PotionEffects[]", "99");
    let (_, warns) = log::capture(Level::Warn, || l.load_player(&mut g, &data));
    assert_eq!(
        g.current_level,
        fdoom::level::lvl_idx(0),
        "fell back to the surface"
    );
    assert!(g.entities.get(g.player_id).is_some(), "player survived");
    assert!(warns.iter().any(|l| l.contains("99")), "warns: {warns:?}");
}

#[test]
fn inventory_stack_count_recovers() {
    let g = bare_game("robust_inv");
    let l = Load::with_version(&g, Version::new("2.0.0"));
    let mut inv = fdoom::item::Inventory::new_player();
    let data = vec!["Apple;banana".to_string(), "Apple;".to_string()];
    let (_, warns) = log::capture(Level::Warn, || l.load_inventory(&g, &mut inv, &data));
    assert_eq!(inv.inv_size(), 1, "both apples merged into one stack");
    assert_eq!(inv.get(0).get_name(), "Apple");
    assert_eq!(inv.get(0).count(), 2, "each bad count fell back to 1");
    assert!(!warns.is_empty(), "warns: {warns:?}");
}

/* ------------------------------- whole-world files ------------------------------- */

/// Write a save file into `<game_dir>/saves/<world>/`.
fn write_save_file(g: &Game, world: &str, name: &str, contents: &str) {
    let dir = g.game_dir.join("saves").join(world);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{name}{}", save::EXTENSION)), contents).unwrap();
}

#[test]
fn loading_missing_world_recovers() {
    let mut g = bare_game("robust_noworld");
    let (_, lines) = log::capture(Level::Warn, || {
        load::load_world_named(&mut g, "no_such_world");
    });
    assert!(g.levels.iter().all(|l| l.is_none()), "nothing was loaded");
    assert!(!lines.is_empty(), "expected diagnostics: {lines:?}");
}

#[test]
fn truncated_game_file_recovers() {
    let mut g = bare_game("robust_game_trunc");
    write_save_file(&g, "t", "Game", "3.0.0,0,3600");
    let (_, lines) = log::capture(Level::Warn, || {
        load::load_world_named(&mut g, "t");
    });
    assert_eq!(g.tick_count, 3600, "the fields present were still used");
    assert!(
        g.levels.iter().all(|l| l.is_some()),
        "all levels substituted"
    );
    // The substituted dungeon is REGENERATED from the seed (a stronger recovery
    // than a bare obsidian slab), so it must contain obsidian for the post-load
    // chest check to place into — that check is capped now, but a dungeon with no
    // obsidian at all would still be a broken level.
    let obsidian = g.tiles.get("Obsidian").id;
    assert!(
        g.level(4).tiles.contains(&obsidian),
        "the regenerated dungeon has obsidian to place chests into"
    );
    assert_eq!(g.current_level, fdoom::level::lvl_idx(0));
    assert!(
        queued(&g, g.current_level)
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Player(_))),
        "player queued onto the surface"
    );
    assert!(!lines.is_empty(), "expected warnings: {lines:?}");
}

#[test]
fn level_width_unparsable_recovers() {
    let mut g = bare_game("robust_lvl_width");
    write_save_file(&g, "w", "Game", "3.0.0,0,10,0,1,false,");
    write_save_file(&g, "w", "Level3", "banana,128,0,");
    let (_, lines) = log::capture(Level::Warn, || {
        load::load_world_named(&mut g, "w");
    });
    let l3 = g.level(3);
    assert_eq!(
        (l3.w, l3.h),
        (g.world_size, g.world_size),
        "empty level substituted"
    );
    assert!(!lines.is_empty(), "expected warnings: {lines:?}");
}

#[test]
fn level_tile_list_truncated_recovers() {
    let mut g = bare_game("robust_lvl_tiles");
    write_save_file(&g, "w", "Game", "3.0.0,0,10,0,1,false,");
    write_save_file(&g, "w", "Level3", "4,4,0,grass,grass,");
    write_save_file(&g, "w", "Level3data", &"0,".repeat(16));
    let (_, lines) = log::capture(Level::Warn, || {
        load::load_world_named(&mut g, "w");
    });
    // A level file that claims 4x4 but carries two tiles is not trusted: the layer
    // is REGENERATED from the world seed rather than loading a stub world that
    // would strand the player in a 4x4 box. The other layers survive.
    let l3 = g.level(3);
    assert_eq!((l3.w, l3.h), (128, 128), "the damaged layer was rebuilt");
    assert_eq!(l3.tiles.len(), (l3.w * l3.h) as usize);
    assert!(
        g.levels.iter().all(|l| l.is_some()),
        "every other layer still loaded"
    );
    assert!(!lines.is_empty(), "expected warnings: {lines:?}");
}

#[test]
fn level_data_file_missing_recovers() {
    let mut g = bare_game("robust_lvl_data");
    write_save_file(&g, "w", "Game", "3.0.0,0,10,0,1,false,");
    write_save_file(&g, "w", "Level3", &format!("4,4,0,{}", "grass,".repeat(16)));
    let (_, lines) = log::capture(Level::Warn, || {
        load::load_world_named(&mut g, "w");
    });
    // Missing Leveldata means the layer cannot be trusted either — same contract:
    // rebuild that one layer from the seed, keep the rest, say so loudly.
    let l3 = g.level(3);
    assert_eq!((l3.w, l3.h), (128, 128), "the damaged layer was rebuilt");
    assert_eq!(l3.data.len(), l3.tiles.len());
    assert!(!lines.is_empty(), "expected warnings: {lines:?}");
}

/* ------------------------------- preferences ------------------------------- */

#[test]
fn prefs_truncated_recovers() {
    let mut g = bare_game("robust_prefs_trunc");
    std::fs::write(
        g.game_dir.join(format!("Preferences{}", save::EXTENSION)),
        "3.0.0,true",
    )
    .unwrap();
    let (_, warns) = log::capture(Level::Warn, || load::load_prefs(&mut g));
    assert!(
        !warns.is_empty(),
        "expected a truncation warning: {warns:?}"
    );
}

#[test]
fn prefs_broken_keymap_skipped() {
    let mut g = bare_game("robust_prefs_keymap");
    std::fs::write(
        g.game_dir.join(format!("Preferences{}", save::EXTENSION)),
        "3.0.0,true,true,60,,,,english,BROKENKEY,",
    )
    .unwrap();
    let (_, warns) = log::capture(Level::Warn, || load::load_prefs(&mut g));
    assert!(
        warns.iter().any(|l| l.contains("BROKENKEY")),
        "warns: {warns:?}"
    );
}

/* ------------------------------- version strings ------------------------------- */

#[test]
fn invalid_version_string_warns() {
    let (v, lines) = log::capture(Level::Warn, || Version::new("total.garbage"));
    assert!(!v.is_valid());
    assert!(
        lines.iter().any(|l| l.contains("total.garbage")),
        "lines: {lines:?}"
    );
}
