//! Port of `fdoom.entity.mob.GlowWorm`. Data + constructor; behavior in `glow_worm` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{compile_sprite_list, MobAnims, Sprite};

use super::PassiveMobData;

pub fn glowworm_col() -> i32 {
    color::get4(-1, -1, 222, 550)
}

// JAVA: a single 1x1 standing frame, duplicated.
static STANDING: LazyLock<Vec<Sprite>> = LazyLock::new(|| {
    let list = compile_sprite_list(26, 19, 1, 1, 0, 1);
    vec![list[0].clone(), list[0].clone()]
});
static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| vec![STANDING.clone()]);

#[derive(Debug, Clone)]
pub struct GlowWormData {
    pub passive: PassiveMobData,
}

/// Java `new GlowWorm()`.
pub fn new(g: &Game) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (passive, col) = PassiveMobData::new(&SPRITES, glowworm_col(), 3, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::GlowWorm(GlowWormData { passive }))
}
