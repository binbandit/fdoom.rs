//! Port of `fdoom.entity.mob.Pig`. Data + constructor; behavior in `pig` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};

use super::PassiveMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(16, 14));

#[derive(Debug, Clone)]
pub struct PigData {
    pub passive: PassiveMobData,
}

/// Java `new Pig()`.
pub fn new(g: &Game) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (passive, col) = PassiveMobData::new(&SPRITES, color::get4(-1, 0, 555, 522), 3, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Pig(PigData { passive }))
}

/// Java `Pig.tick()` — no override; `MobAi.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::mobai_tick_base(g, e);
}

/// Java `Pig.die()`.
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

    let raw_pork = registry::get(g, "raw pork");
    crate::entity::behavior::mobai_drop_items(g, e, min, max, &[raw_pork]);

    crate::entity::behavior::passive_mob_die(g, e);
}
