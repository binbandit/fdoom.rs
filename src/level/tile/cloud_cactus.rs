//! Port of `fdoom.level.tile.CloudCactusTile`.

use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Sprite, color};
use crate::item::{Item, ToolType};

/// Java `CloudCactusTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::CloudCactus);
    def.sprite = Some(Sprite::new(17, 1, 2, 2, color::get4(444, 111, 333, 555), 0));
    def
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    // solid to everything on foot; flying kinds are exempted globally in
    // `dispatch::may_pass`
    false
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    _source: &mut Entity,
    _dmg: i32,
    _attack_dir: Direction,
) -> bool {
    hurt_dmg(g, def, lvl, x, y, 0);
    true
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    if super::tool_use(g, player, item, ToolType::Pickaxe, 6).is_some() {
        hurt_dmg(g, def, lvl, xt, yt, 1);
        return true;
    }
    false
}

pub fn hurt_dmg(g: &mut Game, _def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    let mut dmg = dmg;
    let mut damage = g.level(lvl).get_data(x, y) + dmg;
    let health = 10;
    if g.is_mode("creative") {
        dmg = health;
        damage = health;
    }
    g.play_sound(Sound::MonsterHurt);
    let smash = crate::entity::particle::new_smash_particle(x * 16, y * 16);
    g.level_mut(lvl).add(smash, lvl);
    let text = crate::entity::particle::new_text_particle(
        &dmg.to_string(),
        x * 16 + 8,
        y * 16 + 8,
        color::RED,
        &mut g.random,
    );
    g.level_mut(lvl).add(text, lvl);
    if damage >= health {
        let cloud = g.tiles.get("cloud");
        g.set_tile_default(lvl, x, y, &cloud);
    } else {
        g.level_mut(lvl).set_data(x, y, damage);
    }
}

pub fn bumped_into(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, e: &mut Entity) {
    let _ = lvl;
    // spike damage scales with difficulty; mob_hurt_tile ignores non-mobs
    let dmg = 1 + g.settings.get_idx("diff");
    crate::entity::behavior::mob_hurt_tile(g, e, def, xt, yt, dmg);
}
