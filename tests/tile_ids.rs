//! Interned tile ids: identity proof, compile-time-safety guards, and lookup cost.
//!
//! The world-identity tests here are the contract for the id-interning lane: swapping
//! runtime string lookups for interned handles must not move a single byte of any
//! generated world. `world_fingerprint_is_stable` prints a hash per (seed, depth) that
//! is compared against a recorded baseline captured before the conversion.

use std::time::Instant;

use fdoom::item::ids as iname;
use fdoom::level::infinite_gen::generate_chunk;
use fdoom::level::level_gen::create_and_validate_map;
use fdoom::level::tile::{TileId, Tiles, ids};
use fdoom::rng::Rng;

/// FNV-1a over a byte slice — a stable, dependency-free content hash.
fn fnv1a(bytes: &[u8], seed: u64) -> u64 {
    let mut h = seed;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// Hash of a finite (non-chunked) layer: tiles then data.
fn finite_hash(seed: i64, depth: i32) -> u64 {
    let tiles = Tiles::new();
    let mut history = Rng::new(seed ^ 0x5DEECE66D);
    let (t, d) = create_and_validate_map(
        128,
        128,
        depth,
        &tiles,
        seed,
        "Island",
        "Normal",
        &mut history,
    )
    .expect("generation failed");
    fnv1a(&d, fnv1a(&t, FNV_OFFSET))
}

/// Hash of a square of infinite-layer chunks: tiles then data, chunk by chunk.
fn infinite_hash(seed: i64, depth: i32, radius: i32) -> u64 {
    let tiles = Tiles::new();
    let mut h = FNV_OFFSET;
    for cy in -radius..=radius {
        for cx in -radius..=radius {
            let c = generate_chunk(seed, depth, cx, cy, &tiles);
            h = fnv1a(&c.tiles, h);
            h = fnv1a(&c.data, h);
        }
    }
    h
}

/// The seeds the identity proof covers. Depths run 0 (surface) down to -3/-4, matching
/// the five-layer world shape.
const IDENTITY_SEEDS: [i64; 5] = [1, 4242, -99, 123_456_789, 0x0F05_51C4];

/// World identity: every covered (seed, depth) hashes to its recorded value.
///
/// If this fires after a refactor, world generation moved — that is a bug in the
/// refactor, not a stale expectation. Only re-record when a change to gen is intended.
#[test]
fn world_fingerprint_is_stable() {
    let mut lines = Vec::new();
    for seed in IDENTITY_SEEDS {
        for depth in [0, -1, -2, -3, -4] {
            lines.push(format!(
                "finite seed={seed} depth={depth} {:016x}",
                finite_hash(seed, depth)
            ));
        }
        for depth in [0, -1, -2, -3] {
            lines.push(format!(
                "infinite seed={seed} depth={depth} {:016x}",
                infinite_hash(seed, depth, 2)
            ));
        }
    }
    let report = lines.join("\n");
    let combined = fnv1a(report.as_bytes(), FNV_OFFSET);
    println!("{report}");
    println!("WORLD_FINGERPRINT {combined:016x}");

    assert_eq!(
        format!("{combined:016x}"),
        WORLD_FINGERPRINT,
        "world generation changed:\n{report}"
    );
}

/// Recorded on the pre-conversion code, before a single call site had been touched.
/// Zero behaviour change means this constant never moves; re-record it only when a
/// change to world generation is the actual intent.
const WORLD_FINGERPRINT: &str = "55deec6f0ac7d11b";

/* ------------------------- compile-time-safety guards ------------------------- */

/// Every `ids` constant still names the tile it claims.
///
/// This is what makes the constants trustworthy: a constant is only as good as its
/// binding to the registry, and `Tiles::new` registers *by* these constants, so an id
/// that drifted would have to drift in both places at once to escape.
#[test]
fn id_constants_match_the_registry() {
    let tiles = Tiles::new();
    for &(id, name) in ids::ALL {
        let def = tiles.get_id(id.raw() as i32);
        assert_eq!(
            def.name,
            name,
            "ids constant for id {} names the wrong tile",
            id.raw()
        );
        assert_eq!(def.id, id.raw(), "registry id disagrees with the constant");
        assert_eq!(
            tiles.id_of(name),
            Some(id),
            "name {name:?} does not resolve back to its constant"
        );
    }
}

/// No tile is registered without a constant — otherwise call sites would have to fall
/// back to a string for it, which is the hole this lane closed.
#[test]
fn every_registered_tile_has_a_constant() {
    let tiles = Tiles::new();
    for id in 0..128 {
        if !tiles.contains_tile(id) {
            continue;
        }
        let name = tiles.get_id(id).name.clone();
        assert!(
            ids::ALL.iter().any(|(c, _)| c.raw() as i32 == id),
            "tile {id} ({name}) is registered but has no ids:: constant"
        );
    }
}

/// `TileDef::same_tile` compares ids where Java compared names. That substitution is
/// only sound while names are unique, so prove it — for base tiles *and* for the torch
/// variants, which are materialized lazily and are easy to forget.
#[test]
fn tile_names_are_unique_so_ids_can_stand_in() {
    let tiles = Tiles::new();
    let mut seen: Vec<(String, i32)> = Vec::new();
    for id in 0..256 {
        // force every torch variant into existence before checking
        if id >= 128 && !tiles.contains_tile(id - 128) {
            continue;
        }
        if id < 128 && !tiles.contains_tile(id) {
            continue;
        }
        let def = tiles.get_id(id);
        if let Some((_, other)) = seen.iter().find(|(n, _)| *n == def.name) {
            panic!(
                "tiles {other} and {id} share the name {:?} — same_tile compares ids and \
                 would now disagree with the name comparison it replaced",
                def.name
            );
        }
        seen.push((def.name.clone(), id));
        assert!(
            def.name.is_ascii(),
            "non-ASCII tile name {:?} breaks the fast name path",
            def.name
        );
    }
}

/// A name nothing registers under is a miss, not grass.
#[test]
fn unknown_tile_names_fail_loudly() {
    let tiles = Tiles::new();
    assert!(tiles.get_checked("definitely not a tile").is_none());
    assert!(tiles.get_checked("torch definitely not a tile").is_none());
    assert!(tiles.id_of("definitely not a tile").is_none());
    // the legacy accessor keeps its old, quiet contract for callers outside this lane
    assert_eq!(tiles.get("definitely not a tile").id, ids::GRASS.raw());
}

/// The string path's two pieces of tolerance — the `TORCH ` prefix and the `_data`
/// suffix — still behave, since save files depend on both.
#[test]
fn name_path_tolerances_survive() {
    let tiles = Tiles::new();
    assert_eq!(
        tiles.get_checked("wool_2").map(|t| t.id),
        Some(ids::WOOL.raw())
    );
    assert_eq!(
        tiles.get_checked("HARD ROCK").map(|t| t.id),
        Some(ids::HARD_ROCK.raw())
    );
    assert_eq!(
        tiles.get_checked("hard rock").map(|t| t.id),
        Some(ids::HARD_ROCK.raw())
    );

    let torch_dirt = tiles
        .get_checked("torch dirt")
        .expect("torch dirt resolves");
    assert_eq!(torch_dirt.tid(), ids::DIRT.torch());
    assert_eq!(torch_dirt.tid().base(), ids::DIRT);
    assert!(torch_dirt.tid().is_torch());
    assert_eq!(torch_dirt.name, "TORCH DIRT");
}

/// Every `iname` constant resolves against a real registry.
///
/// Items have no numeric id to intern to, so the constants stay names — this test is
/// what turns "the name is spelled right" from a runtime warning into a build failure.
#[test]
fn every_item_constant_resolves() {
    let tw = fdoom::testutil::TestWorld::infinite()
        .name("item_ids")
        .build();
    for name in iname::ALL {
        assert!(
            fdoom::item::registry::get_checked(&tw.g, name.as_str()).is_some(),
            "item constant {name} does not resolve in the registry"
        );
    }
    assert_eq!(
        iname::ALL.len(),
        tw.g.items.len(),
        "constants and registry are out of step"
    );
}

/// Recipe declarations embed item names in a compact DSL string, so the compiler cannot
/// check them. Check them here instead: a typo in a recipe used to surface only as an
/// `UnknownItem` appearing in a crafting menu.
#[test]
fn every_recipe_name_resolves() {
    let tw = fdoom::testutil::TestWorld::infinite()
        .name("recipe_names")
        .build();
    let r = &tw.g.recipes;
    let lists: [(&str, &Vec<fdoom::item::Recipe>); 8] = [
        ("anvil", &r.anvil),
        ("oven", &r.oven),
        ("furnace", &r.furnace),
        ("workbench", &r.workbench),
        ("enchant", &r.enchant),
        ("craft", &r.craft),
        ("loom", &r.loom),
        ("bench_modules", &r.bench_modules),
    ];
    for (station, list) in lists {
        for recipe in list {
            let product = recipe.product_name();
            assert!(
                fdoom::item::registry::get_checked(&tw.g, product).is_some(),
                "{station} recipe produces unknown item {product:?}"
            );
            for (cost, _) in recipe.get_costs() {
                assert!(
                    fdoom::item::registry::get_checked(&tw.g, cost).is_some(),
                    "{station} recipe for {product:?} costs unknown item {cost:?}"
                );
            }
        }
    }
}

/// Placeable items carry tile *descriptors* (a name plus an optional `_data` suffix), so
/// they cannot become plain `TileId` constants without a descriptor type — see the lane
/// report. Until then, check them here: a typo in a `valid_tiles` entry used to mean the
/// item simply refused to place, with no warning anywhere.
#[test]
fn every_placeable_item_names_real_tiles() {
    use fdoom::item::ItemKind;
    let tw = fdoom::testutil::TestWorld::infinite()
        .name("tile_items")
        .build();
    let tiles = &tw.g.tiles;
    let mut checked = 0;
    for item in tw.g.items.iter() {
        let (model, valid) = match &item.kind {
            ItemKind::TileItem {
                model, valid_tiles, ..
            } => (Some(model), valid_tiles),
            ItemKind::Torch { valid_tiles, .. } => (None, valid_tiles),
            _ => continue,
        };
        if let Some(model) = model {
            assert!(
                tiles.get_checked(model).is_some(),
                "{} places unknown tile {model:?}",
                item.get_name()
            );
        }
        for name in valid {
            assert!(
                tiles.get_checked(name).is_some(),
                "{} lists unknown ground tile {name:?}",
                item.get_name()
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "expected some placeable items to check");
}

/// Item lookup cost. Items keep a linear scan — there is nowhere in this lane to hang a
/// name index, since `Game::items` is declared outside it (see the lane report) — so the
/// only thing measured here is the uppercase allocation the lookup no longer makes.
#[test]
fn item_lookup_cost_report() {
    let tw = fdoom::testutil::TestWorld::infinite()
        .name("item_cost")
        .build();
    const N: usize = 50_000;
    let names: Vec<&str> = iname::ALL.iter().map(|n| n.as_str()).collect();

    // The pre-conversion path, replicated exactly — including the prototype clone, which
    // dominates either way and would flatter the new path if left out.
    let legacy = |name: &str| -> Option<fdoom::item::Item> {
        let upper = name.to_uppercase();
        let found =
            tw.g.items
                .iter()
                .find(|i| i.get_name().eq_ignore_ascii_case(&upper));
        found.map(|proto| {
            let mut item = proto.clone();
            if item.is_stackable() {
                item.set_count(1);
            }
            item
        })
    };

    let t0 = Instant::now();
    let mut hits = 0usize;
    for i in 0..N {
        hits += legacy(names[i % names.len()]).is_some() as usize;
    }
    let old = t0.elapsed();

    let t1 = Instant::now();
    let mut hits2 = 0usize;
    for i in 0..N {
        hits2 +=
            fdoom::item::registry::get_checked(&tw.g, names[i % names.len()]).is_some() as usize;
    }
    let new = t1.elapsed();

    assert_eq!(hits, N);
    assert_eq!(hits2, N);
    let per = |d: std::time::Duration| d.as_nanos() as f64 / N as f64;
    println!(
        "ITEM LOOKUP {N} iters over {} items: old {:.0} ns/op | new {:.0} ns/op ({:.2}x)",
        names.len(),
        per(old),
        per(new),
        per(old) / per(new).max(0.001),
    );
}

/// An item name nothing registers under is a miss, not a silent `UnknownItem`.
#[test]
fn unknown_item_names_fail_loudly() {
    let tw = fdoom::testutil::TestWorld::infinite()
        .name("item_miss")
        .build();
    assert!(fdoom::item::registry::get_checked(&tw.g, "Definitely Not An Item").is_none());
    // the sentinels are placeholders, not items
    assert!(fdoom::item::registry::get_checked(&tw.g, "NULL").is_none());
    assert!(fdoom::item::registry::get_checked(&tw.g, "unknown").is_none());
    // and the count suffix still parses on the checked path
    let stack = fdoom::item::registry::get_checked(&tw.g, "Stone_7").expect("Stone resolves");
    assert_eq!(stack.count(), 7);
}

/// Rough cost of resolving a tile by name vs. by interned id.
///
/// Not a statistical benchmark — a smoke-level ratio, printed so the conversion's claim
/// can be checked rather than trusted. The sample spans the whole id range on purpose:
/// the old lookup was a linear scan, so measuring only low ids (`rock` is 7) flatters it
/// by letting it exit early.
#[test]
fn lookup_cost_report() {
    let tiles = Tiles::new();
    const N: usize = 200_000;

    // one name per id across the table, low and high
    let names: Vec<&str> = ids::ALL.iter().map(|(_, name)| *name).collect();
    let handles: Vec<TileId> = ids::ALL.iter().map(|(id, _)| *id).collect();

    // The pre-conversion lookup, replicated exactly: uppercase into a fresh String, then
    // a linear scan of the 256-slot table comparing names. Reproduced here rather than
    // quoted from a stashed build so the "before" number is measured on this machine, in
    // this run, against the same sample.
    let table: Vec<_> = (0..256)
        .filter(|&i| tiles.contains_tile(i))
        .map(|i| tiles.get_id(i))
        .collect();
    let legacy = |name: &str| -> u8 {
        let mut name = name.to_uppercase();
        if let Some(stripped) = name.strip_prefix("TORCH ") {
            name = stripped.to_string();
        }
        if let Some(idx) = name.find('_') {
            name.truncate(idx);
        }
        table
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.id)
            .unwrap_or(0)
    };

    let tl = Instant::now();
    let mut acc_legacy = 0u64;
    for i in 0..N {
        acc_legacy += legacy(names[i % names.len()]) as u64;
    }
    let by_legacy = tl.elapsed();

    let t0 = Instant::now();
    let mut acc = 0u64;
    for i in 0..N {
        acc += tiles.get(names[i % names.len()]).id as u64;
    }
    let by_name = t0.elapsed();

    let t1 = Instant::now();
    let mut acc2 = 0u64;
    for i in 0..N {
        acc2 += tiles.by_id(handles[i % handles.len()]).id as u64;
    }
    let by_id = t1.elapsed();

    // and the shape most call sites actually collapsed to: a compile-time constant,
    // where the "lookup" is the constant itself
    let t2 = Instant::now();
    let mut acc3 = 0u64;
    for i in 0..N {
        acc3 += handles[i % handles.len()].raw() as u64;
    }
    let by_const = t2.elapsed();

    assert_eq!(acc, acc2, "the two paths must resolve the same tiles");
    assert_eq!(acc2, acc3, "ids and defs must agree");
    assert_eq!(acc_legacy, acc, "the replicated old lookup must agree too");
    let per = |d: std::time::Duration| d.as_nanos() as f64 / N as f64;
    println!(
        "LOOKUP {N} iters over {} tiles:\n  \
         old string scan   {:6.1} ns/op\n  \
         new string index  {:6.1} ns/op ({:.1}x)\n  \
         by_id (runtime)   {:6.1} ns/op ({:.1}x)\n  \
         const id (inlined){:6.1} ns/op ({:.1}x)",
        names.len(),
        per(by_legacy),
        per(by_name),
        per(by_legacy) / per(by_name).max(0.001),
        per(by_id),
        per(by_legacy) / per(by_id).max(0.001),
        per(by_const),
        per(by_legacy) / per(by_const).max(0.001),
    );
}
