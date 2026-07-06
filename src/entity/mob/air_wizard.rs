//! Port of `fdoom.entity.mob.AirWizard`. Data + constructor; behavior in `air_wizard` fns.
//!
//! Java's static `AirWizard.beaten` lives on `Game` (`g.air_wizard_beaten`).

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{compile_mob_sprite_animations, MobAnims};

use super::EnemyMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(8, 14));

#[derive(Debug, Clone)]
pub struct AirWizardData {
    pub enemy: EnemyMobData,
    pub secondform: bool,
    pub attack_delay: i32,
    pub attack_time: i32,
    pub attack_type: i32,
}

/// Java `new AirWizard(secondform)`.
pub fn new(g: &Game, secondform: bool) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    // JAVA: lvlcols is `new int[2]` (all zeros), lifetime is -1 (unlimited).
    let (mut enemy, _col) = EnemyMobData::new(
        if secondform { 2 } else { 1 },
        &SPRITES,
        &[0, 0],
        if secondform { 5000 } else { 2000 },
        false,
        16 * 8,
        -1,
        10,
        50,
        diff_idx,
    );
    if secondform {
        enemy.ai.mob.speed = 3;
    } else {
        enemy.ai.mob.speed = 2;
    }
    enemy.ai.mob.walk_time = 2;
    // top half color / bottom half color
    enemy.lvlcols[0] =
        if secondform { color::get4(-1, 0, 2, 46) } else { color::get4(-1, 100, 500, 555) };
    enemy.lvlcols[1] =
        if secondform { color::get4(-1, 0, 2, 46) } else { color::get4(-1, 100, 500, 532) };
    let mut c = EntityCommon::new(4, 3);
    c.col = enemy.lvlcols[(enemy.lvl - 1) as usize];
    Entity::new(
        c,
        EntityKind::AirWizard(AirWizardData {
            enemy,
            secondform,
            attack_delay: 0,
            attack_time: 0,
            attack_type: 0,
        }),
    )
}

/// Java `air_wizard.tick()`. TODO(port:entity-behavior): leaf behavior.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_tick_base(g, e);
}

/// Java `air_wizard.die()`. TODO(port:entity-behavior): drops.
pub fn die(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_die(g, e);
}

/// TODO(port:entity-behavior): custom render.
pub fn render(g: &mut Game, screen: &mut crate::gfx::Screen, e: &mut Entity) {
    crate::entity::behavior::enemy_mob_render(g, screen, e)
}
