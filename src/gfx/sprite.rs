//! Port of `fdoom.gfx.Sprite`, `Sprite.Px`, and `fdoom.gfx.MobSprite`.
//!
//! Java's `MobSprite` subclass only changed how the pixel grid is built (whole-sprite
//! flipping); here it is the `Sprite::mob` constructor. `ConnectorSprite.makeSprite` also
//! lives here as `make_sprite` (the neighbor-aware ConnectorSprite render logic is in
//! `crate::level::tile`).

use super::color;
use super::screen::Screen;
use crate::java_random::JavaRandom;

/// Java `Sprite.Px` — one 8x8 cell: position on the sheet plus mirroring.
#[derive(Debug, Clone, Copy)]
pub struct Px {
    pub sheet_pos: i32,
    pub mirror: i32,
}

impl Px {
    pub fn new(sheet_x: i32, sheet_y: i32, mirroring: i32) -> Px {
        Px { sheet_pos: sheet_x + 32 * sheet_y, mirror: mirroring }
    }
}

/// Direction (index) + walk-animation-frame indexed sprite set for mobs
/// (Java `MobSprite[][]`; dir order: down, up, left, right — though some mobs
/// use irregular shapes, e.g. Slime's single row).
pub type MobAnims = Vec<Vec<Sprite>>;

#[derive(Debug, Clone)]
pub struct Sprite {
    pub sprite_pixels: Vec<Vec<Px>>,
    pub color: i32,
    /// (x, y, w, h) on the sheet (Java `sheetLoc`).
    pub sheet_loc: (i32, i32, i32, i32),
}

impl Sprite {
    /// Java `new Sprite(pos, color)`.
    pub fn from_pos(pos: i32, color: i32) -> Sprite {
        Sprite::new(pos % 32, pos / 32, 1, 1, color, 0)
    }

    /// Java `new Sprite(sx, sy, color)`.
    pub fn new1x1(sx: i32, sy: i32, color: i32) -> Sprite {
        Sprite::new(sx, sy, 1, 1, color, 0)
    }

    /// Java `new Sprite(sx, sy, sw, sh, color, mirror)`.
    pub fn new(sx: i32, sy: i32, sw: i32, sh: i32, color: i32, mirror: i32) -> Sprite {
        Sprite::new_onepixel(sx, sy, sw, sh, color, mirror, false)
    }

    /// Java `new Sprite(sx, sy, sw, sh, color, mirror, onepixel)`.
    pub fn new_onepixel(sx: i32, sy: i32, sw: i32, sh: i32, color: i32, mirror: i32, onepixel: bool) -> Sprite {
        let mut sprite_pixels = Vec::with_capacity(sh as usize);
        for r in 0..sh {
            let mut row = Vec::with_capacity(sw as usize);
            for c in 0..sw {
                row.push(Px::new(sx + if onepixel { 0 } else { c }, sy + if onepixel { 0 } else { r }, mirror));
            }
            sprite_pixels.push(row);
        }
        Sprite { sprite_pixels, color, sheet_loc: (sx, sy, sw, sh) }
    }

    /// Java `new Sprite(sx, sy, sw, sh, color, onepixel, int[][] mirrors)`.
    pub fn with_mirrors(sx: i32, sy: i32, sw: i32, sh: i32, color: i32, onepixel: bool, mirrors: &[Vec<i32>]) -> Sprite {
        let mut sprite_pixels = Vec::with_capacity(sh as usize);
        for r in 0..sh {
            let mut row = Vec::with_capacity(sw as usize);
            for c in 0..sw {
                row.push(Px::new(
                    sx + if onepixel { 0 } else { c },
                    sy + if onepixel { 0 } else { r },
                    mirrors[r as usize][c as usize],
                ));
            }
            sprite_pixels.push(row);
        }
        Sprite { sprite_pixels, color, sheet_loc: (sx, sy, sw, sh) }
    }

    /// Java `new Sprite(Px[][] pixels, color)`.
    pub fn from_pixels(pixels: Vec<Vec<Px>>, color: i32) -> Sprite {
        Sprite { sprite_pixels: pixels, color, sheet_loc: (0, 0, 0, 0) }
    }

    /// Java `MobSprite(sx, sy, w, h, mirror)` — whole-sprite flipping.
    pub fn mob(sx: i32, sy: i32, w: i32, h: i32, mirror: i32) -> Sprite {
        let flip_x = (0x01 & mirror) > 0;
        let flip_y = (0x02 & mirror) > 0;
        let mut sprite_pixels = Vec::with_capacity(h as usize);
        for r in 0..h {
            let mut row = Vec::with_capacity(w as usize);
            for c in 0..w {
                let x_offset = if flip_x { w - 1 - c } else { c };
                let y_offset = if flip_y { h - 1 - r } else { r };
                row.push(Px::new(sx + x_offset, sy + y_offset, mirror));
            }
            sprite_pixels.push(row);
        }
        Sprite { sprite_pixels, color: 0, sheet_loc: (sx, sy, w, h) }
    }

    /// Java `Sprite.missingTexture(w, h)`.
    pub fn missing_texture(w: i32, h: i32) -> Sprite {
        Sprite::new(30, 30, w, h, color::get(505, 505), 0)
    }

    /// Java `Sprite.blank(w, h, col)`.
    pub fn blank(w: i32, h: i32, col: i32) -> Sprite {
        Sprite::new(7, 2, w, h, color::get(col, col), 0)
    }

    /// Java `Sprite.repeat(sx, sy, w, h, col)`.
    pub fn repeat(sx: i32, sy: i32, w: i32, h: i32, col: i32) -> Sprite {
        make_sprite(w, h, col, 0, true, &[sx + sy * 32])
    }

    /// Java `Sprite.dots(col)`.
    pub fn dots(col: i32) -> Sprite {
        make_sprite(2, 2, col, 0, false, &[0, 1, 2, 3])
    }

    /// Java `Sprite.randomDots(seed, col)`.
    pub fn random_dots(seed: i64, col: i32) -> Sprite {
        let mut ran = JavaRandom::new(seed);
        let mirror = ran.next_int_bound(4);
        let coords =
            [ran.next_int_bound(4), ran.next_int_bound(4), ran.next_int_bound(4), ran.next_int_bound(4)];
        make_sprite(2, 2, col, mirror, false, &coords)
    }

    /// Java `getPos()`.
    pub fn get_pos(&self) -> i32 {
        self.sheet_loc.0 + self.sheet_loc.1 * 32
    }

    /// Java `getSize()` — (w, h) in sheet cells.
    pub fn get_size(&self) -> (i32, i32) {
        (self.sheet_loc.2, self.sheet_loc.3)
    }

    pub fn render(&self, screen: &mut Screen, x: i32, y: i32) {
        self.render_color(screen, x, y, self.color);
    }

    pub fn render_color(&self, screen: &mut Screen, x: i32, y: i32, color: i32) {
        for row in 0..self.sprite_pixels.len() {
            self.render_row_color(row as i32, screen, x, y + row as i32 * 8, color);
        }
    }

    pub fn render_row(&self, r: i32, screen: &mut Screen, x: i32, y: i32) {
        self.render_row_color(r, screen, x, y, self.color);
    }

    pub fn render_row_color(&self, r: i32, screen: &mut Screen, x: i32, y: i32, color: i32) {
        let row = &self.sprite_pixels[r as usize];
        for (c, px) in row.iter().enumerate() {
            screen.render(x + c as i32 * 8, y, px.sheet_pos, color, px.mirror);
        }
    }

    pub fn render_pixel(&self, c: i32, r: i32, screen: &mut Screen, x: i32, y: i32) {
        self.render_pixel_color(c, r, screen, x, y, self.color);
    }

    pub fn render_pixel_color(&self, c: i32, r: i32, screen: &mut Screen, x: i32, y: i32, col: i32) {
        let px = self.sprite_pixels[r as usize][c as usize];
        screen.render(x, y, px.sheet_pos, col, px.mirror);
    }
}

/// Java `MobSprite.compileSpriteList(sheetX, sheetY, width, height, mirror, number)`.
pub fn compile_sprite_list(sheet_x: i32, sheet_y: i32, width: i32, height: i32, mirror: i32, number: i32) -> Vec<Sprite> {
    (0..number).map(|i| Sprite::mob(sheet_x + width * i, sheet_y, width, height, mirror)).collect()
}

/// Java `MobSprite.compileMobSpriteAnimations(sheetX, sheetY)`.
pub fn compile_mob_sprite_animations(sheet_x: i32, sheet_y: i32) -> MobAnims {
    // contents: down 1, up 1, right 1, right 2
    let set1 = compile_sprite_list(sheet_x, sheet_y, 2, 2, 0, 4);
    // contents: down 2, up 2, left 1, left 2
    let set2 = compile_sprite_list(sheet_x, sheet_y, 2, 2, 1, 4);
    let [d1, u1, r1, r2]: [Sprite; 4] = set1.try_into().unwrap();
    let [d2, u2, l1, l2]: [Sprite; 4] = set2.try_into().unwrap();
    vec![vec![d1, d2], vec![u1, u2], vec![l1, l2], vec![r1, r2]]
}

/// Java `ConnectorSprite.makeSprite(w, h, color, mirror, repeat, coords...)`.
pub fn make_sprite(w: i32, h: i32, color: i32, mirror: i32, repeat: bool, coords: &[i32]) -> Sprite {
    let mut pixels: Vec<Vec<Px>> = vec![vec![Px::new(0, 0, 0); w as usize]; h as usize];
    let mut i = 0usize;
    'outer: for row in pixels.iter_mut() {
        for px in row.iter_mut() {
            if i >= coords.len() {
                break 'outer;
            }
            let pos = coords[i];
            *px = Px::new(pos % 32, pos / 32, mirror);
            i += 1;
            if i == coords.len() && repeat {
                i = 0;
            }
        }
    }
    Sprite::from_pixels(pixels, color)
}
