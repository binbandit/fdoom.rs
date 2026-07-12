//! Hunting + Field Notes wave: the deer (natural spawns, the grass stalk, the
//! bolt, drops and the tanning chain) and the survivor's journal (counters, the
//! one-time country cue, save round-trip, old-save tolerance).

use fdoom::core::game::Game;
use fdoom::core::updater::{NORM_SPEED, Time};
use fdoom::entity::{Entity, EntityKind, mob};
use fdoom::item::{ItemKind, cooking, registry};
use fdoom::level::{self, infinite_gen::Biome};
use fdoom::rng::Rng;
use fdoom::saveload::{load, save};
use fdoom::screen::survival_display;
use fdoom::testutil::{TestWorld, bare_game, find_recipe};

fn is_deer(e: &Entity) -> bool {
    matches!(e.kind, EntityKind::Deer(_))
}

fn clear_non_player(tw: &mut TestWorld, lvl: usize) {
    tw.level_mut(lvl).entities_to_add.clear();
    tw.level_mut(lvl).entities_to_remove.clear();
    let player_id = tw.player_id;
    for eid in tw.entities.ids_on_level(lvl) {
        if eid != player_id {
            tw.entities.delete(eid);
        }
    }
    tw.level_mut(lvl).mob_count = 0;
}

fn has_deer(tw: &TestWorld, lvl: usize) -> bool {
    tw.entities
        .entities_on_level(lvl)
        .any(|e| !e.c.removed && is_deer(e))
        || tw.level(lvl).entities_to_add.iter().any(is_deer)
}

fn deer_eid(g: &Game, lvl: usize) -> i32 {
    g.entities
        .entities_on_level(lvl)
        .find(|e| !e.c.removed && is_deer(e))
        .map(|e| e.c.eid)
        .expect("deer on level")
}

fn flee_time(g: &Game, eid: i32) -> i32 {
    match &g.entities.get(eid).expect("deer").kind {
        EntityKind::Deer(d) => d.flee_time,
        _ => panic!("not a deer"),
    }
}

/* ------------------------------- natural spawns ------------------------------- */

fn stage_daylight(tw: &mut TestWorld, biome: Biome, time: Time) -> usize {
    let lvl = tw.current_level;
    tw.goto_biome(biome);
    tw.g.past_day1 = true;
    tw.g.change_time_of_day(time);
    clear_non_player(tw, lvl);
    lvl
}

fn assert_deer_spawns(biome: Biome) {
    let mut tw = TestWorld::infinite().seed(0x5EED).build();
    let lvl = stage_daylight(&mut tw, biome, Time::Day);
    tw.g.random = Rng::new(0xDEE4);
    tw.level_mut(lvl).random = Rng::new(0x0DDD);

    for _ in 0..512 {
        level::try_spawn(&mut tw.g, lvl);
        if has_deer(&tw, lvl) {
            level::tick_level(&mut tw.g, lvl, false);
            assert!(
                has_deer(&tw, lvl),
                "deer was queued but did not become live in {biome:?}"
            );
            return;
        }
        clear_non_player(&mut tw, lvl);
    }
    panic!("no deer spawned naturally in {biome:?} by day");
}

#[test]
fn deer_spawns_naturally_in_forest_and_plains_by_day() {
    assert_deer_spawns(Biome::Forest);
    assert_deer_spawns(Biome::Plains);
}

#[test]
fn deer_does_not_spawn_at_night() {
    let mut tw = TestWorld::infinite().seed(0x5EED).build();
    let lvl = stage_daylight(&mut tw, Biome::Forest, Time::Night);
    tw.g.random = Rng::new(0xDEE4);
    tw.level_mut(lvl).random = Rng::new(0x0DDD);

    for _ in 0..512 {
        level::try_spawn(&mut tw.g, lvl);
        assert!(!has_deer(&tw, lvl), "deer spawned at night");
        clear_non_player(&mut tw, lvl);
    }
}

/* ------------------------------- the stalk & bolt ------------------------------- */

#[test]
fn deer_bolts_from_an_open_approach() {
    let mut tw = TestWorld::infinite().seed(0x77).build();
    let lvl = tw.current_level;
    tw.g.change_time_of_day(Time::Day); // lit, for the flee screenshot
    let (px, py) = tw.player_tile();
    // a flat open strip between hunter and deer
    for dy in -3..=3 {
        for dx in -3..=8 {
            tw.place("grass", dx, dy);
        }
    }

    let deer = mob::deer::new(&tw.g);
    assert!(deer.enemy_mob().is_none(), "the deer must never be hostile");
    tw.g.level_mut(lvl).add_at(deer, px + 5, py, true, lvl);
    tw.tick_n(1); // drain into the arena; the deer ticks once, player 5 tiles off
    let did = deer_eid(&tw.g, lvl);

    // 5 tiles in the open is inside the 6-tile flee radius: it bolts
    assert!(flee_time(&tw.g, did) > 0, "open approach should spook");
    let deer = tw.g.entities.get(did).unwrap();
    let ai = deer.mob_ai().unwrap();
    assert_eq!(ai.mob.walk_time, 1, "bolting deer skips no movement ticks");
    assert_eq!(ai.mob.speed, 2, "bolting deer runs double steps");
    assert_eq!(ai.xa, 1, "deer flees away from the player (east)");

    tw.screenshot("hunting_deer_flee.png");
}

#[test]
fn deer_ignores_a_hunter_hidden_in_tall_grass_until_two_tiles() {
    let mut tw = TestWorld::infinite().seed(0x78).build();
    let lvl = tw.current_level;
    let (px, py) = tw.player_tile();
    for dx in -1..=7 {
        tw.place("grass", dx, 0);
    }
    // the stalk: the hunter stands in tall grass
    tw.place("tall grass", 0, 0);
    tw.place("tall grass", 4, 0);

    let deer = mob::deer::new(&tw.g);
    tw.g.level_mut(lvl).add_at(deer, px + 5, py, true, lvl);
    tw.tick_n(1);
    let did = deer_eid(&tw.g, lvl);

    // same 5-tile distance, but the player is concealed: no bolt
    assert_eq!(
        flee_time(&tw.g, did),
        0,
        "grass stalk must not spook at 5 tiles"
    );
    for _ in 0..5 {
        tw.g.with_entity(did, |d, g| mob::deer::tick(g, d));
    }
    assert_eq!(flee_time(&tw.g, did), 0, "grass stalk must hold over time");

    // creeping to ~1 tile is inside even the concealed radius: it bolts
    tw.teleport(px + 4, py);
    tw.g.with_entity(did, |d, g| mob::deer::tick(g, d));
    assert!(
        flee_time(&tw.g, did) > 0,
        "point-blank presence spooks even from grass"
    );
}

/* ------------------------------- drops & the chain ------------------------------- */

#[test]
fn venison_hide_and_the_tanning_chain() {
    let mut tw = TestWorld::infinite().seed(0x79).build();
    let lvl = tw.current_level;
    let (px, py) = tw.player_tile();

    let deer = mob::deer::new(&tw.g);
    tw.g.level_mut(lvl).add_at(deer, px + 3, py, true, lvl);
    tw.tick_n(1);
    let did = deer_eid(&tw.g, lvl);
    tw.g.with_entity(did, |d, g| mob::deer::die(g, d));

    let drops = tw.dropped_items();
    let venison = drops.iter().filter(|n| n.as_str() == "Venison").count();
    let hide = drops.iter().filter(|n| n.as_str() == "Hide").count();
    assert!(
        (1..=3).contains(&venison),
        "venison drop out of range: {drops:?}"
    );
    assert_eq!(hide, 1, "exactly one hide per deer: {drops:?}");

    // the fire: Venison -> Cooked Venison (raw carries the queasy risk, cooked = 4)
    assert_eq!(cooking::cooked_result("Venison"), Some("Cooked Venison"));
    assert!(cooking::queasy_risk("Venison"));
    assert!(!cooking::queasy_risk("Cooked Venison"));
    let raw = registry::get(&tw.g, "Venison");
    let cooked = registry::get(&tw.g, "Cooked Venison");
    let heal = |i: &fdoom::item::Item| match i.kind {
        ItemKind::Food { heal, .. } => heal,
        _ => panic!("not food"),
    };
    assert_eq!(heal(&raw), 2);
    assert_eq!(heal(&cooked), 4);

    // the tanning chain: Hide*2 + Cord -> Leather*2, personal crafting; the chain
    // still ends at the classic Leather Armor (leather*10 at the workbench)
    let tan = find_recipe(&tw.g.recipes.craft, "Leather");
    assert_eq!(tan.get_amount(), 2);
    let mut costs = tan.get_costs().to_vec();
    costs.sort();
    assert_eq!(
        costs,
        vec![("CORD".to_string(), 1), ("HIDE".to_string(), 2)]
    );
    let armor = find_recipe(&tw.g.recipes.workbench, "Leather Armor");
    assert_eq!(armor.get_costs(), &[("LEATHER".to_string(), 10)]);
}

/* --------------------------------- field notes --------------------------------- */

#[test]
fn notes_counters_tick_and_the_country_cue_fires_once() {
    let mut tw = TestWorld::infinite().seed(0x80).build();
    tw.goto_biome(Biome::Forest);
    tw.tick_n(NORM_SPEED as usize + 2); // one journal sweep

    let bit = 1u16 << (Biome::Forest as u16);
    assert!(
        tw.g.player().player().notes.biomes_seen & bit != 0,
        "standing in the forest should write it into the notes"
    );
    let cue = "New country: the forest.";
    let count = |g: &Game| g.notifications.iter().filter(|n| n.as_str() == cue).count();
    assert_eq!(count(&tw.g), 1, "the country cue should fire");

    tw.tick_n(NORM_SPEED as usize * 3); // more sweeps in the same country
    assert_eq!(count(&tw.g), 1, "the country cue must fire exactly once");

    // fell a tree; the next sweep moves the tally onto the journal
    tw.place("tree", 1, 0);
    tw.hit(1, 0, 20);
    tw.tick_n(NORM_SPEED as usize + 2);
    assert_eq!(tw.g.player().player().notes.trees_felled, 1);

    // never dug: the record still reads surface
    assert_eq!(tw.g.player().player().notes.deepest_depth, 0);
}

#[test]
fn notes_pane_lists_the_journal() {
    let mut tw = TestWorld::infinite().seed(0x81).build();
    {
        let notes = &mut tw.g.player_mut().player_mut().notes;
        notes.days_survived = 4;
        notes.deepest_depth = -2;
        notes.see_biome(Biome::Plains);
        notes.see_biome(Biome::Forest);
        notes.trees_felled = 12;
        notes.fish_caught = 3;
        notes.ore_panned = 7;
    }
    let lines = survival_display::notes_lines(tw.g.player().player());
    let get = |label: &str| {
        lines
            .iter()
            .find(|(l, _)| l == label)
            .unwrap_or_else(|| panic!("missing notes row {label:?}"))
            .1
            .clone()
    };
    assert_eq!(get("DAYS SURVIVED"), "4");
    assert_eq!(get("COUNTRY SEEN"), "2/11");
    assert_eq!(get("PLACES FOUND"), "0/6");
    assert_eq!(get("EVENTS WITNESSED"), "0/5");
    assert_eq!(get("TREES FELLED"), "12");
    assert_eq!(get("FISH CAUGHT"), "3");
    assert_eq!(get("ORE PANNED"), "7");
    assert!(
        get("DEEPEST DIG").contains("B2"),
        "{:?}",
        get("DEEPEST DIG")
    );
    let country = survival_display::seen_country(tw.g.player().player());
    assert_eq!(country, "THE FOREST. THE PLAINS.");

    // the fifth tab renders: E then four RIGHTs lands on NOTES
    tw.press("E");
    for _ in 0..4 {
        tw.press("RIGHT");
    }
    tw.screenshot("notes_pane.png");
}

/* ------------------------------- save round-trip ------------------------------- */

/// A second `Game` over the same save dir (to load back what the first one saved).
fn reopen(g: &Game) -> Game {
    let mut g2 = Game::new(false, false, g.game_dir.clone());
    let mut player = fdoom::entity::mob::player::new(&g2, None);
    player.c.eid = 0;
    g2.entities.put_back(player);
    g2
}

#[test]
fn notes_and_deer_roundtrip_and_old_saves_load_clean() {
    let mut g1 = bare_game("hunting_notes_roundtrip");
    let dir = g1.game_dir.clone();

    let diff = g1.settings.get_idx("diff");
    for (i, &depth) in fdoom::level::IDX_TO_DEPTH.iter().enumerate() {
        let mut level = fdoom::level::Level::empty(128, 128, depth, diff);
        if depth == -4 {
            let obsidian = g1.tiles.get("Obsidian").id;
            level.tiles.iter_mut().for_each(|t| *t = obsidian);
        }
        g1.levels[i] = Some(level);
    }
    g1.settings.set_idx("mode", 0); // survival
    for _ in 0..10 {
        let dc = fdoom::entity::furniture::dungeon_chest::new(&mut g1);
        g1.level_mut(4).add_at(dc, 80, 80, false, 4);
    }

    // a deer on the surface, plus a written-in journal
    let deer = mob::deer::new(&g1);
    g1.level_mut(3).add_at(deer, 100, 120, false, 3);
    g1.current_level = 3;
    {
        let p = g1.player_mut();
        p.c.level = Some(3);
        p.c.removed = false;
        let notes = &mut p.player_mut().notes;
        notes.days_survived = 9;
        notes.deepest_depth = -3;
        notes.see_biome(Biome::Marsh);
        notes.find_place(fdoom::level::structures_gen::StructureKind::Cemetery);
        notes.witness_event(fdoom::core::events::WorldEvent::Aurora);
        notes.trees_felled = 21;
        notes.fish_caught = 5;
        notes.ore_panned = 8;
    }
    let expected = g1.player().player().notes.clone();

    g1.world_name = "huntworld".to_string();
    save::save_world_named(&mut g1, "huntworld");

    let player_path = dir
        .join("saves/huntworld")
        .join(format!("Player{}", save::EXTENSION));
    let player_file = std::fs::read_to_string(&player_path).unwrap();
    assert!(
        player_file.contains(save::NOTES_MARKER),
        "player save should carry the notes marker: {player_file}"
    );

    // round-trip: journal and deer both survive
    let mut g2 = reopen(&g1);
    load::load_world_named(&mut g2, "huntworld");
    assert_eq!(g2.player().player().notes, expected);
    assert!(
        g2.entities.entities_on_level(3).any(is_deer)
            || g2.level(3).entities_to_add.iter().any(is_deer),
        "the deer should survive a save round-trip"
    );

    // old save: strip the notes entry (and prove unknown entities still skip) —
    // the journal opens blank, nothing panics
    let stripped: String = player_file
        .split(',')
        .filter(|f| !f.starts_with(save::NOTES_MARKER))
        .collect::<Vec<_>>()
        .join(",");
    std::fs::write(&player_path, stripped).unwrap();
    let entities_path = dir
        .join("saves/huntworld")
        .join(format!("Entities{}", save::EXTENSION));
    let mut entities_file = std::fs::read_to_string(&entities_path).unwrap();
    entities_file.push_str("Moose[100:100:10:3],");
    std::fs::write(&entities_path, entities_file).unwrap();

    let mut g3 = reopen(&g1);
    load::load_world_named(&mut g3, "huntworld");
    assert_eq!(
        g3.player().player().notes,
        Default::default(),
        "an old save without the marker opens a blank journal"
    );
}

/* --------------------------------- screenshots --------------------------------- */

#[test]
fn deer_grazing_screenshot() {
    let mut tw = TestWorld::infinite().seed(0x82).build();
    let lvl = tw.current_level;
    tw.goto_biome(Biome::Plains);
    tw.g.change_time_of_day(Time::Day); // full light for the art check
    let (px, py) = tw.player_tile();
    // a small clearing with a grass fringe, deer grazing off to the side
    for dy in -4..=4 {
        for dx in -4..=9 {
            tw.place("grass", dx, dy);
        }
    }
    for dx in [-3, -2, 2, 3] {
        tw.place("tall grass", dx, 2);
    }
    let deer = mob::deer::new(&tw.g);
    tw.g.level_mut(lvl).add_at(deer, px + 7, py - 1, true, lvl);
    tw.tick_n(1); // far enough that it keeps grazing (7 tiles > the 6-tile radius)
    let did = deer_eid(&tw.g, lvl);
    assert_eq!(flee_time(&tw.g, did), 0, "deer should be calm at 7 tiles");
    tw.screenshot("hunting_deer_grazing.png");
}
