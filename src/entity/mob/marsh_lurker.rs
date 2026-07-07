//! Marsh Lurker — an original fdoom.rs mob (no Java counterpart). An ambush predator
//! that lurks in marsh pools: quick in the water, sluggish on land, and its first
//! strike out of the water hits much harder than its follow-ups.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};

use super::{EnemyMobData, MovementStyle};

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(8, 14));

pub const LVLCOLS: [i32; 4] = [
    color::get4(-1, 10, 141, 330),
    color::get4(-1, 20, 252, 441),
    color::get4(-1, 30, 254, 552),
    color::get4(-1, 0, 131, 220),
];

/// Ticks a spent ambush takes to re-arm, and only while lurking in water.
const AMBUSH_RECHARGE: i32 = 300;
/// Extra damage dealt by an armed ambush strike.
const AMBUSH_BONUS: i32 = 2;

#[derive(Debug, Clone)]
pub struct MarshLurkerData {
    pub enemy: EnemyMobData,
    /// True while the opening ambush strike is armed.
    pub ambush_armed: bool,
    /// Countdown until the ambush re-arms (only decrements while swimming).
    pub ambush_recharge: i32,
}

pub fn new(g: &Game, lvl: i32) -> Entity {
    let lvl = lvl.clamp(1, LVLCOLS.len() as i32);
    let diff_idx = g.settings.get_idx("diff");
    // Short detect distance: it waits for prey to come close rather than roaming.
    let (mut enemy, col) = EnemyMobData::simple(lvl, &SPRITES, &LVLCOLS, 6, 80, diff_idx);
    // ambush gait: hold dead still, then burst
    enemy.ai.movement_style = MovementStyle::FreezeBurst;
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(
        c,
        EntityKind::MarshLurker(MarshLurkerData {
            enemy,
            ambush_armed: true,
            ambush_recharge: 0,
        }),
    )
}

pub fn tick(g: &mut Game, e: &mut Entity) {
    if !crate::entity::behavior::enemy_mob_tick_base(g, e) {
        return;
    }

    // fast in water, slow on land: swimming halves movement (Mob::move's swim gate),
    // so speed 2 in water nets full walking speed while speed 1 on land nets half.
    let swimming = crate::entity::behavior::is_swimming(g, e);
    if let Some(mob) = e.mob_mut() {
        mob.speed = if swimming { 2 } else { 1 };
    }

    // the ambush only re-arms while it lurks back underwater
    if let EntityKind::MarshLurker(d) = &mut e.kind {
        if !d.ambush_armed && swimming {
            d.ambush_recharge -= 1;
            if d.ambush_recharge <= 0 {
                d.ambush_armed = true;
            }
        }
    }
}

/// Ambush strike: an armed lurker's first touch hits for +`AMBUSH_BONUS` on top of the
/// standard EnemyMob formula, then must soak in water to re-arm.
pub fn touched_by(g: &mut Game, this_e: &mut Entity, by: &mut Entity) {
    if !by.is_player() {
        return;
    }
    let lvl = this_e.enemy_mob().map(|em| em.lvl).unwrap_or(1);
    let hard = g.settings.get("diff").as_str() == "Hard";
    let mut dmg = lvl * if hard { 2 } else { 1 };
    if let EntityKind::MarshLurker(d) = &mut this_e.kind {
        if d.ambush_armed {
            dmg += AMBUSH_BONUS;
            d.ambush_armed = false;
            d.ambush_recharge = AMBUSH_RECHARGE;
        }
    }
    let attack_dir = crate::entity::behavior::get_attack_dir(this_e, by);
    super::player_behavior::hurt_by_mob(g, by, this_e, dmg, attack_dir);
}

pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::entity::behavior::mobai_drop_items;
    use crate::item::registry;

    let fish = registry::get(g, "raw fish");
    mobai_drop_items(g, e, 0, 2, &[fish]);

    if g.random.next_int_bound(12) == 0 {
        let slime = registry::get(g, "Slime");
        mobai_drop_items(g, e, 1, 1, &[slime]);
    }

    crate::entity::behavior::enemy_mob_die(g, e);
}
