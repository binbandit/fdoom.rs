//! Pixel semantics and colour maths — the rules the game's renderer applies, mirrored
//! so the studio previews art exactly as it will be drawn.
//!
//! The contract mirrors `src/gfx/sprite_sheet.rs`: alpha < 128 is transparent, an
//! opaque gray (`r == g == b`) is a *palette pixel* recoloured at draw time, and any
//! saturated colour draws literally. This module owns that classification, the
//! packed-palette lookup both the canvas and the backdrops go through, and the
//! preview palette / backdrop tables sampled from real game code.

use fdoom::gfx::color;

pub(crate) type Rgba = [u8; 4];

/// The only legal palette-mode grays (the loader quantizes `r/64` into shades 0-3).
pub(crate) const GRAYS: [u8; 4] = [0, 85, 170, 255];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Transparent,
    Gray(u8),
    Color,
}

pub(crate) fn classify(p: Rgba) -> Kind {
    if p[3] < 128 {
        Kind::Transparent
    } else if p[0] == p[1] && p[1] == p[2] {
        Kind::Gray(p[0])
    } else {
        Kind::Color
    }
}

pub(crate) fn rgb24(p: Rgba) -> u32 {
    ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32
}

/// Flood-fill / equality key: everything transparent is one bucket, opaque by rgb.
pub(crate) fn key(p: Rgba) -> u32 {
    if p[3] < 128 { u32::MAX } else { rgb24(p) }
}

pub(crate) fn checker(x: i32, y: i32) -> u32 {
    if (x + y) % 2 == 0 { 0x30363E } else { 0x22272E }
}

/// `t/256` blend of `a` over `b` (0 = all b, 256 = all a).
pub(crate) fn blend(a: u32, b: u32, t: u32) -> u32 {
    let f = |sh: u32| {
        let (ca, cb) = ((a >> sh) & 0xFF, (b >> sh) & 0xFF);
        ((ca * t + cb * (256 - t)) >> 8) & 0xFF
    };
    (f(16) << 16) | (f(8) << 8) | f(0)
}

/// Blend weight for the shape-tool ghost drawn over the canvas during a drag —
/// bright enough to read as the stroke, transparent enough to see the art under it.
pub(crate) const ACCENT_BLEND: u32 = 130;

/// The fixed "night grade": a blue-shifted multiply approximating the game's darkest
/// overworld lighting.
pub(crate) fn night(c: u32) -> u32 {
    let r = ((c >> 16 & 0xFF) * 100) >> 8;
    let g = ((c >> 8 & 0xFF) * 115) >> 8;
    let b = ((c & 0xFF) * 175) >> 8;
    (r << 16) | (g << 8) | b
}

/// Resolve shade `0..=3` through a packed `color::get4` word exactly like
/// `Screen::render` does: byte 255 is the transparent marker, anything else upgrades
/// to a 24-bit colour. `None` means "this shade draws nothing".
pub(crate) fn palette_shade(pal: i32, shade: i32) -> Option<u32> {
    let byte = (pal >> ((3 - shade) * 8)) & 0xFF;
    if byte >= 255 {
        None
    } else {
        Some(color::upgrade(byte) as u32)
    }
}

/// Shade 0 of a packed palette word, used as an opaque ground fill. Unlike
/// [`palette_shade`] this never treats 255 as transparent — a tile's base colour is
/// what shows through wherever the texture sprite has nothing.
pub(crate) fn palette_base(pal: i32) -> u32 {
    color::upgrade((pal >> 24) & 0xFF) as u32
}

/// Preview palettes: real `color::get4` words from game code, so palette-mode art
/// previews exactly as the game will draw it. Index 0 = raw grays.
/// Sources: player render (`player_behavior.rs`, default shirt color 110),
/// `zombie::LVLCOLS`, `registry::TOOL_LEVEL_COLORS`.
pub(crate) const PREVIEW_PALS: &[(&str, i32)] = &[
    ("RAW GRAYS", 0),
    ("PLAYER", color::get4(-1, 100, 110, 532)),
    ("ZOMBIE L1", color::get4(-1, 10, 152, 40)),
    ("ZOMBIE L2", color::get4(-1, 100, 522, 40)),
    ("ZOMBIE L3", color::get4(-1, 111, 444, 40)),
    ("ZOMBIE L4", color::get4(-1, 0, 111, 20)),
    ("TOOL CRUDE", color::get4(-1, 100, 221, 332)),
    ("TOOL WOOD", color::get4(-1, 100, 321, 431)),
    ("TOOL ROCK", color::get4(-1, 100, 321, 111)),
    ("TOOL IRON", color::get4(-1, 100, 321, 555)),
    ("TOOL GOLD", color::get4(-1, 100, 321, 550)),
    ("TOOL GEM", color::get4(-1, 100, 321, 55)),
    // terrain texture palettes (grass.rs / sand.rs / water.rs `dots` sprites), for
    // editing `*_texture` rows as the game will color them
    ("GRASS TILE", color::get4(141, 141, 252, 30)),
    ("SAND TILE", color::get4(552, 550, 440, 440)),
    ("WATER TILE", color::get4(5, 105, 115, 115)),
];

/// In-context preview backdrops: the real terrain texture rows sampled from the
/// loaded game sheet, recolored through the exact `get4` words the tile code uses
/// (`grass.rs` / `sand.rs` / `snow.rs` / `water.rs`). `D` cycles, index 4 re-grades
/// grass through `night`.
pub(crate) struct Backdrop {
    pub(crate) name: &'static str,
    pub(crate) cell_x: i32,
    pub(crate) cell_y: i32,
    pub(crate) pal: i32,
    pub(crate) night: bool,
}

pub(crate) fn backdrops() -> [Backdrop; 5] {
    let snow = color::get4(
        color::hex("#ffffff"),
        color::hex("#ffffff"),
        color::hex("#dde6f0"),
        color::hex("#b9c8d8"),
    );
    [
        Backdrop {
            name: "GRASS",
            cell_x: 22,
            cell_y: 0,
            pal: color::get4(141, 141, 252, 30),
            night: false,
        },
        Backdrop {
            name: "SAND",
            cell_x: 26,
            cell_y: 0,
            pal: color::get4(552, 550, 440, 440),
            night: false,
        },
        Backdrop {
            name: "SNOW",
            cell_x: 13,
            cell_y: 3,
            pal: snow,
            night: false,
        },
        Backdrop {
            name: "WATER",
            cell_x: 0,
            cell_y: 0,
            pal: color::get4(5, 105, 115, 115),
            night: false,
        },
        Backdrop {
            name: "NIGHT GRASS",
            cell_x: 22,
            cell_y: 0,
            pal: color::get4(141, 141, 252, 30),
            night: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three-way pixel contract: alpha wins, then gray-vs-saturated.
    #[test]
    fn classify_matches_the_loader_contract() {
        assert_eq!(classify([9, 9, 9, 0]), Kind::Transparent);
        assert_eq!(classify([255, 0, 0, 127]), Kind::Transparent, "alpha < 128");
        assert_eq!(
            classify([255, 0, 0, 128]),
            Kind::Color,
            "alpha 128 is opaque"
        );
        for g in GRAYS {
            assert_eq!(classify([g, g, g, 255]), Kind::Gray(g));
        }
        assert_eq!(
            classify([28, 28, 28, 255]),
            Kind::Gray(28),
            "off-ladder gray"
        );
        assert_eq!(classify([31, 27, 24, 255]), Kind::Color, "nudged channel");
    }

    /// Every legal gray quantizes to its own shade (the `r/64` the loader applies).
    #[test]
    fn the_gray_ladder_quantizes_one_shade_each() {
        let shades: Vec<u8> = GRAYS.iter().map(|g| g / 64).collect();
        assert_eq!(shades, vec![0, 1, 2, 3]);
    }

    /// Flood-fill buckets: all transparency is one key, opacity keys on rgb only.
    #[test]
    fn fill_key_buckets_transparency_together() {
        assert_eq!(key([1, 2, 3, 0]), key([200, 100, 50, 20]));
        assert_eq!(key([10, 20, 30, 255]), 0x0A141E);
        assert_ne!(key([10, 20, 30, 255]), key([10, 20, 31, 255]));
    }

    /// Blend endpoints are exact, and the midpoint sits between them per channel.
    #[test]
    fn blend_endpoints_are_exact() {
        assert_eq!(blend(0xFF8040, 0x102030, 256), 0xFF8040);
        assert_eq!(blend(0xFF8040, 0x102030, 0), 0x102030);
        let mid = blend(0xFF0000, 0x000000, 128);
        assert_eq!(mid, 0x7F0000);
    }

    /// The night grade darkens every channel and leans blue (b keeps the most).
    #[test]
    fn night_grade_is_a_blue_shifted_darken() {
        let c = night(0xFFFFFF);
        let (r, g, b) = (c >> 16 & 0xFF, c >> 8 & 0xFF, c & 0xFF);
        assert!(r < g && g < b, "blue-shifted: {r} < {g} < {b}");
        assert!(b < 0xFF, "still a darken");
        assert_eq!(night(0), 0);
    }

    /// A packed palette word resolves shade-by-shade — shade 0 is the most
    /// significant byte — and the 255 byte is the renderer's "draw nothing" marker.
    #[test]
    fn palette_shade_reads_the_packed_word() {
        // pack four known bytes directly, so this pins the bit layout itself
        let pal = (10 << 24) | (20 << 16) | (30 << 8) | 255;
        assert_eq!(palette_shade(pal, 0), Some(color::upgrade(10) as u32));
        assert_eq!(palette_shade(pal, 1), Some(color::upgrade(20) as u32));
        assert_eq!(palette_shade(pal, 2), Some(color::upgrade(30) as u32));
        assert_eq!(palette_shade(pal, 3), None, "255 is the transparent marker");
    }

    /// `get4(-1, ..)` is how game code spells "shade 0 draws nothing" — the sprites
    /// that recolor at draw time all use it, so preview must honour it.
    #[test]
    fn a_leading_minus_one_makes_shade_zero_transparent() {
        let pal = color::get4(-1, 100, 321, 555);
        assert_eq!(palette_shade(pal, 0), None);
        for shade in 1..=3 {
            assert!(palette_shade(pal, shade).is_some(), "shade {shade} draws");
        }
    }

    /// The backdrop ground fill is shade 0 taken literally — never transparent.
    #[test]
    fn palette_base_ignores_the_transparent_marker() {
        let pal = color::get4(-1, 100, 321, 555);
        assert_eq!(palette_shade(pal, 0), None);
        assert_eq!(palette_base(pal), color::upgrade(255) as u32);
    }

    /// Every preview palette is named, and index 0 is the raw-grays passthrough.
    #[test]
    fn preview_palettes_are_named_and_raw_is_first() {
        assert_eq!(PREVIEW_PALS[0], ("RAW GRAYS", 0));
        assert!(PREVIEW_PALS.iter().all(|(n, _)| !n.is_empty()));
        assert_eq!(backdrops().len(), 5);
        assert!(backdrops()[4].night, "index 4 is the night grade");
    }
}
