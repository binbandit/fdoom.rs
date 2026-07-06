//! Port of `fdoom.level.tile.HardRockTile`.

use super::{ConnectorSprite, TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::sprite::make_sprite;
use crate::gfx::{Sprite, color};
use crate::item::{Item, ItemKind, ToolType};

/// Java `HardRockTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::HardRock);
    // JAVA: Color.get(001, ...) — leading-zero (octal) literal 001 == 1.
    def.csprite = Some(ConnectorSprite::new(
        Sprite::new(4, 0, 3, 3, color::get4(1, 334, 445, 321), 3),
        Sprite::new(7, 0, 2, 2, color::get4(1, 334, 445, 321), 3),
        make_sprite(
            2,
            2,
            color::get4(445, 334, 223, 223),
            0,
            false,
            &[0, 1, 2, 0],
        ),
    ));
    def
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
    if let ItemKind::Tool { ttype, level, .. } = item.kind {
        if g.is_mode("creative") {
            return true;
        }
        if ttype == ToolType::Pickaxe && level == 4 {
            if crate::entity::mob::player_behavior::pay_stamina(player, 4 - level)
                && item.pay_durability(g.is_mode("creative"))
            {
                let dmg = g.random.next_int_bound(10) + level * 5 + 10;
                hurt_dmg(g, def, lvl, xt, yt, dmg);
                return true;
            }
        } else {
            // JAVA: Game.notifications.add
            g.notifications.push("Gem Pickaxe Required.".to_string());
        }
    }
    g.is_mode("creative")
}

pub fn hurt_dmg(g: &mut Game, _def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    let mut dmg = dmg;
    let mut damage = g.level(lvl).get_data(x, y) + dmg;
    let hr_health = 200;
    if g.is_mode("creative") {
        dmg = hr_health;
        damage = hr_health;
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
    if damage >= hr_health {
        let stone = crate::item::registry::get(g, "Stone");
        crate::level::drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 3, &[stone]);
        let coal = crate::item::registry::get(g, "coal");
        crate::level::drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 0, 1, &[coal]);
        let dirt = g.tiles.get("dirt");
        g.set_tile_default(lvl, x, y, &dirt);
    } else {
        g.level_mut(lvl).set_data(x, y, damage);
    }
}

pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let damage = g.level(lvl).get_data(xt, yt);
    if damage > 0 {
        g.level_mut(lvl).set_data(xt, yt, damage - 1);
    }
}
