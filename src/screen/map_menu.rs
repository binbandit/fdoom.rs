//! Port of `fdoom.screen.MapMenu` — the M-key world map, drawn as one colored pixel per
//! tile.

use crate::core::game::Game;
use crate::gfx::{Screen, color, font, screen, sprite_sheet};

use super::display::{Display, DisplayBase, display_tick_default};

/// Java `MapMenu.textColor` (unused by the Java render code; kept for fidelity).
pub const TEXT_COLOR: i32 = color::get4(5, 5, 5, 550);

pub struct MapMenu {
    base: DisplayBase,
    img_pixels: Vec<Option<Vec<i32>>>,
    current_level: usize,
}

impl MapMenu {
    /// Java `new MapMenu()` — the fields are (re)set in `init`, as in Java.
    pub fn new(g: &Game) -> MapMenu {
        MapMenu {
            base: DisplayBase::default(),
            img_pixels: vec![None; g.levels.len()],
            current_level: g.current_level,
        }
    }

    /// Map image dimensions and tile origin: whole level for finite maps, a fixed
    /// window centered on the player for infinite layers.
    pub fn map_window(g: &Game, maplevel: usize) -> (i32, i32, i32, i32) {
        let level = g.level(maplevel);
        if level.is_infinite() {
            let (px, py) = g
                .try_player()
                .map(|p| {
                    (
                        p.c.x / sprite_sheet::TILE_SIZE,
                        p.c.y / sprite_sheet::TILE_SIZE,
                    )
                })
                .unwrap_or((0, 0));
            (128, 128, px - 64, py - 64)
        } else {
            (level.w, level.h, 0, 0)
        }
    }

    /// Java `MapMenu.getMapImage(maplevel)`.
    pub fn get_map_image(g: &Game, maplevel: usize) -> Vec<i32> {
        let level = g.level(maplevel);
        let (mw, mh, ox, oy) = Self::map_window(g, maplevel);
        let mut pixels = vec![0i32; (mw * mh) as usize];

        // JAVA: every Tiles.get(name) below was called once per pixel; the ids are
        // constant, so they're hoisted (identical pixels; avoids re-logging the invalid
        // names — "Stone Bricks", "treeSapling", "cactusSapling", "reed", "tussock" and
        // "campfire" don't exist and fall back to tile 0, exactly as in Java).
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
                    // JAVA: a run of independent ifs — later matches overwrite earlier
                    // ones (Color.get(d) values are raw rgbBytes; preserved quirk).
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
                        // JAVA: "TODO need to fix this once i add the campfire."
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

    /// Java `renderMap(screen, x, y)`.
    pub fn render_map(&self, s: &mut Screen, g: &Game, x: i32, y: i32) {
        if let Some(pixels) = &self.img_pixels[self.current_level] {
            let (mw, mh, _, _) = Self::map_window(g, self.current_level);
            s.render_pixel_array(x, y, mw, mh, pixels);
        }
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
        // JAVA: super.init(null) — will just go back to the game after...
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
        if self.img_pixels[self.current_level].is_some() {
            let (w, h, _, _) = Self::map_window(g, self.current_level);
            let mut x = (screen::W - w) / 2;
            let mut y = (screen::H - h) / 2;
            // JAVA: SpriteSheet.spriteSize (== boxWidth == 8).
            x -= x % sprite_sheet::BOX_WIDTH;
            y -= y % sprite_sheet::BOX_WIDTH;
            font::render_frame(
                s,
                "Map",
                x / sprite_sheet::BOX_WIDTH - 1,
                y / sprite_sheet::BOX_WIDTH - 1,
                (x + w) / sprite_sheet::BOX_WIDTH,
                (y + h) / sprite_sheet::BOX_WIDTH + 1,
            );
            self.render_map(s, g, x, y);
        }
    }
}
