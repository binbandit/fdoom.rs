//! Port of `fdoom.level.Structure` — prefab tile structures (dungeon gate, etc.).
//!
//! Java stores `Tile` references and `Furniture` prototypes; the port stores tile names
//! (resolved through `Tiles` at draw time) and furniture constructor functions (Java
//! `furniture.clone()` becomes constructing a fresh entity). Java's `HashSet<TilePoint>`
//! (equality on x, y, tile id) becomes a `Vec` with the same duplicate rejection; the
//! set's iteration order is unspecified in Java, but no structure places two different
//! tiles at one point, so order does not affect the result.

use crate::entity::Entity;
use crate::entity::furniture::lantern::{self, LanternType};
use crate::gfx::Point;
use crate::level::Level;
use crate::level::tile::{Tiles, dispatch};

/// Java `Structure.TilePoint`.
struct TilePoint {
    x: i32,
    y: i32,
    /// Tile name, resolved via `Tiles::get` (Java stored the `Tile` itself).
    tile: &'static str,
}

/// Java `Structure` — this stores structures that can be drawn at any location.
pub struct Structure {
    tiles: Vec<TilePoint>,
    furniture: Vec<(Point, fn() -> Entity)>,
}

impl Default for Structure {
    fn default() -> Self {
        Self::new()
    }
}

impl Structure {
    pub fn new() -> Structure {
        Structure {
            tiles: Vec::new(),
            furniture: Vec::new(),
        }
    }

    /// Java `setTile(x, y, tile)` — HashSet add (duplicates by (x, y, tile) rejected).
    pub fn set_tile(&mut self, x: i32, y: i32, tile: &'static str) {
        if !self
            .tiles
            .iter()
            .any(|p| p.x == x && p.y == y && p.tile == tile)
        {
            self.tiles.push(TilePoint { x, y, tile });
        }
    }

    /// Java `addFurniture(x, y, furniture)`.
    pub fn add_furniture(&mut self, x: i32, y: i32, make: fn() -> Entity) {
        self.furniture.push((Point::new(x, y), make));
    }

    /// Java `draw(level, xt, yt)`. `lvl_idx` is the level's index in `g.levels` (the
    /// Java `Level` object knew itself; entity queueing needs it here).
    pub fn draw(&self, level: &mut Level, tiles: &Tiles, xt: i32, yt: i32, lvl_idx: usize) {
        for p in &self.tiles {
            // Java level.setTile(x, y, t) — uses the tile's default data.
            let t = tiles.get(p.tile);
            level.set_tile_id(xt + p.x, yt + p.y, t.id, dispatch::get_default_data(&t));
        }

        for (p, make) in &self.furniture {
            level.add_at(make(), xt + p.x, yt + p.y, true, lvl_idx);
        }
    }
}

/// Java `Structure.dungeonGate.draw(this, x, y)` from `Level` — convenience wrapper for
/// call sites that hold a `Game`.
pub fn draw_dungeon_gate(g: &mut crate::core::game::Game, lvl: usize, x: i32, y: i32) {
    let s = dungeon_gate();
    if let Some(level) = g.levels[lvl].as_mut() {
        s.draw(level, &g.tiles, x, y, lvl);
    }
}

/// Java `Structure.dungeonGate` (static initializer). Build once per use; the contents
/// are constant.
pub fn dungeon_gate() -> Structure {
    let mut s = Structure::new();
    s.add_furniture(-1, 1, || lantern::new(LanternType::Iron));
    s.set_tile(-1, 0, "Obsidian");
    s.set_tile(1, 0, "Obsidian");
    s.set_tile(2, 0, "Obsidian Door");
    s.set_tile(-2, 0, "Obsidian Door");
    s.set_tile(0, -1, "Obsidian");
    s.set_tile(0, 1, "Obsidian");
    s.set_tile(0, 2, "Obsidian Door");
    s.set_tile(0, -2, "Obsidian Door");
    s.set_tile(-1, -1, "Obsidian");
    s.set_tile(-1, 1, "Obsidian");
    s.set_tile(1, -1, "Obsidian");
    s.set_tile(1, 1, "Obsidian");
    s.set_tile(3, 0, "Obsidian");
    s.set_tile(-3, 0, "Obsidian");
    s.set_tile(3, -1, "Obsidian");
    s.set_tile(-3, -1, "Obsidian");
    s.set_tile(3, 1, "Obsidian");
    s.set_tile(-3, 1, "Obsidian");
    s.set_tile(4, 0, "Obsidian");
    s.set_tile(-4, 0, "Obsidian");
    s.set_tile(4, -1, "Obsidian");
    s.set_tile(-4, -1, "Obsidian");
    s.set_tile(4, 1, "Obsidian");
    s.set_tile(-4, 1, "Obsidian");
    s.set_tile(0, 3, "Obsidian");
    s.set_tile(0, -3, "Obsidian");
    s.set_tile(1, -3, "Obsidian");
    s.set_tile(-1, -3, "Obsidian");
    s.set_tile(1, 3, "Obsidian");
    s.set_tile(-1, 3, "Obsidian");
    s.set_tile(0, 4, "Obsidian");
    s.set_tile(0, -4, "Obsidian");
    s.set_tile(1, -4, "Obsidian");
    s.set_tile(-1, -4, "Obsidian");
    s.set_tile(1, 4, "Obsidian");
    s.set_tile(-1, 4, "Obsidian");
    s.set_tile(-2, -2, "Obsidian Wall");
    s.set_tile(-3, -2, "Obsidian Wall");
    s.set_tile(-3, 2, "Obsidian Wall");
    s.set_tile(-2, 1, "Obsidian Wall");
    s.set_tile(2, -2, "Obsidian Wall");
    s.set_tile(4, -2, "Obsidian Wall");
    s.set_tile(4, 2, "Obsidian Wall");
    s.set_tile(-4, -2, "Obsidian Wall");
    s.set_tile(-4, 2, "Obsidian Wall");
    s.set_tile(1, -2, "Obsidian Wall");
    s.set_tile(-2, 2, "Obsidian Wall");
    s.set_tile(2, 3, "Obsidian Wall");
    s.set_tile(2, 4, "Obsidian Wall");
    s.set_tile(-2, -3, "Obsidian Wall");
    s.set_tile(-2, -4, "Obsidian Wall");
    s.set_tile(2, -3, "Obsidian Wall");
    s.set_tile(2, -4, "Obsidian Wall");
    s.set_tile(-2, 3, "Obsidian Wall");
    s.set_tile(-2, 4, "Obsidian Wall");
    s.set_tile(3, -2, "Obsidian Wall");
    s.set_tile(3, 2, "Obsidian Wall");
    s.set_tile(2, 2, "Obsidian Wall");
    s.set_tile(-1, 2, "Obsidian Wall");
    s.set_tile(2, -1, "Obsidian Wall");
    s.set_tile(2, 1, "Obsidian Wall");
    s.set_tile(1, 2, "Obsidian Wall");
    s.set_tile(-2, -1, "Obsidian Wall");
    s.set_tile(-1, -2, "Obsidian Wall");
    s
}
