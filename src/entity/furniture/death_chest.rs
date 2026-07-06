//! Port of `fdoom.entity.furniture.DeathChest`.

use crate::core::game::Game;
use crate::core::updater::NORM_SPEED;
use crate::entity::{Entity, EntityKind};
use crate::gfx::color;

use super::chest::ChestData;
use super::furniture_common;

#[derive(Debug, Clone)]
pub struct DeathChestData {
    pub chest: ChestData,
    /// time passed (used for death chest despawn)
    pub time: i32,
    /// shade of red when the chest is about to expire
    pub redtick: i32,
    pub reverse: bool,
}

/// Java `new DeathChest()`.
pub fn new(g: &Game) -> Entity {
    let chest = ChestData::with_name("Death Chest", color::get4(-1, 220, 331, 552));
    // set the expiration time based on the world difficulty
    let time = match g.settings.get("diff").as_str() {
        "Easy" => 300 * NORM_SPEED,
        "Normal" => 120 * NORM_SPEED,
        "Hard" => 30 * NORM_SPEED,
        _ => 0,
    };
    let c = furniture_common(chest.furniture.sprite.color, 3, 3);
    Entity::new(
        c,
        EntityKind::DeathChest(DeathChestData {
            chest,
            time,
            redtick: 0,
            reverse: false,
        }),
    )
}
