//! Port of `fdoom.level.tile` — the tile system.
//!
//! Java tile classes are stateless singletons configured at construction; here each is a
//! `TileDef` (config) with a `TileKind` (class identity + per-class config). Per-tile
//! *state* lives in the level's `tiles`/`data` byte arrays, as in Java. Behavior dispatch
//! (`tick`/`interact`/`hurt`/...) is in `dispatch.rs`, matching on `TileKind` and calling
//! into the per-tile modules.

pub mod berry_bush;
pub mod cactus;
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

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::game::Game;
use crate::entity::Entity;
use crate::entity::mob::player_behavior::pay_stamina;
use crate::gfx::Sprite;
use crate::item::{Item, ItemKind, ToolType};

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

    /// Java `Tile.equals` — by name.
    pub fn same_tile(&self, other: &TileDef) -> bool {
        self.name == other.name
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
        on_type: String,
        grows_to: String,
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
    GraveStone {
        broken: bool,
    },
    Fence,
    /// Java `TorchTile` — wraps the tile it stands on; registered dynamically at
    /// `onType.id + 128`.
    Torch {
        on_type: String,
    },
}

/// Java `Tiles` — the tile registry. Interior mutability because torch tiles register
/// on demand (Java `Tiles.add` from `TorchTile.getTorchTile`).
pub struct Tiles {
    list: RefCell<Vec<Option<Rc<TileDef>>>>,
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

        let mut set = |id: usize, def: TileDef| {
            let mut def = def;
            def.id = id as u8;
            t[id] = Some(Rc::new(def));
        };

        set(0, dispatch::make_grass_tile("Grass"));
        set(1, dispatch::make_dirt_tile("Dirt"));
        set(2, dispatch::make_flower_tile("Flower"));
        set(3, dispatch::make_hole_tile("Hole"));
        set(4, dispatch::make_stairs_tile("Stairs Up", true));
        set(5, dispatch::make_stairs_tile("Stairs Down", false));
        set(6, dispatch::make_water_tile("Water"));
        set(7, dispatch::make_rock_tile("Rock"));
        set(8, dispatch::make_tree_tile("Tree"));
        set(
            9,
            dispatch::make_sapling_tile("Tree Sapling", "Grass", "Tree"),
        );
        set(10, dispatch::make_sand_tile("Sand"));
        set(11, dispatch::make_cactus_tile("Cactus"));
        set(
            12,
            dispatch::make_sapling_tile("Cactus Sapling", "Sand", "Cactus"),
        );
        set(17, dispatch::make_lava_tile("Lava"));
        set(18, dispatch::make_lava_brick_tile("Lava Brick"));
        set(13, dispatch::make_ore_tile(OreType::Iron));
        set(14, dispatch::make_ore_tile(OreType::Gold));
        set(15, dispatch::make_ore_tile(OreType::Gem));
        set(16, dispatch::make_ore_tile(OreType::Lapis));
        set(19, dispatch::make_exploded_tile("Explode"));
        set(20, dispatch::make_farm_tile("Farmland"));
        set(21, dispatch::make_wheat_tile("Wheat"));
        set(22, dispatch::make_hard_rock_tile("Hard Rock"));
        set(23, dispatch::make_infinite_fall_tile("Infinite Fall"));
        set(24, dispatch::make_cloud_tile("Cloud"));
        set(25, dispatch::make_cloud_cactus_tile("Cloud Cactus"));
        set(29, dispatch::make_floor_tile(Material::Wood));
        set(30, dispatch::make_floor_tile(Material::Stone));
        set(31, dispatch::make_floor_tile(Material::Obsidian));
        set(32, dispatch::make_wall_tile(Material::Wood));
        set(33, dispatch::make_wall_tile(Material::Stone));
        set(34, dispatch::make_wall_tile(Material::Obsidian));
        set(26, dispatch::make_door_tile(Material::Wood));
        set(27, dispatch::make_door_tile(Material::Stone));
        set(28, dispatch::make_door_tile(Material::Obsidian));
        set(35, dispatch::make_wool_tile());
        set(36, dispatch::make_quicksand_tile("Quick Sand"));
        set(37, dispatch::make_snow_tile("Snow"));
        set(38, dispatch::make_snow_tree_tile("Snow Tree"));
        set(
            39,
            dispatch::make_tall_grass_tile("Small Grass", "grass", 0),
        );
        set(
            40,
            dispatch::make_tall_grass_tile("Medium Grass", "grass", 1),
        );
        set(41, dispatch::make_tall_grass_tile("Tall Grass", "grass", 2));
        set(42, dispatch::make_pumpkin_tile("pumpkin", false));
        set(62, dispatch::make_pumpkin_tile("Jack-O-Lantern", true));
        set(43, dispatch::make_grave_stone_tile("Grave stone", false));
        set(
            44,
            dispatch::make_grave_stone_tile("Broken Grave Stone", true),
        );
        set(45, dispatch::make_fence_tile("Fence"));
        set(46, super::tile::depth::make_deep_water("Deep Water"));
        set(47, super::tile::depth::make_dug_pit("Dug Pit"));
        set(48, super::tile::depth::make_chasm("Chasm"));
        set(49, super::tile::depth::make_ladder("Ladder"));
        set(50, super::tile::mud::make("Mud"));

        // flora wave (ids 51+): biome tree species, food flora, ocean life, reeds
        set(
            51,
            dispatch::make_tree_species_tile("Pine Tree", TreeSpecies::Pine),
        );
        set(
            52,
            dispatch::make_tree_species_tile("Dead Tree", TreeSpecies::Dead),
        );
        set(
            53,
            dispatch::make_tree_species_tile("Willow", TreeSpecies::Willow),
        );
        set(
            54,
            dispatch::make_tree_species_tile("Palm Tree", TreeSpecies::Palm),
        );
        set(
            55,
            dispatch::make_tree_species_tile("Flat-Crown Tree", TreeSpecies::FlatCrown),
        );
        set(56, dispatch::make_berry_bush_tile("Berry Bush"));
        set(57, dispatch::make_mushroom_tile("Mushroom"));
        set(58, dispatch::make_fruiting_cactus_tile("Fruiting Cactus"));
        set(59, dispatch::make_seaweed_tile("Seaweed"));
        set(60, dispatch::make_coral_tile("Coral"));
        set(61, dispatch::make_tall_grass_tile("Reeds", "grass", 3));
        // 62 = Jack-O-Lantern (registered next to pumpkin above)
        set(63, dispatch::make_dry_bush_tile("Dry Bush"));

        // tides: the intertidal band between ocean and beach (see tidal.rs)
        set(64, super::tile::tidal::make("Tidal Flat"));

        // fossicking: the mine-ceiling support post (see fossick.rs)
        set(65, dispatch::make_timber_prop_tile("Timber Prop"));

        // light & shelter: the glass-paned wall segment (see window.rs)
        set(66, dispatch::make_window_tile("Window"));

        // biome identity: the Mountains highland ground (see heath.rs)
        set(67, dispatch::make_heath_tile("Heath"));

        // farming wave (ids 68+): row crops on farmland + the foraged wild carrot
        set(
            68,
            dispatch::make_crop_tile("Carrot Crop", crop::CropKind::Carrot),
        );
        set(
            69,
            dispatch::make_crop_tile("Potato Crop", crop::CropKind::Potato),
        );
        set(
            70,
            dispatch::make_crop_tile("Corn Crop", crop::CropKind::Corn),
        );
        set(
            71,
            dispatch::make_crop_tile("Pumpkin Vine", crop::CropKind::PumpkinVine),
        );
        set(72, dispatch::make_wild_carrot_tile("Wild Carrot"));
        Tiles {
            list: RefCell::new(t),
        }
    }

    /// Java `Tiles.get(name)` — handles "TORCH x" prefixes and "_data" suffixes.
    pub fn get(&self, name: &str) -> Rc<TileDef> {
        let mut name = name.to_uppercase();

        let mut is_torch = false;
        if let Some(stripped) = name.strip_prefix("TORCH ") {
            is_torch = true;
            name = stripped.to_string();
        }

        if let Some(idx) = name.find('_') {
            name.truncate(idx);
        }

        let getting = self
            .list
            .borrow()
            .iter()
            .flatten()
            .find(|t| t.name == name)
            .cloned();

        let getting = match getting {
            Some(t) => t,
            None => {
                println!("TILES.GET: invalid tile requested: {name}");
                self.list.borrow()[0].clone().expect("tile 0 must exist")
            }
        };

        if is_torch {
            self.get_torch_tile(getting)
        } else {
            getting
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
