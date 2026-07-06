//! Port of `fdoom.gfx.SpriteSheet`.
//!
//! Java kept `width`/`height` as (rebound) statics and had a handful of unused derived
//! statics (`spriteSize`, `spritePerLine`); here the sheet dimensions are per-instance and
//! the two constants that are actually used elsewhere are `BOX_WIDTH` and `TILE_SIZE`.

/// Java `SpriteSheet.boxWidth` — the 8x8 sprite cell size.
pub const BOX_WIDTH: i32 = 8;

/// Java `SpriteSheet.tileSize` — 16 (two sprite cells per tile edge).
pub const TILE_SIZE: i32 = 16;

pub struct SpriteSheet {
    pub width: i32,
    pub height: i32,
    /// Each pixel quantized to a 0-3 sprite-color index (Java: `(argb & 0xff) / 64`).
    pub pixels: Vec<u8>,
}

impl SpriteSheet {
    /// Decode a PNG (the Java constructor took a decoded `BufferedImage`).
    pub fn from_png(png_bytes: &[u8]) -> SpriteSheet {
        let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
        let mut reader = decoder.read_info().expect("invalid spritesheet png");
        let mut buf = vec![0u8; reader.output_buffer_size().expect("png too large")];
        let info = reader
            .next_frame(&mut buf)
            .expect("invalid spritesheet png");
        let width = info.width as i32;
        let height = info.height as i32;

        // The quantization uses the blue channel (last byte of ARGB in Java); the sheet is
        // grayscale so any channel works. Handle the color types png can hand us.
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
            // For RGB(A) take blue; for grayscale take the gray value.
            let blue = match channels {
                1 | 2 => px[0],
                _ => px[2],
            };
            pixels.push(blue / 64);
        }
        SpriteSheet {
            width,
            height,
            pixels,
        }
    }
}
