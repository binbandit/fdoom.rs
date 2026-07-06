//! Port of `fdoom.entity.mob.GlowWorm`. Data + constructor; behavior in `glow_worm` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, Sprite, compile_sprite_list};

use super::PassiveMobData;

pub fn glowworm_col() -> i32 {
    color::get4(-1, -1, 222, 550)
}

// JAVA: a single 1x1 standing frame, duplicated.
static STANDING: LazyLock<Vec<Sprite>> = LazyLock::new(|| {
    let list = compile_sprite_list(26, 19, 1, 1, 0, 1);
    vec![list[0].clone(), list[0].clone()]
});
static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| vec![STANDING.clone()]);

#[derive(Debug, Clone)]
pub struct GlowWormData {
    pub passive: PassiveMobData,
}

/// Java `new GlowWorm()`.
pub fn new(g: &Game) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (passive, col) = PassiveMobData::new(&SPRITES, glowworm_col(), 3, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::GlowWorm(GlowWormData { passive }))
}

/// Java `GlowWorm.tick()` — removes itself outside of evening/night.
pub fn tick(g: &mut Game, e: &mut Entity) {
    use crate::core::updater::Time;

    if !crate::entity::behavior::mobai_tick_base(g, e) {
        return;
    }

    let time = g.get_time();
    if !(time == Time::Night || time == Time::Evening) {
        crate::entity::behavior::remove_entity(g, e);
    }
}

/// Java `GlowWorm.die()` — no override; `PassiveMob.die()`.
pub fn die(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::passive_mob_die(g, e);
}

/// Java `GlowWorm.render(screen)` — always the single standing sprite.
pub fn render(_g: &mut Game, screen: &mut crate::gfx::Screen, e: &mut Entity) {
    let xo = e.c.x - 8;
    let yo = e.c.y - 11;

    let mut col = e.c.col;
    if e.mob().map(|m| m.hurt_time).unwrap_or(0) > 0 {
        col = color::WHITE;
    }

    let cur_sprite = &SPRITES[0][0];
    cur_sprite.render_color(screen, xo, yo, col);
}
