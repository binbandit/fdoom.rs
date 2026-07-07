//! Feral Hound — an original fdoom.rs mob (no Java counterpart). A plains/savanna pack
//! hunter: fast (full walking speed, unlike the usual half-speed mobs), fragile, and
//! spawned in packs of 2-3 (see `level::try_spawn`).

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};

use super::EnemyMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(8, 16));

pub const LVLCOLS: [i32; 4] = [
    color::get4(-1, 110, 322, 543),
    color::get4(-1, 100, 332, 554),
    color::get4(-1, 111, 333, 555),
    color::get4(-1, 0, 211, 433),
];

#[derive(Debug, Clone)]
pub struct FeralHoundData {
    pub enemy: EnemyMobData,
}

pub fn new(g: &Game, lvl: i32) -> Entity {
    let lvl = lvl.clamp(1, LVLCOLS.len() as i32);
    let diff_idx = g.settings.get_idx("diff");
    // Long detect distance (keen nose) but low health: dies to a couple of hits.
    let (mut enemy, col) = EnemyMobData::simple(lvl, &SPRITES, &LVLCOLS, 3, 120, diff_idx);
    // walk_time 1 = no skipped movement ticks: as fast as the player, twice a zombie.
    enemy.ai.mob.walk_time = 1;
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::FeralHound(FeralHoundData { enemy }))
}

pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_tick_base(g, e);
}

pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::entity::behavior::mobai_drop_items;
    use crate::item::registry;

    let leather = registry::get(g, "Leather");
    mobai_drop_items(g, e, 0, 1, &[leather]);

    if g.random.next_int_bound(20) == 0 {
        let beef = registry::get(g, "raw beef");
        mobai_drop_items(g, e, 1, 1, &[beef]);
    }

    crate::entity::behavior::enemy_mob_die(g, e);
}
