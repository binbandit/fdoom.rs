//! Flora wave: deterministic generation of the biome tree species / food flora /
//! ocean life, plus the interactive loops (berry regrowth, palm coconuts, thicket
//! paddock passability).

use std::collections::{HashMap, HashSet};

use fdoom::level::chunk::CHUNK_SIZE;
use fdoom::level::infinite_gen::{Biome, biome_at, generate_chunk};
use fdoom::level::tile::{Tiles, dispatch};
use fdoom::testutil::TestWorld;

const SEED: i64 = 20260707;

/* ------------------------------- generation tests ------------------------------- */

#[test]
fn flora_generation_is_deterministic() {
    let tiles = Tiles::new();
    for (depth, cx, cy) in [(0, 0, 0), (0, 7, -4), (0, -13, 22), (-1, 3, -2)] {
        let a = generate_chunk(SEED, depth, cx, cy, &tiles);
        let b = generate_chunk(SEED, depth, cx, cy, &tiles);
        assert_eq!(
            a.tiles, b.tiles,
            "chunk ({cx},{cy}) depth {depth} not deterministic"
        );
    }
}

/// Every flora species appears in its home biome somewhere in a wide sample.
///
/// Sweep an outward ring of chunk-sized lattice cells; a chunk is generated (once,
/// cached) only when its center sits in a biome some still-missing species calls home,
/// and a species counts as found when its tile id appears in such a chunk.
#[test]
fn species_present_in_their_biomes() {
    let tiles = Tiles::new();
    let id = |name: &str| tiles.get(name).id;

    // (biome, tile name) — every pair must be found
    let mut wanted: Vec<(Biome, &str, u8)> = vec![
        (Biome::Tundra, "Pine Tree", id("Pine Tree")),
        (Biome::Forest, "Pine Tree (cold fringe)", id("Pine Tree")),
        (Biome::Forest, "Tree", id("Tree")),
        (Biome::Forest, "Berry Bush", id("Berry Bush")),
        (Biome::Forest, "Mushroom", id("Mushroom")),
        (Biome::Plains, "Berry Bush", id("Berry Bush")),
        (Biome::Desert, "Dead Tree", id("Dead Tree")),
        (Biome::Desert, "Fruiting Cactus", id("Fruiting Cactus")),
        (Biome::Desert, "Dry Bush", id("Dry Bush")),
        (Biome::Savanna, "Flat-Crown Tree", id("Flat-Crown Tree")),
        (Biome::Savanna, "Dry Bush", id("Dry Bush")),
        (Biome::Marsh, "Willow", id("Willow")),
        (Biome::Marsh, "Reeds", id("Reeds")),
        (Biome::Beach, "Palm Tree", id("Palm Tree")),
        (Biome::Ocean, "Seaweed", id("Seaweed")),
        (Biome::Ocean, "Coral", id("Coral")),
        (Biome::Mountains, "Snow (snow-capped peak)", id("Snow")),
    ];

    let mut cache: HashMap<(i32, i32), Vec<u8>> = HashMap::new();
    let mut chunks_generated = 0;

    'ring: for radius in 0..140i32 {
        for cy in -radius..=radius {
            for cx in -radius..=radius {
                if cx.abs() != radius && cy.abs() != radius {
                    continue; // ring only
                }
                let (x, y) = (
                    cx * CHUNK_SIZE + CHUNK_SIZE / 2,
                    cy * CHUNK_SIZE + CHUNK_SIZE / 2,
                );
                let b = biome_at(SEED, x, y);
                if !wanted.iter().any(|&(wb, _, _)| wb == b) {
                    continue;
                }
                let tiles_ref = &tiles;
                let chunk = cache.entry((cx, cy)).or_insert_with(|| {
                    chunks_generated += 1;
                    generate_chunk(SEED, 0, cx, cy, tiles_ref).tiles
                });
                let present: HashSet<u8> = chunk.iter().copied().collect();
                wanted.retain(|&(wb, _, wid)| !(wb == b && present.contains(&wid)));
                if wanted.is_empty() {
                    break 'ring;
                }
            }
        }
    }

    assert!(
        wanted.is_empty(),
        "species never generated (after {chunks_generated} chunks): {:?}",
        wanted
            .iter()
            .map(|&(b, name, _)| format!("{name} in {b:?}"))
            .collect::<Vec<_>>()
    );
}

#[test]
fn mine_caves_grow_mushrooms() {
    let tiles = Tiles::new();
    let mushroom = tiles.get("Mushroom").id;
    for depth in [-1, -2, -3] {
        let mut n = 0;
        for cy in -2..=2 {
            for cx in -2..=2 {
                let c = generate_chunk(SEED, depth, cx, cy, &tiles);
                n += c.tiles.iter().filter(|&&t| t == mushroom).count();
            }
        }
        assert!(
            n > 0,
            "depth {depth}: no cave mushrooms in a 5x5 chunk sweep"
        );
    }
}

/// Some cemeteries and razed villages keep a lit Jack-O-Lantern (rare, deterministic).
#[test]
fn jack_o_lanterns_haunt_some_structures() {
    use fdoom::level::structures_gen::{StructureKind, placement_in_cell, structure_writes};
    let tiles = Tiles::new();
    let jack = tiles.get("Jack-O-Lantern").id;
    for kind in [StructureKind::Cemetery, StructureKind::Village] {
        let mut with = 0;
        let mut total = 0;
        for cy in -20..20 {
            for cx in -20..20 {
                let Some(p) = placement_in_cell(SEED, kind, cx, cy) else {
                    continue;
                };
                total += 1;
                if structure_writes(SEED, p, &tiles)
                    .iter()
                    .any(|&(_, _, t)| t == jack)
                {
                    with += 1;
                }
            }
        }
        assert!(total > 0, "{kind:?}: no placements in sample");
        assert!(
            with > 0,
            "{kind:?}: no jack-o-lanterns in {total} placements"
        );
        assert!(
            with < total,
            "{kind:?}: every placement has one — supposed to be rare"
        );
    }
}

/* ------------------------------- interactive tests ------------------------------- */

fn has_item(names: &[String], want: &str) -> bool {
    names.iter().any(|n| n.eq_ignore_ascii_case(want))
}

#[test]
fn berry_bush_pick_and_regrow_cycle() {
    let mut tw = TestWorld::infinite().seed(4242).build();
    let lvl = tw.current_level;
    let (tx, ty) = tw.place("Berry Bush", 1, 0);
    assert_eq!(tw.level(lvl).get_data(tx, ty), 0, "fresh bush is ripe");

    // first hit picks the berries: bush survives, goes into regrowth
    assert!(tw.hit(1, 0, 1));
    assert!(
        tw.tile_at(lvl, tx, ty)
            .name
            .eq_ignore_ascii_case("Berry Bush"),
        "picking must not destroy the bush"
    );
    assert_eq!(
        tw.level(lvl).get_data(tx, ty),
        1,
        "bush regrowing after pick"
    );
    assert!(
        has_item(&tw.dropped_items(), "Berry"),
        "picking a ripe bush drops a Berry"
    );

    // random ticks regrow the berries (1-in-2000 per random tick; generous cap)
    let def = tw.tile_at(lvl, tx, ty);
    let mut regrew = false;
    for _ in 0..200_000 {
        dispatch::tick(&mut tw, &def, lvl, tx, ty);
        if tw.level(lvl).get_data(tx, ty) == 0 {
            regrew = true;
            break;
        }
    }
    assert!(regrew, "berries regrow over time");

    // a second pick works after regrowth
    assert!(tw.hit(1, 0, 1));
    assert_eq!(tw.level(lvl).get_data(tx, ty), 1);

    // hitting the bare bush tears it out
    assert!(tw.hit(1, 0, 1));
    assert!(
        tw.tile_at(lvl, tx, ty).name.eq_ignore_ascii_case("Grass"),
        "bare bush breaks to grass"
    );
}

#[test]
fn palm_drops_coconuts_when_felled() {
    let mut tw = TestWorld::infinite().seed(4242).build();
    let (tx, ty) = tw.place("Palm Tree", 1, 0);

    // 20 damage fells a fresh palm (health 20) in one blow
    assert!(tw.hit(1, 0, 20));
    assert!(
        tw.tile_at(tw.current_level, tx, ty)
            .name
            .eq_ignore_ascii_case("Sand"),
        "felled palm leaves its sand base"
    );
    let names = tw.dropped_items();
    assert!(has_item(&names, "Coconut"), "felled palm drops Coconut(s)");
    assert!(has_item(&names, "Wood"), "felled palm drops Wood");
}

#[test]
fn dead_tree_is_brittle_and_drops_sticks_only() {
    let mut tw = TestWorld::infinite().seed(4242).build();
    let (tx, ty) = tw.place("Dead Tree", 1, 0);

    // 8 damage fells the snag in one blow (broadleaf would shrug that off)
    assert!(tw.hit(1, 0, 8));
    assert!(
        tw.tile_at(tw.current_level, tx, ty)
            .name
            .eq_ignore_ascii_case("Sand")
    );
    let names = tw.dropped_items();
    assert!(has_item(&names, "Stick"), "dead tree drops Sticks");
    assert!(!has_item(&names, "Wood"), "dead tree drops no Wood");
}

#[test]
fn pumpkins_and_jack_o_lanterns_drop_their_items() {
    let mut tw = TestWorld::infinite().seed(4242).build();
    let lvl = tw.current_level;

    tw.place("pumpkin", 1, 0);
    assert!(tw.hit(1, 0, 1));
    assert!(has_item(&tw.dropped_items(), "Pumpkin"));

    let (jx, jy) = tw.place("Jack-O-Lantern", 2, 0);
    let jack = tw.tile_at(lvl, jx, jy);
    assert!(
        dispatch::get_light_radius(&tw, &jack, lvl, jx, jy) > 3,
        "a Jack-O-Lantern out-glows a plain pumpkin"
    );
    assert!(tw.hit(2, 0, 1));
    assert!(has_item(&tw.dropped_items(), "Jack-O-Lantern"));
}

#[test]
fn thicket_blocks_only_paddock_cores() {
    let mut tw = TestWorld::infinite().seed(4242).build();
    let lvl = tw.current_level;
    let (px, py) = tw.player_tile();
    // clear a 9x9 stage well away from the player's own tile
    let (ox, oy) = (px + 10, py + 10);
    for dy in -4..=4 {
        for dx in -4..=4 {
            tw.place_at("grass", ox + dx, oy + dy);
        }
    }

    let passable = |tw: &TestWorld, x: i32, y: i32| {
        let def = tw.tile_at(lvl, x, y);
        dispatch::may_pass(tw, &def, lvl, x, y, tw.player())
    };

    // a lone fully-grown tuft is brushed through
    tw.place_at("Tall Grass", ox, oy);
    assert!(passable(&tw, ox, oy), "lone thicket tile must be passable");

    // a 5x5 paddock: the core is impenetrable, the fringe is walkable
    for dy in -2..=2 {
        for dx in -2..=2 {
            tw.place_at("Tall Grass", ox + dx, oy + dy);
        }
    }
    assert!(!passable(&tw, ox, oy), "paddock core must block");
    assert!(passable(&tw, ox - 2, oy), "paddock edge stays walkable");
    assert!(
        passable(&tw, ox + 2, oy + 2),
        "paddock corner stays walkable"
    );
}
