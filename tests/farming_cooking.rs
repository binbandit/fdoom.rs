//! Farming & cooking wave: the foraged-seed crop loop (wild carrots, panned tubers,
//! village fields), the wheat-clock growth machinery with its rain lean, campfire
//! field cooking, the raw-flesh Queasy gamble, composed oven dishes, and the
//! save/load survival of a mid-growth crop.

use fdoom::core::game::Game;
use fdoom::core::weather;
use fdoom::entity::{Direction, EntityKind};
use fdoom::gfx::screen;
use fdoom::item::{Inventory, PotionType, cooking, interact, registry};
use fdoom::level::infinite_gen::Biome;
use fdoom::level::structures_gen::{
    StructureKind, TownAge, placements_in_rect, structure_writes, town_age,
};
use fdoom::level::tile::fossick::{PanFind, pan_outcome};
use fdoom::level::tile::{Tiles, dispatch};
use fdoom::saveload::{load, save};
use fdoom::testutil::{TestWorld, bare_game, find_recipe, save_png, verify_path};

/* --------------------------------- helpers --------------------------------- */

/// Eat/plant/use `item` on the tile at player + `(dx, dy)` through the real
/// item-use dispatch (`item_interact_on_tile`), which TestWorld's tool-oriented
/// `interact_with` does not cover.
fn use_item(tw: &mut TestWorld, item: &mut fdoom::item::Item, dx: i32, dy: i32) -> bool {
    let (px, py) = tw.player_tile();
    let lvl = tw.g.current_level;
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    let used = interact::item_interact_on_tile(
        &mut tw.g,
        item,
        lvl,
        px + dx,
        py + dy,
        &mut player,
        Direction::Down,
    );
    tw.g.entities.put_back(player);
    used
}

/// Advance one crop tile `n` random-tick pulses (the growth clock, isolated from
/// the level's random-tile lottery so tests stay fast and deterministic).
fn pulse_tile(tw: &mut TestWorld, tx: i32, ty: i32, n: usize) {
    let lvl = tw.g.current_level;
    for _ in 0..n {
        let def = tw.g.tile_at(lvl, tx, ty);
        dispatch::tick(&mut tw.g, &def, lvl, tx, ty);
    }
}

fn tile_name(tw: &TestWorld, tx: i32, ty: i32) -> String {
    tw.g.tile_at(tw.g.current_level, tx, ty).name.clone()
}

fn crop_age(tw: &TestWorld, tx: i32, ty: i32) -> i32 {
    tw.g.level(tw.g.current_level).get_data(tx, ty)
}

/// Pin the day clock (tests/weather.rs idiom): jump to one tick before, run one
/// real tick to sync the scheduler, then set the day.
fn pin_clock(tw: &mut TestWorld, day: i32, tick: i32) {
    tw.set_time(tick - 1);
    tw.tick_n(1);
    assert_eq!(tw.tick_count, tick, "clock failed to pin");
    tw.events.day_number = day;
    tw.notifications.clear();
}

/// Mid-slice tick — the rain-intensity plateau.
fn mid(slice: i32) -> i32 {
    slice * weather::SLICE_LEN + weather::SLICE_LEN / 2
}

/// First `(day, slice)` from day 1 with the given rain state.
fn find_slice(seed: i64, raining: bool) -> (i32, i32) {
    (1..120)
        .flat_map(|d| (0..weather::SLICES_PER_DAY).map(move |s| (d, s)))
        .find(|&(d, s)| weather::slice_raining(seed, d, s) == raining)
        .expect("weather schedule offers both states")
}

/* ------------------------------ the seed loop ------------------------------ */

#[test]
fn wild_carrot_forage_starts_the_seed_loop() {
    let mut tw = TestWorld::infinite().seed(42).build();
    tw.place("Wild Carrot", 1, 0);
    assert!(tw.hit(1, 0, 1), "pulling a wild carrot must react");

    let drops = tw.dropped_items();
    assert!(
        drops.iter().any(|n| n == "Carrot"),
        "wild carrot must drop the root: {drops:?}"
    );
    assert!(
        drops.iter().any(|n| n == "Carrot Seeds"),
        "wild carrot must drop seed stock: {drops:?}"
    );
    let (px, py) = tw.player_tile();
    assert_eq!(tile_name(&tw, px + 1, py), "GRASS", "plant tears out");
}

#[test]
fn till_plant_grow_harvest_full_cycle() {
    let mut tw = TestWorld::infinite().seed(42).build();
    let (px, py) = tw.player_tile();
    let (tx, ty) = (px + 1, py);

    // clear ground around the plot so no creek biases the growth clock
    for dy in -1..=1 {
        for dx in 0..=2 {
            tw.place("dirt", dx, dy);
        }
    }

    // till with the hoe machinery
    assert!(tw.interact_with("Crude Hoe", 1, 0), "hoe must till dirt");
    assert_eq!(tile_name(&tw, tx, ty), "FARMLAND");

    // plant the foraged seeds (the only granted item in this loop)
    let mut seeds = registry::get(&tw.g, "Carrot Seeds");
    assert!(use_item(&mut tw, &mut seeds, 1, 0), "seeds must plant");
    assert!(seeds.is_depleted(), "planting consumes the seed packet");
    assert_eq!(tile_name(&tw, tx, ty), "CARROT CROP");
    assert_eq!(crop_age(&tw, tx, ty), 0, "fresh crop starts as a sprout");

    // grow to ripeness on the wheat clock (50% advance odds per pulse)
    for _ in 0..40 {
        pulse_tile(&mut tw, tx, ty, 10);
        if crop_age(&tw, tx, ty) >= 50 {
            break;
        }
    }
    assert!(crop_age(&tw, tx, ty) >= 50, "crop never ripened");

    // harvest: produce plus the seed stock to replant
    assert!(tw.hit(1, 0, 1));
    let drops = tw.dropped_items();
    let carrots = drops.iter().filter(|n| *n == "Carrot").count();
    assert!(carrots >= 2, "ripe harvest pays 2+ carrots: {drops:?}");
    assert!(
        drops.iter().any(|n| n == "Carrot Seeds"),
        "harvest returns seeds: {drops:?}"
    );
    assert_eq!(tile_name(&tw, tx, ty), "DIRT", "harvest pulls the plot");
}

#[test]
fn rain_waters_the_fields() {
    let mut tw = TestWorld::infinite().seed(42).build();
    let (bx, by) = tw.goto_biome(Biome::Plains);
    // a dirt apron so neither test crop sits by water
    for dy in -2..=2 {
        for dx in -2..=2 {
            tw.place_at("dirt", bx + dx, by + dy);
        }
    }
    let seed = tw.g.world_seed;
    let lvl = tw.g.current_level;

    // rain first
    let (rd, rs) = find_slice(seed, true);
    pin_clock(&mut tw, rd, mid(rs));
    assert!(weather::growth_boost(&tw.g), "pinned slice must rain");
    let crop = tw.g.tiles.get("Carrot Crop");
    tw.g.set_tile(lvl, bx + 1, by, &crop, 0);
    pulse_tile(&mut tw, bx + 1, by, 40);
    let rain_age = crop_age(&tw, bx + 1, by);

    // then the same pulses under a calm sky
    let (dd, ds) = find_slice(seed, false);
    pin_clock(&mut tw, dd, mid(ds));
    assert!(!weather::growth_boost(&tw.g), "pinned slice must be calm");
    tw.g.set_tile(lvl, bx - 1, by, &crop, 0);
    pulse_tile(&mut tw, bx - 1, by, 40);
    let dry_age = crop_age(&tw, bx - 1, by);

    assert!(
        rain_age > dry_age,
        "rain must outgrow calm: {rain_age} vs {dry_age}"
    );
    assert!(dry_age > 0, "crops still grow without rain");
}

/* ------------------------------- food design ------------------------------- */

#[test]
fn food_values_ladder_and_cooking_table() {
    let g = bare_game("farm_food_values");
    let heal = |name: &str| match registry::get(&g, name).kind {
        fdoom::item::ItemKind::Food { heal, .. } => heal,
        ref k => panic!("{name} is not food: {k:?}"),
    };

    // foraged raw < cooked single < stick food < composed dish
    assert_eq!(heal("Carrot"), 1);
    assert_eq!(heal("Potato"), 1);
    assert_eq!(heal("Corn"), 1);
    assert_eq!(heal("Baked Potato"), 3);
    assert_eq!(heal("Roast Corn"), 3);
    assert_eq!(heal("Roast Pumpkin"), 4);
    assert_eq!(heal("Mushroom Skewer"), 2);
    assert_eq!(heal("Roasted Skewer"), 6);
    assert_eq!(heal("Fish Chowder"), 7);
    assert_eq!(heal("Hearty Stew"), 8);

    // every table entry resolves both ways in the registry
    for raw in [
        "Raw Pork",
        "Raw Beef",
        "Raw Fish",
        "Big Fish",
        "Cave Eel",
        "Mushroom",
        "Potato",
        "Corn",
        "Pumpkin",
        "Mushroom Skewer",
    ] {
        let cooked = cooking::cooked_result(raw).expect(raw);
        assert!(heal(cooked) > heal(raw), "{raw} must cook into better food");
    }
    // cooked food never carries the raw gamble; raw flesh and raw potato do
    assert!(cooking::queasy_risk("Raw Pork"));
    assert!(cooking::queasy_risk("Potato"));
    assert!(!cooking::queasy_risk("Cooked Pork"));
    assert!(!cooking::queasy_risk("Baked Potato"));
    assert!(cooking::is_hearty("Hearty Stew"));
    assert!(cooking::is_hearty("Fish Chowder"));
    assert!(!cooking::is_hearty("Bread"));

    // the roasts sit at both heat stations; the pot dishes are oven-only
    for product in [
        "Baked Potato",
        "Roast Corn",
        "Roast Pumpkin",
        "Roasted Skewer",
    ] {
        find_recipe(&g.recipes.oven, product);
        find_recipe(&g.recipes.furnace, product);
    }
    find_recipe(&g.recipes.oven, "Hearty Stew");
    find_recipe(&g.recipes.oven, "Fish Chowder");
    find_recipe(&g.recipes.craft, "Mushroom Skewer");
}

/* ------------------------------ field cooking ------------------------------ */

#[test]
fn campfire_roasts_and_spends_fuel() {
    let mut tw = TestWorld::infinite().seed(7).build();
    let mut fire = fdoom::entity::furniture::campfire::new();
    let fuel_before = match &fire.kind {
        EntityKind::Campfire(cf) => cf.fuel,
        _ => unreachable!(),
    };

    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    let mut held = Some(registry::get(&tw.g, "Raw Fish"));
    let cooked = fdoom::entity::furniture::campfire_behavior::interact(
        &mut tw.g,
        &mut fire,
        &mut player,
        &mut held,
        Direction::Down,
    );
    assert!(cooked, "a lit fire must roast raw fish");
    assert!(held.is_none(), "the raw fish is consumed");
    assert!(
        player
            .player()
            .inventory
            .items()
            .iter()
            .any(|i| i.get_name().eq_ignore_ascii_case("Cooked Fish")),
        "the cooked fish lands in the inventory"
    );
    let fuel_after = match &fire.kind {
        EntityKind::Campfire(cf) => cf.fuel,
        _ => unreachable!(),
    };
    assert!(fuel_after < fuel_before, "roasting costs fuel");
    assert!(fuel_after > 0, "roasting never kills the fire outright");

    // cold embers cook nothing
    let mut ember = fdoom::entity::furniture::campfire::new_ember();
    let mut held = Some(registry::get(&tw.g, "Raw Fish"));
    let cooked = fdoom::entity::furniture::campfire_behavior::interact(
        &mut tw.g,
        &mut ember,
        &mut player,
        &mut held,
        Direction::Down,
    );
    assert!(!cooked, "cold embers must not roast");
    assert!(held.is_some(), "nothing consumed at a dead fire");
    tw.g.entities.put_back(player);
}

#[test]
fn raw_flesh_risks_queasy_and_slows_recovery() {
    let mut tw = TestWorld::infinite().seed(11).build();

    let mut turned = false;
    for _ in 0..100 {
        {
            let p = tw.g.player_mut();
            let pd = p.player_mut();
            pd.hunger = 1;
            pd.stamina = fdoom::entity::mob::player::MAX_STAMINA;
        }
        let mut pork = registry::get(&tw.g, "Raw Pork");
        assert!(use_item(&mut tw, &mut pork, 0, 0), "eating must succeed");
        if tw
            .g
            .player_mut()
            .player_mut()
            .potioneffects
            .contains_key(&PotionType::Queasy)
        {
            turned = true;
            break;
        }
    }
    assert!(turned, "raw pork never turned the stomach in 100 meals");

    // Queasy rides the potion-effect machinery: timed, visible, non-brewable
    assert!(PotionType::Queasy.duration() > 0);
    assert!(
        !tw.g
            .items
            .iter()
            .any(|i| i.get_name().eq_ignore_ascii_case("Queasy Potion")),
        "there must be no bottled Queasy"
    );
}

#[test]
fn hearty_stew_crafts_at_the_oven_and_warms() {
    let g = bare_game("farm_stew");
    let recipe = find_recipe(&g.recipes.oven, "Hearty Stew").clone();

    let mut inv = Inventory::new();
    inv.add(registry::get(&g, "Raw Beef"));
    inv.add(registry::get(&g, "Potato"));
    inv.add(registry::get(&g, "Carrot"));
    inv.add(registry::get(&g, "coal"));
    assert!(recipe.craft(&g, &mut inv), "stew must craft from its costs");
    assert!(
        inv.items()
            .iter()
            .any(|i| i.get_name().eq_ignore_ascii_case("Hearty Stew")),
        "the stew lands in the inventory"
    );
    for gone in ["Raw Beef", "Potato", "Carrot", "coal"] {
        assert_eq!(
            inv.count(&registry::get(&g, gone)),
            0,
            "{gone} must be consumed"
        );
    }

    // eating it is the warm-meal payoff: hunger, full stamina, a short Regen
    let mut tw = TestWorld::infinite().seed(3).build();
    {
        let p = tw.g.player_mut();
        let pd = p.player_mut();
        pd.hunger = 1;
        pd.stamina = fdoom::entity::mob::player::MAX_STAMINA; // eating costs 5; bonus refills
    }
    let mut stew = registry::get(&tw.g, "Hearty Stew");
    assert!(use_item(&mut tw, &mut stew, 0, 0));
    let p = tw.g.player_mut();
    let pd = p.player_mut();
    assert_eq!(pd.hunger, 9, "1 hunger + heal 8");
    assert_eq!(
        pd.stamina,
        fdoom::entity::mob::player::MAX_STAMINA,
        "a hot meal refills stamina"
    );
    assert!(
        pd.potioneffects.contains_key(&PotionType::Regen),
        "a hot meal grants a short Regen"
    );
}

/* ----------------------------- world seed stock ----------------------------- */

#[test]
fn pan_turns_up_seed_potatoes() {
    // poor ground: gem .001 / gold .005 / iron .025 / coal .065 / stone .365 / tuber .415
    assert_eq!(pan_outcome(0.0, 0.39), PanFind::Tuber);
    assert_eq!(pan_outcome(0.0, 0.42), PanFind::Nothing);
    // rich ground shifts the mineral bands but keeps the flat tuber band above stone
    assert_eq!(pan_outcome(1.0, 0.60), PanFind::Tuber);
    assert_eq!(pan_outcome(1.0, 0.99), PanFind::Nothing);
}

#[test]
fn village_fields_stamp_crops_and_larders_hold_seeds() {
    const SEED: i64 = 20260707;
    let tiles = Tiles::new();
    let corn = tiles.get("Corn Crop").id;
    let carrot = tiles.get("Carrot Crop").id;
    let farmland = tiles.get("farmland").id;

    // The town-age axis splits the plot duty: a Settled village tends a kitchen
    // garden (farmland, never volunteer crop rows), while aged villages keep the
    // farming wave's field gone to seed — corn rows (and the odd carrot) on the
    // farmland that hasn't reverted to earth. Scan a wide rect to see both.
    let mut settled_gardens = 0;
    let mut aged_fields = 0;
    for p in placements_in_rect(SEED, -4096, -4096, 4096, 4096) {
        if p.kind != StructureKind::Village {
            continue;
        }
        let writes = structure_writes(SEED, p, &tiles);
        let crops = writes.iter().any(|&(_, _, t)| t == corn || t == carrot);
        let farm = writes.iter().any(|&(_, _, t)| t == farmland);
        if town_age(SEED, p) == TownAge::Settled {
            assert!(
                !crops,
                "a Settled village keeps a tended garden, not a gone-to-seed field"
            );
            assert!(farm, "a Settled village tends its kitchen garden");
            settled_gardens += 1;
        } else if crops && farm {
            // not asserted per-village: an Overgrown field can lose its whole
            // crop column (or all its farmland) to the 45% reversion roll
            aged_fields += 1;
        }
    }
    assert!(settled_gardens > 0, "no Settled village within 4096 tiles");
    assert!(
        aged_fields > 0,
        "no aged village keeps a gone-to-seed field of crop rows on farmland"
    );
}

/* ------------------------------- persistence ------------------------------- */

/// A second `Game` over the same save dir (save_load_roundtrip.rs idiom).
fn reopen(g: &Game) -> Game {
    let mut g2 = Game::new(false, false, g.game_dir.clone());
    let mut player = fdoom::entity::mob::player::new(&g2, None);
    player.c.eid = 0;
    g2.entities.put_back(player);
    g2
}

#[test]
fn mid_growth_crop_survives_save_and_load() {
    let mut g1 = bare_game("farm_saveload");

    let diff = g1.settings.get_idx("diff");
    for (i, &depth) in fdoom::level::IDX_TO_DEPTH.iter().enumerate() {
        let mut level = fdoom::level::Level::empty(128, 128, depth, diff);
        if depth == -4 {
            let obsidian = g1.tiles.get("Obsidian").id;
            level.tiles.iter_mut().for_each(|t| *t = obsidian);
        }
        g1.levels[i] = Some(level);
    }
    for _ in 0..10 {
        let dc = fdoom::entity::furniture::dungeon_chest::new(&mut g1);
        g1.level_mut(4).add_at(dc, 80, 80, false, 4);
    }

    // a farm mid-growth: tilled plot, corn at age 23, plus a fresh vine
    let corn = g1.tiles.get("Corn Crop");
    g1.set_tile(3, 6, 7, &corn, 23);
    let vine = g1.tiles.get("Pumpkin Vine");
    g1.set_tile(3, 7, 7, &vine, 3);

    g1.current_level = 3;
    {
        let p = g1.player_mut();
        p.c.level = Some(3);
        p.c.removed = false;
    }

    save::save_world_named(&mut g1, "farmworld");
    let mut g2 = reopen(&g1);
    load::load_world_named(&mut g2, "farmworld");

    assert_eq!(g2.tile_at(3, 6, 7).name, "CORN CROP");
    assert_eq!(
        g2.level(3).get_data(6, 7),
        23,
        "growth age survives the save"
    );
    assert_eq!(g2.tile_at(3, 7, 7).name, "PUMPKIN VINE");
    assert_eq!(g2.level(3).get_data(7, 7), 3);
}

/* ------------------------------- screenshots ------------------------------- */

fn dump3x(tw: &mut TestWorld, name: &str) {
    let pixels = tw.render();
    save_png(
        verify_path(&format!("{name}.png")),
        &pixels,
        screen::W as usize,
        screen::H as usize,
        3,
    );
}

/// Verification stills for target/verify: a working farm plot with crops at every
/// stage, cooking at a lit campfire (smoke up), and the oven's food menu.
#[test]
fn farming_screens() {
    let mut tw = TestWorld::infinite().seed(20260707).build();
    tw.tick_n(8); // stream chunks around spawn
    tw.set_time(fdoom::core::updater::DAY_LENGTH / 2 - 1); // shoot at high noon
    tw.tick_n(1);
    let (px, py) = tw.player_tile();
    let lvl = tw.g.current_level;

    // a grass clearing with a tilled, creek-side field
    for dy in -5..=5 {
        for dx in -8..=8 {
            tw.place_at("grass", px + dx, py + dy);
        }
    }
    for dy in -1..=2 {
        tw.place_at("water", px - 5, py + dy); // irrigation ditch
    }
    let stage = |tw: &mut TestWorld, name: &str, dx: i32, dy: i32, age: i32| {
        let def = tw.g.tiles.get(name);
        tw.g.set_tile(lvl, px + dx, py + dy, &def, age);
    };
    for dx in -4..=2 {
        tw.place_at("farmland", px + dx, py); // the worked row
        tw.place_at("farmland", px + dx, py + 1);
    }
    stage(&mut tw, "Carrot Crop", -4, 0, 5);
    stage(&mut tw, "Carrot Crop", -3, 0, 25);
    stage(&mut tw, "Carrot Crop", -2, 0, 50);
    stage(&mut tw, "Potato Crop", -1, 0, 25);
    stage(&mut tw, "Potato Crop", 0, 0, 50);
    stage(&mut tw, "Corn Crop", 1, 0, 25);
    stage(&mut tw, "Corn Crop", 2, 0, 50);
    stage(&mut tw, "Pumpkin Vine", -4, 1, 30);
    stage(&mut tw, "Wheat", -3, 1, 50);
    tw.place_at("Wild Carrot", px + 4, py - 2);
    tw.place_at("pumpkin", px + 4, py + 2);
    dump3x(&mut tw, "farming_plot");

    // cooking at the fire: roast a fish, smoke rising
    let mut fire = fdoom::entity::furniture::campfire::new();
    fire.c.level = Some(lvl);
    fire.c.x = (px + 2) * 16 + 8;
    fire.c.y = (py - 2) * 16 + 8;
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    let mut held = Some(registry::get(&tw.g, "Raw Fish"));
    assert!(fdoom::entity::furniture::campfire_behavior::interact(
        &mut tw.g,
        &mut fire,
        &mut player,
        &mut held,
        Direction::Down,
    ));
    tw.g.entities.put_back(player);
    let fx = fire.c.x;
    tw.g.level_mut(lvl)
        .add_at(fire, fx, (py - 2) * 16 + 8, false, lvl);
    tw.tick_n(2); // let the smoke particle spawn in
    dump3x(&mut tw, "farming_campfire_cook");

    // the oven menu, pantry stocked so Have:/Cost: panels read meaningfully
    for it in ["Potato_3", "Carrot_2", "Corn_2", "Raw Beef_1", "coal_4"] {
        tw.give(it, 1);
    }
    let player = tw.g.entities.take(tw.g.player_id).expect("player");
    let display = fdoom::screen::crafting_display::CraftingDisplay::new(
        &tw.g,
        tw.g.recipes.oven.clone(),
        "Oven",
        &player,
    );
    tw.g.entities.put_back(player);
    tw.g.set_menu(display);
    tw.tick_n(1); // pending menus apply on the next tick
    dump3x(&mut tw, "farming_oven_menu");

    // an aged village's field gone to seed (grafted onto the towns' age axis):
    // stand just south of its first surviving corn row
    const SEED: i64 = 20260707;
    let tiles = Tiles::new();
    let corn = tiles.get("Corn Crop").id;
    let farmland = tiles.get("farmland").id;
    let (cx, cy) = placements_in_rect(SEED, -4096, -4096, 4096, 4096)
        .into_iter()
        .filter(|p| p.kind == StructureKind::Village && town_age(SEED, *p) != TownAge::Settled)
        .find_map(|p| {
            let writes = structure_writes(SEED, p, &tiles);
            let has_farm = writes.iter().any(|&(_, _, t)| t == farmland);
            let row = writes.iter().find(|&&(_, _, t)| t == corn)?;
            has_farm.then_some((row.0, row.1))
        })
        .expect("an aged village with a surviving field within 4096 tiles");
    let mut tw = TestWorld::infinite().seed(SEED).build();
    tw.g.change_time_of_day(fdoom::core::updater::Time::Day);
    tw.teleport(cx, cy + 2);
    tw.tick_n(12);
    dump3x(&mut tw, "farming_village_field");
}

/* ------------------------------- ripe vines -------------------------------- */

#[test]
fn ripe_pumpkin_vine_becomes_a_pumpkin() {
    let mut tw = TestWorld::infinite().seed(13).build();
    let (px, py) = tw.player_tile();
    let (tx, ty) = (px + 1, py);
    let lvl = tw.g.current_level;
    let vine = tw.g.tiles.get("Pumpkin Vine");
    tw.g.set_tile(lvl, tx, ty, &vine, 50);

    pulse_tile(&mut tw, tx, ty, 10); // a few pulses beat the 50% gate
    assert_eq!(tile_name(&tw, tx, ty), "PUMPKIN", "ripe vine fruits");

    // smashing the pumpkin closes the seed loop
    assert!(tw.hit(1, 0, 1));
    let drops = tw.dropped_items();
    assert!(
        drops.iter().any(|n| n == "Pumpkin"),
        "pumpkin drops its fruit: {drops:?}"
    );
}
