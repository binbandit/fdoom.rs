//! Port of `fdoom.entity.mob.Cow`. Data + constructor; behavior in `cow` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};

use super::PassiveMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(16, 16));

#[derive(Debug, Clone)]
pub struct CowData {
    pub passive: PassiveMobData,
}

/// Java `new Cow()`.
pub fn new(g: &Game) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (passive, col) = PassiveMobData::new(&SPRITES, color::get4(-1, 0, 333, 322), 5, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(c, EntityKind::Cow(CowData { passive }))
}

/// Java `Cow.tick()` — no override; `MobAi.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::mobai_tick_base(g, e);
}

/// Java `Cow.die()`.
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
        max = 1;
    }

    let leather = registry::get(g, "leather");
    let raw_beef = registry::get(g, "raw beef");
    crate::entity::behavior::mobai_drop_items(g, e, min, max, &[leather, raw_beef]);

    // temperature wave: the hide comes with fur (Fur Coat material)
    let fur = registry::get(g, "Fur");
    crate::entity::behavior::mobai_drop_items(g, e, 1, 2, &[fur]);

    crate::entity::behavior::passive_mob_die(g, e);
}
