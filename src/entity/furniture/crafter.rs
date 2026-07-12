//! Port of `fdoom.entity.furniture.Crafter`, plus THE BENCH (UI_REDESIGN §4): the
//! modular prospector's bench that absorbs the bench-shaped stations
//! (workbench/anvil/loom/enchanter). Heat stations (oven, furnace) stay separate —
//! fire is spatial, and villages generate ovens.

use crate::entity::{Entity, EntityKind};
use crate::gfx::{Sprite, color};

use super::{FurnitureData, furniture_common};

/// Java `Crafter.Type` + `Bench`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrafterType {
    Workbench,
    Oven,
    Furnace,
    Anvil,
    Enchanter,
    Loom,
    Bench,
}

impl CrafterType {
    pub const VALUES: [CrafterType; 7] = [
        CrafterType::Workbench,
        CrafterType::Oven,
        CrafterType::Furnace,
        CrafterType::Anvil,
        CrafterType::Enchanter,
        CrafterType::Loom,
        CrafterType::Bench,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CrafterType::Workbench => "Workbench",
            CrafterType::Oven => "Oven",
            CrafterType::Furnace => "Furnace",
            CrafterType::Anvil => "Anvil",
            CrafterType::Enchanter => "Enchanter",
            CrafterType::Loom => "Loom",
            CrafterType::Bench => "Bench",
        }
    }

    pub fn sprite(self) -> Sprite {
        match self {
            CrafterType::Workbench => Sprite::new(8, 8, 2, 2, color::get4(-1, 100, 321, 431), 0),
            CrafterType::Oven => Sprite::new(4, 8, 2, 2, color::get4(-1, 0, 332, 442), 0),
            CrafterType::Furnace => Sprite::new(6, 8, 2, 2, color::get4(-1, 0, 222, 333), 0),
            CrafterType::Anvil => Sprite::new(0, 8, 2, 2, color::get4(-1, 0, 222, 333), 0),
            CrafterType::Enchanter => Sprite::new(12, 8, 2, 2, color::get4(-1, 623, 999, 111), 0),
            CrafterType::Loom => Sprite::new(18, 8, 2, 2, color::get4(-1, 100, 333, 211), 0),
            // the workbench cells in aged-oak browns — TODO(art): dedicated cells
            // with visible module sockets along the top edge
            CrafterType::Bench => Sprite::new(8, 8, 2, 2, color::get4(-1, 110, 210, 432), 0),
        }
    }

    pub fn radius(self) -> (i32, i32) {
        match self {
            CrafterType::Enchanter | CrafterType::Loom => (7, 2),
            _ => (3, 2),
        }
    }
}

/// A bench module: a physical, holdable tool kit that bolts onto THE BENCH and
/// unlocks a recipe family forever on that bench (the SAW is built in).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Module {
    /// Absorbs the anvil list (metal tools and armor).
    Vice,
    /// Absorbs the loom list (wool, clothes, the bed).
    Spindle,
    /// Absorbs the enchanter list, reflavored as prospector's assaying.
    AssayKit,
}

impl Module {
    pub const VALUES: [Module; 3] = [Module::Vice, Module::Spindle, Module::AssayKit];

    /// The pack item that fits this socket.
    pub fn item_name(self) -> &'static str {
        match self {
            Module::Vice => "Vice",
            Module::Spindle => "Spindle",
            Module::AssayKit => "Assay Kit",
        }
    }

    /// The legacy bench-shaped station this module came out of (breakdown source).
    pub fn legacy_station(self) -> CrafterType {
        match self {
            Module::Vice => CrafterType::Anvil,
            Module::Spindle => CrafterType::Loom,
            Module::AssayKit => CrafterType::Enchanter,
        }
    }

    pub fn from_item_name(name: &str) -> Option<Module> {
        Module::VALUES
            .iter()
            .copied()
            .find(|m| m.item_name().eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, Clone)]
pub struct CrafterData {
    pub furniture: FurnitureData,
    pub crafter_type: CrafterType,
    /// Fitted bench modules (empty and meaningless for every other crafter type).
    pub modules: Vec<Module>,
}

/// Java `new Crafter(type)`.
pub fn new(crafter_type: CrafterType) -> Entity {
    let furniture = FurnitureData::new(crafter_type.name(), crafter_type.sprite());
    let (xr, yr) = crafter_type.radius();
    let c = furniture_common(furniture.sprite.color, xr, yr);
    Entity::new(
        c,
        EntityKind::Crafter(CrafterData {
            furniture,
            crafter_type,
            modules: Vec::new(),
        }),
    )
}
