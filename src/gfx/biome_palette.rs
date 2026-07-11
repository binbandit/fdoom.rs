//! Biome -> map color, shared by the in-game map screen and the `worldview` tool so
//! both draw the same picture of a seed (single source of truth for the palette).

use crate::level::infinite_gen::Biome;

/// Map color for a biome, as 0xRRGGBB.
pub fn biome_color(b: Biome) -> u32 {
    match b {
        Biome::DeepOcean => 0x0B2E6B,
        Biome::Ocean => 0x1E5AC8,
        Biome::Beach => 0xE6D793,
        Biome::Mountains => 0x8C8C98,
        Biome::Tundra => 0xE9F1F7,
        Biome::Desert => 0xE4C468,
        Biome::Marsh => 0x4E8A66,
        Biome::Forest => 0x1F7A33,
        Biome::Savanna => 0xC9B457,
        Biome::Plains => 0x7CC353,
    }
}

/// Every biome with its display name, in legend order (water -> coast -> inland).
/// Keep in sync with [`Biome`]; `biome_color`'s exhaustive match is the compile-time
/// reminder when a variant is added.
pub const BIOME_LEGEND: [(Biome, &str); 10] = [
    (Biome::DeepOcean, "DEEP OCEAN"),
    (Biome::Ocean, "OCEAN"),
    (Biome::Beach, "BEACH"),
    (Biome::Plains, "PLAINS"),
    (Biome::Forest, "FOREST"),
    (Biome::Savanna, "SAVANNA"),
    (Biome::Marsh, "MARSH"),
    (Biome::Tundra, "TUNDRA"),
    (Biome::Desert, "DESERT"),
    (Biome::Mountains, "MOUNTAINS"),
];
