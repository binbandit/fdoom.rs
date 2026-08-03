//! Interned tile handles — the compile-time-checked way to name a tile.
//!
//! Tiles are stored in the world as a raw `u8` id, and the registry is a flat 256-slot
//! table indexed by that id. Naming a tile by string therefore costs an uppercase
//! allocation plus a scan, *and* a typo is invisible: [`super::Tiles::get`] prints a
//! warning and hands back grass, so a content bug ships looking like a gameplay bug.
//!
//! [`TileId`] closes both holes. `ids::ROCK` is a `const` — it resolves at compile time,
//! costs nothing at runtime, and `ids::ROKC` does not compile. The id/name pairs below
//! are the single source of truth: [`Tiles::new`](super::Tiles::new) registers by these
//! constants and `tests/tile_ids.rs` asserts every one still resolves to the tile it
//! claims, so the table cannot silently drift from the registry.
//!
//! String lookup survives only where a name genuinely arrives at runtime — save files
//! and the dev console — and there it goes through the fallible
//! [`Tiles::get_checked`](super::Tiles::get_checked).

/// A tile's registry id: its index into the 256-slot tile table and the byte actually
/// stored in a level's tile array.
///
/// Ids 128..=255 are the torch variants of base tile `id - 128`, materialized on demand
/// (see [`Tiles::get_torch_tile`](super::Tiles::get_torch_tile)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileId(u8);

impl TileId {
    /// Wrap a raw id — for ids that genuinely arrive as bytes (level arrays, saves).
    #[inline]
    pub const fn new(raw: u8) -> TileId {
        TileId(raw)
    }

    /// The byte stored in a level's tile array.
    #[inline]
    pub const fn raw(self) -> u8 {
        self.0
    }

    /// Index into the registry table.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The torch-carrying variant of this tile, registered at `id + 128`.
    ///
    /// Only meaningful for base tiles; a torch has no torch variant.
    #[inline]
    pub const fn torch(self) -> TileId {
        TileId(self.0 | 0x80)
    }

    /// True for the torch variants (ids 128..=255).
    #[inline]
    pub const fn is_torch(self) -> bool {
        self.0 >= 128
    }

    /// The tile a torch stands on — itself, for a base tile.
    #[inline]
    pub const fn base(self) -> TileId {
        TileId(self.0 & 0x7f)
    }
}

impl From<TileId> for u8 {
    #[inline]
    fn from(id: TileId) -> u8 {
        id.0
    }
}

/// Declares the id constants and, alongside them, the `(id, registered name)` table the
/// tests check the live registry against.
macro_rules! tile_ids {
    ($($(#[$m:meta])* $konst:ident = $id:literal, $name:literal;)*) => {
        $($(#[$m])* pub const $konst: TileId = TileId($id);)*

        /// Every constant above paired with the uppercase name its tile must register
        /// under. Used by `tests/tile_ids.rs` to prove the table matches the registry.
        pub const ALL: &[(TileId, &str)] = &[$(($konst, $name),)*];
    };
}

tile_ids! {
    GRASS = 0, "GRASS";
    DIRT = 1, "DIRT";
    FLOWER = 2, "FLOWER";
    HOLE = 3, "HOLE";
    STAIRS_UP = 4, "STAIRS UP";
    STAIRS_DOWN = 5, "STAIRS DOWN";
    WATER = 6, "WATER";
    ROCK = 7, "ROCK";
    TREE = 8, "TREE";
    TREE_SAPLING = 9, "TREE SAPLING";
    SAND = 10, "SAND";
    CACTUS = 11, "CACTUS";
    CACTUS_SAPLING = 12, "CACTUS SAPLING";
    IRON_ORE = 13, "IRON ORE";
    GOLD_ORE = 14, "GOLD ORE";
    GEM_ORE = 15, "GEM ORE";
    LAPIS = 16, "LAPIS";
    LAVA = 17, "LAVA";
    LAVA_BRICK = 18, "LAVA BRICK";
    EXPLODE = 19, "EXPLODE";
    FARMLAND = 20, "FARMLAND";
    WHEAT = 21, "WHEAT";
    HARD_ROCK = 22, "HARD ROCK";
    INFINITE_FALL = 23, "INFINITE FALL";
    CLOUD = 24, "CLOUD";
    CLOUD_CACTUS = 25, "CLOUD CACTUS";
    WOOD_DOOR = 26, "WOOD DOOR";
    STONE_DOOR = 27, "STONE DOOR";
    OBSIDIAN_DOOR = 28, "OBSIDIAN DOOR";
    WOOD_PLANKS = 29, "WOOD PLANKS";
    STONE_BRICKS = 30, "STONE BRICKS";
    OBSIDIAN = 31, "OBSIDIAN";
    WOOD_WALL = 32, "WOOD WALL";
    STONE_WALL = 33, "STONE WALL";
    OBSIDIAN_WALL = 34, "OBSIDIAN WALL";
    WOOL = 35, "WOOL";
    QUICK_SAND = 36, "QUICK SAND";
    SNOW = 37, "SNOW";
    SNOW_TREE = 38, "SNOW TREE";
    SMALL_GRASS = 39, "SMALL GRASS";
    MEDIUM_GRASS = 40, "MEDIUM GRASS";
    TALL_GRASS = 41, "TALL GRASS";
    PUMPKIN = 42, "PUMPKIN";
    GRAVE_STONE = 43, "GRAVE STONE";
    BROKEN_GRAVE_STONE = 44, "BROKEN GRAVE STONE";
    FENCE = 45, "FENCE";
    DEEP_WATER = 46, "DEEP WATER";
    DUG_PIT = 47, "DUG PIT";
    CHASM = 48, "CHASM";
    LADDER = 49, "LADDER";
    MUD = 50, "MUD";
    PINE_TREE = 51, "PINE TREE";
    DEAD_TREE = 52, "DEAD TREE";
    WILLOW = 53, "WILLOW";
    PALM_TREE = 54, "PALM TREE";
    FLAT_CROWN_TREE = 55, "FLAT-CROWN TREE";
    BERRY_BUSH = 56, "BERRY BUSH";
    MUSHROOM = 57, "MUSHROOM";
    FRUITING_CACTUS = 58, "FRUITING CACTUS";
    SEAWEED = 59, "SEAWEED";
    CORAL = 60, "CORAL";
    REEDS = 61, "REEDS";
    JACK_O_LANTERN = 62, "JACK-O-LANTERN";
    DRY_BUSH = 63, "DRY BUSH";
    TIDAL_FLAT = 64, "TIDAL FLAT";
    TIMBER_PROP = 65, "TIMBER PROP";
    WINDOW = 66, "WINDOW";
    HEATH = 67, "HEATH";
    CARROT_CROP = 68, "CARROT CROP";
    POTATO_CROP = 69, "POTATO CROP";
    CORN_CROP = 70, "CORN CROP";
    PUMPKIN_VINE = 71, "PUMPKIN VINE";
    WILD_CARROT = 72, "WILD CARROT";
    SPRING_WATER = 73, "SPRING WATER";
    BEEHIVE = 74, "BEEHIVE";
    LAYERED_CLAY = 75, "LAYERED CLAY";
    ORE_FRECKLE = 76, "ORE FRECKLE";
}
