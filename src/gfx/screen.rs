//! Port of `fdoom.gfx.Screen` — the 288x192 software framebuffer everything draws into.
//!
//! Pixels are Java-style signed ints (`i32`); see PORTING.md ("Rendering is i32 all the
//! way"). The main screen holds upgraded 24-bit colors; the light screen holds raw 0-255
//! brightness values.

use std::sync::Arc;

use super::color;
use super::point::Point;
use super::sprite_sheet::SpriteSheet;
use crate::core::updater::{DAY_LENGTH, Time};

/// Screen width (Java `Renderer.WIDTH` / `Screen.w`).
pub const W: i32 = 288;
/// Screen height (Java `Renderer.HEIGHT` / `Screen.h`).
pub const H: i32 = 192;

pub const CENTER: Point = Point { x: W / 2, y: H / 2 };

const MAXDARK: i32 = 128;

const BIT_MIRROR_X: i32 = 0x01;
const BIT_MIRROR_Y: i32 = 0x02;

/* Java comment, kept: the dither values are the minimum light level (0-25 scale) a pixel
must have in order to remain lit, repeating every 4 pixels in both directions. */
const DITHER: [i32; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];

pub struct Screen {
    x_offset: i32,
    y_offset: i32,
    pub pixels: Vec<i32>,
    sheet: Arc<SpriteSheet>,
}

impl Screen {
    pub fn new(sheet: Arc<SpriteSheet>) -> Screen {
        Screen {
            x_offset: 0,
            y_offset: 0,
            pixels: vec![0; (W * H) as usize],
            sheet,
        }
    }

    /// Java `clear(color)`.
    pub fn clear(&mut self, color: i32) {
        self.pixels.fill(color);
    }

    /// Java `render(int[] pixelColors)` — bulk copy (used by the map view).
    pub fn render_slice(&mut self, pixel_colors: &[i32]) {
        let n = pixel_colors.len().min(self.pixels.len());
        self.pixels[..n].copy_from_slice(&pixel_colors[..n]);
    }

    /// Java `render(xp, yp, tile, colors, bits)` — draws one 8x8 sprite cell from the
    /// sheet. `xp`/`yp` are level coordinates; the screen offset converts them.
    pub fn render(&mut self, mut xp: i32, mut yp: i32, tile: i32, colors: i32, bits: i32) {
        xp -= self.x_offset;
        yp -= self.y_offset;
        let mirror_x = (bits & BIT_MIRROR_X) > 0;
        let mirror_y = (bits & BIT_MIRROR_Y) > 0;

        let x_tile = tile % 32;
        let y_tile = tile / 32;
        let toffs = x_tile * 8 + y_tile * 8 * self.sheet.width;

        for y in 0..8 {
            let ys = if mirror_y { 7 - y } else { y };
            if y + yp < 0 || y + yp >= H {
                continue;
            }
            for x in 0..8 {
                if x + xp < 0 || x + xp >= W {
                    continue;
                }
                let xs = if mirror_x { 7 - x } else { x };
                let dest = ((x + xp) + (y + yp) * W) as usize;
                match self.sheet.pixels[(toffs + xs + ys * self.sheet.width) as usize] {
                    // grayscale shade: recolor through the caller's packed palette
                    crate::gfx::sprite_sheet::SheetPixel::Palette(shade) => {
                        let col = (colors >> ((3 - shade as i32) * 8)) & 0xFF;
                        if col < 255 {
                            self.pixels[dest] = color::upgrade(col);
                        }
                    }
                    // true-color art: drawn literally
                    crate::gfx::sprite_sheet::SheetPixel::Rgb(rgb) => {
                        self.pixels[dest] = rgb;
                    }
                    crate::gfx::sprite_sheet::SheetPixel::Transparent => {}
                }
            }
        }
    }

    /// Darken a level-space rectangle by `amount` (0 = untouched, 255 = black); the
    /// screen offset applies like `render`.
    pub fn darken_rect(&mut self, xp: i32, yp: i32, w: i32, h: i32, amount: i32) {
        self.darken_rect_screen(xp - self.x_offset, yp - self.y_offset, w, h, amount);
    }

    /// Darken a rectangle in raw screen coordinates (UI panels).
    pub fn darken_rect_screen(&mut self, xp: i32, yp: i32, w: i32, h: i32, amount: i32) {
        let keep = (255 - amount.clamp(0, 255)) as u32;
        for y in yp.max(0)..(yp + h).min(H) {
            for x in xp.max(0)..(xp + w).min(W) {
                let i = (x + y * W) as usize;
                let p = self.pixels[i] as u32;
                let r = ((p >> 16 & 0xFF) * keep) >> 8;
                let g = ((p >> 8 & 0xFF) * keep) >> 8;
                let b = ((p & 0xFF) * keep) >> 8;
                self.pixels[i] = ((r << 16) | (g << 8) | b) as i32;
            }
        }
    }

    /// Java `renderPixelArray(xp, yp, width, height, imgPixels)`.
    pub fn render_pixel_array(
        &mut self,
        xp: i32,
        yp: i32,
        width: i32,
        height: i32,
        img_pixels: &[i32],
    ) {
        for y in 0..height {
            if y + yp >= 0 && y + yp < H {
                for x in 0..width {
                    if x + xp >= 0 && x + xp < W {
                        self.pixels[(x + xp + (y + yp) * W) as usize] =
                            img_pixels[(x + y * width) as usize];
                    }
                }
            }
        }
    }

    /// Java `setOffset(xOffset, yOffset)` — level→screen coordinate conversion factor.
    pub fn set_offset(&mut self, x_offset: i32, y_offset: i32) {
        self.x_offset = x_offset;
        self.y_offset = y_offset;
    }

    /// Java `overlay(screen2, currentLevel, xa, ya)` — overlays the light screen for
    /// darkness/dithering. `tick_count`/`past_day1`/`time` were Java `Updater` statics.
    #[allow(clippy::too_many_arguments)]
    pub fn overlay(
        &mut self,
        screen2: &Screen,
        current_level: i32,
        xa: i32,
        ya: i32,
        tick_count: i32,
        past_day1: bool,
        time: Time,
    ) {
        let mut tint_factor: f64 = 0.0;
        if (3..5).contains(&current_level) {
            let trans_time = DAY_LENGTH / 4;
            let rel_time = (tick_count % trans_time) as f64 / trans_time as f64;

            tint_factor = match time {
                Time::Morning => {
                    if past_day1 {
                        (1.0 - rel_time) * MAXDARK as f64
                    } else {
                        0.0
                    }
                }
                Time::Day => 0.0,
                Time::Evening => rel_time * MAXDARK as f64,
                Time::Night => MAXDARK as f64,
            };
            if current_level > 3 {
                tint_factor -= if tint_factor < 10.0 {
                    tint_factor
                } else {
                    10.0
                };
            }
            tint_factor *= -1.0; // all previous operations were assuming this was a darkening factor
        } else if current_level >= 5 {
            tint_factor = -MAXDARK as f64;
        }

        let o_pixels = &screen2.pixels;
        let mut i = 0usize;
        for y in 0..H {
            for x in 0..W {
                if o_pixels[i] / 10 <= DITHER[(((x + xa) & 3) + ((y + ya) & 3) * 4) as usize] {
                    // this pixel is not lit enough
                    if current_level < 3 {
                        // in the caves, not being lit means being pitch black
                        self.pixels[i] = 0;
                    } else {
                        // outside the caves, it just means being darker
                        self.pixels[i] = color::tint_color(self.pixels[i], tint_factor as i32);
                    }
                }
                // increase the tinting of all colors by 20
                self.pixels[i] = color::tint_color(self.pixels[i], 20);
                i += 1;
            }
        }
    }

    /// Java `copyRect(screen2, x2, y2, w2, h2)` — copies *from this screen into screen2*.
    pub fn copy_rect(&self, screen2: &mut Screen, x2: i32, y2: i32, w2: i32, h2: i32) {
        for y in 0..h2 {
            for x in 0..w2 {
                screen2.pixels[((x + x2) + (y + y2) * W) as usize] =
                    self.pixels[(x + y * W) as usize];
            }
        }
    }

    /// Saturating per-channel additive blend of one screen-space pixel (bounds-checked).
    /// Used by the lighting pass's event skies (`gfx::lighting`); `darken_rect_screen`
    /// is the multiplicative counterpart.
    #[inline]
    pub fn add_rgb(&mut self, x: i32, y: i32, dr: i32, dg: i32, db: i32) {
        if !(0..W).contains(&x) || !(0..H).contains(&y) {
            return;
        }
        let i = (x + y * W) as usize;
        let p = self.pixels[i];
        let r = (((p >> 16) & 0xFF) + dr).clamp(0, 255);
        let g = (((p >> 8) & 0xFF) + dg).clamp(0, 255);
        let b = ((p & 0xFF) + db).clamp(0, 255);
        self.pixels[i] = (r << 16) | (g << 8) | b;
    }

    /// Java `renderLight(x, y, r)` — writes a radial brightness gradient (light screen).
    pub fn render_light(&mut self, mut x: i32, mut y: i32, r: i32) {
        x -= self.x_offset;
        y -= self.y_offset;
        let x0 = (x - r).max(0);
        let x1 = (x + r).min(W);
        let y0 = (y - r).max(0);
        let y1 = (y + r).min(H);

        for yy in y0..y1 {
            let yd = (yy - y) * (yy - y);
            for xx in x0..x1 {
                let xd = xx - x;
                let dist = xd * xd + yd;
                if dist <= r * r {
                    let br = 255 - dist * 255 / (r * r);
                    let idx = (xx + yy * W) as usize;
                    if self.pixels[idx] < br {
                        self.pixels[idx] = br;
                    }
                }
            }
        }
    }
}
