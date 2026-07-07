//! Ghost — an original fdoom.rs mob (no Java counterpart). Rises from broken grave
//! tiles at night (see `level::try_spawn` + [`try_rise`]); on a Hollow Night
//! (`core::events`) the graves mass-rise. It phases through terrain and entities
//! (the entity-side phase check in `behavior::entity_move2` — the wisp's tile-side
//! `may_pass` carve-out, generalized to the entity layer), drifts with a `SineFloat`
//! bob, and pulses between a translucent phase form and a brief *solid* form: it can
//! only be damaged during the solid pulse. Its touch chills — 3 stamina and 1 health.
//! Fades away at dawn like the Night Wisp. Drops nothing but a rare pinch of gem dust
//! (the gem item, 1/30).

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_sprite_list};

use super::{EnemyMobData, MovementStyle};

// Two 16x16 pulse frames at artgen cells (6,20) and (8,20): [phase, solid].
static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| vec![compile_sprite_list(6, 20, 2, 2, 0, 2)]);

/// Shades 0 AND 1 are transparent (wisp-style): the eye holes are drawn in shade 1 on
/// the sheet, so the night shows through them.
pub const LVLCOLS: [i32; 4] = [
    color::get4(-1, -1, 445, 555),
    color::get4(-1, -1, 455, 555),
    color::get4(-1, -1, 345, 555),
    color::get4(-1, -1, 545, 555),
];

/// Ticks of each pulse half (solid <-> phase).
const PULSE: i32 = 20;
/// Stamina drained by a touch, on top of the 1 health.
const TOUCH_STAMINA_DRAIN: i32 = 3;

#[derive(Debug, Clone)]
pub struct GhostData {
    pub enemy: EnemyMobData,
}

pub fn new(g: &Game, lvl: i32) -> Entity {
    let lvl = lvl.clamp(1, LVLCOLS.len() as i32);
    let diff_idx = g.settings.get_idx("diff");
    let (mut enemy, col) = EnemyMobData::simple(lvl, &SPRITES, &LVLCOLS, 4, 100, diff_idx);
    enemy.ai.movement_style = MovementStyle::SineFloat;
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Ghost(GhostData { enemy }))
}

/// Whether the ghost is in the *solid* half of its pulse — the only window in which
/// it can be hurt (checked by `behavior::do_hurt`) and the frame it renders brightest.
pub fn is_solid_pulse(tick_time: i32) -> bool {
    (tick_time / PULSE) % 2 == 0
}

/// Grave-rise helper used by `level::try_spawn` (and tests): scan a few tiles around
/// `(x, y)` (entity px) for a broken grave; if one is found and the spot passes the
/// ghost clearance gate, a ghost rises from the grave. Returns whether one rose.
pub fn try_rise(g: &mut Game, lvl: usize, x: i32, y: i32, mob_lvl: i32) -> bool {
    use crate::level::tile::TileKind;

    let (xt, yt) = (x >> 4, y >> 4);
    for dy in -4..=4 {
        for dx in -4..=4 {
            let tile = g.tile_at(lvl, xt + dx, yt + dy);
            if !matches!(tile.kind, TileKind::GraveStone { broken: true }) {
                continue;
            }
            let (gx, gy) = ((xt + dx) * 16 + 8, (yt + dy) * 16 + 8);
            if !crate::entity::behavior::ghost_check_start_pos(g, lvl, gx, gy) {
                continue;
            }
            let e = new(g, mob_lvl);
            g.level_mut(lvl).add_at(e, gx, gy, false, lvl);
            return true;
        }
    }
    false
}

pub fn tick(g: &mut Game, e: &mut Entity) {
    use crate::core::updater::Time;

    if !crate::entity::behavior::enemy_mob_tick_base(g, e) {
        return;
    }

    // dawn banishes it (like the Night Wisp); underground it would persist, though
    // nothing rises there today
    let on_surface = e.c.level.map(|lvl| g.level(lvl).depth == 0).unwrap_or(true);
    let time = g.get_time();
    if on_surface && !(time == Time::Night || time == Time::Evening) {
        crate::entity::behavior::remove_entity(g, e);
    }
}

/// Pulsing translucent render: the solid pulse draws bright and steady; the phase
/// pulse dims the palette and skips frames (flicker).
pub fn render(_g: &mut Game, screen: &mut crate::gfx::Screen, e: &mut Entity) {
    if let Some(em) = e.enemy_mob() {
        e.c.col = em.lvlcols[(em.lvl - 1) as usize];
    }
    let Some(mob) = e.mob() else { return };
    let solid = is_solid_pulse(mob.tick_time);

    if !solid && (mob.tick_time / 3) % 3 == 0 {
        return; // phase-form flicker: skip roughly every third 3-tick window
    }

    let col = if mob.hurt_time > 0 {
        color::WHITE
    } else if solid {
        e.c.col
    } else {
        // dimmed phase palette: pull the highlights down a step
        color::get4(-1, -1, 334, 445)
    };
    // the sine-float bob is echoed in the sprite so it reads even when drift is blocked
    let bob = if (mob.tick_time / 16) % 2 == 0 { 0 } else { 1 };
    let frame = if solid { 1 } else { 0 };
    SPRITES[0][frame].render_color(screen, e.c.x - 8, e.c.y - 11 + bob, col);
}

pub fn touched_by(g: &mut Game, this_e: &mut Entity, by: &mut Entity) {
    if !by.is_player() {
        return;
    }
    // gate the stamina drain on the same hurt-cooldown window as the health damage
    let drains = by.mob().map(|m| m.hurt_time).unwrap_or(1) == 0 && !g.is_mode("creative");
    let attack_dir = crate::entity::behavior::get_attack_dir(this_e, by);
    super::player_behavior::hurt_by_mob(g, by, this_e, 1, attack_dir);
    if drains {
        let pd = by.player_mut();
        pd.stamina = (pd.stamina - TOUCH_STAMINA_DRAIN).max(0);
    }
}

pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::entity::behavior::mobai_drop_items;
    use crate::item::registry;

    // a rare pinch of gem dust (the gem item stands in)
    if g.random.next_int_bound(30) == 0 {
        let gem = registry::get(g, "gem");
        mobai_drop_items(g, e, 1, 1, &[gem]);
    }

    crate::entity::behavior::enemy_mob_die(g, e);
}
