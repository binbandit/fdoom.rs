//! The pixel buffer the studio edits, and its PNG round-trip.
//!
//! One decoded RGBA image plus the whole-buffer transforms that do not need any
//! editor state: wrap-nudge and rect blit. Both the interactive editor and the
//! headless `--set`/`--blit`/`--nudge` batch path go through exactly these, so a
//! scripted edit and a hand-drawn one write the same bytes.
//!
//! Loading normalises every 8-bit PNG flavour to RGBA; saving always writes RGBA.

use std::path::{Path, PathBuf};

use crate::color::Rgba;

pub(crate) struct Image {
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) px: Vec<Rgba>,
}

impl Image {
    /// A fully transparent `w x h` image (the `N` modal and `--new`).
    pub(crate) fn blank(w: i32, h: i32) -> Image {
        Image {
            w,
            h,
            px: vec![[0, 0, 0, 0]; (w * h) as usize],
        }
    }

    pub(crate) fn pixel(&self, x: i32, y: i32) -> Rgba {
        self.px[(x + y * self.w) as usize]
    }

    pub(crate) fn contains(&self, x: i32, y: i32) -> bool {
        (0..self.w).contains(&x) && (0..self.h).contains(&y)
    }
}

pub(crate) fn load_png(path: &Path) -> Result<Image, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(format!("{}: only 8-bit PNGs supported", path.display()));
    }
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err("unexpanded indexed png".into()),
    };
    let (w, h) = (info.width as i32, info.height as i32);
    let mut px = Vec::with_capacity((w * h) as usize);
    for p in buf[..info.buffer_size()].chunks_exact(channels) {
        px.push(match channels {
            1 => [p[0], p[0], p[0], 255],
            2 => [p[0], p[0], p[0], p[1]],
            3 => [p[0], p[1], p[2], 255],
            _ => [p[0], p[1], p[2], p[3]],
        });
    }
    Ok(Image { w, h, px })
}

pub(crate) fn write_png(path: &Path, img: &Image) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.w as u32, img.h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    let mut data = Vec::with_capacity(img.px.len() * 4);
    for p in &img.px {
        data.extend_from_slice(p);
    }
    writer.write_image_data(&data).map_err(|e| e.to_string())
}

pub(crate) fn bak_path(path: &Path) -> PathBuf {
    path.with_extension("bak.png")
}

/// Wrap-shift the whole image by `(dx, dy)` (the Shift+arrows nudge / `--nudge`).
pub(crate) fn nudge_image(img: &mut Image, dx: i32, dy: i32) {
    let (w, h) = (img.w, img.h);
    let mut out = img.px.clone();
    for y in 0..h {
        for x in 0..w {
            let (nx, ny) = ((x + dx).rem_euclid(w), (y + dy).rem_euclid(h));
            out[(nx + ny * w) as usize] = img.px[(x + y * w) as usize];
        }
    }
    img.px = out;
}

/// Copy the `w x h` rect at `(sx, sy)` onto `(dx, dy)`, clipped to the image; all
/// pixels are copied, including transparency (paste = exact stamp). Also `--blit`.
pub(crate) fn blit_rect(img: &mut Image, sx: i32, sy: i32, w: i32, h: i32, dx: i32, dy: i32) {
    let mut buf = vec![[0u8; 4]; (w.max(0) * h.max(0)) as usize];
    for y in 0..h {
        for x in 0..w {
            if img.contains(sx + x, sy + y) {
                buf[(x + y * w) as usize] = img.px[(sx + x + (sy + y) * img.w) as usize];
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            if img.contains(dx + x, dy + y) {
                img.px[(dx + x + (dy + y) * img.w) as usize] = buf[(x + y * w) as usize];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked(w: i32, h: i32, marks: &[(i32, i32, u8)]) -> Image {
        let mut img = Image::blank(w, h);
        for &(x, y, v) in marks {
            img.px[(x + y * w) as usize] = [v, 0, 0, 255];
        }
        img
    }

    /// A nudge wraps at every edge, and a full-period nudge is the identity.
    #[test]
    fn nudge_wraps_and_cycles() {
        let mut img = marked(8, 8, &[(0, 0, 9)]);
        nudge_image(&mut img, -1, -1);
        assert_eq!(img.pixel(7, 7), [9, 0, 0, 255], "wrapped to the far corner");
        assert_eq!(img.pixel(0, 0), [0, 0, 0, 0]);

        let before = img.px.clone();
        nudge_image(&mut img, 8, 8);
        assert_eq!(img.px, before, "a full period is the identity");
    }

    /// A nudge only ever permutes pixels — nothing is created or destroyed.
    #[test]
    fn nudge_is_a_permutation() {
        let mut img = marked(16, 8, &[(0, 0, 1), (15, 7, 2), (3, 4, 3)]);
        let mut before: Vec<Rgba> = img.px.clone();
        nudge_image(&mut img, 5, -3);
        let mut after = img.px.clone();
        before.sort();
        after.sort();
        assert_eq!(before, after);
    }

    /// Blit is a copy, not a move, and it stamps transparency too.
    #[test]
    fn blit_copies_including_transparency() {
        let mut img = marked(16, 16, &[(1, 1, 7), (2, 2, 8)]);
        img.px[(9 + 9 * 16) as usize] = [50, 50, 50, 255]; // will be stamped over
        blit_rect(&mut img, 0, 0, 4, 4, 8, 8);
        assert_eq!(img.pixel(9, 9), [7, 0, 0, 255], "marker landed");
        assert_eq!(img.pixel(10, 10), [8, 0, 0, 255]);
        assert_eq!(img.pixel(1, 1), [7, 0, 0, 255], "source still there");
        assert_eq!(img.pixel(8, 8), [0, 0, 0, 0], "transparency copied over");
    }

    /// A blit reading or writing off the edge clips instead of panicking, and the
    /// pixels that do land are the right ones.
    #[test]
    fn blit_clips_at_both_ends() {
        let mut img = marked(8, 8, &[(7, 7, 5)]);
        blit_rect(&mut img, 6, 6, 4, 4, 0, 0); // source runs off the bottom-right
        assert_eq!(img.pixel(1, 1), [5, 0, 0, 255]);

        let mut img = marked(8, 8, &[(0, 0, 5)]);
        blit_rect(&mut img, 0, 0, 4, 4, 6, 6); // destination runs off the edge
        assert_eq!(img.pixel(6, 6), [5, 0, 0, 255]);
        assert_eq!(img.px.len(), 64, "buffer never resized");
    }

    /// Round-trip through the encoder: dimensions and every pixel survive, and the
    /// backup name is the sibling `.bak.png` the save path writes.
    #[test]
    fn png_round_trips_and_names_backups() {
        let dir = std::env::temp_dir().join(format!("pixel_studio_img_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.png");

        let img = marked(8, 16, &[(0, 0, 1), (7, 15, 2), (3, 3, 3)]);
        write_png(&path, &img).unwrap();
        let back = load_png(&path).unwrap();
        assert_eq!((back.w, back.h), (8, 16));
        assert_eq!(back.px, img.px);

        assert_eq!(bak_path(&path), dir.join("t.bak.png"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
