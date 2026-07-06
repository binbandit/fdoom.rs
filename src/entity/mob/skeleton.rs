//! Port of `fdoom.entity.mob.Skeleton`. Data + constructor; behavior in `skeleton` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{compile_mob_sprite_animations, MobAnims};

use super::EnemyMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(8, 16));

pub const LVLCOLS: [i32; 4] = [
    color::get4(-1, 111, 40, 444),
    color::get4(-1, 100, 522, 555),
    color::get4(-1, 111, 444, 555),
    color::get4(-1, 0, 111, 555),
];

#[derive(Debug, Clone)]
pub struct SkeletonData {
    pub enemy: EnemyMobData,
    pub arrowtime: i32,
    pub artime: i32,
}

/// Java `new Skeleton(lvl)`.
pub fn new(g: &Game, lvl: i32) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (enemy, col) = EnemyMobData::with_default_lifetime(lvl, &SPRITES, &LVLCOLS, 6, true, 100, 45, 200, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Skeleton(SkeletonData { enemy, arrowtime: 0, artime: 0 }))
}
