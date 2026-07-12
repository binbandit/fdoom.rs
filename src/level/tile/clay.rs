//! Layered Clay + Ore Freckle (content wave): the ground of the Badlands biome —
//! dry eroded canyon country on the hot side of the world.
//!
//! **Layered Clay** is calm banded strata: the dirt clods-and-stones texture under
//! three rust palettes that alternate in wide horizontal bands (phase-shifted per
//! coarse column so the strata drift like real cut banks instead of ruling the map).
//! Shovels dig it like dirt, opening the usual descent pit.
//!
//! **Ore Freckle** is fossicking's surface tease where there is no water to pan:
//! on genuinely rich ground (the shared `richness_at` field) the clay shows exposed
//! metal pips — pickaxe the tile for 1-2 Iron Ore or Coal and it smooths back to
//! clay. The same field lowers the vein gate in the mines below, so a freckled flat
//! truthfully marks good digging.
//!
//! TODO(art): dedicated banded-clay cells and a real pip overlay — the render
//! reuses the dirt texture block and the nugget item cell.

use super::{TileDef, TileKind, tool_use};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::particle::new_smash_particle;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ToolType};
use crate::level::infinite_gen::{hash, unit};
use crate::level::{drop_item, drop_items_counted};

/// Salt of the strata phase shift (render-only, but pure `f(seed, x)` like all gen).
const BAND_SALT: u64 = 0xBAD_0005;
/// Salt of the freckle's metal pick (iron vs coal), shared with the drop roll.
const FRECKLE_METAL_SALT: u64 = 0xBAD_0004;

/// The three strata palettes (dirt-texture slots: lit clod tops, soil base, clod
/// under-shadow, stones): pale caprock, mid rust, deep oxide.
fn band_colors(band: i32) -> i32 {
    // deliberately close tones: strata should murmur, not stripe (house taste —
    // calm base, sparse detail)
    match band {
        0 => color::get4(
            color::hex("#cf9166"),
            color::hex("#c08258"),
            color::hex("#9c6844"),
            color::hex("#ab825f"),
        ),
        1 => color::get4(
            color::hex("#c48058"),
            color::hex("#b5714a"),
            color::hex("#8f5636"),
            color::hex("#a06a49"),
        ),
        _ => color::get4(
            color::hex("#b7754e"),
            color::hex("#a8663f"),
            color::hex("#82502f"),
            color::hex("#91603f"),
        ),
    }
}

/// Which stratum a tile sits in: 2-tile-tall bands cycling through the three
/// palettes, phase-shifted every 48-tile column so the banding drifts organically.
fn band_at(seed: i64, x: i32, y: i32) -> i32 {
    let phase = (hash(seed, BAND_SALT, x.div_euclid(48), 0) % 3) as i32;
    (y.div_euclid(2) + phase).rem_euclid(3)
}

pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Clay);
    def.sprite = Some(Sprite::dots_at(21, 3, band_colors(1)));
    def.connects_to_sand = true; // desert-edge seams blend, not butt-join
    def.may_spawn = true;
    def
}

pub fn make_freckle(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::OreFreckle);
    def.sprite = Some(Sprite::dots_at(21, 3, band_colors(1)));
    def.connects_to_sand = true;
    def
}

pub fn render(g: &mut Game, screen: &mut Screen, _def: &TileDef, _lvl: usize, x: i32, y: i32) {
    let band = band_at(g.world_seed, x, y);
    Sprite::dots_at(21, 3, band_colors(band)).render(screen, x * 16, y * 16);
}

/// Is this position in clay country (any cardinal neighbor is clay/freckle)? The
/// sand-based scatter tiles (dry bush, dead tree) read it to render a clay base in
/// the Badlands instead of stamping a yellow sand square onto the strata —
/// flora on its true ground (house taste rule).
pub fn clay_country(g: &Game, lvl: usize, x: i32, y: i32) -> bool {
    [(0, -1), (0, 1), (-1, 0), (1, 0)].iter().any(|&(dx, dy)| {
        matches!(
            g.tile_at(lvl, x + dx, y + dy).kind,
            TileKind::Clay | TileKind::OreFreckle
        )
    })
}

/// Does this freckle carry iron (vs coal)? Pure, so render and drop always agree.
fn freckle_is_iron(seed: i64, x: i32, y: i32) -> bool {
    unit(hash(seed, FRECKLE_METAL_SALT, x, y)) < 0.45
}

pub fn freckle_render(
    g: &mut Game,
    screen: &mut Screen,
    def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
) {
    render(g, screen, def, lvl, x, y);
    // the exposed pips: only the nugget cell's two LIGHT shades draw (the dark
    // outline stays transparent), so each pip is a small speck cluster sitting
    // proud of the clay, not a stamped disc; two pips, corners hashed so freckle
    // fields never grid up
    let col = if freckle_is_iron(g.world_seed, x, y) {
        color::get4(-1, -1, 322, 555) // iron: rusty gray with a bright glint
    } else {
        color::get4(-1, -1, 0, 111) // coal: near-black specks
    };
    let h = hash(g.world_seed, FRECKLE_METAL_SALT ^ 0xA5, x, y);
    let (px, py) = ((h % 6) as i32, ((h >> 8) % 6) as i32 + 1);
    Sprite::new1x1(10, 4, col).render(screen, x * 16 + px, y * 16 + py);
    let (qx, qy) = (((h >> 16) % 6) as i32 + 3, ((h >> 24) % 5) as i32 + 4);
    Sprite::new1x1(10, 4, col).render(screen, x * 16 + qx, y * 16 + qy);
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
    // clay digs like dirt: shovel opens the descent pit
    if tool_use(g, player, item, ToolType::Shovel, 4).is_some() {
        let pit = g.tiles.get("Dug Pit");
        g.set_tile_default(lvl, xt, yt, &pit);
        let dirt = crate::item::registry::get(g, "dirt");
        drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, dirt);
        g.play_sound(Sound::MonsterHurt);
        return true;
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub fn freckle_interact(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    if tool_use(g, player, item, ToolType::Pickaxe, 4).is_some() {
        let name = if freckle_is_iron(g.world_seed, xt, yt) {
            "Iron Ore"
        } else {
            "Coal"
        };
        let ore = crate::item::registry::get(g, name);
        drop_items_counted(g, lvl, xt * 16 + 8, yt * 16 + 8, 1, 2, &[ore]);
        let clay = g.tiles.get("Layered Clay");
        g.set_tile_default(lvl, xt, yt, &clay);
        g.level_mut(lvl)
            .add(new_smash_particle(xt * 16, yt * 16), lvl);
        g.play_sound(Sound::MonsterHurt);
        g.notifications
            .push("Ore picked clean from the clay.".to_string());
        return true;
    }
    // the audit rule: a blocked use says why (shovels can't free the pips)
    if tool_use(g, player, item, ToolType::Shovel, 4).is_some() {
        crate::item::interact::place_note(g, "Too stony to shovel - the pips want a pickaxe.");
        return true;
    }
    false
}
