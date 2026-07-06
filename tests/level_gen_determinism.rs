//! World generation must be fully deterministic for a given seed (the JVM byte-parity
//! tests were retired with the java.util.Random port; see the v0.1.0 tag for those).

use fdoom::level::level_gen::create_and_validate_map;
use fdoom::level::tile::Tiles;
use fdoom::rng::Rng;

fn generate(seed: i64, depth: i32) -> (Vec<u8>, Vec<u8>) {
    let tiles = Tiles::new();
    let mut history = Rng::new(seed ^ 0x5DEECE66D);
    create_and_validate_map(
        128,
        128,
        depth,
        &tiles,
        seed,
        "Island",
        "Normal",
        &mut history,
    )
    .expect("generation failed")
}

#[test]
fn same_seed_same_world() {
    for depth in [1, 0, -1, -4] {
        let a = generate(4242, depth);
        let b = generate(4242, depth);
        assert_eq!(a, b, "depth {depth} not deterministic");
    }
}

#[test]
fn different_seeds_differ() {
    let a = generate(1, 0);
    let b = generate(2, 0);
    assert_ne!(a.0, b.0);
}

#[test]
fn all_types_and_themes_generate() {
    let tiles = Tiles::new();
    for gen_type in ["Island", "Box", "Mountain", "Irregular"] {
        for theme in ["Normal", "Forest", "Desert", "Plain", "Hell"] {
            let mut history = Rng::new(99);
            let maps =
                create_and_validate_map(128, 128, 0, &tiles, 12345, gen_type, theme, &mut history);
            assert!(maps.is_some(), "gen failed for {gen_type}/{theme}");
        }
    }
}
