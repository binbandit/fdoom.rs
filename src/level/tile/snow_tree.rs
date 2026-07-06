//! Port of `fdoom.level.tile.SnowTreeTile`.

use super::dispatch;
use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Screen, color};
use crate::item::{Item, ItemKind, ToolType};

// Java static `col`/`col1`/`col2` (assigned in the constructor).
const COL: i32 = color::get4(10, 30, 151, -1);
const COL1: i32 = color::get4(10, 30, 430, -1);
const COL2: i32 = color::get4(10, 30, 320, -1);

/// Java `SnowTreeTile` constructor — `super(name, (ConnectorSprite)null)`.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::SnowTree);
    def.connects_to_snow = true;
    def
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let snow = g.tiles.get("snow");
    dispatch::render(g, screen, &snow, lvl, x, y);

    let bark_col1 = COL1;
    let bark_col2 = COL2;

    let u = g.tile_at(lvl, x, y - 1).id == def.id;
    let l = g.tile_at(lvl, x - 1, y).id == def.id;
    let r = g.tile_at(lvl, x + 1, y).id == def.id;
    let d = g.tile_at(lvl, x, y + 1).id == def.id;
    let ul = g.tile_at(lvl, x - 1, y - 1).id == def.id;
    let ur = g.tile_at(lvl, x + 1, y - 1).id == def.id;
    let dl = g.tile_at(lvl, x - 1, y + 1).id == def.id;
    let dr = g.tile_at(lvl, x + 1, y + 1).id == def.id;

    if u && ul && l {
        screen.render(x * 16, y * 16, 10 + 32, COL, 0);
    } else {
        screen.render(x * 16, y * 16, 9, COL, 0);
    }
    if u && ur && r {
        screen.render(x * 16 + 8, y * 16, 10 + 2 * 32, bark_col2, 0);
    } else {
        screen.render(x * 16 + 8, y * 16, 10, COL, 0);
    }
    if d && dl && l {
        screen.render(x * 16, y * 16 + 8, 10 + 2 * 32, bark_col2, 0);
    } else {
        screen.render(x * 16, y * 16 + 8, 9 + 32, bark_col1, 0);
    }
    if d && dr && r {
        screen.render(x * 16 + 8, y * 16 + 8, 10 + 32, COL, 0);
    } else {
        screen.render(x * 16 + 8, y * 16 + 8, 10 + 3 * 32, bark_col2, 0);
    }
}

pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let damage = g.level(lvl).get_data(xt, yt);
    if damage > 0 {
        g.level_mut(lvl).set_data(xt, yt, damage - 1);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
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
    dmg: i32,
    _attack_dir: Direction,
) -> bool {
    hurt_dmg(g, def, lvl, x, y, dmg);
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
    if let ItemKind::Tool { ttype, level, .. } = item.kind {
        if ttype == ToolType::Axe
            && crate::entity::mob::player_behavior::pay_stamina(player, 4 - level)
            && item.pay_durability(g.is_mode("creative"))
        {
            let dmg = g.random.next_int_bound(10) + level * 5 + 10;
            hurt_dmg(g, def, lvl, xt, yt, dmg);
            return true;
        }
    }
    false
}

pub fn hurt_dmg(g: &mut Game, _def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    if g.random.next_int_bound(100) == 0 {
        let apple = crate::item::registry::get(g, "Apple");
        crate::level::drop_item(g, lvl, x * 16 + 8, y * 16 + 8, apple);
    }

    let mut dmg = dmg;
    let mut damage = g.level(lvl).get_data(x, y) + dmg;
    let tree_health = 20;
    if g.is_mode("creative") {
        dmg = tree_health;
        damage = tree_health;
    }

    // JAVA: SmashParticle's constructor plays Sound.monsterHurt.
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
    if damage >= tree_health {
        let wood = crate::item::registry::get(g, "Wood");
        crate::level::drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[wood]);
        let acorn = crate::item::registry::get(g, "Acorn");
        crate::level::drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[acorn]);
        let snow = g.tiles.get("snow");
        g.set_tile_default(lvl, x, y, &snow);
    } else {
        g.level_mut(lvl).set_data(x, y, damage);
    }
}
