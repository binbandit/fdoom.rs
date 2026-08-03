//! Port of `fdoom.level.tile` — the tile system.
//!
//! Java tile classes are stateless singletons configured at construction; here each is a
//! `TileDef` (config) with a `TileKind` (class identity + per-class config). Per-tile
//! *state* lives in the level's `tiles`/`data` byte arrays, as in Java. Behavior dispatch
//! (`tick`/`interact`/`hurt`/...) is in `dispatch.rs`, matching on `TileKind` and calling
//! into the per-tile modules.
//!
//! # Module index
//!
//! - Ground: `dirt`, `grass`, `sand`, `snow`, `mud`, `heath`, `floor`, `wall`.
//! - Flora: `berry_bush`, `cactus`, `flower`, `mushroom`, `sapling`, `tree`,
//!   `tree_species`, `snow_tree`, `tall_grass`, `wild_carrot`.
//! - Water and sky: `water`, `tidal`, `reef`, `lava`, `cloud`, `snowfall`.
//! - Mining and depth: `rock`, `hard_rock`, `ore`, `fossick`, `hole`, `depth`.
//! - Structures: `door`, `fence`, `fire`, `grave_stone`, `stairs`, `torch`, `window`.
//! - Farming: `crop`, `farm`, `pumpkin`, `wheat`.

pub mod beehive;
pub mod berry_bush;
pub mod cactus;
pub mod clay;
pub mod cloud;
pub mod cloud_cactus;
pub mod crop;
pub mod depth;
pub mod dirt;
pub mod dispatch;
pub mod door;
pub mod dry_bush;
pub mod exploded;
pub mod farm;
pub mod fence;
pub mod fire;
pub mod floor;
pub mod flower;
pub mod fossick;
pub mod grass;
pub mod grave_stone;
pub mod hard_rock;
pub mod heath;
pub mod hole;
pub mod ids;
pub mod infinite_fall;
pub mod lava;
pub mod lava_brick;
pub mod mud;
pub mod mushroom;
pub mod ore;
pub mod pumpkin;
pub mod quicksand;
pub mod reef;
pub mod rock;
pub mod sand;
pub mod sapling;
pub mod snow;
pub mod snow_tree;
pub mod snowfall;
pub mod spring_water;
pub mod stairs;
pub mod tall_grass;
pub mod tidal;
pub mod timber_prop;
pub mod torch;
pub mod tree;
pub mod tree_species;
pub mod wall;
pub mod water;
pub mod wheat;
pub mod wild_carrot;
pub mod window;
pub mod wool;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::rc::Rc;

use crate::core::game::Game;
use crate::entity::Entity;
use crate::entity::mob::player_behavior::pay_stamina;
use crate::gfx::Sprite;
use crate::item::{Item, ItemKind, ToolType};

pub use ids::TileId;

/// The eight tiles surrounding one tile, sampled in cardinal then diagonal order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Neighbors {
    pub u: bool,
    pub d: bool,
    pub l: bool,
    pub r: bool,
    pub ul: bool,
    pub ur: bool,
    pub dl: bool,
    pub dr: bool,
}

impl Neighbors {
    /// Sample neighbors using [`TileDef::same_tile`] semantics.
    pub fn same_tile(g: &Game, def: &TileDef, lvl: usize, x: i32, y: i32) -> Self {
        Self::matching_id(g, lvl, x, y, |id| id == def.tid())
    }

    /// Sample neighbors on a predicate over the full [`TileDef`].
    ///
    /// For predicates on tile *properties* — `connects_to_water`, `blocks_light`,
    /// `flammable` — which need the def, not just its identity. Identity checks belong in
    /// [`Neighbors::matching_id`]: this form materializes eight `Rc<TileDef>` clones per
    /// call and runs per connector-sprite tile per frame.
    pub fn matching<F>(g: &Game, lvl: usize, x: i32, y: i32, mut pred: F) -> Self
    where
        F: FnMut(&TileDef) -> bool,
    {
        Self {
            u: pred(&g.tile_at(lvl, x, y - 1)),
            d: pred(&g.tile_at(lvl, x, y + 1)),
            l: pred(&g.tile_at(lvl, x - 1, y)),
            r: pred(&g.tile_at(lvl, x + 1, y)),
            ul: pred(&g.tile_at(lvl, x - 1, y - 1)),
            ur: pred(&g.tile_at(lvl, x + 1, y - 1)),
            dl: pred(&g.tile_at(lvl, x - 1, y + 1)),
            dr: pred(&g.tile_at(lvl, x + 1, y + 1)),
        }
    }

    /// Sample neighbors by interned id — reads the level's tile bytes directly, with no
    /// registry lookup or `Rc` traffic per probe.
    pub fn matching_id<F>(g: &Game, lvl: usize, x: i32, y: i32, mut pred: F) -> Self
    where
        F: FnMut(TileId) -> bool,
    {
        Self {
            u: pred(tile_id_at(g, lvl, x, y - 1)),
            d: pred(tile_id_at(g, lvl, x, y + 1)),
            l: pred(tile_id_at(g, lvl, x - 1, y)),
            r: pred(tile_id_at(g, lvl, x + 1, y)),
            ul: pred(tile_id_at(g, lvl, x - 1, y - 1)),
            ur: pred(tile_id_at(g, lvl, x + 1, y - 1)),
            dl: pred(tile_id_at(g, lvl, x - 1, y + 1)),
            dr: pred(tile_id_at(g, lvl, x + 1, y + 1)),
        }
    }
}

/// The interned id at `(x, y)`, without building a [`TileDef`].
///
/// Matches [`Game::tile_at`]'s out-of-bounds contract exactly: an unloaded chunk, an
/// out-of-range coordinate, or an absent level all read as rock.
#[inline]
pub fn tile_id_at(g: &Game, lvl: usize, x: i32, y: i32) -> TileId {
    match g.levels[lvl].as_ref().and_then(|l| l.tile_id(x, y)) {
        Some(id) => TileId::new(id),
        None => ids::ROCK,
    }
}

/// The common gate for tool-driven tile interactions (dig, chop, mine).
///
/// Succeeds when `item` is the requested kind of tool and the player pays the stamina
/// cost (`base_cost` less the tool's level) plus one point of tool durability. Returns
/// the tool's level so callers can scale their effect with it; the caller then applies
/// the tile's own result — swap the tile, drop items, play a sound.
///
/// The charge order is deliberate: stamina is spent even when the durability check then
/// fails, exactly like the hand-written per-tile interacts this helper replaced.
pub fn tool_use(
    g: &Game,
    player: &mut Entity,
    item: &mut Item,
    tool: ToolType,
    base_cost: i32,
) -> Option<i32> {
    let ItemKind::Tool { ttype, level, .. } = item.kind else {
        return None;
    };
    let paid = ttype == tool
        && pay_stamina(player, base_cost - level)
        && item.pay_durability(g.is_mode("creative"));
    paid.then_some(level)
}

/// Java `Tile.Material`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    Wood,
    Stone,
    Obsidian,
}

impl Material {
    pub const VALUES: [Material; 3] = [Material::Wood, Material::Stone, Material::Obsidian];

    pub fn name(self) -> &'static str {
        match self {
            Material::Wood => "Wood",
            Material::Stone => "Stone",
            Material::Obsidian => "Obsidian",
        }
    }
}

/// Java `OreTile.OreType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OreType {
    Iron,
    Gold,
    Gem,
    Lapis,
}

/// Flora-wave tree species (sandbox era, no Java counterpart). The classic broadleaf
/// keeps its own `TileKind::Tree`; these are the biome-specific variants that share the
/// tree behavior with different bases, health, and drops (see `tree_species.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeSpecies {
    /// Tundra + cold forest fringe; drops extra sticks.
    Pine,
    /// Desert snag; low health, sticks only.
    Dead,
    /// Marsh, near pools.
    Willow,
    /// Beach; drops Coconuts.
    Palm,
    /// Savanna, lone trees.
    FlatCrown,
}

/// Java `WoolTile.WoolType`-style tile data values are plain data bytes; see wool module.
///
/// Java `ConnectorSprite` (the data half; neighbor-aware rendering is in dispatch.rs).
#[derive(Debug, Clone)]
pub struct ConnectorSprite {
    pub sparse: Sprite,
    pub sides: Sprite,
    pub full: Sprite,
    pub check_corners: bool,
}

impl ConnectorSprite {
    pub fn new(sparse: Sprite, sides: Sprite, full: Sprite) -> ConnectorSprite {
        ConnectorSprite {
            sparse,
            sides,
            full,
            check_corners: true,
        }
    }

    /// Java 2-sprite constructor (sides = sparse, cornersMatter = false).
    pub fn simple(sparse: Sprite, full: Sprite) -> ConnectorSprite {
        ConnectorSprite {
            sides: sparse.clone(),
            sparse,
            full,
            check_corners: false,
        }
    }
}

/// The ground a prop/flora tile stands on, judged by majority vote over its
/// neighbors' actual ground tiles (cardinals count double). Props used to hardcode
/// their base — gravestones and overgrowth stamped grass squares into cemetery
/// dirt, border-band pines stamped snow squares onto grass country (ODDITIES
/// O6/O7). Other props/floors/water don't vote; if nothing votes (e.g. deep inside
/// a same-species tree cluster) the caller's `default` stands.
pub fn ground_beneath(g: &Game, lvl: usize, x: i32, y: i32, default: TileId) -> TileId {
    // vote slots double as the tie-break order: hard/rare grounds outrank fillers
    let mut names = [
        (ids::SNOW, 0i32),
        (ids::SAND, 0),
        (ids::LAYERED_CLAY, 0),
        (ids::HEATH, 0),
        (ids::MUD, 0),
        (ids::DIRT, 0),
        (ids::GRASS, 0),
    ];
    for (dx, dy) in [
        (0, -1),
        (0, 1),
        (-1, 0),
        (1, 0),
        (-1, -1),
        (1, -1),
        (-1, 1),
        (1, 1),
    ] {
        let cardinal = dx == 0 || dy == 0;
        let slot = match g.tile_at(lvl, x + dx, y + dy).kind {
            TileKind::Snow => 0,
            TileKind::Sand => 1,
            TileKind::Clay | TileKind::OreFreckle => 2,
            TileKind::Heath => 3,
            TileKind::Mud => 4,
            TileKind::Dirt | TileKind::Farm => 5,
            TileKind::Grass => 6,
            _ => continue,
        };
        names[slot].1 += if cardinal { 2 } else { 1 };
    }
    // max_by_key keeps the LAST maximum, so reverse: ties go to the earlier slot
    let (id, votes) = names.iter().rev().max_by_key(|(_, n)| *n).unwrap();
    if *votes > 0 { *id } else { default }
}

/// One Java tile class instance (e.g. "GRASS", or "WOOD DOOR").
#[derive(Debug, Clone)]
pub struct TileDef {
    pub id: u8,
    /// Uppercase name, as in Java.
    pub name: String,
    pub connects_to_grass: bool,
    pub connects_to_snow: bool,
    pub connects_to_sand: bool,
    pub connects_to_lava: bool,
    pub connects_to_water: bool,
    pub light: i32,
    pub may_spawn: bool,
    /// Post-port (light & shelter wave): this tile occludes emitter light in the
    /// `gfx::lighting` radiance pass. Walls, rock, and hard rock set it; doors set it
    /// too but are gated on their closed state in `dispatch::blocks_light`. Trees
    /// deliberately don't (forests stay lit); windows are the whole point of not.
    pub blocks_light: bool,
    /// Post-port (fire wave): this tile can catch fire (see `tile::fire`). Wood
    /// walls/doors/planks, trees (all species), tall grass stages + reeds, dry bush,
    /// and berry bush set it; stone, dirt, sand, snow, and mud stay false. The
    /// burning state itself is the high bit of the tile's data byte, not a tile id.
    pub flammable: bool,
    pub sprite: Option<Sprite>,
    pub csprite: Option<ConnectorSprite>,
    pub kind: TileKind,
}

impl TileDef {
    pub fn new(name: &str, kind: TileKind) -> TileDef {
        TileDef {
            id: 0,
            name: name.to_uppercase(),
            connects_to_grass: false,
            connects_to_snow: false,
            connects_to_sand: false,
            connects_to_lava: false,
            connects_to_water: false,
            light: 1,
            may_spawn: false,
            blocks_light: false,
            flammable: false,
            sprite: None,
            csprite: None,
            kind,
        }
    }

    /// Java `connectsToLiquid()`.
    pub fn connects_to_liquid(&self) -> bool {
        self.connects_to_water || self.connects_to_lava
    }

    /// This tile's interned id.
    #[inline]
    pub fn tid(&self) -> TileId {
        TileId::new(self.id)
    }

    /// Tile identity.
    ///
    /// Java compared names; ids are equivalent because every registered tile has a
    /// distinct name (`tests/tile_ids.rs` enforces that, torch variants included), and
    /// they compare in one byte instead of a string.
    pub fn same_tile(&self, other: &TileDef) -> bool {
        self.id == other.id
    }
}

/// One variant per Java tile class (plus per-instance constructor config).
#[derive(Debug, Clone)]
pub enum TileKind {
    Grass,
    Dirt,
    Flower,
    Hole,
    Mud,
    DeepWater,
    DugPit,
    Chasm,
    Ladder,
    Stairs {
        leads_up: bool,
    },
    Water,
    Rock,
    Tree,
    /// Biome tree variants sharing the broadleaf behavior (see `TreeSpecies`).
    TreeSpecies {
        species: TreeSpecies,
    },
    Sapling {
        on_type: TileId,
        grows_to: TileId,
    },
    Sand,
    Cactus,
    Lava,
    LavaBrick,
    Ore {
        ore_type: OreType,
    },
    Exploded,
    Farm,
    Wheat,
    HardRock,
    InfiniteFall,
    Cloud,
    CloudCactus,
    Floor {
        material: Material,
    },
    Wall {
        material: Material,
    },
    Door {
        material: Material,
    },
    /// Light & shelter wave: a wall segment with a glass pane — solid to movement
    /// like a wall, but transparent to light and sight (see `window.rs`).
    Window,
    /// Highland ground of the Mountains biome: stony moor with clustered heather
    /// patches (see `heath.rs`). Walkable; shovels to dirt.
    Heath,
    Wool,
    QuickSand,
    Snow,
    SnowTree,
    TallGrass {
        kind: i32,
    },
    Pumpkin {
        /// Jack-O-Lantern: carved + lit (stronger light, different drop).
        lit: bool,
    },
    /// Farmland row crop (farming wave); per-tile data = age 0..50 on the wheat
    /// clock. See `crop.rs`.
    Crop {
        crop: crop::CropKind,
    },
    /// Foraged root plant on grass — the carrot-farming entry point.
    WildCarrot,
    /// Pickable berry shrub; per-tile data 0 = ripe, 1 = regrowing.
    BerryBush,
    /// Forest-floor / cave-floor fungus; walk-through, breakable pickup.
    Mushroom,
    /// Cactus carrying fruit; a hit knocks the fruit off, leaving a plain Cactus.
    FruitingCactus,
    /// Shallow-water flora: renders over water, drops Grass Fibers.
    Seaweed,
    /// Shallow-water reef: renders over water, drops Stone.
    Coral,
    /// Dry tumbleweed shrub (desert/savanna); breaks bare-handed into Sticks.
    DryBush,
    /// Intertidal shore band: submerged (water) at high tide, exposed wet sand at low
    /// tide; the state is a pure function of the day clock + per-tile elevation.
    TidalFlat,
    /// Mine-ceiling support post; prevents cave-ins nearby (see `fossick.rs`).
    /// Walk-through; one hit knocks it down and refunds the timber.
    TimberProp,
    /// Geothermal pool water (hot springs): swims like water, steams, never
    /// freezes; clamps nearby cold toward comfort (see `spring_water.rs`).
    SpringWater,
    /// Wild hive on a broadleaf forest tree; per-tile data 0 = full, 1 =
    /// regrowing. Bare hands risk a sting; a held torch smokes the bees calm.
    Beehive,
    /// Badlands ground: banded rust strata; shovels like dirt (see `clay.rs`).
    Clay,
    /// Exposed ore pips on rich Badlands clay; pickaxe for 1-2 Iron Ore / Coal.
    OreFreckle,
    GraveStone {
        broken: bool,
    },
    Fence,
    /// Java `TorchTile` — wraps the tile it stands on; registered dynamically at
    /// `onType.id + 128`.
    Torch {
        on_type: TileId,
    },
}

/// Java `Tiles` — the tile registry. Interior mutability because torch tiles register
/// on demand (Java `Tiles.add` from `TorchTile.getTorchTile`).
///
/// Prefer [`Tiles::by_id`] with an [`ids`] constant. The name-keyed lookups are for the
/// boundary where a name genuinely arrives as text (saves, dev console).
pub struct Tiles {
    list: RefCell<Vec<Option<Rc<TileDef>>>>,
    /// Uppercase name -> id, built once from `list`. Replaces the linear scan that every
    /// name lookup used to pay; base tiles only, since the `TORCH ` prefix is stripped
    /// before the table is consulted.
    by_name: HashMap<String, u8, BuildHasherDefault<Fnv1a>>,
}

/// FNV-1a, for the name index only.
///
/// With the default `SipHash` the index barely beat the linear scan it replaced: the
/// table is under 100 entries of ~10 ASCII bytes each, so hashing dominated the lookup.
/// FNV is what turns the index into a real win (measured ~57 ns -> ~34 ns per lookup).
/// Nothing here is exposed to untrusted input — tile names come from the registry and
/// from our own save files.
struct Fnv1a(u64);

impl Default for Fnv1a {
    fn default() -> Self {
        Fnv1a(0xcbf2_9ce4_8422_2325) // FNV offset basis
    }
}

impl std::hash::Hasher for Fnv1a {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut h = self.0;
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
        }
        self.0 = h;
    }
}

/// Registry names are short (longest is "BROKEN GRAVE STONE", 18 bytes); this covers
/// them with room to spare so the common lookup normalizes on the stack.
const NORM_CAP: usize = 32;

/// Split a requested name into `(normalized base name, wants the torch variant)`.
///
/// Mirrors the Java lookup's tolerance: case-insensitive, an optional `TORCH ` prefix,
/// and an optional `_data` suffix that is not part of the name. ASCII names normalize
/// into `buf` without allocating; anything else falls back to `to_uppercase` so the
/// result stays byte-identical to the original Unicode-aware path.
fn normalize_request<'b>(name: &str, buf: &'b mut [u8; NORM_CAP]) -> (Cow<'b, str>, bool) {
    // Non-ASCII takes the original Unicode path verbatim (uppercase, then trim), so the
    // fast path can never disagree with it. No registered name is non-ASCII.
    if !name.is_ascii() || name.len() > NORM_CAP {
        let mut norm = name.to_uppercase();
        let is_torch = norm.starts_with("TORCH ");
        if is_torch {
            norm.drain(..6);
        }
        if let Some(idx) = norm.find('_') {
            norm.truncate(idx);
        }
        return (Cow::Owned(norm), is_torch);
    }

    // ASCII: trim on the raw input first — slicing is free, and neither the prefix test
    // nor the `_` split shifts under ASCII uppercasing.
    let mut base = name;
    let is_torch = base.len() >= 6 && base[..6].eq_ignore_ascii_case("TORCH ");
    if is_torch {
        base = &base[6..];
    }
    if let Some(idx) = base.find('_') {
        base = &base[..idx];
    }

    let n = base.len();
    for (slot, &b) in buf[..n].iter_mut().zip(base.as_bytes()) {
        *slot = b.to_ascii_uppercase();
    }
    // ASCII in, ASCII out — always valid UTF-8.
    (
        Cow::Borrowed(std::str::from_utf8(&buf[..n]).unwrap_or("")),
        is_torch,
    )
}

impl Default for Tiles {
    fn default() -> Self {
        Self::new()
    }
}

impl Tiles {
    /// Java `Tiles.initTileList()`.
    pub fn new() -> Tiles {
        let mut t: Vec<Option<Rc<TileDef>>> = vec![None; 256];

        // Registration is keyed by the `ids` constants, so the constants and the table
        // cannot drift apart; `tests/tile_ids.rs` checks every constant's name too.
        let mut set = |id: TileId, def: TileDef| {
            let mut def = def;
            def.id = id.raw();
            t[id.index()] = Some(Rc::new(def));
        };

        set(ids::GRASS, dispatch::make_grass_tile("Grass"));
        set(ids::DIRT, dispatch::make_dirt_tile("Dirt"));
        set(ids::FLOWER, dispatch::make_flower_tile("Flower"));
        set(ids::HOLE, dispatch::make_hole_tile("Hole"));
        set(
            ids::STAIRS_UP,
            dispatch::make_stairs_tile("Stairs Up", true),
        );
        set(
            ids::STAIRS_DOWN,
            dispatch::make_stairs_tile("Stairs Down", false),
        );
        set(ids::WATER, dispatch::make_water_tile("Water"));
        set(ids::ROCK, dispatch::make_rock_tile("Rock"));
        set(ids::TREE, dispatch::make_tree_tile("Tree"));
        set(
            ids::TREE_SAPLING,
            dispatch::make_sapling_tile("Tree Sapling", ids::GRASS, ids::TREE),
        );
        set(ids::SAND, dispatch::make_sand_tile("Sand"));
        set(ids::CACTUS, dispatch::make_cactus_tile("Cactus"));
        set(
            ids::CACTUS_SAPLING,
            dispatch::make_sapling_tile("Cactus Sapling", ids::SAND, ids::CACTUS),
        );
        set(ids::LAVA, dispatch::make_lava_tile("Lava"));
        set(
            ids::LAVA_BRICK,
            dispatch::make_lava_brick_tile("Lava Brick"),
        );
        set(ids::IRON_ORE, dispatch::make_ore_tile(OreType::Iron));
        set(ids::GOLD_ORE, dispatch::make_ore_tile(OreType::Gold));
        set(ids::GEM_ORE, dispatch::make_ore_tile(OreType::Gem));
        set(ids::LAPIS, dispatch::make_ore_tile(OreType::Lapis));
        set(ids::EXPLODE, dispatch::make_exploded_tile("Explode"));
        set(ids::FARMLAND, dispatch::make_farm_tile("Farmland"));
        set(ids::WHEAT, dispatch::make_wheat_tile("Wheat"));
        set(ids::HARD_ROCK, dispatch::make_hard_rock_tile("Hard Rock"));
        set(
            ids::INFINITE_FALL,
            dispatch::make_infinite_fall_tile("Infinite Fall"),
        );
        set(ids::CLOUD, dispatch::make_cloud_tile("Cloud"));
        set(
            ids::CLOUD_CACTUS,
            dispatch::make_cloud_cactus_tile("Cloud Cactus"),
        );
        set(ids::WOOD_PLANKS, dispatch::make_floor_tile(Material::Wood));
        set(
            ids::STONE_BRICKS,
            dispatch::make_floor_tile(Material::Stone),
        );
        set(ids::OBSIDIAN, dispatch::make_floor_tile(Material::Obsidian));
        set(ids::WOOD_WALL, dispatch::make_wall_tile(Material::Wood));
        set(ids::STONE_WALL, dispatch::make_wall_tile(Material::Stone));
        set(
            ids::OBSIDIAN_WALL,
            dispatch::make_wall_tile(Material::Obsidian),
        );
        set(ids::WOOD_DOOR, dispatch::make_door_tile(Material::Wood));
        set(ids::STONE_DOOR, dispatch::make_door_tile(Material::Stone));
        set(
            ids::OBSIDIAN_DOOR,
            dispatch::make_door_tile(Material::Obsidian),
        );
        set(ids::WOOL, dispatch::make_wool_tile());
        set(ids::QUICK_SAND, dispatch::make_quicksand_tile("Quick Sand"));
        set(ids::SNOW, dispatch::make_snow_tile("Snow"));
        set(ids::SNOW_TREE, dispatch::make_snow_tree_tile("Snow Tree"));
        set(
            ids::SMALL_GRASS,
            dispatch::make_tall_grass_tile("Small Grass", "grass", 0),
        );
        set(
            ids::MEDIUM_GRASS,
            dispatch::make_tall_grass_tile("Medium Grass", "grass", 1),
        );
        set(
            ids::TALL_GRASS,
            dispatch::make_tall_grass_tile("Tall Grass", "grass", 2),
        );
        set(ids::PUMPKIN, dispatch::make_pumpkin_tile("pumpkin", false));
        set(
            ids::JACK_O_LANTERN,
            dispatch::make_pumpkin_tile("Jack-O-Lantern", true),
        );
        set(
            ids::GRAVE_STONE,
            dispatch::make_grave_stone_tile("Grave stone", false),
        );
        set(
            ids::BROKEN_GRAVE_STONE,
            dispatch::make_grave_stone_tile("Broken Grave Stone", true),
        );
        set(ids::FENCE, dispatch::make_fence_tile("Fence"));
        set(
            ids::DEEP_WATER,
            super::tile::depth::make_deep_water("Deep Water"),
        );
        set(ids::DUG_PIT, super::tile::depth::make_dug_pit("Dug Pit"));
        set(ids::CHASM, super::tile::depth::make_chasm("Chasm"));
        set(ids::LADDER, super::tile::depth::make_ladder("Ladder"));
        set(ids::MUD, super::tile::mud::make("Mud"));

        // flora wave (ids 51+): biome tree species, food flora, ocean life, reeds
        set(
            ids::PINE_TREE,
            dispatch::make_tree_species_tile("Pine Tree", TreeSpecies::Pine),
        );
        set(
            ids::DEAD_TREE,
            dispatch::make_tree_species_tile("Dead Tree", TreeSpecies::Dead),
        );
        set(
            ids::WILLOW,
            dispatch::make_tree_species_tile("Willow", TreeSpecies::Willow),
        );
        set(
            ids::PALM_TREE,
            dispatch::make_tree_species_tile("Palm Tree", TreeSpecies::Palm),
        );
        set(
            ids::FLAT_CROWN_TREE,
            dispatch::make_tree_species_tile("Flat-Crown Tree", TreeSpecies::FlatCrown),
        );
        set(
            ids::BERRY_BUSH,
            dispatch::make_berry_bush_tile("Berry Bush"),
        );
        set(ids::MUSHROOM, dispatch::make_mushroom_tile("Mushroom"));
        set(
            ids::FRUITING_CACTUS,
            dispatch::make_fruiting_cactus_tile("Fruiting Cactus"),
        );
        set(ids::SEAWEED, dispatch::make_seaweed_tile("Seaweed"));
        set(ids::CORAL, dispatch::make_coral_tile("Coral"));
        set(
            ids::REEDS,
            dispatch::make_tall_grass_tile("Reeds", "grass", 3),
        );
        // 62 = Jack-O-Lantern (registered next to pumpkin above)
        set(ids::DRY_BUSH, dispatch::make_dry_bush_tile("Dry Bush"));

        // tides: the intertidal band between ocean and beach (see tidal.rs)
        set(ids::TIDAL_FLAT, super::tile::tidal::make("Tidal Flat"));

        // fossicking: the mine-ceiling support post (see fossick.rs)
        set(
            ids::TIMBER_PROP,
            dispatch::make_timber_prop_tile("Timber Prop"),
        );

        // light & shelter: the glass-paned wall segment (see window.rs)
        set(ids::WINDOW, dispatch::make_window_tile("Window"));

        // biome identity: the Mountains highland ground (see heath.rs)
        set(ids::HEATH, dispatch::make_heath_tile("Heath"));

        // farming wave (ids 68+): row crops on farmland + the foraged wild carrot
        set(
            ids::CARROT_CROP,
            dispatch::make_crop_tile("Carrot Crop", crop::CropKind::Carrot),
        );
        set(
            ids::POTATO_CROP,
            dispatch::make_crop_tile("Potato Crop", crop::CropKind::Potato),
        );
        set(
            ids::CORN_CROP,
            dispatch::make_crop_tile("Corn Crop", crop::CropKind::Corn),
        );
        set(
            ids::PUMPKIN_VINE,
            dispatch::make_crop_tile("Pumpkin Vine", crop::CropKind::PumpkinVine),
        );
        set(
            ids::WILD_CARROT,
            dispatch::make_wild_carrot_tile("Wild Carrot"),
        );

        // content wave (ids 73+): hot-spring water, forest beehives, badlands ground
        set(
            ids::SPRING_WATER,
            dispatch::make_spring_water_tile("Spring Water"),
        );
        set(ids::BEEHIVE, dispatch::make_beehive_tile("Beehive"));
        set(ids::LAYERED_CLAY, dispatch::make_clay_tile("Layered Clay"));
        set(
            ids::ORE_FRECKLE,
            dispatch::make_ore_freckle_tile("Ore Freckle"),
        );

        // Built from the registered defs themselves, so the index can never disagree
        // with the table it indexes.
        let by_name = t
            .iter()
            .flatten()
            .map(|def| (def.name.clone(), def.id))
            .collect();

        Tiles {
            list: RefCell::new(t),
            by_name,
        }
    }

    /// The tile registered at `id`, or grass when the slot is empty.
    #[inline]
    fn slot(&self, id: u8) -> Rc<TileDef> {
        self.list.borrow()[id as usize]
            .clone()
            .expect("tile 0 must exist")
    }

    /// Fetch by interned id — an array index, no scan and no allocation.
    ///
    /// This is the form to reach for: `tiles.by_id(ids::ROCK)` is checked at compile
    /// time and costs a bounds check plus an `Rc` bump.
    #[inline]
    pub fn by_id(&self, id: TileId) -> Rc<TileDef> {
        self.get_id(id.raw() as i32)
    }

    /// Resolve a name to its interned id. `None` when nothing registers under it.
    pub fn id_of(&self, name: &str) -> Option<TileId> {
        let mut buf = [0u8; NORM_CAP];
        let (base, is_torch) = normalize_request(name, &mut buf);
        let id = TileId::new(*self.by_name.get(base.as_ref())?);
        Some(if is_torch { id.torch() } else { id })
    }

    /// Name lookup that admits failure — the boundary form.
    ///
    /// Names only genuinely arrive as text from save files and the dev console, and both
    /// would rather see `None` than silently receive grass. Code that knows which tile it
    /// wants should name it with an [`ids`] constant and call [`Tiles::by_id`].
    pub fn get_checked(&self, name: &str) -> Option<Rc<TileDef>> {
        let mut buf = [0u8; NORM_CAP];
        let (base, is_torch) = normalize_request(name, &mut buf);
        let def = self.slot(*self.by_name.get(base.as_ref())?);
        Some(if is_torch {
            self.get_torch_tile(def)
        } else {
            def
        })
    }

    /// Java `Tiles.get(name)` — handles "TORCH x" prefixes and "_data" suffixes.
    ///
    /// Kept for callers that still hold a runtime name; an unknown name warns and yields
    /// grass exactly as before. Prefer [`Tiles::get_checked`] at a real boundary and
    /// [`Tiles::by_id`] everywhere else.
    pub fn get(&self, name: &str) -> Rc<TileDef> {
        if let Some(t) = self.get_checked(name) {
            return t;
        }
        let mut buf = [0u8; NORM_CAP];
        let (base, is_torch) = normalize_request(name, &mut buf);
        println!("TILES.GET: invalid tile requested: {base}");
        let grass = self.slot(0);
        if is_torch {
            self.get_torch_tile(grass)
        } else {
            grass
        }
    }

    /// Java `Tiles.get(id)`.
    pub fn get_id(&self, id: i32) -> Rc<TileDef> {
        let mut id = id;
        if id < 0 {
            id += 256;
        }
        let existing = self.list.borrow()[id as usize].clone();
        if let Some(t) = existing {
            return t;
        }
        if id >= 128 {
            let on = self.get_id(id - 128);
            return self.get_torch_tile(on);
        }
        println!("TILES.GET: unknown tile id requested: {id}");
        self.list.borrow()[0].clone().expect("tile 0 must exist")
    }

    /// Java `TorchTile.getTorchTile(tile)` — fetch or create the torch version.
    pub fn get_torch_tile(&self, on: Rc<TileDef>) -> Rc<TileDef> {
        let torch_id = on.id as i32 + 128;
        if let Some(t) = self.list.borrow()[torch_id as usize].clone() {
            return t;
        }
        // Base tiles without torch support log a warning and reuse Dirt's torch config.
        let mut def = dispatch::make_torch_tile(&on);
        def.id = torch_id as u8;
        let def = Rc::new(def);
        self.list.borrow_mut()[torch_id as usize] = Some(def.clone());
        def
    }

    /// Java `Tiles.containsTile(id)`.
    pub fn contains_tile(&self, id: i32) -> bool {
        self.list.borrow()[id as usize].is_some()
    }

    /// Java `Tiles.getName(descriptName)` — resolves "name_data" to the display name.
    pub fn get_name(&self, descript_name: &str) -> String {
        if !descript_name.contains('_') {
            return descript_name.to_string();
        }
        let parts: Vec<&str> = descript_name.split('_').collect();
        let data: i32 = parts[1].parse().unwrap_or(0);
        dispatch::get_name(&self.get(parts[0]), data)
    }
}
