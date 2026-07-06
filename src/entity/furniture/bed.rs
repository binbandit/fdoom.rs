//! Port of `fdoom.entity.furniture.Bed`.
//!
//! Java's statics (`playersAwake`, `sleepingPlayers`) are game state; they live in
//! `g.bed_state` (see `BedState`).

use crate::entity::{Entity, EntityKind};
use crate::gfx::{color, Sprite};

use super::{furniture_common, FurnitureData};

/// Java `Bed`'s static sleep-tracking state; a `Game` field.
#[derive(Debug, Clone)]
pub struct BedState {
    pub players_awake: i32,
    /// player eid -> (level index, bed eid) (Java: `sleepingPlayers` map).
    pub sleeping_players: std::collections::HashMap<i32, (usize, i32)>,
}

impl Default for BedState {
    fn default() -> Self {
        BedState { players_awake: 1, sleeping_players: std::collections::HashMap::new() }
    }
}

#[derive(Debug, Clone)]
pub struct BedData {
    pub furniture: FurnitureData,
}

/// Java `new Bed()`.
pub fn new() -> Entity {
    let furniture = FurnitureData::new("Bed", Sprite::new(16, 8, 2, 2, color::get4(-1, 100, 444, 400), 0));
    let c = furniture_common(furniture.sprite.color, 3, 2);
    Entity::new(c, EntityKind::Bed(BedData { furniture }))
}
