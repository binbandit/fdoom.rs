//! Heath (sandbox era, no Java counterpart): the highland ground of the Mountains
//! biome — stony moor between the rock crags, so highland reads as highland even
//! where no boulder is in frame.
//!
//! Art: no dedicated cells. The gravel base reuses the dirt clods-and-stones texture
//! (cells 21..24,3) in an olive-gray stone palette; sparse clustered heather/dry
//! tussock patches reuse the grass tuft texture (cells 22..25,0) in muted heather
//! tones. Patch placement is a pure function of (world seed, x, y) — coarse 4x4-tile
//! cells gate WHERE patches gather, a per-tile roll breaks up their edges — giving
//! the house texture taste (calm base, sparse clustered detail) with zero new PNGs.

use super::{TileDef, TileKind, tool_use};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ToolType};
use crate::level::drop_item;
use crate::level::infinite_gen::{hash, unit};

/// Salt of the heather-patch cluster field (coarse cells + per-tile edge roll).
const HEATHER_SALT: u64 = 0x4EA7; // "heat(h)"

pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Heath);
    // fallback sprite (render() below picks the per-tile variant): the gravel base
    def.sprite = Some(Sprite::dots_at(21, 3, gravel_colors()));
    def.may_spawn = true;
    // grass butt-joins heath without a border sprite: the two textures share scale,
    // so the color change alone marks the edge (same treatment as grass|dirt)
    def.connects_to_grass = true;
    def
}

/// Gravel base: dirt-texture slots are 0 = lit clod tops, 1 = soil base,
/// 2 = clod under-shadow, 3 = stones. Olive-gray, faintly warm; the stones lean
/// cool blue-gray (never pure gray — house palette rule).
fn gravel_colors() -> i32 {
    color::get4(
        color::hex("#a3a68e"),
        color::hex("#84876f"),
        color::hex("#636652"),
        color::hex("#8e8e96"),
    )
}

/// Heather tussocks: grass-texture slots are 1 = base, 2 = light blade tips,
/// 3 = dark blade shadows. Dull moor olive with a muted purple bloom on the tips.
fn heather_colors() -> i32 {
    color::get4(
        color::hex("#7d8069"),
        color::hex("#7d8069"),
        color::hex("#a292a8"),
        color::hex("#5e6150"),
    )
}

/// Is this tile part of a heather patch? Coarse 4x4-tile cells cluster the patches
/// (about 3 in 10 cells), and a per-tile roll ruffles their edges so patches read as
/// organic tussock groups, not stamped squares.
fn heather_at(seed: i64, x: i32, y: i32) -> bool {
    unit(hash(seed, HEATHER_SALT, x.div_euclid(4), y.div_euclid(4))) < 0.30
        && unit(hash(seed, HEATHER_SALT ^ 0xA5, x, y)) < 0.60
}

pub fn render(g: &mut Game, screen: &mut Screen, _def: &TileDef, _lvl: usize, x: i32, y: i32) {
    let spr = if heather_at(g.world_seed, x, y) {
        Sprite::dots_at(22, 0, heather_colors())
    } else {
        Sprite::dots_at(21, 3, gravel_colors())
    };
    spr.render(screen, x * 16, y * 16);
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
    // shovelling the stony turf exposes plain dirt; the gravel sometimes pays a stone
    if tool_use(g, player, item, ToolType::Shovel, 4).is_some() {
        let dirt = g.tiles.get("dirt");
        g.set_tile_default(lvl, xt, yt, &dirt);
        g.play_sound(Sound::MonsterHurt);
        if g.random.next_int_bound(3) == 0 {
            let stone = crate::item::registry::get(g, "Stone");
            drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, stone);
        }
        return true;
    }
    false
}
