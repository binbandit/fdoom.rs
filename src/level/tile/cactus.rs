//! Port of `fdoom.level.tile.CactusTile`.

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::entity::behavior::mob_hurt_tile;
use crate::entity::particle::{new_smash_particle, new_text_particle};
use crate::gfx::{Screen, Sprite, color};
use crate::level::drop_items_counted;

/// Java `CactusTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Cactus);
    def.sprite = Some(Sprite::new(8, 2, 2, 2, color::get4(30, 40, 50, -1), 0));
    def.connects_to_sand = true;
    def
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let sand = g.tiles.get("sand");
    dispatch::render(g, screen, &sand, lvl, x, y);
    if let Some(sprite) = &def.sprite {
        sprite.render(screen, x * 16, y * 16);
    }
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    false
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    _source: &mut Entity,
    dmg: i32,
    _attack_dir: Direction,
) -> bool {
    let mut dmg = dmg;
    let mut damage = g.level(lvl).get_data(x, y) + dmg;
    let c_health = 10;
    if g.is_mode("creative") {
        dmg = c_health;
        damage = c_health;
    }
    g.play_sound(Sound::MonsterHurt); // JAVA: the SmashParticle constructor plays this.
    g.level_mut(lvl)
        .add(new_smash_particle(x * 16, y * 16), lvl);
    let text = new_text_particle(
        &dmg.to_string(),
        x * 16 + 8,
        y * 16 + 8,
        color::RED,
        &mut g.random,
    );
    g.level_mut(lvl).add(text, lvl);

    if damage >= c_health {
        let cactus = crate::item::registry::get(g, "Cactus");
        drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 2, 4, &[cactus]);
        let sand = g.tiles.get("sand");
        g.set_tile_default(lvl, x, y, &sand);
    } else {
        g.level_mut(lvl).set_data(x, y, damage);
    }
    true
}

pub fn bumped_into(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, e: &mut Entity) {
    let _ = lvl;
    if e.mob().is_none() {
        return;
    }
    if g.settings.get("diff").as_str() == "Easy" {
        mob_hurt_tile(g, e, def, x, y, 1);
    }
    if g.settings.get("diff").as_str() == "Normal" {
        mob_hurt_tile(g, e, def, x, y, 1);
    }
    if g.settings.get("diff").as_str() == "Hard" {
        mob_hurt_tile(g, e, def, x, y, 2);
    }
}

pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let damage = g.level(lvl).get_data(xt, yt);
    if damage > 0 {
        g.level_mut(lvl).set_data(xt, yt, damage - 1);
    }
}
