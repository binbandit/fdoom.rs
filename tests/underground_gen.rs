use fdoom::level::level_gen::create_and_validate_map;
use fdoom::level::tile::Tiles;
use fdoom::rng::Rng;

#[test]
fn underground_has_ores_and_caves() {
    let tiles = Tiles::new();
    for depth in [-1, -2, -3] {
        let mut hr = Rng::new(1);
        let (map, _) =
            create_and_validate_map(128, 128, depth, &tiles, 777, "Island", "Normal", &mut hr)
                .expect("underground generation failed");
        let count = |name: &str| {
            let id = tiles.get(name).id;
            map.iter().filter(|&&t| t == id).count()
        };
        let ore = match depth {
            -1 => count("iron ore"),
            -2 => count("gold ore"),
            _ => count("gem ore"),
        };
        let rock = count("rock");
        let dirt = count("dirt");
        assert!(ore > 20, "depth {depth}: expected ore veins, got {ore}");
        assert!(rock > 2000, "depth {depth}: expected cave rock, got {rock}");
        assert!(
            dirt > 100,
            "depth {depth}: expected cave floors, got {dirt}"
        );
        assert!(
            count("stairs down") > 0 || depth == -3,
            "depth {depth}: needs stairs down"
        );
        assert_eq!(
            count("grass") + count("tree"),
            0,
            "depth {depth}: no surface tiles underground"
        );
    }
}

#[test]
fn every_layer_has_stairs_down() {
    let tiles = Tiles::new();
    let sd = tiles.get("stairs down").id;
    for depth in [0, -1, -2, -3] {
        let mut hr = Rng::new(5);
        let (map, _) =
            create_and_validate_map(128, 128, depth, &tiles, 31337, "Island", "Normal", &mut hr)
                .unwrap();
        let n = map.iter().filter(|&&t| t == sd).count();
        assert!(
            n > 0,
            "depth {depth} has no stairs down (progression broken)"
        );
        println!("depth {depth}: {n} stairs down");
    }
}
