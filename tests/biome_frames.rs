//! Visual checks: rendered frames across biomes, plus a biome overview map.
//! PNGs land in target/verify (`just shots` / `just biome-map <seed>`).

use fdoom::level::infinite_gen::{Biome, biome_at};
use fdoom::testutil::{TestWorld, save_png, verify_path};

/// Seed override for `just biome-map <seed>`; tests default to the pinned seed.
fn env_seed() -> i64 {
    std::env::var("FDOOM_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20260707)
}

#[test]
fn frames_across_biomes() {
    let mut tw = TestWorld::infinite().name("biomes").build();
    for (biome, name) in [
        (Biome::Tundra, "tundra"),
        (Biome::Desert, "desert"),
        (Biome::Marsh, "marsh"),
        (Biome::Forest, "forest"),
        (Biome::Mountains, "mountains"),
        (Biome::Beach, "beach"),
    ] {
        tw.goto_biome(biome);
        tw.screenshot(&format!("biome_{name}.png"));
    }
}

/// One pixel per 4x4 tiles over a 4096-tile square around the origin — the "where is
/// everything" map for a seed. `FDOOM_SEED=<n>` picks the seed (`just biome-map <n>`).
#[test]
fn biome_map_overview() {
    let seed = env_seed();
    const STEP: i32 = 4; // tiles per pixel
    const HALF: i32 = 2048; // tiles from origin to each edge
    let size = (2 * HALF / STEP) as usize;

    let color = |b: Biome| -> i32 {
        match b {
            Biome::DeepOcean => 0x0a2a55,
            Biome::Ocean => 0x1c4f8f,
            Biome::Beach => 0xe8d9a0,
            Biome::Desert => 0xd9b95c,
            Biome::Savanna => 0xb5a542,
            Biome::Plains => 0x7cb548,
            Biome::Forest => 0x2e7031,
            Biome::Marsh => 0x4a6b4f,
            Biome::Tundra => 0xdfe8ef,
            Biome::Mountains => 0x8a8f96,
        }
    };

    let mut pixels = vec![0i32; size * size];
    for py in 0..size {
        for px in 0..size {
            let (x, y) = (-HALF + px as i32 * STEP, -HALF + py as i32 * STEP);
            pixels[px + py * size] = color(biome_at(seed, x, y));
        }
    }
    // crosshair at the spawn origin
    let mid = size / 2;
    for d in 0..size {
        pixels[mid + d * size] ^= 0x202020;
        pixels[d + mid * size] ^= 0x202020;
    }

    let path = verify_path(&format!("biome_map_{seed}.png"));
    save_png(&path, &pixels, size, size, 1);
    println!("biome map for seed {seed}: {}", path.display());
}
