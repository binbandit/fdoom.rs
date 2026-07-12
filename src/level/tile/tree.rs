//! Port of `fdoom.level.tile.TreeTile`.

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::entity::particle::{new_smash_particle, new_text_particle};
use crate::gfx::{Screen, color};
use crate::item::{Item, ToolType};
use crate::level::{drop_item, drop_items_counted};

/// Legacy leaf palette — the classic cells are true-color art, so this only matters
/// for the rare palette pixel and is kept for compatibility.
const COL: i32 = color::get4(10, 30, 151, -1);

/// Java `TreeTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Tree);
    def.connects_to_grass = true;
    def.flammable = true;
    def
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let grass = g.tiles.get("grass");
    dispatch::render(g, screen, &grass, lvl, x, y);

    // The traced classic cells at their historical pinned addresses (TL/TR/BL at
    // `tree_pieces`, BR + knot fill at `tree_fill`), plus the broadleaf edge sheet.
    let art = super::tree_species::CanopyArt {
        lone: [9, 10, 9 + 32, 10 + 3 * 32],
        fill: 10 + 32,
        knot: 10 + 2 * 32,
        edges: crate::assets::sprite_pos("tiles/tree_canopy"),
    };
    super::tree_species::render_canopy(g, screen, def, lvl, x, y, &art, COL);
}

pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let damage = g.level(lvl).get_data(xt, yt);
    if damage > 0 {
        g.level_mut(lvl).set_data(xt, yt, damage - 1);
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
    if let Some(tool_level) = super::tool_use(g, player, item, ToolType::Axe, 4) {
        let dmg = g.random.next_int_bound(10) + tool_level * 5 + 10;
        hurt_dmg(g, def, lvl, xt, yt, dmg);
        return true;
    }
    false
}

pub fn hurt_dmg(g: &mut Game, _def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    if g.random.next_int_bound(100) == 0 {
        let apple = crate::item::registry::get(g, "Apple");
        drop_item(g, lvl, x * 16 + 8, y * 16 + 8, apple);
    }

    // Glancing blows knock loose sticks (~1 in 6 hits), so even bare-handed low-damage
    // punching yields the handle for the first crude tool before the tree falls.
    if g.random.next_int_bound(6) == 0 {
        let stick = crate::item::registry::get(g, "Stick");
        drop_item(g, lvl, x * 16 + 8, y * 16 + 8, stick);
    }

    let mut dmg = dmg;
    let mut damage = g.level(lvl).get_data(x, y) + dmg;
    let tree_health = 20;
    if g.is_mode("creative") {
        dmg = tree_health;
        damage = tree_health;
    }

    g.play_sound(Sound::MonsterHurt);
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
    if damage >= tree_health {
        let wood = crate::item::registry::get(g, "Wood");
        drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[wood]);
        let acorn = crate::item::registry::get(g, "Acorn");
        drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[acorn]);
        let stick = crate::item::registry::get(g, "Stick");
        drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[stick]);
        let grass = g.tiles.get("grass");
        g.set_tile_default(lvl, x, y, &grass);
    } else {
        g.level_mut(lvl).set_data(x, y, damage);
    }
}
