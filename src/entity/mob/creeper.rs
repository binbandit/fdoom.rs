//! Port of `fdoom.entity.mob.Creeper`. Data + constructor; behavior in `creeper` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{compile_sprite_list, MobAnims, Sprite};

use super::EnemyMobData;

// JAVA: list of 3 frames; walking = {1, 2}, standing = {0, 0}; sprites = [standing].
static LIST: LazyLock<Vec<Sprite>> = LazyLock::new(|| compile_sprite_list(4, 18, 2, 2, 0, 3));
pub static WALKING: LazyLock<Vec<Sprite>> = LazyLock::new(|| vec![LIST[1].clone(), LIST[2].clone()]);
pub static STANDING: LazyLock<Vec<Sprite>> = LazyLock::new(|| vec![LIST[0].clone(), LIST[0].clone()]);
static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| vec![STANDING.clone()]);

pub const LVLCOLS: [i32; 4] = [
    color::get4(-1, 20, 40, 30),
    color::get4(-1, 200, 262, 232),
    color::get4(-1, 200, 272, 222),
    color::get4(-1, 200, 292, 282),
];

pub const MAX_FUSE_TIME: i32 = 60;
pub const BLAST_RADIUS: i32 = 60;
pub const BLAST_DAMAGE: i32 = 10;

#[derive(Debug, Clone)]
pub struct CreeperData {
    pub enemy: EnemyMobData,
    pub fuse_time: i32,
    pub fuse_lit: bool,
}

/// Java `new Creeper(lvl)`.
pub fn new(g: &Game, lvl: i32) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (enemy, col) = EnemyMobData::simple(lvl, &SPRITES, &LVLCOLS, 10, 50, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Creeper(CreeperData { enemy, fuse_time: 0, fuse_lit: false }))
}

/// Java `creeper.tick()`. TODO(port:entity-behavior): leaf behavior.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_tick_base(g, e);
}

/// Java `creeper.die()`. TODO(port:entity-behavior): drops.
pub fn die(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_die(g, e);
}

/// TODO(port:entity-behavior): custom render.
pub fn render(g: &mut Game, screen: &mut crate::gfx::Screen, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_render(g, screen, e)
}

/// TODO(port:entity-behavior): custom touchedBy.
pub fn touched_by(g: &mut Game, this_e: &mut Entity, by: &mut Entity) {
    let _ = (g, this_e, by);
}
