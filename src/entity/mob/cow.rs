//! Port of `fdoom.entity.mob.Cow`. Data + constructor; behavior in `cow` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{compile_mob_sprite_animations, MobAnims};

use super::PassiveMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(16, 16));

#[derive(Debug, Clone)]
pub struct CowData {
    pub passive: PassiveMobData,
}

/// Java `new Cow()`.
pub fn new(g: &Game) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (passive, col) = PassiveMobData::new(&SPRITES, color::get4(-1, 0, 333, 322), 5, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Cow(CowData { passive }))
}

/// Java `cow.tick()`. TODO(port:entity-behavior): leaf behavior.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::mobai_tick_base(g, e);
}

/// Java `cow.die()`. TODO(port:entity-behavior): drops.
pub fn die(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::passive_mob_die(g, e);
}
