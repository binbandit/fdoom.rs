//! Port of `fdoom.entity.mob.Pig`. Data + constructor; behavior in `pig` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{compile_mob_sprite_animations, MobAnims};

use super::PassiveMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(16, 14));

#[derive(Debug, Clone)]
pub struct PigData {
    pub passive: PassiveMobData,
}

/// Java `new Pig()`.
pub fn new(g: &Game) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (passive, col) = PassiveMobData::new(&SPRITES, color::get4(-1, 0, 555, 522), 3, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Pig(PigData { passive }))
}

/// Java `pig.tick()`. TODO(port:entity-behavior): leaf behavior.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::mobai_tick_base(g, e);
}

/// Java `pig.die()`. TODO(port:entity-behavior): drops.
pub fn die(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::passive_mob_die(g, e);
}
