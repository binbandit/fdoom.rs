//! Port of `fdoom.level.tile` — the tile system.
//!
//! Java tile classes are stateless singletons configured at construction; here each is a
//! `TileDef` (config) with a `TileKind` (class identity + per-class config). Per-tile
//! *state* lives in the level's `tiles`/`data` byte arrays, as in Java. Behavior dispatch
//! (`tick`/`interact`/`hurt`/...) is in `dispatch.rs`, matching on `TileKind` and calling
//! into the per-tile modules.

pub mod cactus;
pub mod cloud;
pub mod cloud_cactus;
pub mod depth;
pub mod dirt;
pub mod dispatch;
pub mod door;
pub mod exploded;
pub mod farm;
pub mod fence;
pub mod floor;
pub mod flower;
pub mod grass;
pub mod grave_stone;
pub mod hard_rock;
pub mod hole;
pub mod infinite_fall;
pub mod lava;
pub mod lava_brick;
pub mod ore;
pub mod pumpkin;
pub mod quicksand;
pub mod rock;
pub mod sand;
pub mod sapling;
pub mod snow;
pub mod snow_tree;
pub mod stairs;
pub mod tall_grass;
pub mod torch;
pub mod tree;
pub mod wall;
pub mod water;
pub mod wheat;
pub mod wool;

use std::cell::RefCell;
use std::rc::Rc;

use crate::gfx::Sprite;

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
    Wool,
    QuickSand,
    Snow,
    SnowTree,
    TallGrass {
        kind: i32,
    },
    Pumpkin,
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
        // JAVA: TorchTile is only supported on certain tiles; others log and use Dirt's.
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
