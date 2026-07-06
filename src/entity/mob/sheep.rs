//! Port of `fdoom.entity.mob.Sheep`. Data + constructor; behavior in `sheep` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};

use super::PassiveMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(10, 18));

#[derive(Debug, Clone)]
pub struct SheepData {
    pub passive: PassiveMobData,
}

/// Java `new Sheep()`.
pub fn new(g: &Game) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (passive, col) = PassiveMobData::new(&SPRITES, color::get4(-1, 0, 555, 432), 3, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Sheep(SheepData { passive }))
}

// JAVA: Sheep.java only overrides die(); it has no tick/touchedBy/interact overrides
// (no wool-cutting mechanic exists in this fork).

/// Java `Sheep.tick()` — no override; `MobAi.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::mobai_tick_base(g, e);
}

/// Java `Sheep.die()`.
pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::item::registry;

    let (mut min, mut max) = (0, 0);
    let diff = g.settings.get("diff").as_str().to_string();
    if diff == "Easy" {
        min = 1;
        max = 3;
    }
    if diff == "Normal" {
        min = 1;
        max = 2;
    }
    if diff == "Hard" {
        min = 0;
        max = 2;
    }

    let wool = registry::get(g, "wool");
    crate::entity::behavior::mobai_drop_items(g, e, min, max, &[wool]);

    crate::entity::behavior::passive_mob_die(g, e);
}
