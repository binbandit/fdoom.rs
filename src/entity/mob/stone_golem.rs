//! Stone Golem — an original fdoom.rs mob (no Java counterpart). A mine-dweller:
//! very slow, very tough, hits like a rockslide, and crumbles into stone and ore.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};

use super::EnemyMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(0, 18));

pub const LVLCOLS: [i32; 4] = [
    color::get4(-1, 111, 333, 444),
    color::get4(-1, 100, 323, 445),
    color::get4(-1, 110, 331, 553),
    color::get4(-1, 0, 222, 335),
];

#[derive(Debug, Clone)]
pub struct StoneGolemData {
    pub enemy: EnemyMobData,
}

pub fn new(g: &Game, lvl: i32) -> Entity {
    let lvl = lvl.clamp(1, LVLCOLS.len() as i32);
    let diff_idx = g.settings.get_idx("diff");
    // Tanky (more than double a Zombie's base health) with a short wake-up radius:
    // it guards its patch of the mines rather than roaming after the player.
    let (enemy, col) = EnemyMobData::simple(lvl, &SPRITES, &LVLCOLS, 12, 60, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::StoneGolem(StoneGolemData { enemy }))
}

pub fn tick(g: &mut Game, e: &mut Entity) {
    if !crate::entity::behavior::enemy_mob_tick_base(g, e) {
        return;
    }
    // Heavy tread: on top of the shared walk_time=2 gate (which skips movement on even
    // ticks), zero the chase acceleration outside a 1-in-4 tick window so the golem
    // moves at half a normal mob's pace. (tick_time % 4 == 3 leaves the acceleration
    // set going into the next odd tick, the only tick walk_time lets move.)
    let stall = e.mob().map(|m| m.tick_time % 4 != 3).unwrap_or(false);
    if stall {
        if let Some(ai) = e.mob_ai_mut() {
            ai.xa = 0;
            ai.ya = 0;
        }
    }
}

/// Heavy melee: `2*lvl + diff_idx` per hit instead of EnemyMob's `lvl * (hard ? 2 : 1)`.
pub fn touched_by(g: &mut Game, this_e: &mut Entity, by: &mut Entity) {
    if by.is_player() {
        let lvl = this_e.enemy_mob().map(|em| em.lvl).unwrap_or(1);
        let dmg = 2 * lvl + g.settings.get_idx("diff");
        let attack_dir = crate::entity::behavior::get_attack_dir(this_e, by);
        super::player_behavior::hurt_by_mob(g, by, this_e, dmg, attack_dir);
    }
}

pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::entity::behavior::mobai_drop_items;
    use crate::item::registry;

    let stone = registry::get(g, "Stone");
    mobai_drop_items(g, e, 1, 3, &[stone]);

    let coal = registry::get(g, "Coal");
    mobai_drop_items(g, e, 0, 2, &[coal]);

    if g.random.next_int_bound(8) == 0 {
        let iron = registry::get(g, "iron");
        mobai_drop_items(g, e, 1, 1, &[iron]);
    }
    if g.random.next_int_bound(20) == 0 {
        let gold = registry::get(g, "gold");
        mobai_drop_items(g, e, 1, 1, &[gold]);
    }

    crate::entity::behavior::enemy_mob_die(g, e);
}
