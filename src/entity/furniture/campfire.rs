//! Campfire (fire wave, no Java counterpart): a placeable, fueled fire. Lit it gives
//! warm light (radius 7 through the normal furniture-emitter path, so walls shadow
//! it), animates a two-frame flame, breathes smoke, doubles stamina regen for anyone
//! resting within [`REST_RADIUS_TILES`], cooks mushrooms, and — carelessly placed —
//! ignites adjacent flammable tiles (`level::tile::fire`). Out of fuel it drops to a
//! cold ember: no light, no smoke, relit by feeding it Wood.

use crate::entity::{Entity, EntityKind};
use crate::gfx::{Sprite, color};

use super::{FurnitureData, furniture_common};

/// One Wood burns for 4 in-game minutes (60 ticks/s).
pub const FUEL_PER_WOOD: i32 = 4 * 60 * 60;
/// A fresh campfire starts with its 2 crafting Wood already on the fire (~8 min).
pub const START_FUEL: i32 = 2 * FUEL_PER_WOOD;
/// The fire never holds more than 5 Wood worth of fuel.
pub const MAX_FUEL: i32 = 5 * FUEL_PER_WOOD;
/// Lit light radius (tiles) — brighter than a torch, dimmer than a good lantern.
pub const LIGHT_RADIUS: i32 = 7;
/// Players within this many tiles of a lit fire regain stamina at 2x.
pub const REST_RADIUS_TILES: i32 = 2;

#[derive(Debug, Clone)]
pub struct CampfireData {
    pub furniture: FurnitureData,
    /// Remaining burn ticks; 0 = cold ember.
    pub fuel: i32,
}

fn campfire_color() -> i32 {
    // true-color art ignores the palette; this drives EntityCommon.col only
    color::get4(-1, 100, 520, 550)
}

/// Lit flame, frame A (the canonical furniture sprite while lit).
pub fn lit_sprite() -> Sprite {
    Sprite::new(12, 20, 2, 2, campfire_color(), 0)
}

/// Lit flame, frame B (render-time animation frame; never stored).
pub fn lit_sprite_b() -> Sprite {
    Sprite::new(14, 20, 2, 2, campfire_color(), 0)
}

/// Cold ember ring (the furniture sprite while out of fuel).
pub fn ember_sprite() -> Sprite {
    Sprite::new(16, 20, 2, 2, campfire_color(), 0)
}

/// The held-item icon (cell (8,19), palette-mode: logs dark, flame orange/bright).
fn icon() -> Sprite {
    Sprite::new1x1(8, 19, color::get4(-1, 100, 520, 550))
}

fn build(fuel: i32) -> Entity {
    let sprite = if fuel > 0 {
        lit_sprite()
    } else {
        ember_sprite()
    };
    let mut furniture = FurnitureData::new("Campfire", sprite);
    furniture.icon = Some(icon());
    let c = furniture_common(furniture.sprite.color, 3, 2);
    Entity::new(c, EntityKind::Campfire(CampfireData { furniture, fuel }))
}

/// A freshly-built campfire, lit, holding its crafting wood.
pub fn new() -> Entity {
    build(START_FUEL)
}

/// A long-cold ember campfire (abandoned-camp structure spawns).
pub fn new_ember() -> Entity {
    build(0)
}
