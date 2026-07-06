//! Port of `fdoom.entity.mob.Snake`. Data + constructor; behavior in `snake` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};

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
    let (enemy, col) = EnemyMobData::simple(
        lvl,
        &SPRITES,
        &LVLCOLS,
        if lvl > 1 { 8 } else { 7 },
        100,
        diff_idx,
    );
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Snake(SnakeData { enemy }))
}

/// Java `Snake.tick()` — no override; `EnemyMob.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_tick_base(g, e);
}

/// Java `Snake.touchedBy(entity)` — damage is `lvl + diffIdx` (not EnemyMob's
/// `lvl * (hard ? 2 : 1)`), and it does NOT call super.
///
/// NOTE: `behavior::touched_by` currently routes Snake to the shared EnemyMob
/// touchedBy; it needs a `Snake` dispatch arm calling this function instead.
pub fn touched_by(g: &mut Game, this_e: &mut Entity, by: &mut Entity) {
    if by.is_player() {
        let lvl = this_e.enemy_mob().map(|em| em.lvl).unwrap_or(1);
        let damage = lvl + g.settings.get_idx("diff");
        let attack_dir = crate::entity::behavior::get_attack_dir(this_e, by);
        super::player_behavior::hurt_by_mob(g, by, this_e, damage, attack_dir);
    }
}

/// Java `Snake.die()`.
pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::entity::behavior::mobai_drop_items;
    use crate::item::registry;

    let num = if g.settings.get("diff").as_str() == "Hard" {
        1
    } else {
        0
    };
    let scale = registry::get(g, "scale");
    mobai_drop_items(g, e, num, num + 1, &[scale]);

    let lvl = e.enemy_mob().map(|em| em.lvl).unwrap_or(1);
    let diff_idx = g.settings.get_idx("diff");
    if g.random.next_int_bound(30 / lvl / (diff_idx + 1)) == 0 {
        let key = registry::get(g, "key");
        mobai_drop_items(g, e, 1, 1, &[key]);
    }

    crate::entity::behavior::enemy_mob_die(g, e);
}
