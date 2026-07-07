//! Port of `fdoom.entity.mob.Zombie`. Data + constructor; behavior in `zombie` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};

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
    // clamp to the supported color range: grave-stone interacts request level 5,
    // one past the color table
    let lvl = lvl.clamp(1, LVLCOLS.len() as i32);
    let diff_idx = g.settings.get_idx("diff");
    let (enemy, col) = EnemyMobData::simple(lvl, &SPRITES, &LVLCOLS, 5, 100, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Zombie(ZombieData { enemy }))
}

/// Java `Zombie.tick()` — just `super.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_tick_base(g, e);
}

/// Java `Zombie.die()`.
pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::entity::behavior::mobai_drop_items;
    use crate::item::registry;

    let diff = g.settings.get("diff").as_str().to_string();
    if diff == "Easy" {
        let cloth = registry::get(g, "cloth");
        mobai_drop_items(g, e, 2, 4, &[cloth]);
    }
    if diff == "Normal" {
        let cloth = registry::get(g, "cloth");
        mobai_drop_items(g, e, 2, 3, &[cloth]);
    }
    if diff == "Hard" {
        let cloth = registry::get(g, "cloth");
        mobai_drop_items(g, e, 1, 2, &[cloth]);
    }

    if g.random.next_int_bound(60) == 2 {
        if let Some(lvl) = e.c.level {
            let iron = registry::get(g, "iron");
            crate::level::drop_item(g, lvl, e.c.x, e.c.y, iron);
        }
    }

    if g.random.next_int_bound(40) == 19 {
        let rand = g.random.next_int_bound(3);
        if let Some(lvl) = e.c.level {
            if rand == 0 {
                let item = registry::get(g, "green clothes");
                crate::level::drop_item(g, lvl, e.c.x, e.c.y, item);
            } else if rand == 1 {
                let item = registry::get(g, "red clothes");
                crate::level::drop_item(g, lvl, e.c.x, e.c.y, item);
            } else if rand == 2 {
                let item = registry::get(g, "blue clothes");
                crate::level::drop_item(g, lvl, e.c.x, e.c.y, item);
            }
        }
    }

    crate::entity::behavior::enemy_mob_die(g, e);
}
