//! The sprite sheet: 8x8 cells, hybrid palette/true-color pixels.
//!
//! Two pixel modes coexist per pixel, so old palette-driven art and new true-color art
//! live on the same sheet:
//!
//! - **Palette pixels** (grayscale, `r == g == b`): quantized to a 0-3 shade index and
//!   recolored at draw time through the caller's packed palette (`colors: i32`, four
//!   palette bytes — the classic recolorable-sprite mechanism, still used for things
//!   that genuinely need dynamic colors: mob level tints, shirts, wool).
//! - **True-color pixels** (any saturated color): drawn as-is, ignoring the palette.
//!   All new art should be authored this way.
//!
//! Alpha < 128 is transparent in both modes. (The legacy grayscale sheet has no alpha;
//! its "shade 3 = white = transparent" convention is handled by the palette path.)

/// The 8x8 sprite cell size (Java `SpriteSheet.boxWidth`).
pub const BOX_WIDTH: i32 = 8;

/// One tile is 16px — two sprite cells per edge (Java `SpriteSheet.tileSize`).
pub const TILE_SIZE: i32 = 16;

/// One decoded sheet pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetPixel {
    /// Fully transparent (alpha < 128).
    Transparent,
    /// Grayscale: recolored through the draw call's palette (shade 0-3).
    Palette(u8),
    /// True color: drawn literally (0xRRGGBB).
    Rgb(i32),
}

pub struct SpriteSheet {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<SheetPixel>,
}

impl SpriteSheet {
    pub fn from_png(png_bytes: &[u8]) -> SpriteSheet {
        let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
        let mut reader = decoder.read_info().expect("invalid spritesheet png");
        let mut buf = vec![0u8; reader.output_buffer_size().expect("png too large")];
        let info = reader
            .next_frame(&mut buf)
            .expect("invalid spritesheet png");
        let width = info.width as i32;
        let height = info.height as i32;

        let channels = match info.color_type {
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
            png::ColorType::Indexed => {
                unreachable!("png crate expands indexed images when transformations are default")
            }
        };
        let bytes = &buf[..info.buffer_size()];
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for px in bytes.chunks_exact(channels) {
            let (r, g, b, a) = match channels {
                1 => (px[0], px[0], px[0], 255),
                2 => (px[0], px[0], px[0], px[1]),
                3 => (px[0], px[1], px[2], 255),
                _ => (px[0], px[1], px[2], px[3]),
            };
            pixels.push(if a < 128 {
                SheetPixel::Transparent
            } else if r == g && g == b {
                // grayscale = palette-mapped shade (legacy 0/51/173/255 gray levels)
                SheetPixel::Palette(r / 64)
            } else {
                SheetPixel::Rgb(((r as i32) << 16) | ((g as i32) << 8) | b as i32)
            });
        }
        SpriteSheet {
            width,
            height,
            pixels,
        }
    }
}
