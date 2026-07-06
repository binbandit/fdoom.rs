//! Port of `fdoom.level.tile.RockTile`.

use std::sync::atomic::{AtomicI32, Ordering};

use super::{ConnectorSprite, TileDef, TileKind, dirt, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::entity::mob::player_behavior::pay_stamina;
use crate::entity::particle::{new_smash_particle, new_text_particle};
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ItemKind, ToolType};
use crate::level::drop_items_counted;

/// Java `RockTile.coallvl` — instance state on the singleton tile.
// JAVA: set to 1 on the first pickaxe interact (or creative break) and never reset, so
// every later rock break drops coal.
static COALLVL: AtomicI32 = AtomicI32::new(0);

/// Java `RockTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Rock);
    def.csprite = Some(ConnectorSprite::new(
        Sprite::new(4, 0, 3, 3, color::get4(111, 444, 555, 321), 3),
        Sprite::new(7, 0, 2, 2, color::get4(111, 444, 555, 321), 3),
        Sprite::dots(color::get4(444, 444, 333, 333)),
    ));
    def
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let col = color::get4(111, 444, 555, dirt::d_col(g.level(lvl).depth));
    let full = def.csprite.as_ref().map(|cs| cs.full.color).unwrap_or(0);
    dispatch::csprite_render(g, screen, def, lvl, x, y, Some((col, col, full)));
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    // JAVA: a commented-out debug branch let creative-mode arrows break rock.
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
    hurt_dmg(g, def, lvl, x, y, 1);
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
    if let ItemKind::Tool {
        ttype,
        level: tool_level,
        ..
    } = &item.kind
    {
        let (ttype, tool_level) = (*ttype, *tool_level);
        if ttype == ToolType::Pickaxe
            && pay_stamina(player, 4 - tool_level)
            && item.pay_durability(g.is_mode("creative"))
        {
            COALLVL.store(1, Ordering::Relaxed);
            let dmg = g.random.next_int_bound(10) + tool_level * 5 + 10;
            hurt_dmg(g, def, lvl, xt, yt, dmg);
            return true;
        }
    }
    false
}

pub fn hurt_dmg(g: &mut Game, _def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    let mut dmg = dmg;
    let mut damage = g.level(lvl).get_data(x, y) + dmg;
    let rock_health = 50;
    if g.is_mode("creative") {
        dmg = rock_health;
        damage = rock_health;
        COALLVL.store(1, Ordering::Relaxed);
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
    if damage >= rock_health {
        // JAVA: unused `int count = random.nextInt(1) + 0;` — consumes a random value.
        let _count = g.random.next_int_bound(1);
        let coallvl = COALLVL.load(Ordering::Relaxed);
        if coallvl == 0 {
            let stone = crate::item::registry::get(g, "Stone");
            drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 4, &[stone]);
        }
        if coallvl == 1 {
            let stone = crate::item::registry::get(g, "Stone");
            drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[stone]);
            let mut mincoal = 0;
            let mut maxcoal = 1;
            if g.settings.get("diff").as_str() != "Hard" {
                mincoal += 1;
                maxcoal += 1;
            }
            let coal = crate::item::registry::get(g, "coal");
            drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, mincoal, maxcoal, &[coal]);
        }
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
