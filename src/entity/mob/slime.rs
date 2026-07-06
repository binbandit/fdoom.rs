//! Port of `fdoom.entity.mob.Slime`. Data + constructor; behavior in `slime` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{compile_sprite_list, MobAnims};

use super::EnemyMobData;

// JAVA: slime sprites are a single row of two frames.
static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| vec![compile_sprite_list(0, 18, 2, 2, 0, 2)]);

pub const LVLCOLS: [i32; 4] = [
    color::get4(-1, 20, 40, 222),
    color::get4(-1, 100, 522, 555),
    color::get4(-1, 111, 444, 555),
    color::get4(-1, 0, 111, 224),
];

#[derive(Debug, Clone)]
pub struct SlimeData {
    pub enemy: EnemyMobData,
    /// jumpTimer, also acts as a rest timer before the next jump.
    pub jump_time: i32,
}

/// Java `new Slime(lvl)`.
pub fn new(g: &Game, lvl: i32) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (enemy, col) =
        EnemyMobData::with_default_lifetime(lvl, &SPRITES, &LVLCOLS, 1, true, 50, 60, 40, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Slime(SlimeData { enemy, jump_time: 0 }))
}

/// Java `slime.tick()`. TODO(port:entity-behavior): leaf behavior.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_tick_base(g, e);
}

/// Java `slime.die()`. TODO(port:entity-behavior): drops.
pub fn die(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_die(g, e);
}

/// TODO(port:entity-behavior): custom render.
pub fn render(g: &mut Game, screen: &mut crate::gfx::Screen, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_render(g, screen, e)
}
