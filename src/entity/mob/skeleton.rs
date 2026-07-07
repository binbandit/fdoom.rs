//! Port of `fdoom.entity.mob.Skeleton`. Data + constructor; behavior in `skeleton` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};

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
    // FIX: clamp to the lvlcols range — Java indexed lvlcols[lvl-1] unchecked and an
    // out-of-range level (e.g. from a hand-edited save) crashed the game.
    let lvl = lvl.clamp(1, LVLCOLS.len() as i32);
    let diff_idx = g.settings.get_idx("diff");
    let (enemy, col) = EnemyMobData::with_default_lifetime(
        lvl, &SPRITES, &LVLCOLS, 6, true, 100, 45, 200, diff_idx,
    );
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    // JAVA: arrowtime = 500 / (lvl + 5), from the raw constructor arg (before the 0->1
    // clamp; a negative arg could divide by zero). FIX: computed from the clamped level.
    let arrowtime = 500 / (lvl + 5);
    Entity::new(
        c,
        EntityKind::Skeleton(SkeletonData {
            enemy,
            arrowtime,
            artime: arrowtime,
        }),
    )
}

fn data_mut(e: &mut Entity) -> &mut SkeletonData {
    match &mut e.kind {
        EntityKind::Skeleton(d) => d,
        _ => panic!("entity is not a skeleton"),
    }
}

/// Java `Skeleton.tick()` — shoots arrows at a nearby player.
pub fn tick(g: &mut Game, e: &mut Entity) {
    use crate::entity::behavior::get_closest_player;

    if !crate::entity::behavior::enemy_mob_tick_base(g, e) {
        return;
    }

    // Java `skipTick()` (private in MobAi): slowtick && (tickTime+1) % 4 == 0.
    {
        let Some(ai) = e.mob_ai() else { return };
        if ai.slowtick && (ai.mob.tick_time + 1) % 4 == 0 {
            return;
        }
    }

    if let Some(pid) = get_closest_player(g, e) {
        let random_walk_time = e.mob_ai().map(|ai| ai.random_walk_time).unwrap_or(0);
        if random_walk_time == 0 {
            data_mut(e).artime -= 1;

            if let Some(player) = g.entities.get(pid) {
                let xd = player.c.x - e.c.x;
                let yd = player.c.y - e.c.y;
                if xd * xd + yd * yd < 100 * 100 {
                    let d = data_mut(e);
                    if d.artime < 1 {
                        let arrowtime = d.arrowtime;
                        d.artime = arrowtime;
                        let dir = e
                            .mob()
                            .map(|m| m.dir)
                            .unwrap_or(crate::entity::Direction::Down);
                        let lvl_dmg = e.enemy_mob().map(|em| em.lvl).unwrap_or(1);
                        if let Some(lvl) = e.c.level {
                            let arrow = crate::entity::projectile::new_arrow(
                                e.c.eid, e.c.x, e.c.y, dir, lvl_dmg,
                            );
                            g.level_mut(lvl).add(arrow, lvl);
                        }
                    }
                }
            }
        }
    }
}

/// Java `Skeleton.die()`.
pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::item::registry;

    let diffrands = [20, 20, 30];
    let diffvals = [13, 18, 28];
    let diff = g.settings.get_idx("diff");

    let count = g.random.next_int_bound(3 - diff) + 1;
    let bookcount = g.random.next_int_bound(1) + 1;
    let rand = g.random.next_int_bound(diffrands[diff as usize]);
    if let Some(lvl) = e.c.level {
        if rand <= diffvals[diff as usize] {
            // Java level.dropItem(x, y, count, bone, arrow) — each item `count` times.
            for _ in 0..count {
                let bone = registry::get(g, "bone");
                crate::level::drop_item(g, lvl, e.c.x, e.c.y, bone);
                let arrow = registry::get(g, "arrow");
                crate::level::drop_item(g, lvl, e.c.x, e.c.y, arrow);
            }
        } else if diff == 0 && rand >= 19 {
            // JAVA: rare chance of 10 arrows on easy mode
            for _ in 0..10 {
                let arrow = registry::get(g, "arrow");
                crate::level::drop_item(g, lvl, e.c.x, e.c.y, arrow);
            }
        } else {
            for _ in 0..bookcount {
                let book = registry::get(g, "Antidious");
                crate::level::drop_item(g, lvl, e.c.x, e.c.y, book);
                let arrow = registry::get(g, "arrow");
                crate::level::drop_item(g, lvl, e.c.x, e.c.y, arrow);
            }
        }
    }

    crate::entity::behavior::enemy_mob_die(g, e);
}
