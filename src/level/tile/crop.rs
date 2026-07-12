//! Row crops (farming wave): Carrot, Potato and Corn crops plus the Pumpkin Vine —
//! all planted on farmland from world-sourced seed stock (wild plants, panned
//! tubers, village plots) and grown on the wheat clock.
//!
//! Per-tile data byte = age 0..50, exactly like wheat: three drawn stages
//! (sprout / young / mature), a partial harvest from age 40 and the full yield at
//! 50. Growth leans into the weather sim — a creek-side plot grows faster, and rain
//! waters the field ([`crate::core::weather::growth_boost`]). The Pumpkin Vine is
//! the odd one out: at full age the vine *becomes* a pumpkin tile, which is then
//! smashed for its fruit (and seeds) like any wild pumpkin.
//!
//! Art: per-crop stage strips `tiles/crop_*.png` (3 stage blocks of 2x2 cells,
//! auto-allocated, true color), drawn over the farmland base.

use super::{TileDef, TileKind, dispatch, tool_use};
use crate::core::game::Game;
use crate::entity::{Direction, Entity};
use crate::gfx::Screen;
use crate::item::{Item, ToolType};
use crate::level::{drop_item, drop_items_counted};

/// Which crop a `TileKind::Crop` tile is growing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropKind {
    Carrot,
    Potato,
    Corn,
    /// Matures into a `pumpkin` tile instead of being harvested in place.
    PumpkinVine,
}

/// Full age: the crop is ripe (`data` byte, wheat's scale).
const RIPE_AGE: i32 = 50;
/// A thin early harvest is available from this age (matches wheat).
const PART_AGE: i32 = 40;

impl CropKind {
    /// The stage-strip sprite part under `assets/sprites/`.
    fn art(self) -> &'static str {
        match self {
            CropKind::Carrot => "tiles/crop_carrot",
            CropKind::Potato => "tiles/crop_potato",
            CropKind::Corn => "tiles/crop_corn",
            CropKind::PumpkinVine => "tiles/pumpkin_vine",
        }
    }

    /// Stage block index into the strip for a given age.
    fn stage(self, age: i32) -> i32 {
        match self {
            // the vine has two drawn stages; ripeness turns the tile into a pumpkin
            CropKind::PumpkinVine => i32::from(age >= 25),
            _ => {
                if age >= PART_AGE {
                    2
                } else if age >= 17 {
                    1
                } else {
                    0
                }
            }
        }
    }

    /// (seed item, produce item) dropped by [`harvest`].
    fn drops(self) -> (&'static str, &'static str) {
        match self {
            CropKind::Carrot => ("Carrot Seeds", "Carrot"),
            CropKind::Potato => ("Seed Potato", "Potato"),
            CropKind::Corn => ("Corn Kernels", "Corn"),
            CropKind::PumpkinVine => ("Pumpkin Seeds", "Pumpkin"),
        }
    }
}

pub fn make(name: &str, crop: CropKind) -> TileDef {
    TileDef::new(name, TileKind::Crop { crop })
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let TileKind::Crop { crop } = def.kind else {
        return;
    };
    let farmland = g.tiles.get("farmland");
    dispatch::render(g, screen, &farmland, lvl, x, y);

    let age = g.level(lvl).get_data(x, y);
    let cell = crate::assets::sprite_pos(crop.art()) + crop.stage(age) * 2;
    screen.render(x * 16, y * 16, cell, 0, 0);
    screen.render(x * 16 + 8, y * 16, cell + 1, 0, 0);
    screen.render(x * 16, y * 16 + 8, cell + 32, 0, 0);
    screen.render(x * 16 + 8, y * 16 + 8, cell + 33, 0, 0);
}

/// Wheat's `IfWater` check: any water in the surrounding ring.
fn near_water(g: &Game, lvl: usize, xs: i32, ys: i32) -> bool {
    crate::level::get_area_tiles(g, lvl, xs, ys, 1, 1)
        .iter()
        .any(|t| t.name == "WATER")
}

pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let TileKind::Crop { crop } = def.kind else {
        return;
    };
    if g.random.next_int_bound(2) == 0 {
        return;
    }
    let age = g.level(lvl).get_data(xt, yt);
    if age >= RIPE_AGE {
        // ripeness turns a vine into the real pumpkin tile; row crops just wait
        if crop == CropKind::PumpkinVine {
            let pumpkin = g.tiles.get("pumpkin");
            g.set_tile_default(lvl, xt, yt, &pumpkin);
        }
        return;
    }
    // the wheat clock, leaning into the weather sim: creek-side plots drink from
    // the bank, and rain waters the whole field
    let mut step = 1;
    if near_water(g, lvl, xt, yt) {
        step += 1;
    }
    if crate::core::weather::growth_boost(g) {
        step += 1;
    }
    g.level_mut(lvl)
        .set_data(xt, yt, (age + step).min(RIPE_AGE));
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
    if tool_use(g, player, item, ToolType::Shovel, 4).is_some() {
        let dirt = g.tiles.get("dirt");
        g.set_tile_default(lvl, xt, yt, &dirt);
        return true;
    }
    false
}

/// Trampling: same odds family as wheat — a careless boot can harvest early.
pub fn stepped_on(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, e: &mut Entity) {
    if g.random.next_int_bound(60) != 0 {
        return;
    }
    if g.level(lvl).get_data(xt, yt) < 2 {
        return;
    }
    harvest(g, def, lvl, xt, yt, e);
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    source: &mut Entity,
    _dmg: i32,
    _attack_dir: Direction,
) -> bool {
    harvest(g, def, lvl, x, y, source);
    true
}

/// Wheat's harvest scheme: seed stock always comes back, produce scales with age.
fn harvest(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, entity: &mut Entity) {
    let TileKind::Crop { crop } = def.kind else {
        return;
    };
    let age = g.level(lvl).get_data(x, y);
    let (seed_name, produce_name) = crop.drops();

    let seeds = crate::item::registry::get(g, seed_name);
    drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[seeds]);

    // an unripe vine only returns seeds; a ripe one is already a pumpkin tile
    if crop != CropKind::PumpkinVine {
        let count = if age >= RIPE_AGE {
            g.random.next_int_bound(2) + 2
        } else if age >= PART_AGE {
            g.random.next_int_bound(2) + 1
        } else {
            0
        };
        let produce = crate::item::registry::get(g, produce_name);
        for _ in 0..count {
            drop_item(g, lvl, x * 16 + 8, y * 16 + 8, produce.clone());
        }
        if age >= RIPE_AGE && entity.is_player() {
            let points = g.random.next_int_bound(5) + 1;
            let score_mode = g.is_mode("score");
            entity.player_mut().add_score(points, score_mode);
        }
    }

    let dirt = g.tiles.get("dirt");
    g.set_tile_default(lvl, x, y, &dirt);
}
