//! The sprite atlas: 8x8 cells, hybrid palette/true-color pixels, stitched at load
//! time from the split sprite files under `assets/sprites/**` (see docs/ART_GUIDE.md).
//!
//! Two pixel modes coexist per pixel, so palette-driven art and true-color art live
//! on the same atlas:
//!
//! - **Palette pixels** (grayscale, `r == g == b`): quantized to a 0-3 shade index and
//!   recolored at draw time through the caller's packed palette (`colors: i32`, four
//!   palette bytes — the classic recolorable-sprite mechanism, still used for things
//!   that genuinely need dynamic colors: mob level tints, shirts, wool).
//! - **True-color pixels** (any saturated color): drawn as-is, ignoring the palette.
//!   All new art should be authored this way.
//!
//! Alpha < 128 is transparent in both modes.
//!
//! # Stitching
//!
//! The art lives as individual PNGs (one file per sprite / frame strip / connector
//! set). `assets/sprites/manifest.txt` pins each legacy file to its historical cell
//! rectangle on the 256x256 base atlas — render call sites still address those cells
//! by number. Files **not** in the manifest are auto-allocated onto appended rows
//! (the atlas grows downward past row 32) and are addressed by name through
//! [`SpriteSheet::cell`] — new art never needs a manifest edit.

use std::collections::HashMap;

/// The 8x8 sprite cell size (Java `SpriteSheet.boxWidth`).
pub const BOX_WIDTH: i32 = 8;

/// One tile is 16px — two sprite cells per edge (Java `SpriteSheet.tileSize`).
pub const TILE_SIZE: i32 = 16;

/// Atlas width in cells: fixed at 32 (256 px) so `pos = cx + cy * 32` cell addressing
/// stays valid; the atlas only ever grows in *height*.
pub const SHEET_CELLS: i32 = 32;

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

/// A named sprite's cell rectangle on the atlas (units: 8x8 cells).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl CellRect {
    /// Sheet position of the top-left cell (`pos = x + y * 32`) — the index render
    /// calls take.
    pub fn pos(&self) -> i32 {
        self.x + self.y * SHEET_CELLS
    }
}

/// Pixel-mode rule a sprite file declares in the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartMode {
    /// Only the legal palette grays 0/85/170/255 (+ transparent).
    Palette,
    /// True color: anything, but never `r == g == b` (that reads as a palette pixel).
    TrueColor,
}

/// One `manifest.txt` line: `<path> <cell_x> <cell_y> <w_cells> <h_cells> <pal|rgb>`.
#[derive(Debug, Clone)]
pub struct ManifestEntry {
    pub path: String,
    pub rect: CellRect,
    pub mode: PartMode,
}

/// Parse `manifest.txt` (`#` comments and blank lines ignored).
pub fn parse_manifest(text: &str) -> Result<Vec<ManifestEntry>, String> {
    let mut entries = Vec::new();
    for (ln, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() != 6 {
            return Err(format!(
                "manifest line {}: expected 6 fields: {line}",
                ln + 1
            ));
        }
        let num = |s: &str| -> Result<i32, String> {
            s.parse()
                .map_err(|_| format!("manifest line {}: bad number {s:?}", ln + 1))
        };
        let mode = match f[5] {
            "pal" => PartMode::Palette,
            "rgb" => PartMode::TrueColor,
            m => return Err(format!("manifest line {}: bad mode {m:?}", ln + 1)),
        };
        let rect = CellRect {
            x: num(f[1])?,
            y: num(f[2])?,
            w: num(f[3])?,
            h: num(f[4])?,
        };
        if rect.x < 0
            || rect.y < 0
            || rect.w <= 0
            || rect.h <= 0
            || rect.x + rect.w > SHEET_CELLS
            || rect.y + rect.h > SHEET_CELLS
        {
            return Err(format!(
                "manifest line {}: pin {rect:?} outside the 32x32 base grid",
                ln + 1
            ));
        }
        entries.push(ManifestEntry {
            path: f[0].to_string(),
            rect,
            mode,
        });
    }
    Ok(entries)
}

/// Decode any PNG to straight RGBA8. Grayscale/RGB/indexed inputs are expanded.
pub fn decode_rgba(png_bytes: &[u8]) -> Result<(i32, i32, Vec<u8>), String> {
    let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (width, height) = (info.width as i32, info.height as i32);
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
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for px in bytes.chunks_exact(channels) {
        match channels {
            1 => rgba.extend_from_slice(&[px[0], px[0], px[0], 255]),
            2 => rgba.extend_from_slice(&[px[0], px[0], px[0], px[1]]),
            3 => rgba.extend_from_slice(&[px[0], px[1], px[2], 255]),
            _ => rgba.extend_from_slice(px),
        }
    }
    Ok((width, height, rgba))
}

/// The composed atlas: raw RGBA plus the name -> cells table.
pub struct Stitched {
    pub width: i32,
    pub height: i32,
    pub rgba: Vec<u8>,
    pub cells: HashMap<String, CellRect>,
}

/// Compose sprite part files into one atlas.
///
/// `parts` maps a manifest-relative path (e.g. `"items/berry.png"`) to PNG bytes.
/// Pinned files land on their manifest rectangle; unpinned files (new art) are
/// shelf-packed onto appended rows starting at row 32, in path order, and the atlas
/// height grows to fit. Every part is registered in [`Stitched::cells`] under its
/// path minus the `.png` suffix (`"items/berry"`).
pub fn stitch(manifest: &str, parts: &[(&str, &[u8])]) -> Result<Stitched, String> {
    let entries = parse_manifest(manifest)?;
    let by_path: HashMap<&str, &ManifestEntry> =
        entries.iter().map(|e| (e.path.as_str(), e)).collect();
    if by_path.len() != entries.len() {
        return Err("manifest contains duplicate paths".into());
    }

    let mut decoded: Vec<(&str, i32, i32, Vec<u8>)> = Vec::with_capacity(parts.len());
    let mut seen = HashMap::new();
    for &(path, bytes) in parts {
        if seen.insert(path, ()).is_some() {
            return Err(format!("duplicate sprite part {path:?}"));
        }
        let (w, h, rgba) = decode_rgba(bytes).map_err(|e| format!("{path}: {e}"))?;
        if w % BOX_WIDTH != 0 || h % BOX_WIDTH != 0 {
            return Err(format!("{path}: size {w}x{h} is not a multiple of 8"));
        }
        decoded.push((path, w / BOX_WIDTH, h / BOX_WIDTH, rgba));
    }
    for e in &entries {
        if !seen.contains_key(e.path.as_str()) {
            return Err(format!("manifest entry {:?} has no sprite file", e.path));
        }
    }

    // pinned parts keep their cells; unpinned parts shelf-pack onto appended rows
    let mut placed: Vec<(&str, CellRect, &[u8])> = Vec::with_capacity(decoded.len());
    let mut loose: Vec<(&str, i32, i32, &[u8])> = Vec::new();
    for (path, w, h, rgba) in &decoded {
        if let Some(e) = by_path.get(path) {
            if (e.rect.w, e.rect.h) != (*w, *h) {
                return Err(format!(
                    "{path}: file is {w}x{h} cells but the manifest pins {}x{}",
                    e.rect.w, e.rect.h
                ));
            }
            placed.push((path, e.rect, rgba));
        } else {
            loose.push((path, *w, *h, rgba));
        }
    }
    loose.sort_by_key(|(path, ..)| *path);
    let (mut cur_x, mut cur_y, mut shelf_h) = (0, SHEET_CELLS, 0);
    for (path, w, h, rgba) in loose {
        if w > SHEET_CELLS {
            return Err(format!("{path}: wider than the 32-cell atlas"));
        }
        if cur_x + w > SHEET_CELLS {
            cur_y += shelf_h;
            (cur_x, shelf_h) = (0, 0);
        }
        placed.push((
            path,
            CellRect {
                x: cur_x,
                y: cur_y,
                w,
                h,
            },
            rgba,
        ));
        cur_x += w;
        shelf_h = shelf_h.max(h);
    }

    let height_cells = placed
        .iter()
        .map(|(_, r, _)| r.y + r.h)
        .max()
        .unwrap_or(0)
        .max(SHEET_CELLS);
    let (width, height) = (SHEET_CELLS * BOX_WIDTH, height_cells * BOX_WIDTH);
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    let mut cells = HashMap::new();
    for (path, rect, src) in placed {
        let (px_w, px_h) = (rect.w * BOX_WIDTH, rect.h * BOX_WIDTH);
        for y in 0..px_h {
            let dst0 = (((rect.y * BOX_WIDTH + y) * width + rect.x * BOX_WIDTH) * 4) as usize;
            let src0 = (y * px_w * 4) as usize;
            rgba[dst0..dst0 + (px_w * 4) as usize]
                .copy_from_slice(&src[src0..src0 + (px_w * 4) as usize]);
        }
        cells.insert(path.trim_end_matches(".png").to_string(), rect);
    }
    Ok(Stitched {
        width,
        height,
        rgba,
        cells,
    })
}

pub struct SpriteSheet {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<SheetPixel>,
    /// Name -> cells table for parts-stitched sheets (`"items/berry"` style keys;
    /// empty for sheets decoded from a monolithic PNG).
    pub cells: HashMap<String, CellRect>,
}

impl SpriteSheet {
    /// Decode a monolithic atlas PNG (golden fixture, legacy sheets, studio previews).
    pub fn from_png(png_bytes: &[u8]) -> SpriteSheet {
        let (width, height, rgba) = decode_rgba(png_bytes).expect("invalid spritesheet png");
        SpriteSheet {
            width,
            height,
            pixels: pixels_from_rgba(&rgba),
            cells: HashMap::new(),
        }
    }

    /// Stitch split sprite files (see [`stitch`]) and decode the result.
    pub fn from_parts(manifest: &str, parts: &[(&str, &[u8])]) -> SpriteSheet {
        let s = stitch(manifest, parts).expect("invalid sprite parts");
        SpriteSheet {
            width: s.width,
            height: s.height,
            pixels: pixels_from_rgba(&s.rgba),
            cells: s.cells,
        }
    }

    /// Look up a sprite by name — the file path under `assets/sprites/` minus the
    /// `.png` suffix, e.g. `sheet.cell("items/berry")`. Works for both pinned and
    /// auto-allocated parts.
    pub fn cell(&self, name: &str) -> Option<CellRect> {
        self.cells.get(name).copied()
    }
}

/// RGBA8 -> the renderer's per-pixel palette/true-color classification.
fn pixels_from_rgba(rgba: &[u8]) -> Vec<SheetPixel> {
    rgba.chunks_exact(4)
        .map(|px| {
            let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
            if a < 128 {
                SheetPixel::Transparent
            } else if r == g && g == b {
                // grayscale = palette-mapped shade (the 0/85/170/255 gray ladder)
                SheetPixel::Palette(r / 64)
            } else {
                SheetPixel::Rgb(((r as i32) << 16) | ((g as i32) << 8) | b as i32)
            }
        })
        .collect()
}
