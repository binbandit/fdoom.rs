//! Port of `fdoom.entity.mob.Zombie`. Data + constructor; behavior in `zombie` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{compile_mob_sprite_animations, MobAnims};

use super::EnemyMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(0, 14));

pub const LVLCOLS: [i32; 4] = [
    color::get4(-1, 10, 152, 40),
    color::get4(-1, 100, 522, 40),
    color::get4(-1, 111, 444, 40),
    color::get4(-1, 0, 111, 20),
];

#[derive(Debug, Clone)]
pub struct ZombieData {
    pub enemy: EnemyMobData,
}

/// Java `new Zombie(lvl)`.
pub fn new(g: &Game, lvl: i32) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (enemy, col) = EnemyMobData::simple(lvl, &SPRITES, &LVLCOLS, 5, 100, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Zombie(ZombieData { enemy }))
}

/// Java `zombie.tick()`. TODO(port:entity-behavior): leaf behavior.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_tick_base(g, e);
}

/// Java `zombie.die()`. TODO(port:entity-behavior): drops.
pub fn die(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_die(g, e);
}
