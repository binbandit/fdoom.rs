//! Port of `fdoom.level.tile.FlowerTile`.

use super::dispatch;
use super::{TileDef, TileKind, tool_use};
use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ToolType};
use crate::level::{drop_item, drop_items_counted};

/// Three meadow species on the `tiles/flower_species` strip (one 8x8 cell each):
/// daisy, poppy, cornflower. Palette-mode shade roles: 1 = stem, 2 = petals,
/// 3 = flower center.
const SPECIES_PALETTES: [i32; 3] = [
    color::get4(-1, 20, 555, 550), // daisy: white petals, yellow heart
    color::get4(-1, 20, 500, 100), // poppy: red petals, dark heart
    color::get4(-1, 20, 115, 335), // cornflower: blue petals, paler heart
];

/// One blossom of species `k` (0..3), optionally mirrored for within-tile variety.
fn flower_sprite(k: usize, mirror: i32) -> Sprite {
    let c = crate::assets::sprite_cell("tiles/flower_species");
    Sprite::new(c.x + k as i32, c.y, 1, 1, SPECIES_PALETTES[k], mirror)
}

/// Java `FlowerTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Flower);
    def.connects_to_grass = true;
    def.may_spawn = true;
    def
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    _source: &mut Entity,
    _dmg: i32,
    _attack_dir: Direction,
) -> bool {
    let flower = crate::item::registry::get(g, "Flower");
    drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[flower]);
    let rose = crate::item::registry::get(g, "Rose");
    drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 0, 1, &[rose]);
    let grass = g.tiles.get("grass");
    g.set_tile_default(lvl, x, y, &grass);
    true
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    if tool_use(g, player, item, ToolType::Shovel, 2).is_some() {
        let flower = crate::item::registry::get(g, "Flower");
        drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, flower);
        let rose = crate::item::registry::get(g, "Rose");
        drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, rose);
        let grass = g.tiles.get("grass");
        g.set_tile_default(lvl, xt, yt, &grass);
        return true;
    }
    false
}

pub fn render(g: &mut Game, screen: &mut Screen, _def: &TileDef, lvl: usize, x: i32, y: i32) {
    let grass = g.tiles.get("grass");
    dispatch::render(g, screen, &grass, lvl, x, y);

    // Species and blossom placement vary per position (gen never writes flower
    // data, so the hash carries the variety; data still nudges `shape` for tiles
    // that do set it). One species per tile — patches read as a meadow, not confetti.
    let h = crate::level::infinite_gen::hash(g.world_seed, 0x464C_5257, x, y);
    let species = (h % 3) as usize;
    let data = g.level(lvl).get_data(x, y);
    let shape = (data / 16 + ((h >> 2) & 1) as i32) % 2;

    let x = x << 4;
    let y = y << 4;

    flower_sprite(species, 0).render(screen, x + 8 * shape, y);
    // the second blossom mirrors, so a tile never shows twin stamps
    flower_sprite(species, 1).render(screen, x + 8 * (if shape == 0 { 1 } else { 0 }), y + 8);
}
