//! Port of `fdoom.level.tile.WallTile`.

use super::Material;
use super::{ConnectorSprite, TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Sprite, color};
use crate::item::{Item, ItemKind, ToolType};

/// Java `Material.ordinal()`.
fn ordinal(material: Material) -> i32 {
    match material {
        Material::Wood => 0,
        Material::Stone => 1,
        Material::Obsidian => 2,
    }
}

/// Java `WallTile` constructor.
pub fn make(material: Material) -> TileDef {
    let mut def = TileDef::new(
        &format!("{} Wall", material.name()),
        TileKind::Wall { material },
    );
    def.blocks_light = true; // all wall materials occlude emitter light
    def.flammable = material == Material::Wood;
    def.csprite = Some(match material {
        Material::Wood => ConnectorSprite::new(
            Sprite::new(4, 22, 3, 3, color::get4(100, 430, 320, 540), 3),
            Sprite::new(7, 22, 2, 2, color::get4(100, 430, 320, 540), 3),
            Sprite::new_onepixel(5, 23, 2, 2, color::get4(430, 430, 320, 320), 0, true),
        ),
        Material::Stone => ConnectorSprite::new(
            Sprite::new(4, 25, 3, 3, color::get4(111, 333, 444, 444), 3),
            Sprite::new(7, 24, 2, 2, color::get(111, 444), 3),
            Sprite::blank(2, 2, 444),
        ),
        Material::Obsidian => ConnectorSprite::new(
            Sprite::new(4, 25, 3, 3, color::get4(0, 203, 103, 103), 3),
            Sprite::new(7, 24, 2, 2, color::get(0, 103), 3),
            Sprite::blank(2, 2, 103),
        ),
    });
    def
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    false
}

/// Walls connect to other walls (any material, the Java `same_class` default) and,
/// post-port, to Windows — so a paned segment merges into the masonry run.
pub fn connects_to(def: &TileDef, other: &TileDef, _is_side: bool) -> bool {
    super::dispatch::same_class(def, other) || matches!(other.kind, TileKind::Window)
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
    let TileKind::Wall { material } = def.kind else {
        return false;
    };
    // (The Java "beat the Air Wizard first" lock on deep obsidian walls is gone with
    // the sandbox pivot — walls obey their material rules everywhere.)
    let _ = material;
    // JAVA: `random.nextInt(6) / 6 * dmg / 2` — integer division made this always 0,
    // so bare-hand/mob hits never damaged walls. FIX: multiply before dividing so the
    // intended random scaling (0..dmg/2, averaging ~dmg/5) actually applies.
    let d = dmg * g.random.next_int_bound(6) / 6 / 2;
    hurt_dmg(g, def, lvl, x, y, d);
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
    let TileKind::Wall { material } = def.kind else {
        return false;
    };
    let _ = material;
    if let ItemKind::Tool { ttype, level, .. } = item.kind {
        if ttype == ToolType::Pickaxe
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

pub fn hurt_dmg(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    let TileKind::Wall { material } = def.kind else {
        return;
    };
    let mut dmg = dmg;
    let mut damage = g.level(lvl).get_data(x, y) + dmg;
    let sbw_health = 100;
    if g.is_mode("creative") {
        dmg = sbw_health;
        damage = sbw_health;
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
    if damage >= sbw_health {
        let (item_name, tilename) = match material {
            Material::Wood => ("Plank", "Wood Planks"),
            Material::Stone => ("Stone Brick", "Stone Bricks"),
            Material::Obsidian => ("Obsidian Brick", "Obsidian"),
        };

        let item = crate::item::registry::get(g, item_name);
        crate::level::drop_items_counted(
            g,
            lvl,
            x * 16 + 8,
            y * 16 + 8,
            1,
            3 - ordinal(material),
            &[item],
        );
        let tile = g.tiles.get(tilename);
        g.set_tile_default(lvl, x, y, &tile);
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

/// Java `WallTile.getName(data)`.
pub fn get_name(def: &TileDef, data: i32) -> String {
    // JAVA: `Material.values[data]` — an out-of-range data value threw
    // ArrayIndexOutOfBoundsException. FIX: fall back to the def's own name.
    match Material::VALUES.get(data as usize) {
        Some(material) => format!("{} Wall", material.name()),
        None => def.name.clone(),
    }
}
