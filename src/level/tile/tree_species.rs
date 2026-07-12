//! Biome tree species (sandbox era, no Java counterpart): Pine, Dead Tree, Willow,
//! Palm, Flat-Crown. They share the broadleaf `tree.rs` behavior — same axe interact,
//! same damage-in-data-byte accounting — but differ in base ground tile, canopy palette,
//! health, and drops. The classic broadleaf stays `TileKind::Tree` in `tree.rs`.
//!
//! Each species has its own dedicated true-color cell set (pinned rows 26..=28): a
//! 2x3 block of [TL, TR / BL, BR standalone quarters / fill, knot-fill] — the same
//! six roles the broadleaf samples. Canopy-forming species additionally have an
//! unpinned 2x6 edge sheet (`tiles/tree_*_canopy.png`) consumed by [`render_canopy`],
//! which this module also lends to `tree.rs` so every tree family merges into
//! little forests the same way.

use super::{Neighbors, TileDef, TileKind, TreeSpecies, dispatch, tool_use};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::particle::{new_smash_particle, new_text_particle};
use crate::entity::{Direction, Entity};
use crate::gfx::sprite_sheet::cell;
use crate::gfx::{Screen, color};
use crate::item::{Item, ToolType};
use crate::level::infinite_gen::hash;
use crate::level::{drop_item, drop_items_counted};

/// Salt for the interior-canopy variation hash — pure `f(seed, x, y)` like the
/// excavation contours in `depth.rs`, so forests render identically every frame.
const CANOPY_SALT: u64 = 0xCA_0F_00_57;

/// Cell addresses for one tree family's merged-canopy art: the lone quarters and
/// interior fills every family always had, plus the 2x6 edge sheet
/// (`tiles/tree_canopy.png` and friends: top strips / side strips / south-face
/// strips with trunk / inner corners / fill variants) that lets orthogonally
/// adjacent trees of the same family join into one woodland roof.
pub(super) struct CanopyArt {
    /// TL, TR, BL, BR standalone quarters (the traced lone-tree cells).
    pub lone: [i32; 4],
    /// Interior leaf texture.
    pub fill: i32,
    /// Interior texture with a bark knot.
    pub knot: i32,
    /// Top-left cell of the 2x6 edge sheet.
    pub edges: i32,
}

/// Neighbor-aware canopy assembly, one 8x8 quarter at a time. Each quarter looks at
/// the same-family neighbors on its two orthogonal sides (`v`/`h`) plus its diagonal:
/// no neighbor keeps the lone-tree silhouette, one neighbor swaps in an edge strip
/// (crown line or side contour continues into the next tile; south-face strips keep
/// the trunk, matching the original game's read), and both neighbors fill solid —
/// hash-varied between plain texture, a bark knot, and a sunlit tuft so big canopies
/// stay calm with sparse clustered detail rather than a periodic stamp.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_canopy(
    g: &mut Game,
    screen: &mut Screen,
    def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    art: &CanopyArt,
    col: i32,
) {
    let Neighbors {
        u,
        d,
        l,
        r,
        ul,
        ur,
        dl,
        dr,
    } = Neighbors::same_tile(g, def, lvl, x, y);
    let corners = [ul, ur, dl, dr];
    for qy in 0..2i32 {
        for qx in 0..2i32 {
            let v = if qy == 0 { u } else { d };
            let h = if qx == 0 { l } else { r };
            let diag = corners[(qy * 2 + qx) as usize];
            let cell = match (v, h) {
                (false, false) => art.lone[(qy * 2 + qx) as usize],
                (true, true) if diag => interior_cell(g.world_seed, x * 2 + qx, y * 2 + qy, art),
                // diagonal gap: fill with a rounded notch so clearings stay organic
                (true, true) => art.edges + 96 + qy * 32 + qx,
                // runs vertically: straight-ish side contour
                (true, false) => art.edges + 32 + qx,
                // runs horizontally: crown-top strip up top, trunk strip on the south face
                (false, true) => art.edges + qy * 64 + qx,
            };
            screen.render(x * 16 + qx * 8, y * 16 + qy * 8, cell, col, 0);
        }
    }
}

/// Interior canopy texture for the half-tile at `(qx, qy)` (quarter coordinates,
/// 2 per tile axis): mostly plain fill, with sparse knots and sunlit tufts.
fn interior_cell(seed: i64, qx: i32, qy: i32, art: &CanopyArt) -> i32 {
    match hash(seed, CANOPY_SALT, qx, qy) % 16 {
        0 => art.knot,
        1 | 2 => art.edges + 160, // sunlit tuft
        3..=8 => art.edges + 161, // offset texture, breaks the 8px repeat
        _ => art.fill,
    }
}

/// The edge sheet of species whose crowns merge into a shared canopy. Palm, dead
/// tree, willow and flat-crown stay individual: lone silhouettes are their whole
/// read (a bare snag or a leaning palm has no canopy to share).
fn canopy_edges(species: TreeSpecies) -> Option<i32> {
    match species {
        TreeSpecies::Pine => Some(crate::assets::sprite_pos("tiles/tree_pine_canopy")),
        _ => None,
    }
}

/// Per-species config: base ground tile, health, canopy palette, dead-look darken.
struct Info {
    /// Tile the species stands on (rendered under the canopy, restored when felled).
    base: &'static str,
    health: i32,
    /// Canopy / bark palettes, mirroring `tree.rs`'s COL/COL1/COL2 roles. The
    /// dedicated species art is true color, so these only tint the rare palette
    /// pixel; they are kept for compatibility.
    col: i32,
    col1: i32,
    col2: i32,
    /// Extra full-tile darken (was used while the Dead Tree shared broadleaf art).
    darken: i32,
    /// Base cell (bx, by) of the species' 2x3 art block on the sheet.
    art: (i32, i32),
}

fn info(species: TreeSpecies) -> Info {
    match species {
        TreeSpecies::Pine => Info {
            base: "snow",
            health: 20,
            col: color::get4(10, 20, 141, -1), // blue-cast fir needles
            col1: color::get4(10, 20, 430, -1),
            col2: color::get4(10, 20, 320, -1),
            darken: 0,
            art: (0, 26),
        },
        TreeSpecies::Dead => Info {
            base: "sand",
            health: 8, // brittle snag
            col: color::get4(110, 211, 322, -1),
            col1: color::get4(110, 211, 430, -1),
            col2: color::get4(110, 211, 320, -1),
            darken: 0,
            art: (2, 26),
        },
        TreeSpecies::Willow => Info {
            base: "grass",
            health: 20,
            col: color::get4(10, 41, 252, -1), // pale drooping green
            col1: color::get4(10, 41, 430, -1),
            col2: color::get4(10, 41, 320, -1),
            darken: 0,
            art: (7, 26),
        },
        TreeSpecies::Palm => Info {
            base: "sand",
            health: 20,
            col: color::get4(20, 40, 251, -1), // warm frond green
            col1: color::get4(20, 40, 541, -1),
            col2: color::get4(20, 40, 431, -1),
            darken: 0,
            art: (9, 26),
        },
        TreeSpecies::FlatCrown => Info {
            base: "grass",
            health: 16,
            col: color::get4(10, 30, 241, -1), // olive savanna crown
            col1: color::get4(10, 30, 430, -1),
            col2: color::get4(10, 30, 320, -1),
            darken: 0,
            art: (11, 26),
        },
    }
}

/// The ground tile a species stands on (rendered beneath the canopy, restored when
/// felled). Public so the ground-blend/seam pass classifies species tiles by their
/// *real* base — a pine must read as snow and a dead tree as sand, or seam blending
/// stipples grass-green into snowfields and dunes (playtest bug #6).
pub fn base_tile(species: TreeSpecies) -> &'static str {
    info(species).base
}

fn kind_species(def: &TileDef) -> TreeSpecies {
    match def.kind {
        TileKind::TreeSpecies { species } => species,
        _ => unreachable!("tree_species fns called on a non-species tile"),
    }
}

pub fn make(name: &str, species: TreeSpecies) -> TileDef {
    let mut def = TileDef::new(name, TileKind::TreeSpecies { species });
    match info(species).base {
        "snow" => def.connects_to_snow = true,
        "sand" => def.connects_to_sand = true,
        _ => def.connects_to_grass = true,
    }
    def.flammable = true;
    def
}

/// Same canopy assembly as `tree.rs::render`, parameterized by species base + palette.
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let species = kind_species(def);
    let inf = info(species);
    let base = g.tiles.get(inf.base);
    dispatch::render(g, screen, &base, lvl, x, y);

    if let Some(edges) = canopy_edges(species) {
        let (bx, by) = inf.art;
        let art = CanopyArt {
            lone: [
                cell(bx, by),
                cell(bx + 1, by),
                cell(bx, by + 1),
                cell(bx + 1, by + 1),
            ],
            fill: cell(bx, by + 2),
            knot: cell(bx + 1, by + 2),
            edges,
        };
        render_canopy(g, screen, def, lvl, x, y, &art, inf.col);
        if inf.darken > 0 {
            screen.darken_rect(x * 16, y * 16, 16, 16, inf.darken);
        }
        return;
    }

    let Neighbors {
        u,
        d,
        l,
        r,
        ul,
        ur,
        dl,
        dr,
    } = Neighbors::same_tile(g, def, lvl, x, y);

    // species art block: TL/TR/BL/BR standalone quarters, fill, knot-fill (see
    // module docs); fully-surrounded corners swap in the fill cells so canopies of
    // adjacent trees merge into one roof
    let (bx, by) = inf.art;

    if u && ul && l {
        screen.render(x * 16, y * 16, cell(bx, by + 2), inf.col, 0);
    } else {
        screen.render(x * 16, y * 16, cell(bx, by), inf.col, 0);
    }
    if u && ur && r {
        screen.render(x * 16 + 8, y * 16, cell(bx + 1, by + 2), inf.col2, 0);
    } else {
        screen.render(x * 16 + 8, y * 16, cell(bx + 1, by), inf.col, 0);
    }
    if d && dl && l {
        screen.render(x * 16, y * 16 + 8, cell(bx + 1, by + 2), inf.col2, 0);
    } else {
        screen.render(x * 16, y * 16 + 8, cell(bx, by + 1), inf.col1, 0);
    }
    if d && dr && r {
        screen.render(x * 16 + 8, y * 16 + 8, cell(bx, by + 2), inf.col, 0);
    } else {
        screen.render(x * 16 + 8, y * 16 + 8, cell(bx + 1, by + 1), inf.col2, 0);
    }
    if inf.darken > 0 {
        screen.darken_rect(x * 16, y * 16, 16, 16, inf.darken);
    }
}

/// Accumulated damage (in the data byte) heals over time, like the broadleaf.
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
    if let Some(tool_level) = tool_use(g, player, item, ToolType::Axe, 4) {
        let dmg = g.random.next_int_bound(10) + tool_level * 5 + 10;
        hurt_dmg(g, def, lvl, xt, yt, dmg);
        return true;
    }
    false
}

pub fn hurt_dmg(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    let species = kind_species(def);
    let inf = info(species);

    // glancing blows knock loose sticks, like the broadleaf (~1 in 6 hits)
    if g.random.next_int_bound(6) == 0 {
        let stick = crate::item::registry::get(g, "Stick");
        drop_item(g, lvl, x * 16 + 8, y * 16 + 8, stick);
    }
    // palms occasionally shake a coconut loose before falling
    if species == TreeSpecies::Palm && g.random.next_int_bound(16) == 0 {
        let coconut = crate::item::registry::get(g, "Coconut");
        drop_item(g, lvl, x * 16 + 8, y * 16 + 8, coconut);
    }

    let mut dmg = dmg;
    let mut damage = g.level(lvl).get_data(x, y) + dmg;
    if g.is_mode("creative") {
        dmg = inf.health;
        damage = inf.health;
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

    if damage >= inf.health {
        // felled: species drop tables (`registry::get` falls back to UnknownItem, so a
        // not-yet-registered food item never panics here)
        let (cx, cy) = (x * 16 + 8, y * 16 + 8);
        match species {
            TreeSpecies::Pine => {
                let wood = crate::item::registry::get(g, "Wood");
                drop_items_counted(g, lvl, cx, cy, 1, 2, &[wood]);
                // resinous branches: extra sticks instead of a resin item
                let stick = crate::item::registry::get(g, "Stick");
                drop_items_counted(g, lvl, cx, cy, 2, 4, &[stick]);
            }
            TreeSpecies::Dead => {
                let stick = crate::item::registry::get(g, "Stick");
                drop_items_counted(g, lvl, cx, cy, 2, 3, &[stick]);
            }
            TreeSpecies::Willow | TreeSpecies::FlatCrown => {
                let wood = crate::item::registry::get(g, "Wood");
                drop_items_counted(g, lvl, cx, cy, 1, 2, &[wood]);
                let stick = crate::item::registry::get(g, "Stick");
                drop_items_counted(g, lvl, cx, cy, 1, 2, &[stick]);
            }
            TreeSpecies::Palm => {
                let wood = crate::item::registry::get(g, "Wood");
                drop_items_counted(g, lvl, cx, cy, 1, 2, &[wood]);
                let coconut = crate::item::registry::get(g, "Coconut");
                drop_items_counted(g, lvl, cx, cy, 1, 2, &[coconut]);
            }
        }
        let base = g.tiles.get(inf.base);
        g.set_tile_default(lvl, x, y, &base);
    } else {
        g.level_mut(lvl).set_data(x, y, damage);
    }
}
