//! Port of `fdoom.entity.mob.Knight`. Data + constructor; behavior in `knight` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};

use super::EnemyMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(24, 14));

pub const LVLCOLS: [i32; 5] = [
    color::get4(-1, 0, 555, 10),
    color::get4(-1, 0, 555, 220),
    color::get4(-1, 0, 555, 5),
    color::get4(-1, 0, 555, 400),
    color::get4(-1, 0, 555, 459),
];

#[derive(Debug, Clone)]
pub struct KnightData {
    pub enemy: EnemyMobData,
}

/// Java `new Knight(lvl)`.
pub fn new(g: &Game, lvl: i32) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (enemy, col) = EnemyMobData::simple(lvl, &SPRITES, &LVLCOLS, 9, 100, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Knight(KnightData { enemy }))
}

/// Java `Knight.tick()` — no override; `EnemyMob.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_tick_base(g, e);
}

/// Java `Knight.die()`.
pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::entity::behavior::mobai_drop_items;
    use crate::item::registry;

    let shard = registry::get(g, "shard");
    if g.settings.get("diff").as_str() == "Easy" {
        mobai_drop_items(g, e, 1, 3, &[shard]);
    } else {
        mobai_drop_items(g, e, 0, 2, &[shard]);
    }

    let lvl = e.enemy_mob().map(|em| em.lvl).unwrap_or(1);
    let diff_idx = g.settings.get_idx("diff");
    if g.random.next_int_bound(30 / lvl / (diff_idx + 1)) == 0 {
        let key = registry::get(g, "key");
        mobai_drop_items(g, e, 1, 1, &[key]);
    }

    crate::entity::behavior::enemy_mob_die(g, e);
}
