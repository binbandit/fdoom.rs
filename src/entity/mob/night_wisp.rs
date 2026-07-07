//! Night Wisp — an original fdoom.rs mob (no Java counterpart). A nocturnal floating
//! light that drifts over any terrain (`tiles::may_pass`/`bumped_into` let it through,
//! the way the removed AirWizard's flight tiles did), fades away at dawn like the
//! GlowWorm, and harasses the player with ranged zaps (the `Zap` projectile — the old
//! AirWizard Spark, renamed and re-owned).

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_sprite_list};

use super::EnemyMobData;

// A single row of two 16x16 pulse frames (like the old Slime sheet shape).
static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| vec![compile_sprite_list(0, 20, 2, 2, 0, 2)]);

pub const LVLCOLS: [i32; 4] = [
    color::get4(-1, -1, 445, 555),
    color::get4(-1, -1, 345, 455),
    color::get4(-1, -1, 145, 355),
    color::get4(-1, -1, 435, 545),
];

/// Zap when the player is within this many pixels (8 tiles).
const ZAP_RANGE: i32 = 8 * 16;
/// Base ticks between zaps (plus a random 0..60 spread).
const ZAP_COOLDOWN: i32 = 90;
/// Zap projectile speed in px/tick.
const ZAP_SPEED: f64 = 1.5;

#[derive(Debug, Clone)]
pub struct NightWispData {
    pub enemy: EnemyMobData,
    /// Ticks until the next zap.
    pub zap_cooldown: i32,
}

pub fn new(g: &Game, lvl: i32) -> Entity {
    let lvl = lvl.clamp(1, LVLCOLS.len() as i32);
    let diff_idx = g.settings.get_idx("diff");
    let (enemy, col) = EnemyMobData::simple(lvl, &SPRITES, &LVLCOLS, 2, 100, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(
        c,
        EntityKind::NightWisp(NightWispData {
            enemy,
            zap_cooldown: ZAP_COOLDOWN,
        }),
    )
}

pub fn tick(g: &mut Game, e: &mut Entity) {
    use crate::core::updater::Time;

    if !crate::entity::behavior::enemy_mob_tick_base(g, e) {
        return;
    }

    // like the GlowWorm, it only exists in the dark (surface); underground it persists
    let on_surface = e.c.level.map(|lvl| g.level(lvl).depth == 0).unwrap_or(true);
    let time = g.get_time();
    if on_surface && !(time == Time::Night || time == Time::Evening) {
        crate::entity::behavior::remove_entity(g, e);
        return;
    }

    // ranged zap at the closest player in range
    let ready = {
        let EntityKind::NightWisp(d) = &mut e.kind else {
            return;
        };
        if d.zap_cooldown > 0 {
            d.zap_cooldown -= 1;
        }
        d.zap_cooldown == 0
    };
    if !ready {
        return;
    }
    let Some(pid) = crate::entity::behavior::get_closest_player(g, e) else {
        return;
    };
    let Some((px, py)) = g.entities.get(pid).map(|p| (p.c.x, p.c.y)) else {
        return;
    };
    let (xd, yd) = ((px - e.c.x) as f64, (py - e.c.y) as f64);
    let dist = xd.hypot(yd);
    if dist as i32 > ZAP_RANGE || dist == 0.0 {
        return;
    }
    if let Some(lvl) = e.c.level {
        let zap = crate::entity::projectile::new_zap(
            e.c.eid,
            e.c.x,
            e.c.y,
            xd / dist * ZAP_SPEED,
            yd / dist * ZAP_SPEED,
            &mut g.random,
        );
        g.level_mut(lvl).add(zap, lvl);
    }
    if let EntityKind::NightWisp(d) = &mut e.kind {
        d.zap_cooldown = ZAP_COOLDOWN + g.random.next_int_bound(60);
    }
}

/// Pulsing render: alternates the two frames on a timer (it "walks" without terrain
/// friction, so `walk_dist` alone animates poorly while hovering in place).
pub fn render(_g: &mut Game, screen: &mut crate::gfx::Screen, e: &mut Entity) {
    if let Some(em) = e.enemy_mob() {
        e.c.col = em.lvlcols[(em.lvl - 1) as usize];
    }
    let Some(mob) = e.mob() else { return };
    let col = if mob.hurt_time > 0 {
        color::WHITE
    } else {
        e.c.col
    };
    let frame = ((mob.tick_time >> 4) & 1) as usize;
    let sprite = &SPRITES[0][frame];
    sprite.render_color(screen, e.c.x - 8, e.c.y - 11, col);
}

pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::entity::behavior::mobai_drop_items;
    use crate::item::registry;

    if g.random.next_int_bound(25) == 0 {
        let gem = registry::get(g, "gem");
        mobai_drop_items(g, e, 1, 1, &[gem]);
    }

    crate::entity::behavior::enemy_mob_die(g, e);
}
