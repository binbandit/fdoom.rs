//! Port of `fdoom.level.tile.OreTile`.

use super::{OreType, TileDef, TileKind, dirt};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::entity::mob::player_behavior::pay_stamina;
use crate::entity::particle::{new_smash_particle, new_text_particle};
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ItemKind, ToolType};
use crate::level::drop_item;

/// Java `OreType.color`.
pub fn ore_color(ore_type: OreType) -> i32 {
    match ore_type {
        OreType::Iron => color::get4(-1, 100, 322, 544),
        OreType::Lapis => color::get4(-1, 5, 115, 115),
        OreType::Gold => color::get4(-1, 110, 440, 553),
        OreType::Gem => color::get4(-1, 101, 404, 545),
    }
}

/// Java `OreType.drop` (the item name; `OreType.getOre()` clones it).
fn ore_item_name(ore_type: OreType) -> &'static str {
    match ore_type {
        OreType::Iron => "Iron Ore",
        OreType::Lapis => "Lapis",
        OreType::Gold => "Gold Ore",
        OreType::Gem => "Gem",
    }
}

/// Java `OreType.getOre()`.
pub fn get_ore(g: &Game, ore_type: OreType) -> Item {
    crate::item::registry::get(g, ore_item_name(ore_type))
}

/// Java `OreTile` constructor.
pub fn make(ore_type: OreType) -> TileDef {
    let name = match ore_type {
        OreType::Lapis => "Lapis".to_string(),
        OreType::Iron => "Iron Ore".to_string(),
        OreType::Gold => "Gold Ore".to_string(),
        OreType::Gem => "Gem Ore".to_string(),
    };
    let mut def = TileDef::new(&name, TileKind::Ore { ore_type });
    def.sprite = Some(Sprite::new(17, 1, 2, 2, ore_color(ore_type), 0));
    def
}

fn kind_ore_type(def: &TileDef) -> OreType {
    match def.kind {
        TileKind::Ore { ore_type } => ore_type,
        _ => unreachable!("ore fns called on non-ore tile"),
    }
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let ore_type = kind_ore_type(def);
    let col = (ore_color(ore_type) & 0x00ff_ffff)
        | (color::get_byte(dirt::d_col(g.level(lvl).depth)) << 24);
    if let Some(sprite) = &def.sprite {
        sprite.render_color(screen, x * 16, y * 16, col);
    }
}

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
    let play_hurt = if g.is_mode("creative") {
        g.random.next_int_bound(4)
    } else {
        0
    };
    hurt_dmg(g, def, lvl, x, y, play_hurt);
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
            && pay_stamina(player, 6 - tool_level)
            && item.pay_durability(g.is_mode("creative"))
        {
            hurt_dmg(g, def, lvl, xt, yt, 1);
            return true;
        }
    }
    false
}

pub fn hurt_dmg(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    let ore_type = kind_ore_type(def);
    let mut dmg = dmg;
    // JAVA: damage always increments by 1, regardless of dmg.
    let mut damage = g.level(lvl).get_data(x, y) + 1;
    let ore_h = g.random.next_int_bound(10) + 3;
    if g.is_mode("creative") {
        dmg = ore_h;
        damage = ore_h;
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
    if dmg > 0 {
        let mut count = g.random.next_int_bound(2);
        if damage >= ore_h {
            let dirt = g.tiles.get("dirt");
            g.set_tile_default(lvl, x, y, &dirt);
            count += 2;
            // fossicking: ore still hiding within 2 tiles sparkles briefly, so the
            // player chases the vein instead of strip-mining the wall
            super::fossick::vein_ping(g, lvl, x, y);
        } else {
            g.level_mut(lvl).set_data(x, y, damage);
        }
        // JAVA: dropItem(x, y, count, item) — the count overload.
        for _ in 0..count {
            let ore = get_ore(g, ore_type);
            drop_item(g, lvl, x * 16 + 8, y * 16 + 8, ore);
        }
    }
}

pub fn bumped_into(
    _g: &mut Game,
    _def: &TileDef,
    _lvl: usize,
    _xt: i32,
    _yt: i32,
    _e: &mut Entity,
) {
    // JAVA: empty — "this was used at one point to hurt the player if they touched the
    // ore; that's probably why the sprite is so spikey-looking."
}
