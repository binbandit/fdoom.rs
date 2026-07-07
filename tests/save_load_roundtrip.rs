//! Save/load round-trip tests: fabricate game state, save it with the Java-compatible
//! writers, load it back into a fresh headless `Game`, and assert the state survives.

use std::path::{Path, PathBuf};

use fdoom::core::game::Game;
use fdoom::entity::EntityKind;
use fdoom::item::PotionType;
use fdoom::saveload::{load, save};

fn temp_game_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A headless game with the main player created (as `Game.main` does before loading).
fn new_game(dir: &Path) -> Game {
    let mut g = Game::new(false, false, dir.to_path_buf());
    let mut player = fdoom::entity::mob::player::new(&g, None);
    player.c.eid = 0; // Java main() gives the main player eid 0
    g.entities.put_back(player);
    g
}

#[test]
fn prefs_roundtrip() {
    let dir = temp_game_dir("prefs_roundtrip");

    let mut g1 = new_game(&dir);
    g1.settings.set("sound", false);
    g1.settings.set("autosave", true);
    g1.settings.set("fps", 90);
    g1.settings.set("unlockedskin", true);
    g1.input.set_key("UP", "K", g1.debug);

    save::save_prefs(&mut g1);

    // Byte-level format checks (must match the Java writer).
    let prefs =
        std::fs::read_to_string(dir.join(format!("Preferences{}", save::EXTENSION))).unwrap();
    assert!(
        prefs.starts_with("3.0.0,false,true,90,,,,english,"),
        "unexpected Preferences prefix: {prefs}"
    );
    assert!(
        prefs.contains("UP;K:DOWN;DOWN|S:"),
        "keymap not saved: {prefs}"
    );
    assert!(
        prefs.ends_with(','),
        "world-save files end with a comma: {prefs}"
    );
    let unlocks = std::fs::read_to_string(dir.join(format!("Unlocks{}", save::EXTENSION))).unwrap();
    assert_eq!(unlocks, "AirSkin,");

    let mut g2 = new_game(&dir);
    load::load_prefs(&mut g2);

    assert!(!g2.settings.get("sound").as_bool());
    assert!(g2.settings.get("autosave").as_bool());
    assert_eq!(g2.settings.get("fps").as_int(), 90);
    assert_eq!(g2.input.get_mapping("UP"), "K");
    assert!(g2.settings.get("unlockedskin").as_bool());
}

#[test]
fn world_roundtrip() {
    let dir = temp_game_dir("world_roundtrip");
    let mut g1 = new_game(&dir);

    /* ---------------------------- fabricate a world ---------------------------- */

    let diff = g1.settings.get_idx("diff");
    for (i, &depth) in fdoom::level::IDX_TO_DEPTH.iter().enumerate() {
        let mut level = fdoom::level::Level::empty(128, 128, depth, diff);
        if depth == -4 {
            // give the dungeon an obsidian floor (checkChestCount hunts for obsidian)
            let obsidian = g1.tiles.get("Obsidian").id;
            level.tiles.iter_mut().for_each(|t| *t = obsidian);
        }
        g1.levels[i] = Some(level);
    }
    // some tile variety on the surface
    let rock = g1.tiles.get("rock");
    g1.set_tile(3, 5, 7, &rock, 13);

    g1.settings.set_idx("mode", 0); // survival
    g1.air_wizard_beaten = true; // also keeps checkAirWizard from spawning one on load
    g1.set_time(3600);
    g1.game_time = 70000;

    // entities
    let mut zombie = fdoom::entity::mob::zombie::new(&g1, 2);
    zombie.mob_mut().unwrap().health = 7;
    g1.level_mut(3).add_at(zombie, 100, 120, false, 3);

    let mut chest = fdoom::entity::furniture::chest::new();
    chest
        .chest_mut()
        .unwrap()
        .inventory
        .add(fdoom::item::registry::get(&g1, "apple_5"));
    chest
        .chest_mut()
        .unwrap()
        .inventory
        .add(fdoom::item::registry::get(&g1, "Iron Ore_12"));
    g1.level_mut(3).add_at(chest, 200, 220, false, 3);

    let lantern = fdoom::entity::furniture::lantern::new(
        fdoom::entity::furniture::lantern::LanternType::Iron,
    );
    g1.level_mut(2).add_at(lantern, 40, 40, false, 2);

    let skeleton = fdoom::entity::mob::skeleton::new(&g1, 2);
    let mut rnd = g1.random.clone();
    let spawner = fdoom::entity::furniture::spawner::new(skeleton, &mut rnd);
    g1.random = rnd;
    g1.level_mut(1).add_at(spawner, 64, 64, false, 1);

    // ten locked dungeon chests, so the load-side checkChestCount is satisfied
    for _ in 0..10 {
        let dc = fdoom::entity::furniture::dungeon_chest::new(&mut g1);
        g1.level_mut(4).add_at(dc, 80, 80, false, 4);
    }

    // player state
    g1.current_level = 3;
    {
        let p = g1.player_mut();
        p.c.x = 264;
        p.c.y = 152;
        p.c.level = Some(3);
        p.c.removed = false;
        let pd = p.player_mut();
        pd.spawnx = 16;
        pd.spawny = 9;
        pd.mob.health = 7;
        pd.hunger = 5;
        pd.armor = 50;
        pd.armor_damage_buffer = 3;
        pd.set_score(1234);
        pd.shirt_color = 520;
        pd.skinon = true;
        pd.potioneffects.insert(PotionType::Regen, 100);
    }
    let iron_armor = fdoom::item::registry::get(&g1, "Iron Armor");
    let pickaxe = fdoom::item::registry::get(&g1, "Wood Pickaxe");
    let wood = fdoom::item::registry::get(&g1, "Wood_10");
    {
        let pd = g1.player_mut().player_mut();
        pd.cur_armor = Some(iron_armor);
        pd.active_item = Some(pickaxe);
        pd.inventory.add(wood);
    }

    /* ----------------------------------- save ----------------------------------- */

    g1.world_name = "testworld".to_string();
    save::save_world_named(&mut g1, "testworld");

    let world_dir = dir.join("saves/testworld");
    for f in [
        "Game",
        "Level0",
        "Level4",
        "Level0data",
        "Player",
        "Inventory",
        "Entities",
    ] {
        assert!(
            world_dir.join(format!("{f}{}", save::EXTENSION)).exists(),
            "missing save file {f}"
        );
    }

    // Java writer format checks.
    let game_file =
        std::fs::read_to_string(world_dir.join(format!("Game{}", save::EXTENSION))).unwrap();
    assert_eq!(game_file, "3.0.0,0,3600,70000,1,true,");
    let player_file =
        std::fs::read_to_string(world_dir.join(format!("Player{}", save::EXTENSION))).unwrap();
    assert_eq!(
        player_file,
        "264,152,16,9,7,5,50,3,Iron Armor,1234,3,PotionEffects[Regen;100],520,true,"
    );

    /* ----------------------------------- load ----------------------------------- */

    let mut g2 = new_game(&dir);
    load::load_world_named(&mut g2, "testworld");

    // game data
    assert_eq!(g2.tick_count, 3600);
    assert_eq!(g2.game_time, 70000);
    assert!(g2.past_day1);
    assert!(g2.air_wizard_beaten);
    assert_eq!(g2.settings.get_idx("diff"), g1.settings.get_idx("diff"));
    assert_eq!(g2.settings.get_idx("mode"), 0);

    // levels
    for i in 0..5 {
        let (l1, l2) = (g1.level(i), g2.level(i));
        assert_eq!(l1.depth, l2.depth, "depth differs on level {i}");
        assert_eq!((l1.w, l1.h), (l2.w, l2.h), "size differs on level {i}");
        assert_eq!(l1.tiles, l2.tiles, "tiles differ on level {i}");
        assert_eq!(l1.data, l2.data, "tile data differs on level {i}");
    }
    assert_eq!(g2.level(4).chest_count, 10); // locked DungeonChests counted on load

    // drain the entity queues into the arena (Java drained them on the next Level.tick)
    for i in 0..5 {
        fdoom::level::tick_level(&mut g2, i, false);
    }

    // player
    assert_eq!(g2.current_level, 3);
    let p = g2.player();
    assert_eq!((p.c.x, p.c.y), (264, 152));
    assert_eq!(p.c.level, Some(3));
    let pd = p.player();
    assert_eq!((pd.spawnx, pd.spawny), (16, 9));
    assert_eq!(pd.mob.health, 7);
    assert_eq!(pd.hunger, 5);
    assert_eq!(pd.armor, 50);
    assert_eq!(pd.armor_damage_buffer, 3);
    assert_eq!(
        pd.cur_armor.as_ref().map(|a| a.get_name()),
        Some("Iron Armor")
    );
    assert_eq!(pd.get_score(), 1234);
    assert_eq!(pd.shirt_color, 520);
    assert!(pd.skinon);
    assert_eq!(pd.potioneffects.get(&PotionType::Regen), Some(&100));

    // inventory: Java loads the saved active item as the first inventory slot
    assert!(pd.active_item.is_none());
    assert_eq!(pd.inventory.get(0).get_name(), "Wood Pickaxe");
    assert_eq!(pd.inventory.get(1).get_name(), "Wood");
    assert_eq!(pd.inventory.get(1).count(), 10);

    // entities
    let zombie = g2
        .entities
        .entities_on_level(3)
        .find(|e| matches!(e.kind, EntityKind::Zombie(_)))
        .expect("zombie not loaded");
    assert_eq!(zombie.mob().unwrap().health, 7);
    assert_eq!(zombie.enemy_mob().unwrap().lvl, 2);
    assert_eq!((zombie.c.x, zombie.c.y), (100, 120));

    let chest = g2
        .entities
        .entities_on_level(3)
        .find(|e| matches!(e.kind, EntityKind::Chest(_)))
        .expect("chest not loaded");
    let inv = &chest.chest().unwrap().inventory;
    assert_eq!(inv.inv_size(), 2);
    assert_eq!((inv.get(0).get_name(), inv.get(0).count()), ("Apple", 5));
    assert_eq!(
        (inv.get(1).get_name(), inv.get(1).count()),
        ("Iron Ore", 12)
    );

    let lantern = g2
        .entities
        .entities_on_level(2)
        .find(|e| matches!(e.kind, EntityKind::Lantern(_)))
        .expect("lantern not loaded");
    match &lantern.kind {
        EntityKind::Lantern(l) => assert_eq!(
            l.lantern_type,
            fdoom::entity::furniture::lantern::LanternType::Iron
        ),
        _ => unreachable!(),
    }

    let spawner = g2
        .entities
        .entities_on_level(1)
        .find(|e| matches!(e.kind, EntityKind::Spawner(_)))
        .expect("spawner not loaded");
    match &spawner.kind {
        EntityKind::Spawner(sp) => {
            assert!(matches!(sp.mob.kind, EntityKind::Skeleton(_)));
            assert_eq!(sp.mob.enemy_mob().unwrap().lvl, 2);
        }
        _ => unreachable!(),
    }

    let dungeon_chests = g2
        .entities
        .entities_on_level(4)
        .filter(|e| matches!(e.kind, EntityKind::DungeonChest(_)))
        .count();
    assert_eq!(dungeon_chests, 10);
}
