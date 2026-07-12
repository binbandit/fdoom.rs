//! Content wave: hot springs, abandoned mine shafts, bees & honey, the Badlands
//! biome. Pure-gen assertions on pinned seeds plus TestWorld interaction runs.

use std::collections::HashMap;

use fdoom::core::temperature::{self, Modifiers};
use fdoom::entity::EntityKind;
use fdoom::item::registry;
use fdoom::level::chunk::{CHUNK_SIZE, chunk_coord};
use fdoom::level::features_gen;
use fdoom::level::infinite_gen::{self, Biome, biome_at, biome_at_blended};
use fdoom::level::tile::{Tiles, dispatch, fossick};
use fdoom::testutil::{TestWorld, find_recipe};

const SEED: i64 = 20260707;

/// Generated tile id + data at a global position, generating (and caching) the
/// owning chunk — so assertions never care whether a feature straddles a border.
struct GenReader<'a> {
    tiles: &'a Tiles,
    seed: i64,
    depth: i32,
    cache: HashMap<(i32, i32), fdoom::level::chunk::Chunk>,
}

impl<'a> GenReader<'a> {
    fn new(tiles: &'a Tiles, seed: i64, depth: i32) -> Self {
        GenReader {
            tiles,
            seed,
            depth,
            cache: HashMap::new(),
        }
    }
    fn at(&mut self, x: i32, y: i32) -> (u8, u8) {
        let (cx, cy) = (chunk_coord(x), chunk_coord(y));
        let c = self.cache.entry((cx, cy)).or_insert_with(|| {
            infinite_gen::generate_chunk(self.seed, self.depth, cx, cy, self.tiles)
        });
        let i = ((x - cx * CHUNK_SIZE) + (y - cy * CHUNK_SIZE) * CHUNK_SIZE) as usize;
        (c.tiles[i], c.data[i])
    }
    fn name(&mut self, x: i32, y: i32) -> String {
        let id = self.at(x, y).0;
        self.tiles.get_id(i32::from(id)).name.clone()
    }
}

/* ----------------------------------- hot springs ----------------------------------- */

#[test]
fn hot_springs_generate_ragged_pools_in_the_cold() {
    let tiles = Tiles::new();
    let spring_id = tiles.get("Spring Water").id;
    let springs = features_gen::springs_in_rect(SEED, -3000, -3000, 3000, 3000);
    assert!(!springs.is_empty(), "no hot springs within 6k x 6k");

    let mut tundra_checked = false;
    for &(sx, sy) in &springs {
        let b = biome_at(SEED, sx, sy);
        assert!(
            matches!(b, Biome::Tundra | Biome::Mountains),
            "spring at ({sx}, {sy}) placed in {b:?}"
        );
        let mut r = GenReader::new(&tiles, SEED, 0);
        let mut pool = 0;
        for dy in -2..=2 {
            for dx in -2..=2 {
                if r.at(sx + dx, sy + dy).0 == spring_id {
                    pool += 1;
                }
            }
        }
        assert!(
            (2..=5).contains(&pool),
            "spring at ({sx}, {sy}): pool has {pool} tiles"
        );
        tundra_checked |= b == Biome::Tundra;
    }
    assert!(tundra_checked, "no spring landed in Tundra proper");
}

#[test]
fn hot_spring_clamps_cold_and_never_freezes() {
    // the pure mitigation pipeline: basking range clamps any cold to comfort but
    // never adds heat (the campfire contract)
    let bask = Modifiers {
        near_spring: true,
        ..Modifiers::default()
    };
    assert_eq!(temperature::apply_modifiers(-2.4, &bask), 0.0);
    assert_eq!(temperature::apply_modifiers(-0.6, &bask), 0.0);
    assert_eq!(temperature::apply_modifiers(1.2, &bask), 1.2);

    // live world: stand beside (then in) a staged pool on tundra snow
    let mut tw = TestWorld::infinite().seed(SEED).build();
    tw.goto_biome(Biome::Tundra);
    let (px, py) = tw.player_tile();
    for (dx, dy) in [(2, 0), (3, 0), (2, 1)] {
        tw.place_at("Spring Water", px + dx, py + dy);
    }
    let lvl = tw.current_level;
    {
        let p = tw.g.player();
        let m = temperature::modifiers_for(&tw.g, p);
        assert!(
            m.near_spring,
            "pool 2 tiles away must read as basking range"
        );
        assert!(
            temperature::score_for(&tw.g, p) >= 0.0,
            "cold not clamped at the spring rim"
        );
    }
    // swimming in it fully warms: a tundra body of spring water reads Comfort
    tw.teleport(px + 2, py + 1);
    tw.tick_n(2);
    {
        let p = tw.g.player();
        assert_eq!(
            temperature::band_for(&tw.g, p),
            temperature::Band::Comfort,
            "swimming in the spring must land in Comfort"
        );
    }
    // the pool never freezes or snows over: hammer its random tick
    let def = tw.g.tile_at(lvl, px + 2, py);
    assert_eq!(def.name, "SPRING WATER");
    for _ in 0..2000 {
        let def = tw.g.tile_at(lvl, px + 2, py);
        dispatch::tick(&mut tw.g, &def, lvl, px + 2, py);
    }
    assert_eq!(
        tw.g.tile_at(lvl, px + 2, py).name,
        "SPRING WATER",
        "spring water converted under random ticks"
    );
}

/* ------------------------------- abandoned mine shafts ------------------------------- */

#[test]
fn mine_shaft_headframe_has_its_carved_gallery_below() {
    let tiles = Tiles::new();
    let shafts = features_gen::shafts_in_rect(SEED, -3000, -3000, 3000, 3000);
    assert!(!shafts.is_empty(), "no mine shafts within 6k x 6k");
    let (sx, sy) = shafts[0];
    assert_eq!(biome_at(SEED, sx, sy), Biome::Mountains);

    // surface: the chasm mouth under a timber headframe on a spoil apron
    let mut surf = GenReader::new(&tiles, SEED, 0);
    assert_eq!(surf.name(sx, sy), "CHASM", "shaft mouth missing");
    assert_eq!(surf.name(sx - 1, sy - 1), "TIMBER PROP");
    assert_eq!(surf.name(sx + 1, sy - 1), "TIMBER PROP");
    assert_eq!(surf.name(sx, sy + 1), "DIRT", "no spoil apron");

    // one layer down: the gallery — ladder home, carved floor, standing timber,
    // an iron bias in the walls, and weak roof-fall rubble
    let mut mine = GenReader::new(&tiles, SEED, -1);
    assert_eq!(mine.name(sx, sy), "LADDER", "no ladder back up");
    assert_eq!(mine.name(sx + 1, sy + 1), "DIRT", "no crate floor");
    assert_eq!(mine.name(sx - 1, sy + 1), "TIMBER PROP", "no gallery prop");
    let mut carved = 0;
    let mut ore = 0;
    for dy in -4..=4 {
        for dx in -4..=4 {
            let name = mine.name(sx + dx, sy + dy);
            if name == "DIRT" || name == "LADDER" || name == "TIMBER PROP" {
                carved += 1;
            }
            if name == "IRON ORE" || name == "LAPIS" {
                ore += 1;
            }
        }
    }
    assert!(carved >= 10, "gallery too small: {carved} open tiles");
    assert!(ore >= 2, "gallery vein bias missing: {ore} ore tiles");
    // rubble rocks carry the weak-rubble data flag
    let writes = features_gen::shaft_gallery_writes(SEED, sx, sy, &tiles);
    let rock_id = tiles.get("rock").id;
    assert!(
        writes
            .iter()
            .any(|&(_, _, t, d)| t == rock_id && i32::from(d) & fossick::RUBBLE_FLAG != 0),
        "no rubble-flagged rock in the gallery blueprint"
    );
}

#[test]
fn shaft_gallery_stocks_a_mining_crate() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    let mine_lvl = 2; // depth -1
    let shafts = features_gen::shafts_in_rect(SEED, -3000, -3000, 3000, 3000);
    let mut found = None;
    for &(sx, sy) in shafts.iter().take(6) {
        fdoom::level::ensure_chunks_at(&mut tw.g, mine_lvl, sx, sy, true);
        let hit = tw.g.level(mine_lvl).entities_to_add.iter().any(|e| {
            matches!(&e.kind, EntityKind::ScavContainer(sc)
                    if (e.c.x >> 4, e.c.y >> 4) == (sx + 1, sy + 1)
                        && sc.chest.inventory.inv_size() > 0)
        });
        if hit {
            found = Some((sx, sy));
            break;
        }
    }
    assert!(
        found.is_some(),
        "none of the first shafts spawned a stocked supply crate"
    );
}

/* ---------------------------------- bees & honey ---------------------------------- */

#[test]
fn beehive_harvest_smoke_and_regrowth() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    let lvl = tw.current_level;
    let (tx, ty) = tw.place("Beehive", 1, 0);

    // 1) a held torch smokes the bees calm: always comb, never a sting
    let mut stings = 0;
    for _ in 0..30 {
        tw.g.level_mut(lvl).set_data(tx, ty, 0); // re-fill
        let before = tw.g.player().player().mob.health;
        assert!(tw.interact_with("Torch", 1, 0), "torch smoke failed");
        assert_eq!(tw.g.level(lvl).get_data(tx, ty), 1, "hive not harvested");
        if tw.g.player().player().mob.health < before {
            stings += 1;
        }
        tw.tick_n(3);
    }
    assert_eq!(stings, 0, "smoked harvests must never sting");
    assert!(
        tw.dropped_items().iter().any(|n| n == "Honeycomb"),
        "no honeycomb from smoked hives"
    );

    // 2) bare-handed: comb always, a sting roughly 1 in 3 (statistical band wide
    //    enough to never flake, tight enough to catch a dead or constant roll)
    let mut bare_stings = 0;
    for _ in 0..60 {
        tw.g.level_mut(lvl).set_data(tx, ty, 0);
        {
            let hp = &mut tw.g.player_mut().player_mut().mob.health;
            *hp = fdoom::entity::mob::player::MAX_HEALTH;
        }
        tw.g.notifications.clear();
        assert!(tw.hit(1, 0, 1), "bare harvest failed");
        if tw.g.notifications.iter().any(|n| n == "Bees!") {
            bare_stings += 1;
        }
        tw.tick_n(6); // clear hurt cooldown between pulls
    }
    assert!(
        (6..=40).contains(&bare_stings),
        "sting odds off: {bare_stings}/60 (expect ~20)"
    );

    // 3) regrowth: the harvested hive re-fills on its random-tick timer
    tw.g.level_mut(lvl).set_data(tx, ty, 1);
    let def = tw.g.tile_at(lvl, tx, ty);
    let mut refilled = false;
    for _ in 0..200_000 {
        dispatch::tick(&mut tw.g, &def, lvl, tx, ty);
        if tw.g.level(lvl).get_data(tx, ty) == 0 {
            refilled = true;
            break;
        }
    }
    assert!(refilled, "hive never regrew");

    // 4) a spent hive knocks down to the plain tree (choppable afterward)
    tw.g.level_mut(lvl).set_data(tx, ty, 1);
    assert!(tw.hit(1, 0, 1));
    assert_eq!(tw.g.tile_at(lvl, tx, ty).name, "TREE");
}

#[test]
fn honey_recipes_craft() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    tw.give("Honeycomb", 3);
    tw.give("glass", 1);
    tw.give("Cooked Fish", 1);

    let recipes = tw.g.recipes.clone();
    let jar = find_recipe(&recipes.workbench, "Honey Jar").clone();
    let glaze = find_recipe(&recipes.oven, "Honey-Glazed Fish").clone();
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    {
        let inv = &mut player.player_mut().inventory;
        assert!(jar.craft(&tw.g, inv), "Honey Jar failed to craft");
        assert!(glaze.craft(&tw.g, inv), "Honey-Glazed Fish failed to craft");
        assert!(inv.count(&registry::get(&tw.g, "Honey Jar")) >= 1);
        assert!(inv.count(&registry::get(&tw.g, "Honey-Glazed Fish")) >= 1);
    }
    tw.g.entities.put_back(player);

    // wild hives actually generate in forests at the pinned seed
    let tiles = Tiles::new();
    let hive = tiles.get("Beehive").id;
    let mut n = 0;
    'out: for cy in -40..0i32 {
        for cx in -41..-30i32 {
            let c = infinite_gen::generate_chunk(SEED, 0, cx, cy, &tiles);
            n += c.tiles.iter().filter(|&&t| t == hive).count();
            if n >= 1 {
                break 'out;
            }
        }
    }
    assert!(n >= 1, "no wild beehive in the forest sweep");
}

/* ------------------------------------ badlands ------------------------------------ */

#[test]
fn badlands_generate_dry_banded_and_freckled() {
    let tiles = Tiles::new();
    let water = tiles.get("water").id;
    let deep = tiles.get("Deep Water").id;
    let clay = tiles.get("Layered Clay").id;
    let freckle = tiles.get("Ore Freckle").id;
    let rock = tiles.get("rock").id;

    // presence across seeds (kept out of biomes_are_large_and_all_present's family
    // list — badlands is the desert's rare core, so it gets its own wider sweep)
    for seed in [SEED, 1i64, 4242, 99] {
        let mut n = 0;
        for y in (-3000..3000).step_by(24) {
            for x in (-3000..3000).step_by(24) {
                if biome_at(seed, x, y) == Biome::Badlands {
                    n += 1;
                }
            }
        }
        assert!(
            n >= 40,
            "seed {seed}: badlands too scarce ({n} lattice hits)"
        );
    }

    // the pinned region: clay country with mesas and freckles, bone dry
    let (bx, by) = (944, 1376);
    assert_eq!(biome_at(SEED, bx, by), Biome::Badlands);
    let mut r = GenReader::new(&tiles, SEED, 0);
    let (mut clays, mut freckles, mut rocks) = (0, 0, 0);
    for dy in -64..64 {
        for dx in -64..64 {
            let (x, y) = (bx + dx, by + dy);
            let t = r.at(x, y).0;
            if biome_at_blended(SEED, x, y) != Biome::Badlands {
                continue;
            }
            assert_ne!(t, water, "open water inside badlands at ({x}, {y})");
            assert_ne!(t, deep, "deep water inside badlands at ({x}, {y})");
            if t == clay {
                clays += 1;
            }
            if t == freckle {
                freckles += 1;
            }
            if t == rock {
                rocks += 1;
            }
        }
    }
    assert!(
        clays > 2000,
        "badlands not clay country: {clays} clay tiles"
    );
    assert!(rocks > 50, "no mesa/hoodoo rock: {rocks}");
    assert!(
        freckles >= 1,
        "no ore freckles in a 128x128 rich-side sweep"
    );

    // and the cold can never touch it: no tundra within 12 tiles of any badlands
    // sample (the biome_at gate guarantees ~100+, this is the cheap regression net)
    for dy in (-64..64).step_by(4) {
        for dx in (-64..64).step_by(4) {
            assert_ne!(
                biome_at_blended(SEED, bx + dx, by + dy),
                Biome::Tundra,
                "tundra inside the badlands window"
            );
        }
    }
}

/* ----------------------------------- screenshots ----------------------------------- */

/// Visual dump for the content wave (target/verify/): the steaming spring in snow,
/// the shaft headframe and its gallery, a hive harvest, and the badlands vista.
#[test]
fn content_wave_screens() {
    // 1) hot spring in tundra snow — walk up, let the steam breathe
    let mut tw = TestWorld::infinite().seed(SEED).name("cwave1").build();
    tw.g.change_time_of_day(fdoom::core::updater::Time::Day);
    let springs = features_gen::springs_in_rect(SEED, -3000, -3000, 3000, 3000);
    let &(sx, sy) = springs
        .iter()
        .find(|&&(x, y)| biome_at(SEED, x, y) == Biome::Tundra)
        .expect("tundra spring");
    tw.teleport(sx - 3, sy);
    tw.tick_n(240); // stream chunks + let steam wisps accumulate
    tw.screenshot("content_spring_tundra.png");

    // 2) the shaft headframe, then ride the chasm down to its gallery
    let shafts = features_gen::shafts_in_rect(SEED, -3000, -3000, 3000, 3000);
    let (hx, hy) = shafts[0];
    tw.teleport(hx - 2, hy + 1);
    tw.tick_n(12);
    tw.screenshot("content_shaft_headframe.png");
    let surface = tw.current_level;
    tw.teleport(hx, hy);
    tw.g.player_mut().player_mut().on_stair_delay = 0;
    for _ in 0..200 {
        tw.tick();
        if tw.current_level != surface {
            break;
        }
    }
    assert_eq!(tw.current_level, surface - 1, "chasm ride failed");
    // light the room for the shot (three torches so the whole gallery reads)
    tw.place_at("torch dirt", hx - 1, hy - 1);
    tw.place_at("torch dirt", hx + 2, hy);
    tw.place_at("torch dirt", hx - 2, hy + 1);
    tw.teleport(hx + 1, hy);
    tw.tick_n(12);
    tw.screenshot("content_shaft_gallery.png");

    // 3) a wild hive on a forest tree, smoked
    let mut tw = TestWorld::infinite().seed(SEED).name("cwave2").build();
    tw.g.change_time_of_day(fdoom::core::updater::Time::Day);
    tw.goto_biome(Biome::Forest);
    let (px, py) = tw.player_tile();
    tw.place_at("Beehive", px + 1, py);
    tw.place_at("tree", px + 1, py - 1);
    tw.place_at("tree", px + 2, py);
    tw.tick_n(4);
    tw.screenshot("content_beehive.png");
    assert!(tw.interact_with("Torch", 1, 0));
    tw.tick_n(3);
    tw.screenshot("content_beehive_smoked.png");

    // 4) the badlands vista
    let mut tw = TestWorld::infinite().seed(SEED).name("cwave3").build();
    tw.g.change_time_of_day(fdoom::core::updater::Time::Day);
    tw.teleport(944, 1376);
    tw.tick_n(12);
    tw.screenshot("content_badlands.png");
}

#[test]
fn ore_freckle_picks_and_clay_shovels() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    let lvl = tw.current_level;

    // pickaxe the freckle: 1-2 ore out, clay left behind
    let (tx, ty) = tw.place("Ore Freckle", 1, 0);
    assert!(tw.interact_with("Gem Pickaxe", 1, 0), "pickaxe refused");
    assert_eq!(tw.g.tile_at(lvl, tx, ty).name, "LAYERED CLAY");
    let drops = tw.dropped_items();
    assert!(
        drops.iter().any(|n| n == "Iron Ore" || n == "Coal"),
        "freckle dropped no ore: {drops:?}"
    );

    // a shovel says why it won't work on the freckle...
    tw.place("Ore Freckle", 1, 0);
    assert!(tw.interact_with("Gem Shovel", 1, 0));
    assert_eq!(tw.g.tile_at(lvl, tx, ty).name, "ORE FRECKLE");

    // ...and plain clay digs like dirt (the descent works in badlands)
    tw.place("Layered Clay", 0, 1);
    assert!(tw.interact_with("Gem Shovel", 0, 1), "shovel refused clay");
    let (px, py) = tw.player_tile();
    assert_eq!(tw.g.tile_at(lvl, px, py + 1).name, "DUG PIT");
}
