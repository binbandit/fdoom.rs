//! Port of `fdoom.entity.mob.Snake`. Data + constructor; behavior in `snake` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{compile_mob_sprite_animations, MobAnims};

use super::EnemyMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(18, 18));

pub const LVLCOLS: [i32; 5] = [
    color::get4(-1, 0, 444, 30),
    color::get4(-1, 0, 555, 220),
    color::get4(-1, 0, 555, 5),
    color::get4(-1, 0, 555, 400),
    color::get4(-1, 0, 555, 459),
];

#[derive(Debug, Clone)]
pub struct SnakeData {
    pub enemy: EnemyMobData,
}

/// Java `new Snake(lvl)`.
pub fn new(g: &Game, lvl: i32) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (enemy, col) = EnemyMobData::simple(lvl, &SPRITES, &LVLCOLS, if lvl > 1 { 8 } else { 7 }, 100, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Snake(SnakeData { enemy }))
}

/// Java `snake.tick()`. TODO(port:entity-behavior): leaf behavior.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_tick_base(g, e);
}

/// Java `snake.die()`. TODO(port:entity-behavior): drops.
pub fn die(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_die(g, e);
}
