//! Behavior dispatch for tiles: the Rust equivalent of Java's virtual methods on `Tile`.
//! Each function matches `TileKind` and calls the per-tile module for classes that
//! override the method; everything else gets the `Tile.java` default.

#[allow(clippy::wildcard_imports)]
use super::*;
use crate::core::game::Game;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, color};
use crate::item::Item;

/* ---------------- constructors (Java `new XyzTile(...)`) ---------------- */

pub fn make_grass_tile(name: &str) -> TileDef {
    grass::make(name)
}
pub fn make_dirt_tile(name: &str) -> TileDef {
    dirt::make(name)
}
pub fn make_flower_tile(name: &str) -> TileDef {
    flower::make(name)
}
pub fn make_hole_tile(name: &str) -> TileDef {
    hole::make(name)
}
pub fn make_stairs_tile(name: &str, leads_up: bool) -> TileDef {
    stairs::make(name, leads_up)
}
pub fn make_water_tile(name: &str) -> TileDef {
    water::make(name)
}
pub fn make_rock_tile(name: &str) -> TileDef {
    rock::make(name)
}
pub fn make_tree_tile(name: &str) -> TileDef {
    tree::make(name)
}
pub fn make_sapling_tile(name: &str, on_type: &str, grows_to: &str) -> TileDef {
    sapling::make(name, on_type, grows_to)
}
pub fn make_sand_tile(name: &str) -> TileDef {
    sand::make(name)
}
pub fn make_cactus_tile(name: &str) -> TileDef {
    cactus::make(name)
}
pub fn make_lava_tile(name: &str) -> TileDef {
    lava::make(name)
}
pub fn make_lava_brick_tile(name: &str) -> TileDef {
    lava_brick::make(name)
}
pub fn make_ore_tile(ore_type: OreType) -> TileDef {
    ore::make(ore_type)
}
pub fn make_exploded_tile(name: &str) -> TileDef {
    exploded::make(name)
}
pub fn make_farm_tile(name: &str) -> TileDef {
    farm::make(name)
}
pub fn make_wheat_tile(name: &str) -> TileDef {
    wheat::make(name)
}
pub fn make_hard_rock_tile(name: &str) -> TileDef {
    hard_rock::make(name)
}
pub fn make_infinite_fall_tile(name: &str) -> TileDef {
    infinite_fall::make(name)
}
pub fn make_cloud_tile(name: &str) -> TileDef {
    cloud::make(name)
}
pub fn make_cloud_cactus_tile(name: &str) -> TileDef {
    cloud_cactus::make(name)
}
pub fn make_floor_tile(material: Material) -> TileDef {
    floor::make(material)
}
pub fn make_wall_tile(material: Material) -> TileDef {
    wall::make(material)
}
pub fn make_door_tile(material: Material) -> TileDef {
    door::make(material)
}
pub fn make_wool_tile() -> TileDef {
    wool::make()
}
pub fn make_quicksand_tile(name: &str) -> TileDef {
    quicksand::make(name)
}
pub fn make_snow_tile(name: &str) -> TileDef {
    snow::make(name)
}
pub fn make_snow_tree_tile(name: &str) -> TileDef {
    snow_tree::make(name)
}
pub fn make_tall_grass_tile(name: &str, on_tile: &str, kind: i32) -> TileDef {
    tall_grass::make(name, on_tile, kind)
}
pub fn make_pumpkin_tile(name: &str, lit: bool) -> TileDef {
    pumpkin::make(name, lit)
}
pub fn make_grave_stone_tile(name: &str, broken: bool) -> TileDef {
    grave_stone::make(name, broken)
}
pub fn make_fence_tile(name: &str) -> TileDef {
    fence::make(name)
}
pub fn make_torch_tile(on: &TileDef) -> TileDef {
    torch::make(on)
}

/* ---------------- dispatch (Java virtual methods) ---------------- */

/// Java `Tile.render` (default: sprite and/or csprite).
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    match &def.kind {
        TileKind::Mud => mud::render(g, screen, lvl, x, y),
        TileKind::DeepWater => depth::deep_water_render(g, screen, lvl, x, y),
        TileKind::DugPit => depth::dug_pit_render(g, screen, lvl, x, y),
        TileKind::Chasm => depth::chasm_render(g, screen, lvl, x, y),
        TileKind::Ladder => depth::ladder_render(g, screen, lvl, x, y),
        TileKind::Dirt => dirt::render(g, screen, def, lvl, x, y),
        TileKind::Flower => flower::render(g, screen, def, lvl, x, y),
        TileKind::Hole => hole::render(g, screen, def, lvl, x, y),
        TileKind::Stairs { .. } => stairs::render(g, screen, def, lvl, x, y),
        TileKind::Water => water::render(g, screen, def, lvl, x, y),
        TileKind::Rock => rock::render(g, screen, def, lvl, x, y),
        TileKind::Tree => tree::render(g, screen, def, lvl, x, y),
        TileKind::Sapling { .. } => sapling::render(g, screen, def, lvl, x, y),
        TileKind::Sand => sand::render(g, screen, def, lvl, x, y),
        TileKind::Cactus => cactus::render(g, screen, def, lvl, x, y),
        TileKind::Lava => lava::render(g, screen, def, lvl, x, y),
        TileKind::Ore { .. } => ore::render(g, screen, def, lvl, x, y),
        TileKind::Wheat => wheat::render(g, screen, def, lvl, x, y),
        TileKind::InfiniteFall => infinite_fall::render(g, screen, def, lvl, x, y),
        TileKind::Door { .. } => door::render(g, screen, def, lvl, x, y),
        TileKind::Wool => wool::render(g, screen, def, lvl, x, y),
        TileKind::QuickSand => quicksand::render(g, screen, def, lvl, x, y),
        TileKind::Snow => snow::render(g, screen, def, lvl, x, y),
        TileKind::SnowTree => snow_tree::render(g, screen, def, lvl, x, y),
        TileKind::TallGrass { .. } => tall_grass::render(g, screen, def, lvl, x, y),
        TileKind::Pumpkin => pumpkin::render(g, screen, def, lvl, x, y),
        TileKind::GraveStone { .. } => grave_stone::render(g, screen, def, lvl, x, y),
        TileKind::Fence => fence::render(g, screen, def, lvl, x, y),
        TileKind::Torch { .. } => torch::render(g, screen, def, lvl, x, y),
        _ => default_render(g, screen, def, lvl, x, y),
    }
}

/// The `Tile.java` default render.
pub fn default_render(
    g: &mut Game,
    screen: &mut Screen,
    def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
) {
    if let Some(sprite) = &def.sprite {
        sprite.render(screen, x << 4, y << 4);
    }
    if def.csprite.is_some() {
        csprite_render(g, screen, def, lvl, x, y, None);
    }
}

/// Java `Tile.tick` (default: nothing).
pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    match &def.kind {
        TileKind::Grass => grass::tick(g, def, lvl, xt, yt),
        TileKind::Water => water::tick(g, def, lvl, xt, yt),
        TileKind::Rock => rock::tick(g, def, lvl, xt, yt),
        TileKind::Tree => tree::tick(g, def, lvl, xt, yt),
        TileKind::Sapling { .. } => sapling::tick(g, def, lvl, xt, yt),
        TileKind::Sand => sand::tick(g, def, lvl, xt, yt),
        TileKind::Cactus => cactus::tick(g, def, lvl, xt, yt),
        TileKind::Lava => lava::tick(g, def, lvl, xt, yt),
        TileKind::Farm => farm::tick(g, def, lvl, xt, yt),
        TileKind::Wheat => wheat::tick(g, def, lvl, xt, yt),
        TileKind::HardRock => hard_rock::tick(g, def, lvl, xt, yt),
        TileKind::InfiniteFall => infinite_fall::tick(g, def, lvl, xt, yt),
        TileKind::Wall { .. } => wall::tick(g, def, lvl, xt, yt),
        TileKind::Snow => snow::tick(g, def, lvl, xt, yt),
        TileKind::SnowTree => snow_tree::tick(g, def, lvl, xt, yt),
        TileKind::TallGrass { .. } => tall_grass::tick(g, def, lvl, xt, yt),
        TileKind::GraveStone { .. } => grave_stone::tick(g, def, lvl, xt, yt),
        _ => {}
    }
}

/// Java `Tile.mayPass` (default: true).
pub fn may_pass(g: &Game, def: &TileDef, lvl: usize, x: i32, y: i32, e: &Entity) -> bool {
    // The Night Wisp floats over every tile (the removed AirWizard's flight,
    // generalized from per-tile overrides to one gate here).
    if matches!(e.kind, crate::entity::EntityKind::NightWisp(_)) {
        return true;
    }
    match &def.kind {
        TileKind::DeepWater => depth::deep_water_may_pass(g, e),
        TileKind::DugPit | TileKind::Chasm | TileKind::Ladder => true,
        TileKind::Hole => hole::may_pass(g, def, lvl, x, y, e),
        TileKind::Water => water::may_pass(g, def, lvl, x, y, e),
        TileKind::Rock => rock::may_pass(g, def, lvl, x, y, e),
        TileKind::Tree => tree::may_pass(g, def, lvl, x, y, e),
        TileKind::Cactus => cactus::may_pass(g, def, lvl, x, y, e),
        TileKind::Lava => lava::may_pass(g, def, lvl, x, y, e),
        TileKind::LavaBrick => lava_brick::may_pass(g, def, lvl, x, y, e),
        TileKind::Ore { .. } => ore::may_pass(g, def, lvl, x, y, e),
        TileKind::Exploded => exploded::may_pass(g, def, lvl, x, y, e),
        TileKind::HardRock => hard_rock::may_pass(g, def, lvl, x, y, e),
        TileKind::InfiniteFall => infinite_fall::may_pass(g, def, lvl, x, y, e),
        TileKind::Cloud => cloud::may_pass(g, def, lvl, x, y, e),
        TileKind::CloudCactus => cloud_cactus::may_pass(g, def, lvl, x, y, e),
        TileKind::Floor { .. } => floor::may_pass(g, def, lvl, x, y, e),
        TileKind::Wall { .. } => wall::may_pass(g, def, lvl, x, y, e),
        TileKind::Door { .. } => door::may_pass(g, def, lvl, x, y, e),
        TileKind::Wool => wool::may_pass(g, def, lvl, x, y, e),
        TileKind::SnowTree => snow_tree::may_pass(g, def, lvl, x, y, e),
        TileKind::TallGrass { .. } => tall_grass::may_pass(g, def, lvl, x, y, e),
        TileKind::Pumpkin => pumpkin::may_pass(g, def, lvl, x, y, e),
        TileKind::GraveStone { .. } => grave_stone::may_pass(g, def, lvl, x, y, e),
        TileKind::Fence => fence::may_pass(g, def, lvl, x, y, e),
        _ => true,
    }
}

/// Java `Tile.getLightRadius` (default: 0).
pub fn get_light_radius(g: &Game, def: &TileDef, lvl: usize, x: i32, y: i32) -> i32 {
    match &def.kind {
        TileKind::Lava => lava::get_light_radius(g, def, lvl, x, y),
        TileKind::Pumpkin => pumpkin::get_light_radius(g, def, lvl, x, y),
        TileKind::GraveStone { .. } => grave_stone::get_light_radius(g, def, lvl, x, y),
        TileKind::Torch { .. } => torch::get_light_radius(g, def, lvl, x, y),
        _ => 0,
    }
}

/// Java `Tile.hurt(level, x, y, Mob source, dmg, attackDir)` (default: false).
#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    source: &mut Entity,
    dmg: i32,
    attack_dir: Direction,
) -> bool {
    match &def.kind {
        TileKind::Flower => flower::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir),
        TileKind::Rock => rock::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir),
        TileKind::Tree => tree::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir),
        TileKind::Sapling { .. } => sapling::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir),
        TileKind::Cactus => cactus::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir),
        TileKind::Ore { .. } => ore::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir),
        TileKind::Wheat => wheat::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir),
        TileKind::HardRock => hard_rock::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir),
        TileKind::CloudCactus => cloud_cactus::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir),
        TileKind::Wall { .. } => wall::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir),
        TileKind::Door { .. } => door::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir),
        TileKind::SnowTree => snow_tree::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir),
        TileKind::TallGrass { .. } => {
            tall_grass::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir)
        }
        TileKind::GraveStone { .. } => {
            grave_stone::hurt_by(g, def, lvl, x, y, source, dmg, attack_dir)
        }
        _ => false,
    }
}

/// Java `Tile.hurt(level, x, y, dmg)` (default: nothing).
pub fn hurt_dmg(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    match &def.kind {
        TileKind::Rock => rock::hurt_dmg(g, def, lvl, x, y, dmg),
        TileKind::Tree => tree::hurt_dmg(g, def, lvl, x, y, dmg),
        TileKind::Ore { .. } => ore::hurt_dmg(g, def, lvl, x, y, dmg),
        TileKind::HardRock => hard_rock::hurt_dmg(g, def, lvl, x, y, dmg),
        TileKind::CloudCactus => cloud_cactus::hurt_dmg(g, def, lvl, x, y, dmg),
        TileKind::Wall { .. } => wall::hurt_dmg(g, def, lvl, x, y, dmg),
        TileKind::SnowTree => snow_tree::hurt_dmg(g, def, lvl, x, y, dmg),
        _ => {}
    }
}

/// Java `Tile.bumpedInto` (default: nothing).
pub fn bumped_into(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, e: &mut Entity) {
    // Floating over a cactus shouldn't prick you: the Night Wisp never contacts tiles.
    if matches!(e.kind, crate::entity::EntityKind::NightWisp(_)) {
        return;
    }
    match &def.kind {
        TileKind::Cactus => cactus::bumped_into(g, def, lvl, xt, yt, e),
        TileKind::LavaBrick => lava_brick::bumped_into(g, def, lvl, xt, yt, e),
        TileKind::Ore { .. } => ore::bumped_into(g, def, lvl, xt, yt, e),
        TileKind::CloudCactus => cloud_cactus::bumped_into(g, def, lvl, xt, yt, e),
        _ => {}
    }
}

/// Java `Tile.steppedOn` (default: nothing).
pub fn stepped_on(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, e: &mut Entity) {
    match &def.kind {
        TileKind::Exploded => exploded::stepped_on(g, def, lvl, xt, yt, e),
        TileKind::Farm => farm::stepped_on(g, def, lvl, xt, yt, e),
        TileKind::Wheat => wheat::stepped_on(g, def, lvl, xt, yt, e),
        TileKind::Sand => sand::stepped_on(g, def, lvl, xt, yt, e),
        TileKind::Snow => snow::stepped_on(g, def, lvl, xt, yt, e),
        _ => {}
    }
}

/// Java `Tile.interact` (default: false).
#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    attack_dir: Direction,
) -> bool {
    match &def.kind {
        TileKind::Mud => mud::interact(g, lvl, xt, yt, player, item, attack_dir),
        TileKind::DugPit => depth::dug_pit_interact(g, lvl, xt, yt, player, item, attack_dir),
        TileKind::Grass => grass::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Dirt => dirt::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Flower => flower::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Rock => rock::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Tree => tree::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Sand => sand::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::LavaBrick => lava_brick::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Ore { .. } => ore::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Farm => farm::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Wheat => wheat::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::HardRock => hard_rock::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Cloud => cloud::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::CloudCactus => {
            cloud_cactus::interact(g, def, lvl, xt, yt, player, item, attack_dir)
        }
        TileKind::Floor { .. } => floor::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Wall { .. } => wall::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Door { .. } => door::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Wool => wool::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::Snow => snow::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::SnowTree => snow_tree::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        TileKind::GraveStone { .. } => {
            grave_stone::interact(g, def, lvl, xt, yt, player, item, attack_dir)
        }
        TileKind::Torch { .. } => torch::interact(g, def, lvl, xt, yt, player, item, attack_dir),
        _ => false,
    }
}

/// Java `ConnectorSprite.connectsTo` (default: same class as owner).
pub fn connects_to(def: &TileDef, other: &TileDef, is_side: bool) -> bool {
    match &def.kind {
        TileKind::Grass => grass::connects_to(def, other, is_side),
        TileKind::Hole => hole::connects_to(def, other, is_side),
        TileKind::Water => water::connects_to(def, other, is_side),
        TileKind::Sand => sand::connects_to(def, other, is_side),
        TileKind::Lava => lava::connects_to(def, other, is_side),
        TileKind::Exploded => exploded::connects_to(def, other, is_side),
        TileKind::Cloud => cloud::connects_to(def, other, is_side),
        TileKind::Snow => snow::connects_to(def, other, is_side),
        _ => same_class(def, other),
    }
}

/// Java default `tile.getClass() == owner` — same TileKind variant.
pub fn same_class(def: &TileDef, other: &TileDef) -> bool {
    std::mem::discriminant(&def.kind) == std::mem::discriminant(&other.kind)
}

/// Java `ConnectorSprite.getSparseColor` (default: origCol).
pub fn get_sparse_color(def: &TileDef, tile: &TileDef, orig_col: i32) -> i32 {
    match &def.kind {
        TileKind::Hole => hole::get_sparse_color(def, tile, orig_col),
        TileKind::Water => water::get_sparse_color(def, tile, orig_col),
        TileKind::Lava => lava::get_sparse_color(def, tile, orig_col),
        _ => orig_col,
    }
}

/// Java `Tile.getName(data)` (default: name).
pub fn get_name(def: &TileDef, data: i32) -> String {
    match &def.kind {
        TileKind::Wall { .. } => wall::get_name(def, data),
        TileKind::Wool => wool::get_name(def, data),
        _ => def.name.clone(),
    }
}

/// Java `Tile.getData(String)` (default: parse int, 0 on failure).
pub fn get_data_str(def: &TileDef, data: &str) -> i32 {
    match &def.kind {
        TileKind::Wool => wool::get_data_str(def, data),
        _ => data.parse().unwrap_or(0),
    }
}

/// Java `Tile.matches(thisData, tileInfo)` (default: name equality on the base name).
pub fn matches(def: &TileDef, this_data: i32, tile_info: &str) -> bool {
    match &def.kind {
        TileKind::Wool => wool::matches(def, this_data, tile_info),
        _ => def.name == tile_info.split('_').next().unwrap_or(""),
    }
}

/// Java `Tile.getDefaultData` (no tile in this fork overrides it).
pub fn get_default_data(_def: &TileDef) -> i32 {
    0
}

/// Java `Tile.maySpawn()`.
pub fn may_spawn(def: &TileDef) -> bool {
    def.may_spawn
}

/// Java `Tile.getConnectColor(level)`.
pub fn get_connect_color(g: &Game, def: &TileDef, lvl: usize) -> i32 {
    let scolor = if let Some(sprite) = &def.sprite {
        sprite.color
    } else if let Some(csprite) = &def.csprite {
        csprite.sparse.color
    } else {
        return dirt_color(g.level(lvl).depth);
    };
    color::separate_encoded_sprite(scolor)[3]
}

/// Java `DirtTile.dCol(depth)` — delegated to the dirt tile module.
pub fn dirt_color(depth: i32) -> i32 {
    dirt::d_col(depth)
}

/// Java `ConnectorSprite.render(screen, level, x, y)` with optional color overrides
/// (Java's 3-color variant is used by some tiles).
pub fn csprite_render(
    g: &Game,
    screen: &mut Screen,
    def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    colors: Option<(i32, i32, i32)>,
) {
    let Some(cs) = &def.csprite else { return };

    let (colsparse0, colside, colfull) = match colors {
        Some(c) => c,
        None => (cs.sparse.color, cs.sides.color, cs.full.color),
    };

    let ut = g.tile_at(lvl, x, y - 1);
    let dt = g.tile_at(lvl, x, y + 1);
    let lt = g.tile_at(lvl, x - 1, y);
    let rt = g.tile_at(lvl, x + 1, y);

    let u = connects_to(def, &ut, true);
    let d = connects_to(def, &dt, true);
    let l = connects_to(def, &lt, true);
    let r = connects_to(def, &rt, true);

    let ul = connects_to(def, &g.tile_at(lvl, x - 1, y - 1), false);
    let dl = connects_to(def, &g.tile_at(lvl, x - 1, y + 1), false);
    let ur = connects_to(def, &g.tile_at(lvl, x + 1, y - 1), false);
    let dr = connects_to(def, &g.tile_at(lvl, x + 1, y + 1), false);

    let x = x << 4;
    let y = y << 4;

    let orig = colsparse0;

    let mut colsparse = get_sparse_color(def, &ut, orig);
    colsparse = get_sparse_color(def, &lt, colsparse);

    if u && l {
        if ul || !cs.check_corners {
            cs.full.render_pixel_color(1, 1, screen, x, y, colfull);
        } else {
            cs.sides.render_pixel_color(0, 0, screen, x, y, colside);
        }
    } else {
        cs.sparse.render_pixel_color(
            if l { 1 } else { 2 },
            if u { 1 } else { 2 },
            screen,
            x,
            y,
            colsparse,
        );
    }

    let mut colsparse = get_sparse_color(def, &ut, orig);
    colsparse = get_sparse_color(def, &rt, colsparse);

    if u && r {
        if ur || !cs.check_corners {
            cs.full.render_pixel_color(0, 1, screen, x + 8, y, colfull);
        } else {
            cs.sides.render_pixel_color(1, 0, screen, x + 8, y, colside);
        }
    } else {
        cs.sparse.render_pixel_color(
            if r { 1 } else { 0 },
            if u { 1 } else { 2 },
            screen,
            x + 8,
            y,
            colsparse,
        );
    }

    let mut colsparse = get_sparse_color(def, &dt, orig);
    colsparse = get_sparse_color(def, &lt, colsparse);

    if d && l {
        if dl || !cs.check_corners {
            cs.full.render_pixel_color(1, 0, screen, x, y + 8, colfull);
        } else {
            cs.sides.render_pixel_color(0, 1, screen, x, y + 8, colside);
        }
    } else {
        cs.sparse.render_pixel_color(
            if l { 1 } else { 2 },
            if d { 1 } else { 0 },
            screen,
            x,
            y + 8,
            colsparse,
        );
    }

    let mut colsparse = get_sparse_color(def, &dt, orig);
    colsparse = get_sparse_color(def, &rt, colsparse);

    if d && r {
        if dr || !cs.check_corners {
            cs.full
                .render_pixel_color(0, 0, screen, x + 8, y + 8, colfull);
        } else {
            cs.sides
                .render_pixel_color(1, 1, screen, x + 8, y + 8, colside);
        }
    } else {
        cs.sparse.render_pixel_color(
            if r { 1 } else { 0 },
            if d { 1 } else { 0 },
            screen,
            x + 8,
            y + 8,
            colsparse,
        );
    }
}
