//! The M-key world map.
//!
//! Surface: a biome-colored chart (shared palette with `worldview`) revealed chunk by
//! chunk as the player explores — a chunk counts as explored once the player has caused
//! it to stream in, whether it is loaded right now or was persisted to the save's
//! `chunks/` directory when it streamed back out. Structure pips mark placements whose
//! origin chunk has been explored, and an arrow marks the player.
//!
//! Mines and finite set-piece levels keep the classic per-tile map with visibility fog.

use std::collections::HashSet;

use crate::core::game::Game;
use crate::gfx::biome_palette::biome_color;
use crate::gfx::{Screen, color, font, sprite_sheet};
use crate::level::chunk::{CHUNK_SIZE, chunk_coord};
use crate::level::infinite_gen::biome_at;
use crate::level::structures_gen::placements_in_rect;

use super::display::{Display, DisplayBase, display_tick_default};

/// Surface chart: pixels per chunk, so one map pixel covers `CHUNK_SIZE / PX_PER_CHUNK`
/// (= 16) tiles and the radius-2 streaming halo shows as a readable blot, not a speck.
const PX_PER_CHUNK: i32 = 4;
const TILES_PER_PX: i32 = CHUNK_SIZE / PX_PER_CHUNK;
/// Surface chart size in pixels — 36x36 chunks (2304 tiles) around the player.
/// Multiple of both `PX_PER_CHUNK` and the 8px UI grid.
const SURFACE_MAP: i32 = 144;

const UNEXPLORED: i32 = 0x11_1318;
const PIP_COLOR: i32 = 0xFF8C1A;
const ARROW_COLOR: i32 = 0xFF2A2A;

pub struct MapMenu {
    base: DisplayBase,
    img_pixels: Vec<Option<Vec<i32>>>,
    current_level: usize,
}

impl MapMenu {
    pub fn new(g: &Game) -> MapMenu {
        MapMenu {
            base: DisplayBase::default(),
            img_pixels: vec![None; g.levels.len()],
            current_level: g.current_level,
        }
    }

    /// The surface of an infinite world gets the biome chart; everything else (mines,
    /// finite set-piece levels) keeps the per-tile map.
    fn is_surface_chart(g: &Game, maplevel: usize) -> bool {
        let level = g.level(maplevel);
        level.is_infinite() && level.depth == 0
    }

    fn player_tile(g: &Game) -> (i32, i32) {
        g.try_player()
            .map(|p| {
                (
                    p.c.x / sprite_sheet::TILE_SIZE,
                    p.c.y / sprite_sheet::TILE_SIZE,
                )
            })
            .unwrap_or((0, 0))
    }

    /// Top-left chunk of the surface chart window (player chunk centered).
    fn surface_origin(g: &Game) -> (i32, i32) {
        let (ptx, pty) = Self::player_tile(g);
        let half = SURFACE_MAP / PX_PER_CHUNK / 2;
        (chunk_coord(ptx) - half, chunk_coord(pty) - half)
    }

    /// Map image dimensions and tile origin: biome chart window for the infinite
    /// surface, a fixed tile window centered on the player for infinite mines, the
    /// whole level for finite maps.
    pub fn map_window(g: &Game, maplevel: usize) -> (i32, i32, i32, i32) {
        let level = g.level(maplevel);
        if Self::is_surface_chart(g, maplevel) {
            let (c0x, c0y) = Self::surface_origin(g);
            (SURFACE_MAP, SURFACE_MAP, c0x * CHUNK_SIZE, c0y * CHUNK_SIZE)
        } else if level.is_infinite() {
            let (px, py) = Self::player_tile(g);
            (128, 128, px - 64, py - 64)
        } else {
            (level.w, level.h, 0, 0)
        }
    }

    /// Chunks the player has caused to exist on this level: loaded right now, or
    /// persisted to the save's chunk directory when they streamed back out.
    ///
    /// The path mirrors `saveload::save::chunk_dir` (kept private there); the map only
    /// needs file names, never contents.
    pub fn revealed_chunks(g: &Game, maplevel: usize) -> HashSet<(i32, i32)> {
        let level = g.level(maplevel);
        let mut out: HashSet<(i32, i32)> = level
            .chunks
            .as_ref()
            .map(|c| c.loaded_coords().into_iter().collect())
            .unwrap_or_default();
        let dir = g
            .game_dir
            .join("saves")
            .join(g.world_name.to_lowercase())
            .join("chunks")
            .join(level.depth.to_string());
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let name = e.file_name();
                let Some(stem) = name.to_str().and_then(|n| n.strip_suffix(".bin")) else {
                    continue;
                };
                let Some((cx, cy)) = stem.split_once('_') else {
                    continue;
                };
                if let (Ok(cx), Ok(cy)) = (cx.parse(), cy.parse()) {
                    out.insert((cx, cy));
                }
            }
        }
        out
    }

    /// Java `MapMenu.getMapImage(maplevel)`, dispatched by level kind.
    pub fn get_map_image(g: &Game, maplevel: usize) -> Vec<i32> {
        if Self::is_surface_chart(g, maplevel) {
            Self::surface_map_image(g, maplevel)
        } else {
            Self::tile_map_image(g, maplevel)
        }
    }

    /// The biome chart: explored chunks in biome colors, the rest dark.
    fn surface_map_image(g: &Game, maplevel: usize) -> Vec<i32> {
        let seed = g.world_seed;
        let revealed = Self::revealed_chunks(g, maplevel);
        let all = g.is_mode("Creative");
        let (c0x, c0y) = Self::surface_origin(g);
        let mw = SURFACE_MAP;
        let mut pixels = vec![UNEXPLORED; (mw * mw) as usize];

        for py in 0..mw {
            let ccy = c0y + py / PX_PER_CHUNK;
            let ty = ccy * CHUNK_SIZE + (py % PX_PER_CHUNK) * TILES_PER_PX + TILES_PER_PX / 2;
            for px in 0..mw {
                let ccx = c0x + px / PX_PER_CHUNK;
                if all || revealed.contains(&(ccx, ccy)) {
                    let tx =
                        ccx * CHUNK_SIZE + (px % PX_PER_CHUNK) * TILES_PER_PX + TILES_PER_PX / 2;
                    pixels[(px + py * mw) as usize] = biome_color(biome_at(seed, tx, ty)) as i32;
                }
            }
        }

        // structure pips — placement origins are pure gen, so "discovered" simply means
        // the origin chunk has been explored (no separate tracking needed)
        let (tx0, ty0) = (c0x * CHUNK_SIZE, c0y * CHUNK_SIZE);
        let span = mw * TILES_PER_PX;
        for p in placements_in_rect(seed, tx0, ty0, tx0 + span - 1, ty0 + span - 1) {
            if all || revealed.contains(&(chunk_coord(p.x), chunk_coord(p.y))) {
                let (mx, my) = ((p.x - tx0) / TILES_PER_PX, (p.y - ty0) / TILES_PER_PX);
                Self::stamp(&mut pixels, mw, mx, my, &PIP_SHAPE, PIP_COLOR);
            }
        }

        // the player arrow (points up; the chart is north-up)
        let (ptx, pty) = Self::player_tile(g);
        let (mx, my) = ((ptx - tx0) / TILES_PER_PX, (pty - ty0) / TILES_PER_PX);
        Self::stamp(&mut pixels, mw, mx, my, &ARROW_SHAPE, ARROW_COLOR);

        pixels
    }

    /// Stamp a small shape with a 1px black outline (so markers read on any biome).
    fn stamp(pixels: &mut [i32], mw: i32, cx: i32, cy: i32, shape: &[(i32, i32)], col: i32) {
        let mut set = |x: i32, y: i32, c: i32| {
            if x >= 0 && y >= 0 && x < mw && y < mw {
                pixels[(x + y * mw) as usize] = c;
            }
        };
        for &(dx, dy) in shape {
            for ny in -1..=1 {
                for nx in -1..=1 {
                    set(cx + dx + nx, cy + dy + ny, 0x000000);
                }
            }
        }
        for &(dx, dy) in shape {
            set(cx + dx, cy + dy, col);
        }
    }

    /// The classic per-tile map with visibility fog (mines + finite levels).
    fn tile_map_image(g: &Game, maplevel: usize) -> Vec<i32> {
        let level = g.level(maplevel);
        let (mw, mh, ox, oy) = Self::map_window(g, maplevel);
        let mut pixels = vec![0i32; (mw * mh) as usize];

        // Hoisted tile-id lookups (they're constant per frame). Note some legacy names —
        // "treeSapling", "cactusSapling", "reed", "tussock", "campfire" — are not
        // registered tiles: they log once and fall back to tile 0, and their map colors
        // are effectively dead. Kept so the palette table below matches the old map.
        let water = g.tiles.get("water").id;
        let deep_water = g.tiles.get("Deep Water").id;
        let dug_pit = g.tiles.get("Dug Pit").id;
        let chasm = g.tiles.get("Chasm").id;
        let ladder = g.tiles.get("Ladder").id;
        let iron_ore = g.tiles.get("iron Ore").id;
        let gold_ore = g.tiles.get("gold Ore").id;
        let gem_ore = g.tiles.get("gem Ore").id;
        let grass = g.tiles.get("grass").id;
        let rock = g.tiles.get("rock").id;
        let dirt = g.tiles.get("dirt").id;
        let sand = g.tiles.get("sand").id;
        let stone_bricks = g.tiles.get("Stone Bricks").id;
        let tree = g.tiles.get("tree").id;
        let obsidian_wall = g.tiles.get("Obsidian Wall").id;
        let obsidian = g.tiles.get("Obsidian").id;
        let lava = g.tiles.get("lava").id;
        let cloud = g.tiles.get("cloud").id;
        let stairs_down = g.tiles.get("Stairs Down").id;
        let stairs_up = g.tiles.get("Stairs Up").id;
        let cloud_cactus = g.tiles.get("Cloud Cactus").id;
        let flower = g.tiles.get("flower").id;
        let cactus = g.tiles.get("cactus").id;
        let hole = g.tiles.get("hole").id;
        let tree_sapling = g.tiles.get("treeSapling").id;
        let cactus_sapling = g.tiles.get("cactusSapling").id;
        let farmland = g.tiles.get("farmland").id;
        let wheat = g.tiles.get("wheat").id;
        let infinite_fall = g.tiles.get("Infinite Fall").id;
        let reed = g.tiles.get("reed").id;
        let tussock = g.tiles.get("tussock").id;
        let campfire = g.tiles.get("campfire").id;
        let snow = g.tiles.get("snow").id;
        let snow_tree = g.tiles.get("snow tree").id;
        let tall_grass = g.tiles.get("Tall Grass").id;

        let creative = g.is_mode("Creative");

        let mut y = 0;
        while y < mh {
            let mut x = 0;
            while x < mw {
                let i = (x + y * mw) as usize;
                let (tx, ty) = (x + ox, y + oy);
                let (seen, tile_here) = match &level.chunks {
                    Some(chunks) => (chunks.is_visible(tx, ty), chunks.tile(tx, ty)),
                    None => (
                        level.visible[(tx + ty * level.w) as usize],
                        Some(level.tiles[(tx + ty * level.w) as usize]),
                    ),
                };
                if let Some(check_value) = tile_here.filter(|_| seen || creative) {
                    // a run of independent ifs, deliberately: later matches overwrite
                    // earlier ones
                    if check_value == water {
                        pixels[i] = 0x000080;
                    }
                    if check_value == deep_water {
                        pixels[i] = 0x000040;
                    }
                    if check_value == dug_pit {
                        pixels[i] = 0x3d2b1f;
                    }
                    if check_value == chasm {
                        pixels[i] = 0x101010;
                    }
                    if check_value == ladder {
                        pixels[i] = 0xc0c060;
                    }
                    if check_value == iron_ore {
                        pixels[i] = 0x000080;
                    }
                    if check_value == gold_ore {
                        pixels[i] = 0x000080;
                    }
                    if check_value == gem_ore {
                        pixels[i] = 0x000080;
                    }
                    if check_value == grass {
                        pixels[i] = 0x00ff00;
                    }
                    if check_value == rock {
                        pixels[i] = 0xa0a0a0;
                    }
                    if check_value == dirt {
                        pixels[i] = 0x604040;
                    }
                    if check_value == sand {
                        pixels[i] = 0xa0a040;
                    }
                    if check_value == stone_bricks {
                        pixels[i] = 0xa0a040;
                    }
                    if check_value == tree {
                        pixels[i] = 0x003000;
                    }
                    if check_value == obsidian_wall {
                        pixels[i] = 0x0aa0a0;
                    }
                    if check_value == obsidian {
                        pixels[i] = 0x000000;
                    }
                    if check_value == lava {
                        pixels[i] = 0xff2020;
                    }
                    if check_value == cloud {
                        pixels[i] = 0xa0a0a0;
                    }
                    if check_value == stairs_down {
                        pixels[i] = 0xffffff;
                    }
                    if check_value == stairs_up {
                        pixels[i] = 0xffffff;
                    }
                    if check_value == cloud_cactus {
                        pixels[i] = 0xff00ff;
                    }

                    if check_value == flower {
                        pixels[i] = 0x208020;
                    }
                    if check_value == cactus {
                        pixels[i] = color::get_byte(level.sand_color - 110);
                    }
                    if check_value == hole {
                        pixels[i] = 0x604040;
                    }
                    if check_value == tree_sapling {
                        pixels[i] = 0x003000;
                    }
                    if check_value == cactus_sapling {
                        pixels[i] = color::get_byte(level.sand_color);
                    }
                    if check_value == farmland {
                        pixels[i] = 0x604040;
                    }
                    if check_value == wheat {
                        pixels[i] = 0x604040;
                    }
                    if check_value == infinite_fall {
                        pixels[i] = color::get_byte(334);
                    }
                    if check_value == reed {
                        pixels[i] = color::get_byte(125);
                    }
                    if check_value == tussock {
                        pixels[i] = color::get_byte(30);
                    }
                    if check_value == campfire {
                        // dead branch: "campfire" is not a tile name (see the note above)
                        pixels[i] = color::get_byte(410);
                    }
                    if check_value == snow {
                        pixels[i] = 0xffffff;
                    }
                    if check_value == snow_tree {
                        pixels[i] = 0x800080;
                    }
                    if check_value == tall_grass {
                        pixels[i] = 0xff0000;
                    }
                }
                x += 1;
            }
            y += 1;
        }

        // the player marker (a red + shape)
        if let Some(player) = g.try_player() {
            let px = player.c.x / sprite_sheet::TILE_SIZE - ox;
            let py = player.c.y / sprite_sheet::TILE_SIZE - oy;
            let mut y2 = py - 1;
            while y2 <= py + 1 {
                let mut x = px - 1;
                while x <= px + 1 {
                    if (y2 == py || x == px) && x >= 0 && y2 >= 0 && x < mw && y2 < mh {
                        pixels[(x + y2 * mw) as usize] = 0xff0000;
                    }
                    x += 1;
                }
                y2 += 1;
            }
        }

        pixels
    }

    /// Smoked-glass panel with the standard slate border sprites (the Menu look,
    /// without needing a Menu). `w`/`h` in pixels, multiples of 8.
    fn glass_frame(s: &mut Screen, title: &str, x0: i32, y0: i32, w: i32, h: i32) {
        s.darken_rect_screen(x0, y0, w, h, 185);
        // dark slate edges, matching MenuBuilder's defaults (stroke 0 / fill 111 / 333)
        let edge = color::get4(-1, 0, 111, 333);
        let right = x0 + w - sprite_sheet::BOX_WIDTH;
        let bottom = y0 + h - sprite_sheet::BOX_WIDTH;
        let mut y = y0;
        while y <= bottom {
            let mut x = x0;
            while x <= right {
                let xend = x == x0 || x == right;
                let yend = y == y0 || y == bottom;
                if xend || yend {
                    let spriteoffset = if xend && yend {
                        0
                    } else if yend {
                        1
                    } else {
                        2
                    };
                    let mirrors =
                        (if x == right { 1 } else { 0 }) + (if y == bottom { 2 } else { 0 });
                    s.render(x, y, spriteoffset + 13 * 32, edge, mirrors);
                }
                x += sprite_sheet::BOX_WIDTH;
            }
            y += sprite_sheet::BOX_WIDTH;
        }
        let tx = x0 + (w - title.chars().count() as i32 * 8) / 2;
        font::draw(title, s, tx, y0, font::default_title_color());
    }
}

impl Display for MapMenu {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn init(&mut self, g: &mut Game) {
        // no parent display: closing the map goes straight back to the game
        g.display.stack.clear();
        self.img_pixels = vec![None; g.levels.len()];
        self.current_level = g.current_level;
    }

    fn tick(&mut self, g: &mut Game) {
        display_tick_default(&mut self.base, g);
        if g.input.get_key("pause").clicked {
            g.exit_menu();
        }
        if self.img_pixels[self.current_level].is_none() {
            self.img_pixels[self.current_level] = Some(Self::get_map_image(g, self.current_level));
        }
    }

    fn render(&mut self, s: &mut Screen, g: &mut Game) {
        let Some(pixels) = &self.img_pixels[self.current_level] else {
            return;
        };
        let level = g.level(self.current_level);
        let depth = level.depth;
        let (mw, mh, _, _) = Self::map_window(g, self.current_level);

        // panel: 8px border ring + map area + two text lines, on the 8px UI grid
        let inner_w = (mw.max(SURFACE_MAP) + 7) / 8 * 8;
        let inner_h = (mh + 20 + 7) / 8 * 8;
        let (pw, ph) = (inner_w + 16, inner_h + 16);
        let px0 = (s.w - pw) / 2 / 8 * 8;
        let py0 = (s.h - ph) / 2 / 8 * 8;
        Self::glass_frame(s, "Map", px0, py0, pw, ph);

        let map_x = px0 + 8 + (inner_w - mw) / 2;
        let map_y = py0 + 8;
        s.render_pixel_array(map_x, map_y, mw, mh, pixels);

        // coordinates + seed, under the map
        let (ptx, pty) = Self::player_tile(g);
        let line1 = if depth == 0 {
            format!("X {ptx} Y {pty}")
        } else {
            format!("X {ptx} Y {pty} DEPTH {depth}")
        };
        let line2 = format!("SEED {}", g.world_seed);
        let text_y = map_y + mh + 4;
        font::draw(&line1, s, px0 + 8, text_y, font::default_text_color());
        font::draw(
            &line2,
            s,
            px0 + 8,
            text_y + 9,
            color::get4(-1, 333, 333, 333),
        );
    }
}

/// Player marker: an up arrow (map is north-up).
const ARROW_SHAPE: [(i32, i32); 11] = [
    (0, -2),
    (-1, -1),
    (0, -1),
    (1, -1),
    (-2, 0),
    (-1, 0),
    (0, 0),
    (1, 0),
    (2, 0),
    (0, 1),
    (0, 2),
];

/// Structure pip: a 2x2 dot.
const PIP_SHAPE: [(i32, i32); 4] = [(0, 0), (1, 0), (0, 1), (1, 1)];
