//! Biome tree species (sandbox era, no Java counterpart): Pine, Dead Tree, Willow,
//! Palm, Flat-Crown. They share the broadleaf `tree.rs` behavior — same axe interact,
//! same damage-in-data-byte accounting — but differ in base ground tile, canopy palette,
//! health, and drops. The classic broadleaf stays `TileKind::Tree` in `tree.rs`.
//!
//! Each species has its own dedicated true-color cell set (artgen `flora_cells`,
//! rows 26..=28): a 2x3 block of [TL, TR / BL, BR standalone quarters / fill,
//! knot-fill] — the same six roles the broadleaf samples — so adjacent trees merge
//! into one connected woodland roof exactly like `tree.rs` does.

use super::{TileDef, TileKind, TreeSpecies, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::mob::player_behavior::pay_stamina;
use crate::entity::particle::{new_smash_particle, new_text_particle};
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, color};
use crate::item::{Item, ItemKind, ToolType};
use crate::level::{drop_item, drop_items_counted};

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
    def
}

/// Same canopy assembly as `tree.rs::render`, parameterized by species base + palette.
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let inf = info(kind_species(def));
    let base = g.tiles.get(inf.base);
    dispatch::render(g, screen, &base, lvl, x, y);

    let u = g.tile_at(lvl, x, y - 1).same_tile(def);
    let l = g.tile_at(lvl, x - 1, y).same_tile(def);
    let r = g.tile_at(lvl, x + 1, y).same_tile(def);
    let d = g.tile_at(lvl, x, y + 1).same_tile(def);
    let ul = g.tile_at(lvl, x - 1, y - 1).same_tile(def);
    let ur = g.tile_at(lvl, x + 1, y - 1).same_tile(def);
    let dl = g.tile_at(lvl, x - 1, y + 1).same_tile(def);
    let dr = g.tile_at(lvl, x + 1, y + 1).same_tile(def);

    // species art block: TL/TR/BL/BR standalone quarters, fill, knot-fill (see
    // module docs); fully-surrounded corners swap in the fill cells so canopies of
    // adjacent trees merge into one roof
    let (bx, by) = inf.art;
    let pos = |cx: i32, cy: i32| cx + cy * 32;

    if u && ul && l {
        screen.render(x * 16, y * 16, pos(bx, by + 2), inf.col, 0);
    } else {
        screen.render(x * 16, y * 16, pos(bx, by), inf.col, 0);
    }
    if u && ur && r {
        screen.render(x * 16 + 8, y * 16, pos(bx + 1, by + 2), inf.col2, 0);
    } else {
        screen.render(x * 16 + 8, y * 16, pos(bx + 1, by), inf.col, 0);
    }
    if d && dl && l {
        screen.render(x * 16, y * 16 + 8, pos(bx + 1, by + 2), inf.col2, 0);
    } else {
        screen.render(x * 16, y * 16 + 8, pos(bx, by + 1), inf.col1, 0);
    }
    if d && dr && r {
        screen.render(x * 16 + 8, y * 16 + 8, pos(bx, by + 2), inf.col, 0);
    } else {
        screen.render(x * 16 + 8, y * 16 + 8, pos(bx + 1, by + 1), inf.col2, 0);
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
    if let ItemKind::Tool {
        ttype,
        level: tool_level,
        ..
    } = &item.kind
    {
        let (ttype, tool_level) = (*ttype, *tool_level);
        if ttype == ToolType::Axe
            && pay_stamina(player, 4 - tool_level)
            && item.pay_durability(g.is_mode("creative"))
        {
            let dmg = g.random.next_int_bound(10) + tool_level * 5 + 10;
            hurt_dmg(g, def, lvl, xt, yt, dmg);
            return true;
        }
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

    g.play_sound(Sound::MonsterHurt); // JAVA convention: SmashParticle plays this
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
