//! artgen — the deterministic art generator for `assets/sprites.png`.
//!
//! **This file is the source of truth for the sprite sheet.** Run
//! `cargo run --bin artgen` to (re)generate `assets/sprites.png`; never edit the PNG by
//! hand. Everything below is plain `std` + the `png` crate, no randomness (a tiny hash
//! provides stable "noise"), so the output is bit-for-bit reproducible.
//!
//! # Sheet contract (see `src/gfx/sprite_sheet.rs`)
//!
//! The sheet is 256x256: a 32x32 grid of 8x8 cells, addressed as `pos = cx + cy * 32`.
//! Each pixel is one of three kinds:
//!
//! - **Palette pixel** — grayscale (`r == g == b`, alpha 255). The renderer quantizes the
//!   gray `/64` into a shade index 0..3 and recolors it through the *draw call's* packed
//!   palette (`color::get4(a, b, c, d)`: shade 0 -> `a` ... shade 3 -> `d`; a byte of
//!   `-1` makes that shade transparent). We use exactly four gray levels:
//!   `G0=0, G1=85, G2=170, G3=255`.
//! - **True-color pixel** — any non-gray RGB (alpha 255). Drawn literally; the call-site
//!   palette is ignored. NEVER use an `r==g==b` color in true-color art — it would be
//!   mistaken for a palette pixel (the `rgb()` helper asserts this).
//! - **Transparent** — alpha 0 (only meaningful for true-color art; palette cells encode
//!   transparency through the palette instead and stay alpha 255).
//!
//! # Which cells are palette-mode vs true-color (derived from every call site)
//!
//! *Palette (grayscale)* — anything drawn with more than one meaningful palette:
//! terrain connector pieces + "dots" texture cells (shared by dirt/grass/sand/water/
//! lava/snow/hole/rock/cloud/exploded/sky), wool, ore nubs, stairs, floors, wheat,
//! farmland, walls, doors, every item icon (rows 4-5, tool tiers!), every mob body
//! (mob level tints, player shirt), chest/lantern/spawner furniture, the furniture item
//! icons (row 10), the UI frame + HUD icons (rows 12-13), the splash cells and the font.
//!
//! *True-color* — cells whose call sites all pass one fixed palette (which true-color
//! pixels ignore anyway): trees, cactus, sapling, torch, pumpkin, tall grass,
//! grave stones, quicksand, most furniture (workbench/oven/furnace/anvil/enchanter/
//! loom/tnt/bed) and the title logo.
//!
//! # Shade-role conventions used by the game's palettes (do not break these)
//!
//! - dots cells (0..3,0): shade1 = base field, shade2 = sparse specks. Full coverage.
//! - blob sparse 3x3s: shade1 = interior, shade2 = edge band, shade3 = "outside"
//!   (recolored to the surrounding terrain / transparent for cloud).
//! - rock blob (4,0): shade0 = dark outline ring, 1 = face, 2 = highlight, 3 = outside.
//! - item icons: shade0 = background (transparent via `get4(-1, ..)`), 1 = dark/outline,
//!   2 = mid, 3 = light. (Key (26,4): shades 0 AND 1 are transparent — art in 2-3 only.)
//! - tools: 1 = outline, 2 = wooden handle, 3 = head (gets the tier color).
//! - mobs: shade0 = background (transparent), 1 = outline/dark, 2 = mid (dynamic color:
//!   player shirt, mob level tint), 3 = light (skin/highlight).
//!   Glow worm (26,19): shades 0 and 1 are both transparent — art in 2-3 only.
//! - font: background shade0, stroke shade3 (drawn with `get4(-1,555,555,555)`; some
//!   callers color shade0 as a backing box, e.g. the focus nagger).
//! - stairs: 0 = surrounding ground, 1 = dark void, 2 = step face, 3 = step highlight.
//! - farmland (2,1): 0 = trench soil, 1 = ridge, 2 = ridge crest, 3 = glints.
//! - ore nub (17,1 2x2): 0 = ground (recolored to dirt), 1..3 = crystal cluster.
//! - wool (17,0): 0 = curl shadow lines, 3 = fleece body, 2 = mid dither.
//! - wheat (4..7,3): 0 = soil, 1 = ridge, 2 = stalks, 3 = shoots/heads.
//! - frame (0..2,13): 1 = dark line, 2 = panel face, 3 = light rim, 0 = outside.
//! - doors: 0 = frame, 1 = door face, 2 = dark detail; open door's walk-through gap = 3.
//! - walls: 0 = seams/outline, 1 = face, 2 = face shading, 3 = outside/highlight.
//! - fire particle (9,19): a layered blob (outer 1, mid 2, core 3), drawn in the flame
//!   palette by the spawner. (It once doubled as the removed Creeper's foot.)
//! - night wisp (0..3,20..21): shades 0 and 1 are both transparent — art in 2-3 only.
//! - FREE cells (from the mob-roster overhaul): (8,18), (9,18), (8,19) — the remainder
//!   of the old Creeper block. The old AirWizard (8,14), Skeleton (8,16), and Slime
//!   (0,18) blocks were reused for the Marsh Lurker, Feral Hound, and Stone Golem.
//!
//! # Art-wave additions (see the per-recipe docs for exact roles)
//!
//! - Title lockup: DOOM strip (0..14,6..7), FOSSICKERS kicker strip (15..31,6..7).
//! - Per-material terrain textures (`Sprite::dots_at` reads 4 cells in a row as one
//!   16x16 tile): grass (22..25,0), sand (26..29,0), snow (13..16,3),
//!   dirt (21..24,3), stone (25..28,3); dedicated mud block (24..25,1..2).
//! - Weapon icons (22..25,5): spear, crossbow, throwing knife, slingshot.
//! - Food icons (11..17,10): berry, mushroom, cactus fruit, coconut, cooked meat,
//!   jack-o-lantern, pumpkin.
//! - Grave variety (rows 11..12): rounded (15,11), stone cross (17,11), cracked slab
//!   (19,11), rubble B (21,11), wooden cross (23,11), broken wooden cross (25,11).
//! - Flora rows 26..29: species tree sets + decor + variants (see `flora_cells`).
//!
//! Item icons are auto-centered in their 8x8 cell (`item_icon`/`center8`) so they all
//! share the same bounding-box alignment in inventory lists and the HUD.
//!
//! Cells (8..11,5) are RESERVED for the upcoming crafting overhaul (fiber, stick,
//! cord, sharp stone — grayscale item icons, not referenced by game code yet); see
//! `items_row5`.
//!
//! The full referenced-cell inventory lives in `tests/artgen_sheet.rs`.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/* ==============================  drawing kit  ============================== */

/// One RGBA pixel.
pub type Ink = [u8; 4];

/// Fully transparent (true-color cells only).
pub const TR: Ink = [0, 0, 0, 0];
/// The four palette grays (quantized /64 to shades 0..3 by the loader).
pub const G0: Ink = [0, 0, 0, 255];
pub const G1: Ink = [85, 85, 85, 255];
pub const G2: Ink = [170, 170, 170, 255];
pub const G3: Ink = [255, 255, 255, 255];

/// A true color. Asserts it can never be mistaken for a palette gray.
pub const fn rgb(r: u8, g: u8, b: u8) -> Ink {
    assert!(!(r == g && g == b), "true-color ink must not be gray");
    [r, g, b, 255]
}

pub const SHEET_W: usize = 256;
pub const SHEET_H: usize = 256;

pub struct Sheet {
    pub px: Vec<Ink>,
}

impl Default for Sheet {
    fn default() -> Sheet {
        Sheet::new()
    }
}

impl Sheet {
    pub fn new() -> Sheet {
        Sheet {
            px: vec![TR; SHEET_W * SHEET_H],
        }
    }

    pub fn set(&mut self, x: i32, y: i32, ink: Ink) {
        if (0..SHEET_W as i32).contains(&x) && (0..SHEET_H as i32).contains(&y) {
            self.px[y as usize * SHEET_W + x as usize] = ink;
        }
    }

    pub fn get(&self, x: i32, y: i32) -> Ink {
        self.px[y as usize * SHEET_W + x as usize]
    }

    pub fn save(&self, path: &Path) {
        let mut bytes = Vec::with_capacity(SHEET_W * SHEET_H * 4);
        for p in &self.px {
            bytes.extend_from_slice(p);
        }
        let file = File::create(path).expect("create sprites.png");
        let mut enc = png::Encoder::new(BufWriter::new(file), SHEET_W as u32, SHEET_H as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .expect("png header")
            .write_image_data(&bytes)
            .expect("png data");
    }
}

/// A drawing canvas anchored at cell (cx, cy) — can span multiple cells.
pub struct C<'a> {
    s: &'a mut Sheet,
    ox: i32,
    oy: i32,
}

/// Canvas anchored at cell (cx, cy).
pub fn cell(s: &mut Sheet, cx: i32, cy: i32) -> C<'_> {
    C {
        s,
        ox: cx * 8,
        oy: cy * 8,
    }
}

impl C<'_> {
    pub fn set(&mut self, x: i32, y: i32, ink: Ink) {
        self.s.set(self.ox + x, self.oy + y, ink);
    }

    /// Filled rectangle.
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, ink: Ink) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, ink);
            }
        }
    }

    pub fn hline(&mut self, x: i32, y: i32, w: i32, ink: Ink) {
        self.rect(x, y, w, 1, ink);
    }

    pub fn vline(&mut self, x: i32, y: i32, h: i32, ink: Ink) {
        self.rect(x, y, 1, h, ink);
    }

    /// Filled disc centered at (cx, cy) (pixel centers), radius r.
    pub fn disc(&mut self, cx: i32, cy: i32, r: i32, ink: Ink) {
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                let (dx, dy) = (x - cx, y - cy);
                if dx * dx + dy * dy <= r * r + r / 2 {
                    self.set(x, y, ink);
                }
            }
        }
    }

    /// Checkerboard dither over a rect (phase 0 or 1 picks which diagonal).
    pub fn dither(&mut self, x: i32, y: i32, w: i32, h: i32, phase: i32, ink: Ink) {
        for yy in y..y + h {
            for xx in x..x + w {
                if (xx + yy) & 1 == phase & 1 {
                    self.set(xx, yy, ink);
                }
            }
        }
    }

    /// ASCII-art painter: one string per row; `map` translates chars to inks;
    /// unmapped chars (usually '.') leave the pixel untouched.
    pub fn pat(&mut self, x: i32, y: i32, rows: &[&str], map: &[(char, Ink)]) {
        for (ry, row) in rows.iter().enumerate() {
            for (rx, ch) in row.chars().enumerate() {
                if let Some((_, ink)) = map.iter().find(|(c, _)| *c == ch) {
                    self.set(x + rx as i32, y + ry as i32, *ink);
                }
            }
        }
    }

    /// Surrounds already-drawn opaque pixels in the given region with `ink`
    /// (4-neighborhood) — quick 1px outline for true-color sprites.
    pub fn outline(&mut self, x: i32, y: i32, w: i32, h: i32, ink: Ink) {
        let mut adds = Vec::new();
        for yy in y..y + h {
            for xx in x..x + w {
                if self.s.get(self.ox + xx, self.oy + yy)[3] != 0 {
                    continue;
                }
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (xx + dx, yy + dy);
                    if nx < x || ny < y || nx >= x + w || ny >= y + h {
                        continue;
                    }
                    let p = self.s.get(self.ox + nx, self.oy + ny);
                    if p[3] != 0 && p != ink {
                        adds.push((xx, yy));
                        break;
                    }
                }
            }
        }
        for (xx, yy) in adds {
            self.set(xx, yy, ink);
        }
    }
}

/// Tiny deterministic hash — stable speckle noise without an RNG.
pub fn speck(x: i32, y: i32, seed: u32, one_in: u32) -> bool {
    let mut h = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(2_246_822_519));
    h ^= h >> 13;
    h = h.wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    h % one_in == 0
}

/// Is (x, y) inside a w x h rounded rectangle inset by `inset`, corner radius `r`?
pub fn rounded_inside(x: i32, y: i32, w: i32, h: i32, inset: i32, r: i32) -> bool {
    let (x0, y0, x1, y1) = (inset, inset, w - 1 - inset, h - 1 - inset);
    if x < x0 || y < y0 || x > x1 || y > y1 {
        return false;
    }
    let r = (r - inset).max(0);
    let (cx, cy) = (
        if x < x0 + r {
            x0 + r
        } else if x > x1 - r {
            x1 - r
        } else {
            x
        },
        if y < y0 + r {
            y0 + r
        } else if y > y1 - r {
            y1 - r
        } else {
            y
        },
    );
    let (dx, dy) = (x - cx, y - cy);
    dx * dx + dy * dy <= r * r + r / 2
}

/* ==========================  shared true-color palette  ========================== */
/* ~24 colors used everywhere so the sheet reads as one coherent set. */

pub const OUT: Ink = rgb(31, 27, 24); // near-black warm outline
pub const LEAF_DK: Ink = rgb(45, 84, 51); // moss shadow
pub const LEAF_MD: Ink = rgb(82, 124, 62); // sage canopy
pub const LEAF_LT: Ink = rgb(126, 160, 84); // canopy light
pub const LEAF_HI: Ink = rgb(176, 199, 111); // canopy rim highlight
pub const BARK: Ink = rgb(122, 85, 54); // trunk
pub const BARK_DK: Ink = rgb(84, 57, 39); // trunk shadow
pub const WOOD_LT: Ink = rgb(196, 149, 96); // furniture wood light
pub const WOOD_MD: Ink = rgb(156, 111, 68); // furniture wood mid
pub const WOOD_DK: Ink = rgb(108, 73, 46); // furniture wood dark
pub const SAND_LT: Ink = rgb(226, 202, 144); // warm sand
pub const SAND_DK: Ink = rgb(192, 162, 106); // sand shadow
pub const STONE_LT: Ink = rgb(172, 176, 186); // slate light
pub const STONE_MD: Ink = rgb(124, 128, 140); // slate mid
pub const STONE_DK: Ink = rgb(78, 82, 94); // slate dark
pub const IRON_LT: Ink = rgb(206, 210, 220); // metal light
pub const IRON_DK: Ink = rgb(120, 124, 138); // metal dark
pub const FLAME_YL: Ink = rgb(255, 216, 96); // fire core
pub const FLAME_OR: Ink = rgb(235, 138, 52); // fire mid
pub const FLAME_RD: Ink = rgb(197, 72, 40); // fire edge / tnt red
pub const CREAM: Ink = rgb(240, 229, 198); // bed sheets / paper
pub const RED_CL: Ink = rgb(198, 62, 56); // blanket / logo mid red
pub const MAGIC: Ink = rgb(152, 88, 198); // enchanter purple
pub const MAGIC_LT: Ink = rgb(209, 156, 240); // enchanter glow
pub const PUMPK: Ink = rgb(224, 122, 48); // pumpkin body
pub const PUMPK_DK: Ink = rgb(172, 84, 36); // pumpkin shade
pub const GOLDEN: Ink = rgb(229, 181, 76); // seed heads / accents
pub const MOSS: Ink = rgb(104, 141, 92); // gravestone moss
pub const PINE_DK: Ink = rgb(36, 62, 44); // evergreen shadow / kelp
pub const CORAL: Ink = rgb(222, 108, 122); // coral pink
pub const CORAL_DK: Ink = rgb(168, 64, 88); // coral shadow

/* ==============================  terrain  ============================== */

/// Cells (0..3,0): the four "dots" texture cells (`Sprite::dots` / `random_dots`).
/// Interior texture for dirt/grass/sand/water/lava/snow/hole/sky/etc — shade1 base with
/// a few shade2 specks, full coverage. Each cell places its specks differently so the
/// randomized picker gives a lively field.
/// NOTE: these cells are shared by every "flat field" tile (dirt/grass/sand/snow/
/// water/lava/rock/hole/sky), and several of those palettes leave shades 0 and 3
/// hostile (snow's shade0 is near-black, its shade3 is dirt-brown) — so the texture
/// must stay strictly shade1 (base) + shade2 (accent). shade2 is *lighter* than the
/// base for grass/snow/water (mottle/sparkle/glints) and *darker* for sand/dirt
/// (ripple shadows / clods), so the same strokes read material-appropriately.
fn dots_cells(s: &mut Sheet) {
    // per-variant accents: short broken strokes (ripples/clod edges/drifts) + specks
    let strokes: [&[(i32, i32, i32)]; 4] = [
        // (x, y, len) of a horizontal dash with a 1px droop at the end
        &[(1, 2, 3), (4, 6, 3)],
        &[(3, 1, 3), (0, 5, 2)],
        &[(4, 3, 3), (0, 7, 2)],
        &[(2, 0, 2), (5, 4, 3)],
    ];
    let speck_sets: [&[(i32, i32)]; 4] = [
        &[(6, 0), (2, 4), (7, 5)],
        &[(1, 3), (6, 6), (7, 0)],
        &[(1, 1), (6, 5), (3, 6)],
        &[(0, 2), (4, 2), (7, 7), (1, 6)],
    ];
    for i in 0..4usize {
        let mut c = cell(s, i as i32, 0);
        c.rect(0, 0, 8, 8, G1);
        for &(x, y, len) in strokes[i] {
            c.hline(x, y, len, G2);
            c.set(x + len, y + 1, G2); // drooping tail: reads organic, not gridded
        }
        for &(x, y) in speck_sets[i] {
            c.set(x, y, G2);
        }
    }
}

/// The 24x24 "blob" used by connector sparse sprites: a rounded island of the tile's
/// material. `bands`: ink per inset depth from the blob edge inward (last = interior).
/// `outside` fills everything beyond the blob (recolored to surrounding terrain).
fn blob24(c: &mut C, r: i32, outside: Ink, bands: &[Ink], seed: u32) {
    let interior = *bands.last().unwrap();
    for y in 0..24 {
        for x in 0..24 {
            // organic edge: wobble the blob boundary by 1px here and there
            let wob = i32::from(speck(x / 2, y / 2, seed, 5));
            let mut ink = outside;
            if rounded_inside(x, y, 24, 24, 0, r + wob) {
                ink = interior;
                for (depth, band) in bands.iter().enumerate() {
                    if !rounded_inside(x, y, 24, 24, depth as i32 + 1, r) {
                        ink = *band;
                        break;
                    }
                }
            }
            c.set(x, y, ink);
        }
    }
    // interior speckle, matching the dots cells
    for y in 0..24 {
        for x in 0..24 {
            if rounded_inside(x, y, 24, 24, bands.len() as i32 + 1, r)
                && speck(x, y, seed.wrapping_add(7), 11)
            {
                c.set(x, y, G2);
            }
        }
    }
}

/// A 16x16 "sides" block (rock/cloud/walls): face fill with an inner-corner notch at
/// the block center. Each 8x8 quarter is drawn mirrored in-game so the notch lands on
/// the tile corner that has a missing diagonal neighbor.
fn sides16(c: &mut C, face: Ink, ring: Ink, outside: Ink) {
    c.rect(0, 0, 16, 16, face);
    for y in 0..16 {
        for x in 0..16 {
            if speck(x, y, 3, 13) {
                c.set(x, y, G2);
            }
        }
    }
    c.disc(8, 8, 4, ring);
    c.disc(8, 8, 2, outside);
}

/// Cells (4..6,0..2) rock/hard-rock/cloud sparse blob + (7..8,0..1) their sides block.
/// Roles: 0 = dark outline ring, 1 = face, 2 = highlight, 3 = outside.
fn rock_connector(s: &mut Sheet) {
    let mut c = cell(s, 4, 0);
    blob24(&mut c, 7, G3, &[G0, G0, G1], 21);
    // highlight sweep along the top-left of the dome
    for y in 3..9 {
        for x in 3..14 {
            if rounded_inside(x, y, 24, 24, 3, 7) && (x + y * 2) % 3 == 0 {
                c.set(x, y, G2);
            }
        }
    }
    // cracked facets: short shade0 fault lines across the dome, each with a shade2
    // lit edge hugging its lower-right, so the face reads as split planes instead of
    // a flat fill. (Shared with cloud, whose shade0 is a soft mid-gray — the cracks
    // read as gentle creases there; keep them sparse.)
    let cracks: &[&[(i32, i32)]] = &[
        &[(9, 11), (10, 12), (11, 12), (12, 13)],
        &[(16, 7), (16, 8), (17, 9)],
        &[(6, 15), (7, 16), (8, 16)],
        &[(15, 16), (16, 17), (17, 17)],
    ];
    for line in cracks {
        for &(x, y) in *line {
            if rounded_inside(x, y, 24, 24, 3, 7) {
                c.set(x, y, G0);
                if rounded_inside(x + 1, y + 1, 24, 24, 3, 7) {
                    c.set(x + 1, y + 1, G2);
                }
            }
        }
    }
    let mut sd = cell(s, 7, 0);
    sides16(&mut sd, G1, G0, G3);
    // matching hairline cracks on the sides/face block (kept clear of the center
    // notch, which the connector logic owns)
    for &(x, y) in &[(2, 3), (3, 4), (12, 11), (13, 12), (11, 2), (4, 13)] {
        sd.set(x, y, G0);
    }
}

/// Cells (11..13,0..2): grass/sand/snow sparse blob.
/// Roles: 1 = interior, 2 = light fringe band, 3 = outside (dirt).
///
/// The four corner cells get a cleanup pass: the wobbled blob edge used to leave
/// stray shade3 pixels ("outside" = dirt-brown for the sand/snow palettes) intruding
/// into the turf, which showed up as brown corner nubs at biome borders. Those
/// pixels are pulled back into the fringe/interior shades, and the corner wedge
/// beyond the arc is softened with a fringe rim so the eroded corner fades out
/// through grass-family shades instead of jumping straight to brown.
fn grass_connector(s: &mut Sheet) {
    let mut c = cell(s, 11, 0);
    blob24(&mut c, 5, G3, &[G2, G1], 33);
    for y in 0..24 {
        for x in 0..24 {
            // only the four 8x8 corner cells of the 3x3 block
            let corner = !(8..16).contains(&x) && !(8..16).contains(&y);
            if !corner {
                continue;
            }
            if rounded_inside(x, y, 24, 24, 0, 5) {
                // inside the nominal arc: no brown allowed — wobble strays become turf
                if c.s.get(c.ox + x, c.oy + y) == G3 {
                    c.set(x, y, G2);
                }
            } else if rounded_inside(x, y, 24, 24, -1, 5) {
                // 1px rim just beyond the arc: eroded fringe crumbs, not hard dirt
                if speck(x, y, 34, 2) {
                    c.set(x, y, G2);
                }
            }
        }
    }
}

/// Cells (14..16,0..2): water/lava/hole sparse blob.
/// Roles: 1 = liquid interior, 2 = wet shore band (2px), 3 = outside.
fn water_connector(s: &mut Sheet) {
    let mut c = cell(s, 14, 0);
    blob24(&mut c, 6, G3, &[G2, G2, G1], 47);
}

/* ==============  per-material terrain textures (dedicated cell rows)  ============== */

/// A 16x16 scratch buffer committed to four consecutive row cells in `Sprite::dots`
/// quadrant order (TL, TR, BL, BR) — the layout `Sprite::dots_at(base_cx, cy, col)`
/// reads back as one 16x16 tile. Lets a texture be art-directed as a whole tile while
/// stored in a single sheet row.
struct T16 {
    px: [[Ink; 16]; 16],
}

impl T16 {
    fn new(base: Ink) -> T16 {
        T16 {
            px: [[base; 16]; 16],
        }
    }

    fn set(&mut self, x: i32, y: i32, ink: Ink) {
        if (0..16).contains(&x) && (0..16).contains(&y) {
            self.px[y as usize][x as usize] = ink;
        }
    }

    fn commit(&self, s: &mut Sheet, base_cx: i32, cy: i32) {
        for (i, (ox, oy)) in [(0, 0), (8, 0), (0, 8), (8, 8)].iter().enumerate() {
            let mut c = cell(s, base_cx + i as i32, cy);
            for y in 0..8 {
                for x in 0..8 {
                    c.set(x, y, self.px[(oy + y) as usize][(ox + x) as usize]);
                }
            }
        }
    }
}

/// Cells (22..25,0): GRASS — layered short-blade tufting with 2-tone mottling.
/// Wired via `Sprite::dots_at(22, 0, color::get4(141, 141, 252, 30))`:
/// 1 = meadow base, 2 = light blade tips / mottle, 3 = dark blade shadows.
fn grass_texture(s: &mut Sheet) {
    // CALM BASE, SPARSE CLUSTERED DETAIL: ~90% flat meadow base; three small blade
    // tufts and two soft sunlit patches, offset between the tile's four cells so
    // tiling never grid-repeats.
    let mut t = T16::new(G1);
    // soft light patches (3-5px irregular blobs)
    for &(x, y) in &[(8, 1), (9, 1), (8, 2), (9, 2), (10, 2)] {
        t.set(x, y, G2);
    }
    for &(x, y) in &[(2, 10), (3, 10), (2, 11), (3, 11)] {
        t.set(x, y, G2);
    }
    // blade tufts: a small dark L of 3px with a light tip on the tallest blade
    for &(x, y) in &[(3, 3), (12, 7), (6, 13)] {
        t.set(x, y, G3);
        t.set(x, y + 1, G3);
        t.set(x + 1, y + 1, G3);
        t.set(x + 1, y, G2);
    }
    // two lone specks
    t.set(14, 13, G2);
    t.set(0, 6, G2);
    t.commit(s, 22, 0);
}

/// Cells (26..29,0): SAND — flowing dune ripple lines every 4 rows (they tile
/// seamlessly across cells), each with a lit crest above and loose grains between.
/// Wired via `Sprite::dots_at(26, 0, color::get4(552, 550, 440, 440))`:
/// 0 = sunlit crest, 1 = sand base, 2/3 = ripple shadow.
fn sand_texture(s: &mut Sheet) {
    // CALM BASE: three 1px wavy ripple lines ~5px apart (they tile seamlessly across
    // cells), no crest band, plus three lone grains on the flats.
    let mut t = T16::new(G1);
    let ripples: [(i32, u16); 3] = [
        (3, 0b0001_1100_0000_0111),
        (8, 0b1100_0000_1110_0000),
        (13, 0b0000_0111_1000_0011),
    ];
    for &(base_y, mask) in &ripples {
        for x in 0..16 {
            let y = base_y - ((mask >> (15 - x)) & 1) as i32;
            t.set(x, y, G3);
        }
    }
    for &(x, y) in &[(11, 5), (4, 10), (14, 15)] {
        t.set(x, y, G3);
    }
    t.commit(s, 26, 0);
}

/// Cells (13..16,3): SNOW — wind-piled drift arcs and sparse glints on a bright
/// field. Wired via `Sprite::dots_at(13, 3, ...)` with a cool palette
/// (`get4(#ffffff, #ffffff, #dde6f0, #b9c8d8)`): 1 = snow, 2 = soft drift shading,
/// 3 = deep drift edge / glint sparkle.
fn snow_texture(s: &mut Sheet) {
    // CALM BASE: a bright open field with three wind-piled drift arcs (shade2 with a
    // shade3 undercut at the trailing tip) and three lone glints — nothing else.
    let mut t = T16::new(G1);
    let drifts: [&[(i32, i32)]; 3] = [
        &[(2, 3), (3, 3), (4, 3), (5, 4)],
        &[(10, 8), (11, 8), (12, 7), (13, 7)],
        &[(5, 13), (6, 13), (7, 14)],
    ];
    for arc in &drifts {
        for &(x, y) in *arc {
            t.set(x, y, G2);
        }
        let &(tx, ty) = arc.last().unwrap();
        t.set(tx + 1, ty + 1, G3);
    }
    for &(x, y) in &[(9, 1), (1, 9), (14, 12)] {
        t.set(x, y, G3);
    }
    t.commit(s, 13, 3);
}

/// Cells (21..24,3): DIRT — earth clods and small stones. Wired via
/// `Sprite::dots_at(21, 3, ...)` with dirt.rs's depth palette
/// (`get4(dcol+111, dcol, dcol-111, dcol-111)`): 0 = lit clod top, 1 = soil base,
/// 2 = clod under-shadow, 3 = stones.
fn dirt_texture(s: &mut Sheet) {
    // CALM BASE: ~93% flat soil; three clods (lit arc up-left, shadow down-right)
    // and two small stone chips, offset between cells.
    let mut t = T16::new(G1);
    let clods = [(3, 3), (11, 8), (5, 12)];
    for &(x, y) in &clods {
        t.set(x, y, G0);
        t.set(x + 1, y, G0);
        t.set(x + 2, y + 1, G2);
    }
    for &(x, y) in &[(13, 2), (2, 8)] {
        t.set(x, y, G3);
        t.set(x + 1, y, G3);
    }
    t.commit(s, 21, 3);
}

/// Cells (25..28,3): STONE — fractured plates: a crack network with junction pits
/// and lit plate edges. Wired via `Sprite::dots_at(25, 3, ...)` in rock.rs
/// (`get4(555, 444, 333, 111)`): 0 = lit plate edge, 1 = stone face, 2 = crack,
/// 3 = deep crack pits. Crack ends meet the tile edges at matching offsets so the
/// network continues across tiles.
fn stone_texture(s: &mut Sheet) {
    // CALM BASE: flat faces split by two long 1px cracks (they meet the tile edges
    // at matching offsets so the network continues across tiles); a pit only at the
    // junction.
    let mut t = T16::new(G1);
    let paths: [&[(i32, i32)]; 2] = [
        // main fault: left edge (y6) through the center, out the bottom (x5)
        &[
            (0, 6),
            (1, 6),
            (2, 6),
            (3, 6),
            (4, 7),
            (5, 7),
            (6, 8),
            (7, 8),
            (8, 8),
            (8, 9),
            (8, 10),
            (7, 11),
            (7, 12),
            (6, 13),
            (5, 14),
            (5, 15),
        ],
        // branch: junction out the right edge (y6)
        &[(9, 8), (10, 7), (11, 7), (12, 6), (13, 6), (14, 6), (15, 6)],
    ];
    for path in &paths {
        for &(x, y) in *path {
            t.set(x, y, G2);
        }
    }
    // junction pit only — bright plate-edge highlights read as repeating sparkles
    t.set(8, 8, G3);
    t.commit(s, 25, 3);
}

/// Cell (17,0): wool — curly fleece. 0 = curl shadows, 3 = fleece, 2 = softening.
fn wool_cell(s: &mut Sheet) {
    let mut c = cell(s, 17, 0);
    c.pat(
        0,
        0,
        &[
            "33333333", //
            "30032330", //
            "32303032", //
            "33232333", //
            "30333023", //
            "32030330", //
            "33323033", //
            "30333330", //
        ],
        &[('3', G3), ('2', G2), ('0', G0)],
    );
}

/// Cells (18..20,0): cloud interior variants — shade2 puffs on a shade1 base.
fn cloud_full_cells(s: &mut Sheet) {
    let puffs: [&[(i32, i32, i32)]; 3] = [
        &[(2, 3, 2), (6, 6, 1)],
        &[(5, 2, 2), (1, 6, 1)],
        &[(4, 5, 2), (7, 1, 1)],
    ];
    for (i, set) in puffs.iter().enumerate() {
        let mut c = cell(s, 18 + i as i32, 0);
        c.rect(0, 0, 8, 8, G1);
        for &(x, y, r) in *set {
            c.disc(x, y, r, G2);
        }
    }
}

/// Cell (2,1): farmland furrows. 0 = trench, 1 = ridge, 2 = crest, 3 = glints.
fn farm_cell(s: &mut Sheet) {
    let mut c = cell(s, 2, 1);
    c.pat(
        0,
        0,
        &[
            "00000000", //
            "11211211", //
            "22122122", //
            "11111111", //
            "00000000", //
            "12112112", //
            "22322232", //
            "11111111", //
        ],
        &[('0', G0), ('1', G1), ('2', G2), ('3', G3)],
    );
}

/// Cell (3,1): the footprint stamp for stepped-on sand/snow (base matches the dots).
/// Two clear boot prints on a walking offset: sole (toe cap, deep instep, ball) plus
/// a separate heel dab. 2 = pressed rim, 3 = deepest part of the print (the sand and
/// snow palettes both put their strongest press tone on shade3).
fn footprint_cell(s: &mut Sheet) {
    let mut c = cell(s, 3, 1);
    c.rect(0, 0, 8, 8, G1);
    c.pat(
        0,
        0,
        &[
            ".22.....", // left toe cap
            ".33.....", // left instep (deep)
            ".33..22.", // right toe cap
            ".22..33.", // left ball / right instep (deep)
            ".....33.", //
            ".22..22.", // left heel / right ball
            "........", //
            ".....22.", // right heel
        ],
        &[('2', G2), ('3', G3)],
    );
}

/// Cells (24..25,1..2): DEDICATED MUD tile block (16x16, palette mode) — dark wet
/// brown with puddle hollows and sheen specks. Drawn for the mud tile so it can stop
/// rendering darkened dirt; not wired yet. Suggested wiring in `mud.rs`:
/// `Sprite::new(24, 1, 2, 2, color::get4(100, 210, 321, 433), 0)` —
/// roles: 0 = wet puddle hollows (darkest), 1 = mud base, 2 = drier clod ridges,
/// 3 = sheen glints on the puddle rims.
fn mud_cells(s: &mut Sheet) {
    // CALM BASE: three wet puddle hollows with clod ridges hugging their rims and a
    // single sheen glint each — the rest is flat mud.
    let mut c = cell(s, 24, 1);
    c.pat(
        0,
        0,
        &[
            "1111111111111111", //
            "1111100111111111", //
            "1113000011111111", // puddle, upper-left (3 = sheen glint on the rim)
            "1110000211111111", //
            "1112002111111111", //
            "1111221111111211", //
            "1111111111100111", //
            "1111111111000011", // puddle, right
            "1111111113000211", //
            "1111111111002111", //
            "1111111111221111", //
            "1100111111111111", //
            "1000011111211111", // puddle, lower-left
            "1100002111111111", //
            "1131021111111111", //
            "1111111111111111", //
        ],
        PMAP,
    );
}

/// Cells (17..18,1..2): the ore nub — a crystal cluster on a shade0 ground (the ground
/// shade is recolored to the level's dirt color; the cluster gets the ore's palette).
/// Shared by iron/gold/gem/lapis and the cloud cactus.
fn ore_cells(s: &mut Sheet) {
    let mut c = cell(s, 17, 1);
    c.rect(0, 0, 16, 16, G0);
    for y in 0..16 {
        for x in 0..16 {
            if speck(x, y, 9, 17) {
                c.set(x, y, G1);
            }
        }
    }
    c.pat(
        2,
        2,
        &[
            "....23......", //
            "...1332.....", //
            "..133321..2.", //
            "..12332..232", //
            ".2112321.121", //
            "232..131.1..", //
            "121...1.....", //
            ".1..12321...", //
            "...1233321..", //
            "...112211...", //
            "............", //
        ],
        &[('1', G1), ('2', G2), ('3', G3)],
    );
}

/// Cells (22..23,1..2): quicksand (true color) — a slow swirl in the sand.
fn quicksand_cells(s: &mut Sheet) {
    let mut c = cell(s, 22, 1);
    c.rect(0, 0, 16, 16, SAND_DK);
    for y in 0..16 {
        for x in 0..16 {
            // concentric swirl rings around the center
            let (dx, dy) = (x - 8, y - 8);
            let d2 = dx * dx + dy * dy;
            if (20..34).contains(&d2) || (54..70).contains(&d2) {
                c.set(x, y, SAND_LT);
            }
            if d2 < 6 {
                c.set(x, y, BARK_DK);
            }
        }
    }
    c.set(8, 8, OUT);
    c.set(7, 8, OUT);
    // drift flecks
    c.set(3, 12, SAND_LT);
    c.set(12, 3, SAND_LT);
}

/// Cells (0..1,2..3) stairs down, (2..3,2..3) stairs up.
/// Roles: 0 = surrounding ground, 1 = dark void, 2 = step face, 3 = step highlight.
fn stairs_cells(s: &mut Sheet) {
    // down: a rounded pit with steps sinking toward the dark bottom-right
    let mut c = cell(s, 0, 2);
    c.rect(0, 0, 16, 16, G0);
    for y in 0..16 {
        for x in 0..16 {
            if rounded_inside(x, y, 16, 16, 1, 4) {
                c.set(x, y, G1);
            }
        }
    }
    c.pat(
        2,
        2,
        &[
            "333333......", //
            "222222......", //
            "..33333.....", //
            "..22222.....", //
            "....3333....", //
            "....2222....", //
            "......333...", //
            "......222...", //
            "........33..", //
        ],
        &[('2', G2), ('3', G3)],
    );

    // up: full-tile steps climbing toward the top-right light
    let mut c = cell(s, 2, 2);
    c.rect(0, 0, 16, 16, G0);
    for i in 0..5 {
        let y = 2 + i * 3;
        let x = 2 + (4 - i) * 2;
        c.rect(x, y, 16 - x - 1, 1, G3); // tread edge
        c.rect(x, y + 1, 16 - x - 1, 2, G2); // tread face
        c.vline(x, y, 3, G1); // riser shadow
    }
}

/// Cell (7,2): `Sprite::blank` — a flat filled cell (stone/obsidian wall interiors).
fn blank_cell(s: &mut Sheet) {
    let mut c = cell(s, 7, 2);
    c.rect(0, 0, 8, 8, G1);
}

/// Cells (8..9,2..3): cactus (true color, transparent bg — sand is drawn underneath).
fn cactus_cells(s: &mut Sheet) {
    let mut c = cell(s, 8, 2);
    c.pat(
        0,
        0,
        &[
            "......gg........", //
            ".....mllm.......", //
            ".....mlds.......", //
            ".mm..mlds..s....", //
            "mlds.mlds.msm...", //
            "mlds.mldssmlm...", //
            "mldssmlds.......", //
            ".mmsmmlds.......", //
            ".....mlds.......", //
            ".....mlds.......", //
            ".....mlds.......", //
            ".....mlds.......", //
            ".....mlds.......", //
            "....dmlds.......", //
            "................", //
            "................", //
        ],
        &[
            ('m', LEAF_MD),
            ('l', LEAF_LT),
            ('d', LEAF_DK),
            ('s', LEAF_DK),
            ('g', GOLDEN),
        ],
    );
    c.outline(0, 0, 16, 16, OUT);
}

/// Cells (4..7,3): the four wheat growth stages (each drawn 4x per tile).
fn wheat_cells(s: &mut Sheet) {
    for stage in 0..4 {
        let mut c = cell(s, 4 + stage, 3);
        // soil bed
        c.pat(
            0,
            0,
            &[
                "00000000", "01101101", "00000000", "10110110", "00000000", "01101101", "00000000",
                "10110110",
            ],
            &[('0', G0), ('1', G1)],
        );
        match stage {
            0 => {
                // freshly seeded: sparse shoots poking out
                for &(x, y) in &[(1, 5), (4, 3), (6, 6)] {
                    c.set(x, y, G3);
                }
            }
            1 => {
                for &x in &[1, 3, 5, 7] {
                    c.vline(x, 4, 3, G2);
                    c.set(x, 4, G3);
                }
            }
            2 => {
                for &x in &[0, 2, 4, 6] {
                    c.vline(x, 2, 5, G2);
                    c.set(x, 2, G3);
                    c.set(x, 3, G3);
                }
            }
            _ => {
                // mature: dense stalks with heavy heads
                for x in 0..8 {
                    c.vline(x, 3, 5, G2);
                    if x % 2 == 0 {
                        c.set(x, 1, G3);
                        c.set(x, 2, G3);
                    } else {
                        c.set(x, 2, G3);
                        c.set(x, 3, G3);
                    }
                }
            }
        }
    }
}

/// Cell (11,3): tree/cactus sapling (true color, drawn over its ground tile).
fn sapling_cell(s: &mut Sheet) {
    let mut c = cell(s, 11, 3);
    c.pat(
        0,
        0,
        &[
            "..ll....", //
            ".lmml...", //
            "lm.mml..", //
            ".l.bm.l.", //
            "...b.ml.", //
            "...b....", //
            "..kbk...", //
            "........", //
        ],
        &[('l', LEAF_LT), ('m', LEAF_MD), ('b', BARK), ('k', BARK_DK)],
    );
}

/// Cell (12,3): placed torch (true color; rendered at +4,+4 within the tile).
fn torch_cell(s: &mut Sheet) {
    let mut c = cell(s, 12, 3);
    c.pat(
        0,
        0,
        &[
            "...y....", //
            "..yyo...", //
            "..oyyo..", //
            "..royr..", //
            "...wk...", //
            "...wk...", //
            "...wk...", //
            "........", //
        ],
        &[
            ('y', FLAME_YL),
            ('o', FLAME_OR),
            ('r', FLAME_RD),
            ('w', WOOD_MD),
            ('k', WOOD_DK),
        ],
    );
}

/// Cells (19..20,2..3): brick/plank floor tiles. (19,2) alone is the whole floor tile
/// (repeated 4x); the 2x2 block is the lava-brick tile. Beveled: seams 0/1, face 2,
/// grain 3.
fn floor_cells(s: &mut Sheet) {
    for (i, (cx, cy)) in [(19, 2), (20, 2), (19, 3), (20, 3)].iter().enumerate() {
        let mut c = cell(s, *cx, *cy);
        c.rect(0, 0, 8, 8, G2);
        c.hline(0, 0, 8, G1); // top seam
        c.vline(0, 0, 8, G1); // left seam
        c.set(7, 7, G1);
        // grain / wear marks vary per cell
        let marks: [&[(i32, i32)]; 4] = [
            &[(3, 3), (5, 5), (2, 6)],
            &[(4, 2), (6, 5)],
            &[(2, 3), (5, 6), (6, 2)],
            &[(3, 5), (5, 3)],
        ];
        for &(x, y) in marks[i] {
            c.set(x, y, G3);
        }
        c.set(6, 6, G1);
    }
}

/// Tree cells (true color; transparent shows the grass/snow drawn underneath):
/// (9,0) top-left, (10,0) top-right, (9,1) bottom-left, (10,3) bottom-right of a
/// free-standing tree; (10,1) full-canopy fill, (10,2) canopy fill with a bark gap.
///
/// Forests spawn trees adjacent, and `TreeTile::render` only swaps in the canopy-fill
/// cells on fully surrounded corners — every other tree in a cluster is drawn with
/// these standalone quarters. So the canopy is a full-tile dome whose left/right/top
/// edges reach the tile border: adjacent trees visually merge into one forest roof,
/// and only the rounded corners let grass scallop the cluster edge. The interior
/// texture uses the same speck seeds as the (10,1) fill cell so they blend seamlessly.
fn tree_cells(s: &mut Sheet) {
    // free-standing tree, 16x16; the four 8x8 quarters land on their cells below
    let mut px = [[TR; 16]; 16];
    for y in 0..13i32 {
        for x in 0..16i32 {
            if !rounded_inside(x, y, 16, 13, 0, 5) {
                continue;
            }
            let mut ink = LEAF_MD;
            if !rounded_inside(x, y, 16, 13, 1, 5) {
                ink = LEAF_DK; // dark rim
            } else {
                if speck(x, y, 61, 4) {
                    ink = LEAF_DK;
                }
                if speck(x, y, 62, 9) {
                    ink = LEAF_LT;
                }
                if y >= 10 {
                    // under-canopy shadow above the trunk
                    if (x + y) & 1 == 0 {
                        ink = LEAF_DK;
                    }
                } else if x + y <= 9 && !rounded_inside(x, y, 16, 13, 3, 5) {
                    // sun-lit top-left sweep
                    ink = if x + y <= 6 { LEAF_HI } else { LEAF_LT };
                }
            }
            px[y as usize][x as usize] = ink;
        }
    }
    // trunk peeking out under the canopy
    for row in px.iter_mut().take(15).skip(13) {
        row[6] = OUT;
        row[7] = BARK;
        row[8] = BARK_DK;
        row[9] = OUT;
    }
    let quarters = [(9, 0, 0, 0), (10, 0, 8, 0), (9, 1, 0, 8), (10, 3, 8, 8)];
    for (cx, cy, ox, oy) in quarters {
        let mut c = cell(s, cx, cy);
        for y in 0..8i32 {
            for x in 0..8i32 {
                let ink = px[(oy + y) as usize][(ox + x) as usize];
                if ink[3] != 0 {
                    c.set(x, y, ink);
                }
            }
        }
    }
    // (10,1): solid canopy fill for forest interiors
    let mut c = cell(s, 10, 1);
    c.rect(0, 0, 8, 8, LEAF_MD);
    for y in 0..8 {
        for x in 0..8 {
            if speck(x, y, 61, 4) {
                c.set(x, y, LEAF_DK);
            }
            if speck(x, y, 62, 9) {
                c.set(x, y, LEAF_LT);
            }
        }
    }
    // (10,2): canopy fill with a small bark knot (trunks peeking through the roof)
    let mut c = cell(s, 10, 2);
    c.rect(0, 0, 8, 8, LEAF_MD);
    for y in 0..8 {
        for x in 0..8 {
            if speck(x, y, 61, 4) {
                c.set(x, y, LEAF_DK);
            }
            if speck(x, y, 62, 9) {
                c.set(x, y, LEAF_LT);
            }
        }
    }
    c.pat(
        2,
        3,
        &[
            ".dd.", //
            "dbkd", //
            ".dd.", //
        ],
        &[('d', LEAF_DK), ('b', BARK), ('k', BARK_DK)],
    );
}

/* ==============================  items (rows 4-5)  ============================== */

/// Grayscale palette map used by every icon/mob pattern:
/// '.' = leave (pre-filled shade0 background), 0..3 = the four shades.
const PMAP: &[(char, Ink)] = &[('0', G0), ('1', G1), ('2', G2), ('3', G3)];

/// One 8x8 item/UI icon: shade0 background + pattern (7-8 rows of 8 chars).
fn icon8(s: &mut Sheet, cx: i32, cy: i32, rows: &[&str]) {
    for r in rows {
        assert_eq!(r.len(), 8, "icon8 row width at ({cx},{cy})");
    }
    let mut c = cell(s, cx, cy);
    c.rect(0, 0, 8, 8, G0);
    c.pat(0, 0, rows, PMAP);
}

/// Recenter the non-shade0 content of an 8x8 palette cell (floor-of-half margins on
/// both axes), so every item icon shares the same bounding-box alignment in the UI.
fn center8(s: &mut Sheet, cx: i32, cy: i32) {
    let (ox, oy) = (cx * 8, cy * 8);
    let (mut x0, mut y0, mut x1, mut y1) = (8i32, 8i32, -1i32, -1i32);
    for y in 0..8 {
        for x in 0..8 {
            if s.get(ox + x, oy + y) != G0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x1 < 0 {
        return; // empty cell
    }
    let dx = (8 - (x1 - x0 + 1)) / 2 - x0;
    let dy = (8 - (y1 - y0 + 1)) / 2 - y0;
    if dx == 0 && dy == 0 {
        return;
    }
    let mut buf = [[G0; 8]; 8];
    for (y, row) in buf.iter_mut().enumerate() {
        for (x, p) in row.iter_mut().enumerate() {
            *p = s.get(ox + x as i32, oy + y as i32);
        }
    }
    for y in 0..8i32 {
        for x in 0..8i32 {
            let (sx, sy) = (x - dx, y - dy);
            let ink = if (0..8).contains(&sx) && (0..8).contains(&sy) {
                buf[sy as usize][sx as usize]
            } else {
                G0
            };
            s.set(ox + x, oy + y, ink);
        }
    }
}

/// An 8x8 *item* icon: `icon8` plus auto-centering. Use this for anything that shows
/// up in inventory/crafting lists or the HUD status rows; keep raw `icon8` for
/// position-sensitive cells (frame pieces, slashes, particles).
fn item_icon(s: &mut Sheet, cx: i32, cy: i32, rows: &[&str]) {
    icon8(s, cx, cy, rows);
    center8(s, cx, cy);
}

/// Row 4: the stackable/food/tile item icons. shade0 = transparent background,
/// 1 = dark/outline, 2 = mid, 3 = light (see header for per-item palette roles).
#[rustfmt::skip]
fn items_row4(s: &mut Sheet) {
    // (0,4) flower/rose: stem 1, petals 2, heart 3
    item_icon(s, 0, 4, &[
        "........",
        "..2.2...",
        ".23332..",
        "..2.2...",
        "...1....",
        ".1.1....",
        "..11....",
        "...1....",
    ]);
    // (1,4) plank / brick / cloth: a rounded slab with grain
    item_icon(s, 1, 4, &[
        "........",
        ".111111.",
        "12223221",
        "12232221",
        "12322321",
        ".111111.",
        "........",
        "........",
    ]);
    // (2,4) generic lump (dirt/sand/wool/stone/gunpowder/cloud/wool colors)
    item_icon(s, 2, 4, &[
        "........",
        "..1111..",
        ".132221.",
        ".1322221",
        ".1222221",
        "..12221.",
        "...111..",
        "........",
    ]);
    // (3,4) acorn: cap 3, body 2
    item_icon(s, 3, 4, &[
        "...11...",
        "..1331..",
        ".133331.",
        ".111111.",
        ".122221.",
        "..1221..",
        "...11...",
        "........",
    ]);
    // (4,4) cactus (item): body 2, rib 1, bloom 3
    item_icon(s, 4, 4, &[
        "...32...",
        "...32...",
        "2..32..2",
        "2..12..2",
        "22212222",
        "...12...",
        "...12...",
        "........",
    ]);
    // (5,4) seeds: a scattered handful
    item_icon(s, 5, 4, &[
        "........",
        "..2..3..",
        ".....2..",
        ".3......",
        "..2...2.",
        "....3...",
        ".2......",
        "........",
    ]);
    // (6,4) wheat sheaf: three heavy heads over gathered stems (NOTE: the fence tile
    // reuses this cell with an inverted palette; keep the art in shades 1-3 so the
    // fence stays mostly readable)
    item_icon(s, 6, 4, &[
        ".3..3..3",
        ".3.33.3.",
        "..3.33..",
        "..2.2.2.",
        "...222..",
        "...122..",
        "..2122..",
        "..1.1.1.",
    ]);
    // (7,4) power glove
    item_icon(s, 7, 4, &[
        "........",
        "..1111..",
        ".122221.",
        ".122221.",
        "1222221.",
        ".11111..",
        "..333...",
        "........",
    ]);
    // (8,4) bread loaf with score marks
    item_icon(s, 8, 4, &[
        "........",
        "..1111..",
        ".123221.",
        "12232321",
        "12223221",
        ".111111.",
        "........",
        "........",
    ]);
    // (9,4) apple: body 3, shaded side 2, stem 1, leaf 2 off the stem
    item_icon(s, 9, 4, &[
        "....1.22",
        "...1.22.",
        ".333332.",
        "3333332.",
        "3333322.",
        ".333322.",
        ".33322..",
        "..222...",
    ]);
    // (10,4) ore chunk (coal/iron/lapis/gold/slime ball)
    item_icon(s, 10, 4, &[
        "........",
        "..111...",
        ".13231..",
        "1232321.",
        "1223221.",
        ".12221..",
        "..111...",
        "........",
    ]);
    // (11,4) ingot bar
    item_icon(s, 11, 4, &[
        "........",
        "........",
        ".111111.",
        "13333321",
        "12222221",
        ".111111.",
        "........",
        "........",
    ]);
    // (12,4) glass pane with a diagonal shine
    item_icon(s, 12, 4, &[
        ".111111.",
        "12222321",
        "12223221",
        "12232221",
        "12322221",
        "13222221",
        ".111111.",
        "........",
    ]);
    // (13,4) gem: cut diamond
    item_icon(s, 13, 4, &[
        "........",
        "..111...",
        ".13231..",
        "1233321.",
        ".12321..",
        "..131...",
        "...1....",
        "........",
    ]);
    // (14,4) book: cover 2, page edge 3
    item_icon(s, 14, 4, &[
        "........",
        ".11111..",
        "1222231.",
        "1222231.",
        "1232231.",
        "1222231.",
        ".11111..",
        "........",
    ]);
    // (15,4) bone
    item_icon(s, 15, 4, &[
        "........",
        ".23...3.",
        "33323333",
        ".322223.",
        "33323333",
        ".23...3.",
        "........",
        "........",
    ]);
    // (16,4) wall (item): bricks 2/3, mortar 1
    item_icon(s, 16, 4, &[
        "........",
        "11111111",
        "12231221",
        "11111111",
        "13212231",
        "11111111",
        "........",
        "........",
    ]);
    // (17,4) door (item): face 2, knob 3
    item_icon(s, 17, 4, &[
        ".11111..",
        ".12221..",
        ".12221..",
        ".12231..",
        ".12221..",
        ".12221..",
        ".11111..",
        "........",
    ]);
    // (18,4) torch item: INVERTED palette roles — flame 1(red)/2(orange), stick 3
    item_icon(s, 18, 4, &[
        "....1...",
        "...12...",
        "..1221..",
        "..1221..",
        "...33...",
        "...33...",
        "...33...",
        "...33...",
    ]);
    // (19,4) leather hide
    item_icon(s, 19, 4, &[
        "........",
        ".2...2..",
        ".22222..",
        "2223222.",
        "2222222.",
        ".22222..",
        ".2...2..",
        "........",
    ]);
    // (20,4) meat chop: meat 2, bone stub 3
    item_icon(s, 20, 4, &[
        "........",
        "..1111..",
        ".122221.",
        ".123221.",
        ".12221..",
        "..333...",
        "...3....",
        "........",
    ]);
    // (21,4) bucket: body 1, contents 2 (fill color!), rim 3
    item_icon(s, 21, 4, &[
        "........",
        ".333333.",
        ".122221.",
        ".122221.",
        "..1221..",
        "..1111..",
        "........",
        "........",
    ]);
    // (22,4) scale: fan shell
    item_icon(s, 22, 4, &[
        "........",
        "...11...",
        "..1221..",
        ".122221.",
        ".123321.",
        "..1221..",
        "...11...",
        "........",
    ]);
    // (23,4) shard: angular fragment
    item_icon(s, 23, 4, &[
        "....1...",
        "...12...",
        "..1232..",
        ".12332..",
        ".1232...",
        "..12....",
        "..1.....",
        "........",
    ]);
    // (24,4) fish: body 2, eye/belly 3
    item_icon(s, 24, 4, &[
        "........",
        "..2222.2",
        ".2322222",
        ".2233222",
        "..2222.2",
        "........",
        "........",
        "........",
    ]);
    // (25,4) string: a loose coil
    item_icon(s, 25, 4, &[
        "........",
        "..2222..",
        ".23..32.",
        ".2....2.",
        ".23..32.",
        "..2222..",
        "....2...",
        ".....2..",
    ]);
    // (26,4) key: shades 0 AND 1 are transparent for keys — art in 2-3 only.
    // Ring bow left, shaft right, two teeth hanging at the tip.
    item_icon(s, 26, 4, &[
        "........",
        ".33.....",
        "3..32222",
        "3..3.2.2",
        ".33..2.2",
        "........",
        "........",
        "........",
    ]);
    // (27,4) potion: glass 1, cork 2, liquid 3
    item_icon(s, 27, 4, &[
        "...22...",
        "...11...",
        "..1331..",
        ".133331.",
        ".133331.",
        ".133331.",
        "..1111..",
        "........",
    ]);
    // (28,4) wood log with end rings
    item_icon(s, 28, 4, &[
        "........",
        ".111111.",
        "12222321",
        "13232221",
        "12223231",
        ".111111.",
        "........",
        "........",
    ]);
}

/// Row 5: tools (1 = outline, 2 = wooden handle, 3 = head/tier color), the four flight
/// arrows, a couple of stackables, and four cells RESERVED for the upcoming crafting
/// overhaul (grayscale item icons, shade0 bg, not referenced by game code yet):
///
/// - (8,5)  FIBER — grass-blade bundle tied at the middle
/// - (9,5)  STICK — diagonal branch with a twig stub
/// - (10,5) CORD — coiled rope with a loose end
/// - (11,5) SHARP STONE — knapped flake
#[rustfmt::skip]
fn items_row5(s: &mut Sheet) {
    // (0,5) shovel
    icon8(s, 0, 5, &[
        ".....33.",
        "....3333",
        "...13333",
        "..12.33.",
        ".12.....",
        "12......",
        "21......",
        "........",
    ]);
    // (1,5) hoe: angled blade plate with thickness, handle diagonal
    icon8(s, 1, 5, &[
        "...3333.",
        "..13333.",
        "..12.33.",
        "..12....",
        ".12.....",
        ".12.....",
        "12......",
        "2.......",
    ]);
    // (2,5) sword: 2px blade with a proper crossguard and pommel
    icon8(s, 2, 5, &[
        "......33",
        ".....333",
        "....333.",
        ".1.333..",
        "..1331..",
        "..131...",
        ".121....",
        "12.1....",
    ]);
    // (3,5) pickaxe
    icon8(s, 3, 5, &[
        "...333..",
        ".33.133.",
        "3..12.33",
        "...12..3",
        "..12....",
        ".12.....",
        "12......",
        "........",
    ]);
    // (4,5) axe
    icon8(s, 4, 5, &[
        "...331..",
        "..3333.1",
        "..33312.",
        "..33.2..",
        "...12...",
        "..12....",
        ".12.....",
        "12......",
    ]);
    // (5,5) bow: limb 3, string 2
    icon8(s, 5, 5, &[
        ".333...2",
        "3...3.2.",
        "3....32.",
        ".3...32.",
        ".3..3.2.",
        "..33...2",
        "........",
        "........",
    ]);
    // (6,5) fishing rod: rod 1/2, line + hook 3
    icon8(s, 6, 5, &[
        ".....22.",
        "....21.3",
        "...21..3",
        "..21...3",
        ".21...33",
        "21....3.",
        "........",
        "........",
    ]);
    // (7,5) claymore: broad blade, wide guard
    icon8(s, 7, 5, &[
        ".....333",
        "....333.",
        "...333..",
        "1.333...",
        ".1331...",
        ".131....",
        ".21.1...",
        "12......",
    ]);
    // (13,5) arrow right (also the HUD ammo icon)
    icon8(s, 13, 5, &[
        "........",
        "........",
        "......3.",
        "11222333",
        "......3.",
        "........",
        "........",
        "........",
    ]);
    // (14,5) arrow left
    icon8(s, 14, 5, &[
        "........",
        "........",
        ".3......",
        "33322211",
        ".3......",
        "........",
        "........",
        "........",
    ]);
    // (15,5) arrow up
    icon8(s, 15, 5, &[
        "...3....",
        "..333...",
        "...2....",
        "...2....",
        "...2....",
        "...1....",
        "...1....",
        "........",
    ]);
    // (16,5) arrow down
    icon8(s, 16, 5, &[
        "...1....",
        "...1....",
        "...2....",
        "...2....",
        "...2....",
        "..333...",
        "...3....",
        "........",
    ]);
    // (20,5) stick
    item_icon(s, 20, 5, &[
        "......3.",
        ".....32.",
        "....32..",
        "...32...",
        "..32....",
        ".32.....",
        "32......",
        "........",
    ]);
    // (21,5) grass fibers
    item_icon(s, 21, 5, &[
        "........",
        "3..3..3.",
        ".2.2.2..",
        ".2.32...",
        "..232...",
        "..22....",
        "..2.....",
        "........",
    ]);

    // ---- RESERVED crafting-overhaul icons (see fn doc) ----
    // (8,5) FIBER: grass-blade bundle fanned at both ends, tie band 1 at the waist
    item_icon(s, 8, 5, &[
        ".3..3.3.",
        "..2.32..",
        "..2322..",
        "..1111..",
        "..2322..",
        ".232.2..",
        ".2...2..",
        "........",
    ]);
    // (9,5) STICK: thick diagonal branch (3 = lit top, 2 = shadow), twig stub upper-left
    item_icon(s, 9, 5, &[
        "......13",
        "..3..132",
        "...3132.",
        "...132..",
        "..132...",
        ".132....",
        "132.....",
        "........",
    ]);
    // (10,5) CORD: coiled rope donut (strand twists 3) with a loose end
    item_icon(s, 10, 5, &[
        "..1111..",
        ".123231.",
        "123..321",
        "132..231",
        ".123321.",
        "..1111..",
        "....221.",
        "........",
    ]);
    // (11,5) SHARP STONE: knapped flake — point up, sharp edge 3, facet body 2
    item_icon(s, 11, 5, &[
        "....11..",
        "...131..",
        "..1331..",
        ".13321..",
        ".13221..",
        "12221...",
        "1111....",
        "........",
    ]);

    // ---- NEW weapon icons (22..25,5) — not referenced by game code yet; tool-style
    // shade roles (1 = outline/dark, 2 = wooden handle, 3 = head/tier metal) so the
    // item owner can reuse the tool tier palettes. ----
    // (22,5) SPEAR: leaf head top-right, lashing, long shaft
    icon8(s, 22, 5, &[
        "......33",
        ".....333",
        "....133.",
        "...11...",
        "..12....",
        ".12.....",
        "12......",
        "2.......",
    ]);
    // (23,5) CROSSBOW: metal limbs 3, string 1, wooden stock 2, stirrup 1
    icon8(s, 23, 5, &[
        ".3....3.",
        ".33..33.",
        "..3333..",
        ".112211.",
        "...22...",
        "...22...",
        "...22...",
        "...11...",
    ]);
    // (24,5) THROWING KNIFE: slim pointed blade, short wrapped grip
    icon8(s, 24, 5, &[
        "...3....",
        "..331...",
        "..331...",
        "..331...",
        "..111...",
        "..221...",
        "..221...",
        "...1....",
    ]);
    // (25,5) SLINGSHOT: wooden fork 2, band 3 dipping to the pouch
    icon8(s, 25, 5, &[
        ".3....3.",
        ".23..32.",
        "..2332..",
        "..2..2..",
        "..2222..",
        "...22...",
        "...22...",
        "...1....",
    ]);
}

/// NEW forage/food icons, cells (11..17,10) — drawn for the flora/food work, not
/// referenced by game code yet. Standard icon roles (1 = dark/outline, 2 = mid,
/// 3 = light) so owners pick the hues per item.
#[rustfmt::skip]
fn food_icons(s: &mut Sheet) {
    // (11,10) BERRY: fat berry with a shine + a small second berry
    item_icon(s, 11, 10, &[
        "...1....",
        "..1.1...",
        ".2332...",
        "23332.1.",
        "23322.32",
        ".2222.22",
        "..22....",
        "........",
    ]);
    // (12,10) MUSHROOM: domed cap 2 with light spots 3, pale stalk 3
    item_icon(s, 12, 10, &[
        "..1111..",
        ".123321.",
        "12232221",
        "12222321",
        ".111111.",
        "...33...",
        "...33...",
        "..3333..",
    ]);
    // (13,10) CACTUS FRUIT: prickly pear — leaning oval, tuft 1, spine specks 1
    item_icon(s, 13, 10, &[
        "....11..",
        "..1331..",
        ".13332..",
        ".13322..",
        "1.3222..",
        ".2222.1.",
        "..222...",
        "...1....",
    ]);
    // (14,10) COCONUT: husked shell 2, three eyes 1, highlight 3
    item_icon(s, 14, 10, &[
        "..1111..",
        ".132221.",
        "13222221",
        "12212121",
        "12222221",
        ".122221.",
        "..1111..",
        "........",
    ]);
    // (15,10) COOKED MEAT: glazed roast slab, grill marks 1, glaze shine 3
    item_icon(s, 15, 10, &[
        "........",
        "..11111.",
        ".1332321",
        "12322321",
        "12232221",
        ".122221.",
        "..1111..",
        "........",
    ]);
    // (16,10) JACK-O-LANTERN: pumpkin 2, stem 1, lit triangle eyes + grin 3
    item_icon(s, 16, 10, &[
        "...11...",
        ".111111.",
        "12222221",
        "12312321",
        "12222221",
        "12333321",
        ".122221.",
        "..1111..",
    ]);
    // (17,10) PUMPKIN (item): ribbed gourd 2 with rib lines 1, highlight 3
    item_icon(s, 17, 10, &[
        "...11...",
        "..1111..",
        ".132321.",
        "13222321",
        "13222321",
        "12322321",
        ".122321.",
        "..1111..",
    ]);
}

/* ==============================  UI (rows 11-13)  ============================== */

/// Row 12: HUD status icons + the smash particle + the clothing item.
///
/// HUD palettes (renderer.rs render_gui): heart get4(-1,200,500,533) — 1 = dark-red
/// rim, 2 = fill, 3 = shine; empty variants swap shades 2-3 to black, so the shade1
/// rim must carry the full silhouette on its own.
#[rustfmt::skip]
fn ui_row12(s: &mut Sheet) {
    // (0,12) heart: classic two-lobe heart, full dark rim 1, fill 2, shine 3
    item_icon(s, 0, 12, &[
        ".11.11..",
        "1322231.",
        "1222221.",
        "1222221.",
        ".12221..",
        "..121...",
        "...1....",
        "........",
    ]);
    // (1,12) stamina: classic zigzag lightning bolt — pale core 3, yellow shade 2,
    // dark trailing edge 1 (keeps the silhouette readable in the depleted variant,
    // whose palette turns shades 2-3 black)
    item_icon(s, 1, 12, &[
        "...3321.",
        "..3321..",
        ".3321...",
        ".333321.",
        "...321..",
        "..321...",
        ".321....",
        "........",
    ]);
    // (2,12) hunger: drumstick — round meat 2 (orange), roast shading 3, thin bone
    // with a knob (shade3; the hunger palette has no light shade, so the bone reads
    // as the dark part of the silhouette)
    item_icon(s, 2, 12, &[
        "..111...",
        ".12221..",
        "122221..",
        "122231..",
        ".12231..",
        "...133..",
        "....333.",
        "........",
    ]);
    // (3,12) armor: kite shield (also the armor items' sprite) — face 2, chevron 3
    item_icon(s, 3, 12, &[
        ".111111.",
        "12233221",
        "12233221",
        "12233221",
        ".122221.",
        "..1221..",
        "...11...",
        "........",
    ]);
    // (5,12) smash particle: one quadrant of the burst, mirrored around the
    // tile center in-game (rays radiate from this cell's top-right corner)
    icon8(s, 5, 12, &[
        "...2.3.3",
        "......3.",
        ".....3.3",
        "....3...",
        "...3...2",
        "..3.....",
        ".2......",
        "........",
    ]);
    // (6,12) clothes: folded shirt, body 3, folds 2
    item_icon(s, 6, 12, &[
        "........",
        ".11..11.",
        "13311331",
        "11333311",
        ".133331.",
        ".132231.",
        ".111111.",
        "........",
    ]);
}

/// Row 13: menu frame pieces, swim ripple, attack slashes, zap bolt.
#[rustfmt::skip]
fn ui_row13(s: &mut Sheet) {
    // (0,13) frame corner (rounded), (1,13) top edge, (2,13) left edge / flat fill.
    // Roles: 3 = light rim, 1 = dark line, 2 = panel face, 0 = outside.
    icon8(s, 0, 13, &[
        "........",
        "...33333",
        "..311111",
        ".3112222",
        ".3122222",
        ".3122222",
        ".3122222",
        ".3122222",
    ]);
    icon8(s, 1, 13, &[
        "........",
        "33333333",
        "11111111",
        "22222222",
        "22222222",
        "22222222",
        "22222222",
        "22222222",
    ]);
    icon8(s, 2, 13, &[
        ".3122222",
        ".3122222",
        ".3122222",
        ".3122222",
        ".3122222",
        ".3122222",
        ".3122222",
        ".3122222",
    ]);
    // (5,13) swim ripple: covers the player's legs; crest 3, water body 2
    icon8(s, 5, 13, &[
        "........",
        "........",
        "........",
        "3..3..3.",
        "23.23.23",
        "32233223",
        "22222222",
        "22222222",
    ]);
    // (6,13) horizontal slash (up/down attack arc, drawn twice mirrored)
    icon8(s, 6, 13, &[
        "........",
        "........",
        "......33",
        "....332.",
        "..332...",
        "332.....",
        "2.......",
        "........",
    ]);
    // (7,13) vertical slash (left/right attack arc, drawn twice stacked)
    icon8(s, 7, 13, &[
        "....3...",
        ".....3..",
        ".....23.",
        "......23",
        "......23",
        ".....23.",
        ".....3..",
        "....3...",
    ]);
    // (8,13) zap bolt (the night wisp's projectile; formerly the air wizard's spark)
    icon8(s, 8, 13, &[
        "........",
        "...3....",
        "..232...",
        ".23332..",
        "..232...",
        "...3....",
        "........",
        "........",
    ]);
}

/// Cells (0,11) and (3,11): the intro splash effect (drawn with animated palettes).
fn splash_cells(s: &mut Sheet) {
    let mut c = cell(s, 0, 11);
    c.rect(0, 0, 8, 8, G1);
    for y in 0..8 {
        for x in 0..8 {
            if speck(x, y, 71, 5) {
                c.set(x, y, G2);
            }
            if speck(x, y, 72, 9) {
                c.set(x, y, G3);
            }
        }
    }
    let mut c = cell(s, 3, 11);
    for y in 0..8i32 {
        for x in 0..8i32 {
            let d = (x + y).rem_euclid(4);
            let ink = if d < 2 { G2 } else { G0 };
            c.set(x, y, ink);
            if (x - y).rem_euclid(8) == 0 {
                c.set(x, y, G3);
            }
        }
    }
}

/* ==========================  decor tiles & structures  ========================== */

/// A 16x16 grayscale sprite: shade0 background + pattern.
fn spr16(s: &mut Sheet, cx: i32, cy: i32, rows: &[&str]) {
    assert_eq!(rows.len(), 16, "spr16 rows at ({cx},{cy})");
    for r in rows {
        assert_eq!(r.len(), 16, "spr16 row width at ({cx},{cy})");
    }
    let mut c = cell(s, cx, cy);
    c.rect(0, 0, 16, 16, G0);
    c.pat(0, 0, rows, PMAP);
}

/// A 16x16 true-color sprite on a transparent background.
fn tc16(s: &mut Sheet, cx: i32, cy: i32, rows: &[&str], map: &[(char, Ink)]) {
    assert_eq!(rows.len(), 16, "tc16 rows at ({cx},{cy})");
    for r in rows {
        assert_eq!(r.len(), 16, "tc16 row width at ({cx},{cy})");
    }
    let mut c = cell(s, cx, cy);
    c.pat(0, 0, rows, map);
}

/// Cells (0..1,24..25) open door, (2..3,24..25) closed door (full-tile, grayscale).
/// Roles: 0 = frame, 1 = door face, 2 = detail, open door's walk-through gap = 3.
fn door_cells(s: &mut Sheet) {
    // closed: 1px frame, planked face, knob
    let mut c = cell(s, 2, 24);
    c.rect(0, 0, 16, 16, G0);
    c.rect(1, 1, 14, 14, G1);
    for (x, y) in [(1, 1), (14, 1), (1, 14), (14, 14)] {
        c.set(x, y, G0); // rounded frame corners
    }
    c.vline(5, 1, 14, G2); // plank seams
    c.vline(10, 1, 14, G2);
    c.set(2, 3, G2); // hinges
    c.set(2, 12, G2);
    c.rect(12, 7, 1, 2, G3); // knob

    // open: frame around a dark gap, door leaf swung against the left jamb
    let mut c = cell(s, 0, 24);
    c.rect(0, 0, 16, 16, G0);
    c.rect(1, 1, 14, 14, G3); // the walk-through gap
    c.rect(1, 1, 3, 14, G1); // swung leaf
    c.vline(4, 1, 14, G2); // leaf edge
    c.hline(1, 1, 14, G1); // lintel
    c.hline(1, 14, 14, G1); // threshold
    c.set(14, 1, G0);
    c.set(14, 14, G0);
}

/// Wood wall: sparse blob (4..6,22..24) whose center cell doubles as the full tile,
/// plus sides block (7..8,22..23). Stone/obsidian wall: sparse (4..6,25..27) + sides
/// (7..8,24..25) (their full tile is `Sprite::blank`). Roles: 0 = seams/outline,
/// 1 = face, 2 = face shading, 3 = outside.
fn wall_cells(s: &mut Sheet) {
    // -- wood: horizontal planks --
    let mut c = cell(s, 4, 22);
    blob24(&mut c, 5, G3, &[G0, G1], 91);
    for y in 0..24 {
        for x in 0..24 {
            if !rounded_inside(x, y, 24, 24, 2, 5) {
                continue;
            }
            match y % 4 {
                0 => c.set(x, y, G0), // plank seam
                3 => c.set(x, y, G2), // plank lower shading
                _ => {
                    if (x + (y / 4) * 5) % 9 == 0 {
                        c.set(x, y, G0); // butt joint
                    }
                }
            }
        }
    }
    let mut sd = cell(s, 7, 22);
    sd.rect(0, 0, 16, 16, G1);
    for y in 0..16 {
        if y % 4 == 0 {
            sd.hline(0, y, 16, G0);
        }
        if y % 4 == 3 {
            sd.hline(0, y, 16, G2);
        }
    }
    sd.disc(8, 8, 4, G0);
    sd.disc(8, 8, 2, G3);

    // -- stone: running-bond bricks --
    let mut c = cell(s, 4, 25);
    blob24(&mut c, 5, G3, &[G0, G1], 92);
    for y in 0..24 {
        for x in 0..24 {
            if !rounded_inside(x, y, 24, 24, 2, 5) {
                continue;
            }
            if y % 4 == 0 {
                c.set(x, y, G0); // mortar course
            } else if (x + (y / 4 % 2) * 4) % 8 == 0 {
                c.set(x, y, G0); // head joint, offset per course
            } else if y % 4 == 1 && speck(x, y, 15, 4) {
                c.set(x, y, G2); // top-lit brick faces
            }
        }
    }
    // stone sides: only shade0 differs from the face at its call sites
    let mut sd = cell(s, 7, 24);
    sd.rect(0, 0, 16, 16, G1);
    for y in 0..16 {
        if y % 4 == 0 {
            sd.hline(0, y, 16, G0);
        }
    }
    sd.disc(8, 8, 4, G0);
}

/// Grave stones (true color, drawn over grass): (11..12,11..12) standing slab,
/// (13..14,11..12) broken/rubble. Variety shapes (picked per tile position by
/// `grave_stone.rs`), all 2x2 blocks on rows 11..=12: (15,11) rounded headstone,
/// (17,11) stone cross, (19,11) cracked slab, (23,11) wooden cross — plus broken
/// variants: (21,11) second stone rubble, (25,11) collapsed wooden cross.
fn gravestone_cells(s: &mut Sheet) {
    let map: &[(char, Ink)] = &[
        ('o', OUT),
        ('l', STONE_LT),
        ('m', STONE_MD),
        ('d', STONE_DK),
        ('g', MOSS),
        ('k', LEAF_DK),
    ];
    tc16(
        s,
        11,
        11,
        &[
            "................",
            "....oooooo......",
            "...omllllmo.....",
            "..olllllllmo....",
            "..olllllllmo....",
            "..olddldllmo....",
            "..olllllllmo....",
            "..oldldldlmo....",
            "..olllllllmo....",
            "..olldldllmo....",
            "..ogllllllmo....",
            "..oglllllgmo....",
            ".ogglllllggmo...",
            ".okgggggggggo...",
            "..ooooooooooo...",
            "................",
        ],
        map,
    );
    tc16(
        s,
        13,
        11,
        &[
            "................",
            "................",
            "................",
            "................",
            "................",
            "...ooo..........",
            "..olmmo....oo...",
            "..ollmo...ommo..",
            "..olldmo..oldo..",
            ".ogllldmo..oo...",
            ".oglllldmo..om..",
            ".okgglllgmo.oo..",
            "..ooggggggo.....",
            "...oooooooo.....",
            "................",
            "................",
        ],
        map,
    );

    // (15,11) rounded headstone: arched top, worn face
    tc16(
        s,
        15,
        11,
        &[
            "................",
            ".....oooo.......",
            "...oolllloo.....",
            "..ollllllllo....",
            "..olllllllmo....",
            "..olddldllmo....",
            "..olllllllmo....",
            "..oldldldlmo....",
            "..olllllllmo....",
            "..olldldllmo....",
            "..ogllllllmo....",
            "..oglllllgmo....",
            ".ogglllllggmo...",
            ".okgggggggggo...",
            "..ooooooooooo...",
            "................",
        ],
        map,
    );

    // (17,11) stone cross on a mossy base
    tc16(
        s,
        17,
        11,
        &[
            "................",
            ".....oooo.......",
            "....olllmo......",
            "....olllmo......",
            "..oooolllmooo...",
            ".ollllllllllmo..",
            ".olmmmlllmmmmo..",
            "..oooolllmooo...",
            "....olllmo......",
            "....olldmo......",
            "....olldmo......",
            "...oglllmgo.....",
            "..ogglllllggo...",
            ".okggggggggggo..",
            "..oooooooooo....",
            "................",
        ],
        map,
    );

    // (19,11) cracked slab: still standing, split by a zigzag fracture, chipped
    // top-right corner
    tc16(
        s,
        19,
        11,
        &[
            "................",
            "....ooooo.......",
            "...ollllloo.....",
            "..olllllllmo....",
            "..olddlolllmo...",
            "..olllollllmo...",
            "..ollolldllmo...",
            "..oldolllllmo...",
            "..ollloldllmo...",
            "..olldollllmo...",
            "..oglllolllmo...",
            "..ogllllolgmo...",
            ".ogglllllogmo...",
            ".okgggggggggo...",
            "..ooooooooooo...",
            "................",
        ],
        map,
    );

    // (21,11) rubble variant: a different collapse — leaning stump, strewn shards
    tc16(
        s,
        21,
        11,
        &[
            "................",
            "................",
            "................",
            "................",
            "................",
            "........oo......",
            "..oo...ollmo....",
            ".olmo..olldo....",
            ".ollmo..oldo....",
            ".oglldo..oo.om..",
            ".ogllldmo...oo..",
            ".okgglllgo.oldo.",
            "..ooggggggo.oo..",
            "...ooooooo......",
            "................",
            "................",
        ],
        map,
    );

    // Wooden markers — weathered two-plank crosses, so cemeteries mix stone and wood.
    let wood_map: &[(char, Ink)] = &[
        ('o', OUT),
        ('t', WOOD_LT),
        ('w', WOOD_MD),
        ('k', WOOD_DK),
        ('g', MOSS),
        ('x', LEAF_DK),
    ];
    // (23,11) standing wooden cross
    tc16(
        s,
        23,
        11,
        &[
            "................",
            ".....oo.........",
            "....otwo........",
            "....otko........",
            ".ooootkoooo.....",
            "otttttkwwwko....",
            "okkkktkkkkko....",
            ".oooootkooo.....",
            ".....otko.......",
            ".....otko.......",
            ".....otko.......",
            "....ogtkgo......",
            "...oggtkggo.....",
            "..oxgggggggo....",
            "...ooooooooo....",
            "................",
        ],
        wood_map,
    );
    // (25,11) broken wooden cross: snapped post leaning, crossarm fallen at the base
    tc16(
        s,
        25,
        11,
        &[
            "................",
            "................",
            "................",
            "........ow......",
            ".......otko.....",
            ".......otko.....",
            "......otko......",
            "......otko......",
            ".....otko.......",
            "....ogtkgo......",
            "...oggkkggo.....",
            "..oxggggggo.....",
            ".ootttwkkoo.....",
            ".okkkkkkkko.....",
            "..oooooooo......",
            "................",
        ],
        wood_map,
    );
}

/// Cells (22..23,8..9): pumpkin (true color, drawn over grass).
fn pumpkin_cells(s: &mut Sheet) {
    tc16(
        s,
        22,
        8,
        &[
            "......ss........",
            "......ss........",
            "....oooooo......",
            "..oopppppdoo....",
            ".opppdppppddo...",
            ".oppdppppdpdo...",
            "opppdppppdppdo..",
            "opydppppppdydo..",
            "opyydpppppdyydo.", // eyes
            "oppdpppppdpppdo.",
            "opppdppppdpppdo.",
            ".opddpyyyydpdo..",
            ".oppdpyyyypddo..",
            "..ooppddddppo...",
            "....oooooooo....",
            "................",
        ],
        &[
            ('o', OUT),
            ('p', PUMPK),
            ('d', PUMPK_DK),
            ('y', FLAME_YL),
            ('s', LEAF_DK),
        ],
    );
}

/// Cells (26..31,8..9): tall grass — tall (26), small (28), medium (30). True color,
/// drawn over grass. The three growth stages are deliberately unmistakable: small is
/// a few pale sprouts hugging the ground, medium is knee-high mid-green tufts, tall
/// is a dense full-height stand crowned with golden seed heads (only the tall stage
/// has any gold, and each stage darkens the greens a step).
fn tall_grass_cells(s: &mut Sheet) {
    let map: &[(char, Ink)] = &[
        ('d', LEAF_DK),
        ('m', LEAF_MD),
        ('l', LEAF_LT),
        ('h', LEAF_HI),
        ('g', GOLDEN),
    ];
    // TALL: dense, screen-filling blades, dark at the roots, golden heads on top
    tc16(
        s,
        26,
        8,
        &[
            ".g...g..g....g..",
            ".gl..g..lg...l..",
            ".ml.gl.ml.g.mlg.",
            ".ml.ml..l.lg.l..",
            ".dl.ml.ml.ml.ml.",
            ".dl.dl.ml.ml.ml.",
            ".dl.dl.dl.dl.dl.",
            ".ml.dl..l.dl.dl.",
            ".ml.ml.ml.dl..l.",
            ".dl.ml.ml.ml.ml.",
            ".dl.dl.dl.ml.ml.",
            ".dd.dl.dl.dl.dl.",
            ".dd.dd.dd.dd.dd.",
            "..d..d..d..d..d.",
            "................",
            "................",
        ],
        map,
    );
    // SMALL: a few short pale sprouts at ground level — clearly freshly grown
    tc16(
        s,
        28,
        8,
        &[
            "................",
            "................",
            "................",
            "................",
            "................",
            "................",
            "................",
            "................",
            "................",
            "....h......h....",
            "...hl...h..hl...",
            "...ll..hl..ll...",
            "...ml..ll..ml.h.",
            "....m..m....m.l.",
            "................",
            "................",
        ],
        map,
    );
    // MEDIUM: knee-high mid-green tufts, no seed heads yet
    tc16(
        s,
        30,
        8,
        &[
            "................",
            "................",
            "................",
            "................",
            "................",
            "................",
            "..l....l....l...",
            ".ml...ml...ml...",
            ".ml.l.ml.l.ml...",
            ".ml.l.ml.l.ml.l.",
            ".dl.m.dl.m.dl.m.",
            ".dl.m.dl.m.dl.m.",
            "..d.d..d.d..d.d.",
            "................",
            "................",
            "................",
        ],
        map,
    );
}

/* ==========================  flora tiles (rows 26-28)  ========================== */

/// NEW flora cells (true color, transparent background — the ground tile is drawn
/// underneath), rows 26..=28.
///
/// **Species tree sets** — six cells each, 2 cols x 3 rows, the same six roles the
/// broadleaf mechanism samples (`tree.rs` / `tree_species.rs` / `snow_tree.rs`):
/// `(bx,by)` TL / `(bx+1,by)` TR / `(bx,by+1)` BL / `(bx+1,by+1)` BR standalone
/// quarters, `(bx,by+2)` full canopy fill, `(bx+1,by+2)` fill with a bark knot.
/// Standalone silhouettes are species-distinct (pine = tiered triangle, willow =
/// drooping curtains, palm = curved trunk + frond burst, flat-crown = umbrella on a
/// bare trunk, dead = bare forks); cluster interiors merge through the fill cells.
/// Bases `(bx,by)`:
///
///   pine (0,26) | dead tree (2,26) | willow (7,26) | palm (9,26)
///   flat-crown (11,26) | snow pine (13,26)
///
/// **Second shape variants** (standalone-only 2x2 blocks, rows 28..=29, for a
/// position-hash pick on the tile side — not wired yet): pine B (19,28),
/// dead B (21,28), willow B (23,28), palm B (25,28), flat-crown B (27,28),
/// snow pine B (29,28).
///
/// **Other flora** (2x2 blocks): berry bush ripe (15,26) / picked (17,26),
/// reed tuft (19,26), seaweed (21,26), coral (23,26), fruiting saguaro (25,26),
/// barrel cactus (27,26), jack-o-lantern lit (29,26), mushroom (15,28),
/// dry bush tumbleweed (17,28).
///
/// (Cells (4..6,25..27) in this region belong to the stone wall sparse blob.)
fn flora_cells(s: &mut Sheet) {
    /* ----- species trees ----- */

    // Paint a hand-drawn 16x16 standalone tree and split it into 2x2 quarter cells
    // at (bx, by) — silhouettes are the species identity, so these are drawn by
    // hand instead of sharing a dome.
    fn split16(s: &mut Sheet, bx: i32, by: i32, rows: &[&str; 16], map: &[(char, Ink)]) {
        let mut px = [[TR; 16]; 16];
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if let Some((_, ink)) = map.iter().find(|(c, _)| *c == ch) {
                    px[y][x] = *ink;
                }
            }
        }
        for (qx, qy, ox, oy) in [(0, 0, 0, 0), (1, 0, 8, 0), (0, 1, 0, 8), (1, 1, 8, 8)] {
            let mut c = cell(s, bx + qx, by + qy);
            for y in 0..8i32 {
                for x in 0..8i32 {
                    let ink = px[(oy + y) as usize][(ox + x) as usize];
                    if ink[3] != 0 {
                        c.set(x, y, ink);
                    }
                }
            }
        }
    }

    // Fill + knot-fill cells at (bx, by+2) from a texture closure (cluster interiors).
    fn fill_cells(s: &mut Sheet, bx: i32, by: i32, tex: &dyn Fn(i32, i32) -> Ink) {
        for knot in 0..2i32 {
            let mut c = cell(s, bx + knot, by + 2);
            for y in 0..8 {
                for x in 0..8 {
                    c.set(x, y, tex(x + 4, y + 2));
                }
            }
            if knot == 1 {
                c.pat(
                    2,
                    3,
                    &[
                        ".dd.", //
                        "dbkd", //
                        ".dd.", //
                    ],
                    &[('d', LEAF_DK), ('b', BARK), ('k', BARK_DK)],
                );
            }
        }
    }

    let frost = rgb(236, 242, 250);
    let frost_dim = rgb(205, 216, 230);
    let trees: &[(char, Ink)] = &[
        ('h', LEAF_HI),
        ('l', LEAF_LT),
        ('m', LEAF_MD),
        ('d', LEAF_DK),
        ('e', PINE_DK),
        ('b', BARK),
        ('k', BARK_DK),
        ('g', GOLDEN),
        ('r', RED_CL),
        ('o', OUT),
        ('w', frost),
        ('v', frost_dim),
    ];

    // PINE (0,26) + variant B (19,28): tall narrow triangle, visible tiers
    let pine_a: [&str; 16] = [
        ".......dd.......",
        "......dmed......",
        ".....dlmeed.....",
        "....dllmmeed....",
        "......dmeed.....",
        ".....dlmmeed....",
        "....dllmmeeed...",
        "...dllmmmeeeed..",
        ".....dlmmeed....",
        "....dllmmeeed...",
        "...dllmmmeeeed..",
        "..dlllmmmeeeeed.",
        ".......bk.......",
        ".......bk.......",
        "......dbkd......",
        "................",
    ];
    split16(s, 0, 26, &pine_a, trees);
    cell(s, 0, 26).outline(0, 0, 16, 16, OUT);
    cell(s, 1, 26).outline(0, 0, 8, 8, OUT);
    let pine_tex = |x: i32, y: i32| -> Ink {
        // tier-shadow bands + scattered dark needles share the shadow ink
        if (y.rem_euclid(4) == 3 && speck(x, y, 83, 2)) || speck(x, y, 81, 4) {
            PINE_DK
        } else if speck(x, y, 82, 6) {
            LEAF_MD
        } else {
            LEAF_DK
        }
    };
    fill_cells(s, 0, 26, &pine_tex);
    let pine_b: [&str; 16] = [
        "................",
        "........dd......",
        ".......dmed.....",
        "......dlmeed....",
        ".......dmed.....",
        "......dlmeed....",
        ".....dllmeeed...",
        "......dmmeed....",
        ".....dlmmeeed...",
        "....dllmmeeeed..",
        "...dllmmmeeeeed.",
        "........bk......",
        "........bk......",
        ".......dbkd.....",
        "................",
        "................",
    ];
    split16(s, 19, 28, &pine_b, trees);
    cell(s, 19, 28).outline(0, 0, 16, 16, OUT);

    // DEAD TREE (2,26) + variant B (21,28): bare forked branches
    let dead_a: [&str; 16] = [
        "..k.......k.....",
        "..kk..k..kk.....",
        "...k..k.kk...k..",
        "...kk.bkk...kk..",
        "....k.bk...kk...",
        ".k..kkbbk.kk....",
        ".kk...bbkkk.....",
        "..kkk.bbk.......",
        "....kbbk........",
        ".....bbk........",
        ".....bbk........",
        "......bk........",
        ".....bbkk.......",
        "....bbkkkk......",
        "...kk...........",
        "................",
    ];
    split16(s, 2, 26, &dead_a, trees);
    for knot in 0..2i32 {
        let mut c = cell(s, 2 + knot, 28);
        for y in 0..8i32 {
            for x in 0..8i32 {
                if (x + y).rem_euclid(5) == 0 || (x - y).rem_euclid(7) == 0 {
                    c.set(x, y, BARK_DK);
                } else if speck(x, y, 87, 9) {
                    c.set(x, y, BARK);
                }
            }
        }
        if knot == 1 {
            c.pat(
                2,
                3,
                &[".kk.", "kbbk", ".kk."],
                &[('b', BARK), ('k', BARK_DK)],
            );
        }
    }
    let dead_b: [&str; 16] = [
        "................",
        "....k.....k.....",
        "....kk...kk.....",
        ".....k...k......",
        "..k..kk.kk..k...",
        "..kk..bbk..kk...",
        "...kkkbbkkkk....",
        ".....kbbk.......",
        "......bbk.......",
        "......bk........",
        "......bbk.......",
        ".....bbk........",
        ".....bbkk.......",
        "....bkk.kk......",
        "................",
        "................",
    ];
    split16(s, 21, 28, &dead_b, trees);

    // WILLOW (7,26) + variant B (23,28): drooping curtain strands to the ground
    let willow_a: [&str; 16] = [
        "....ddddddd.....",
        "..ddllllllldd...",
        ".dlhllllllllld..",
        ".dlhlllllllllld.",
        ".dlldlldlldlld..",
        ".dl.dl.dl.dl.d..",
        ".ml.dl.bk.dl....",
        ".ml.ml.bk.ml....",
        "..l.ml.bk.ml....",
        ".ml..l.bk..l....",
        "..d.ml.bk.ml....",
        "....ml.bk.ml....",
        ".....d.bk..d....",
        "......dbkd......",
        "................",
        "................",
    ];
    split16(s, 7, 26, &willow_a, trees);
    cell(s, 7, 26).outline(0, 0, 16, 16, OUT);
    cell(s, 8, 26).outline(0, 0, 8, 8, OUT);
    let willow_tex = |x: i32, y: i32| -> Ink {
        match x.rem_euclid(3) {
            0 => LEAF_DK,
            1 => {
                if speck(x, y, 84, 5) {
                    LEAF_LT
                } else {
                    LEAF_MD
                }
            }
            _ => {
                if y.rem_euclid(4) == 2 {
                    LEAF_MD
                } else {
                    LEAF_LT
                }
            }
        }
    };
    fill_cells(s, 7, 26, &willow_tex);
    let willow_b: [&str; 16] = [
        "................",
        ".....ddddd......",
        "...ddlllllddd...",
        "..dlhllllllld...",
        "..dlldlldllld...",
        "..dl.dl.dl.ld...",
        "..ml.dl.bkdl....",
        "..ml.ml.bk.l....",
        "...l.ml.bk.ml...",
        "..ml..l.bk..l...",
        "...d.ml.bk.l....",
        ".....ml.bk.m....",
        "......d.bk......",
        ".....dbkd.......",
        "................",
        "................",
    ];
    split16(s, 23, 28, &willow_b, trees);
    cell(s, 23, 28).outline(0, 0, 16, 16, OUT);

    // PALM (9,26) + variant B (25,28): curved trunk + frond burst, coconuts
    let palm_a: [&str; 16] = [
        "..dd..dd..dd....",
        ".dllddlldllld...",
        "dll.dllllld.ld..",
        "dl..dllllld..d..",
        ".d..dlgglld.....",
        "....dggbkd......",
        "......obk.......",
        "......bk........",
        ".....obk........",
        ".....bk.........",
        "....obk.....gg..",
        "....bk......kg..",
        "...obk..........",
        "...bbk..........",
        "..dbbkd.........",
        "................",
    ];
    split16(s, 9, 26, &palm_a, trees);
    let palm_tex = |x: i32, y: i32| -> Ink {
        if (x + y).rem_euclid(4) == 0 {
            LEAF_DK
        } else if (x - y).rem_euclid(4) == 2 {
            LEAF_LT
        } else {
            LEAF_MD
        }
    };
    fill_cells(s, 9, 26, &palm_tex);
    let palm_b: [&str; 16] = [
        "....dd..dd..dd..",
        "...dlldllddlld..",
        "..dl.dllllld.ld.",
        "..d..dllllld..d.",
        ".....dlgglld....",
        "......dggkbd....",
        ".......okb......",
        "........kb......",
        ".........kbo....",
        ".........kb.....",
        "..........kbo...",
        "..gg......kb....",
        "..kg......kbo...",
        ".........bkb....",
        "........dbkbd...",
        "................",
    ];
    split16(s, 25, 28, &palm_b, trees);

    // FLAT-CROWN (11,26) + variant B (27,28): wide umbrella on a bare forked trunk
    let flat_a: [&str; 16] = [
        "................",
        "...ddddddddd....",
        ".ddhlllllllldd..",
        ".dhllllllllllmd.",
        "dhllllllllllmmd.",
        ".ddmmmmmmmmmdd..",
        "...ddm...mdd....",
        ".......bk.......",
        "......bbk.......",
        "......bk........",
        "......bk........",
        ".....bbk........",
        "....dbkd........",
        "................",
        "................",
        "................",
    ];
    split16(s, 11, 26, &flat_a, trees);
    cell(s, 11, 26).outline(0, 0, 16, 16, OUT);
    cell(s, 12, 26).outline(0, 0, 8, 8, OUT);
    let flat_tex = |x: i32, y: i32| -> Ink {
        if y.rem_euclid(8) < 2 {
            if speck(x, y, 85, 4) { LEAF_HI } else { LEAF_LT }
        } else if y.rem_euclid(8) >= 6 && speck(x, y, 86, 2) {
            LEAF_DK
        } else if speck(x, y, 88, 7) {
            LEAF_LT
        } else {
            LEAF_MD
        }
    };
    fill_cells(s, 11, 26, &flat_tex);
    let flat_b: [&str; 16] = [
        "................",
        "....ddddddddd...",
        "..ddhlllllllldd.",
        ".dhllllllllllmd.",
        ".ddmmmmmmmmmmdd.",
        "..dddd..bk......",
        ".dhllmd.bk......",
        ".ddmmdd.bk......",
        "....k..bbk......",
        ".....kbbk.......",
        "......bk........",
        ".....bbk........",
        ".....bk.........",
        "....dbkd........",
        "................",
        "................",
    ];
    split16(s, 27, 28, &flat_b, trees);
    cell(s, 27, 28).outline(0, 0, 16, 16, OUT);

    // SNOW PINE (13,26) + variant B (29,28): the pine silhouette under snow caps
    let snowpine_a: [&str; 16] = [
        ".......ww.......",
        "......wwwd......",
        ".....dwwwed.....",
        "....dvlmeeed....",
        "......wwed......",
        ".....wwwwed.....",
        "....dvlmeeed....",
        "...dvllmmeeed...",
        ".....wwwwd......",
        "....wwwwwwed....",
        "...dvllmmeeeed..",
        "..dvlllmmeeeeed.",
        ".......bk.......",
        ".......bk.......",
        "......dbkd......",
        "................",
    ];
    split16(s, 13, 26, &snowpine_a, trees);
    cell(s, 13, 26).outline(0, 0, 16, 16, OUT);
    cell(s, 14, 26).outline(0, 0, 8, 8, OUT);
    let snowpine_tex = |x: i32, y: i32| -> Ink {
        if speck(x, y, 89, 3) {
            frost
        } else if speck(x, y, 90, 6) {
            frost_dim
        } else if (y.rem_euclid(4) == 3 && speck(x, y, 83, 2)) || speck(x, y, 81, 4) {
            PINE_DK
        } else {
            LEAF_DK
        }
    };
    fill_cells(s, 13, 26, &snowpine_tex);
    let snowpine_b: [&str; 16] = [
        "................",
        "........ww......",
        ".......wwwd.....",
        "......dvleed....",
        ".......wwd......",
        "......wwwed.....",
        ".....dvlmeeed...",
        "......wwwed.....",
        ".....wwwweed....",
        "....dvlmmeeed...",
        "...dvllmmeeeed..",
        "........bk......",
        "........bk......",
        ".......dbkd.....",
        "................",
        "................",
    ];
    split16(s, 29, 28, &snowpine_b, trees);
    cell(s, 29, 28).outline(0, 0, 16, 16, OUT);

    /* ----- other flora (2x2 blocks) ----- */

    let trees: &[(char, Ink)] = &[
        ('h', LEAF_HI),
        ('l', LEAF_LT),
        ('m', LEAF_MD),
        ('d', LEAF_DK),
        ('e', PINE_DK),
        ('b', BARK),
        ('k', BARK_DK),
        ('g', GOLDEN),
        ('r', RED_CL),
        ('o', OUT),
    ];

    // (15,26) BERRY BUSH, RIPE: low rounded bush studded with red berry pairs
    tc16(
        s,
        15,
        26,
        &[
            "................",
            "................",
            "................",
            "....dddddd......",
            "..ddlllllldd....",
            ".dllrrllllrrld..",
            ".dllrrllllrrld..",
            "dlllllrrlllllld.",
            "dllrllrrllrrlld.",
            ".dlrrllllllrld..",
            "..ddlllllldd....",
            "....ddkkdd......",
            "................",
            "................",
            "................",
            "................",
        ],
        trees,
    );
    cell(s, 15, 26).outline(0, 3, 16, 10, OUT);

    // (17,26) BERRY BUSH, PICKED: same silhouette, clearly bare (no red anywhere)
    tc16(
        s,
        17,
        26,
        &[
            "................",
            "................",
            "................",
            "....dddddd......",
            "..ddlllllldd....",
            ".dllmllldllmld..",
            ".dlmlldllmllld..",
            "dllldmllldmlld..",
            "dlmllldllllmld..",
            ".dllldllmllld...",
            "..ddlllllldd....",
            "....ddkkdd......",
            "................",
            "................",
            "................",
            "................",
        ],
        trees,
    );
    cell(s, 17, 26).outline(0, 3, 16, 10, OUT);

    // (19,26) REED TUFT: cattail stems with seed-head sausages
    tc16(
        s,
        19,
        26,
        &[
            "......g.....l...",
            "..l...b.....l...",
            "..l...b..g..l...",
            ".ll...b..b..ll..",
            ".l..l.k..b...l..",
            ".l..l.k..b...l..",
            "....l.k..k..l...",
            ".l..l....k..l...",
            ".l...l...k...l..",
            "..l..l..l...l...",
            "..l...l.l..l....",
            "...l..l.l..l....",
            "................",
            "................",
            "................",
            "................",
        ],
        trees,
    );

    // (21,26) SEAWEED PATCH: wavy kelp fronds (drawn over water)
    tc16(
        s,
        21,
        26,
        &[
            "................",
            "...m........m...",
            "...em...m..me...",
            "...e.m..m..e....",
            "..e..e..em.e....",
            "..e..e...e..e...",
            "..em.e...e..e...",
            "...e.em..em.e...",
            "...e..e...e.em..",
            "..e...e...e..e..",
            "..e..e...e...e..",
            "...e.e...e..e...",
            "...e..e..e..e...",
            "................",
            "................",
            "................",
        ],
        trees,
    );

    // (23,26) CORAL PATCH: branching pink fan on a stone base (drawn over water)
    tc16(
        s,
        23,
        26,
        &[
            "................",
            "....c...c.......",
            "...ac..ac..c....",
            "...ca..ca.ac....",
            "....c.ac..c.....",
            "....acc..ac.....",
            ".c...ca..ca.....",
            ".ac...cac.c..c..",
            "..ca..aca..cac..",
            "...caccac.ca....",
            "....accacca.....",
            "..ssaccaccss....",
            ".ssssssssssss...",
            "................",
            "................",
            "................",
        ],
        &[('c', CORAL), ('a', CORAL_DK), ('s', STONE_MD), ('o', OUT)],
    );

    // (25,26) FRUITING SAGUARO: staggered arms, magenta fruits on the crown/arms
    tc16(
        s,
        25,
        26,
        &[
            "......ff........",
            ".....mlds.......",
            ".....mlds.......",
            ".mm..mlds.......",
            "flds.mlds.......",
            "mldssmlds..sf...",
            ".mmsmmlds.msm...",
            ".....mldssmlm...",
            ".....mlds.mm....",
            ".....mlds.......",
            ".....mlds.......",
            ".....mlds.......",
            ".....mlds.......",
            "....dmlds.......",
            "................",
            "................",
        ],
        &[
            ('m', LEAF_MD),
            ('l', LEAF_LT),
            ('d', LEAF_DK),
            ('s', LEAF_DK),
            ('f', CORAL),
        ],
    );
    cell(s, 25, 26).outline(0, 0, 16, 16, OUT);

    // (27,26) BARREL CACTUS: squat ribbed barrel with a bloom
    tc16(
        s,
        27,
        26,
        &[
            "................",
            "................",
            "................",
            "................",
            "................",
            "......rgr.......",
            ".......r........",
            "....mmlmdm......",
            "...mllmldmm.....",
            "..mlldmldmdm....",
            "..mlldmldmdm....",
            "..mlldmldmdm....",
            "...mldmldmd.....",
            "....mmdddm......",
            "................",
            "................",
        ],
        trees,
    );
    cell(s, 27, 26).outline(0, 4, 16, 11, OUT);

    // (29,26) JACK-O-LANTERN TILE: the pumpkin, carved and lit from within
    tc16(
        s,
        29,
        26,
        &[
            "......ss........",
            "......ss........",
            "....oooooo......",
            "..oopppppdoo....",
            ".opppdppppddo...",
            ".oppdppppdpdo...",
            "opppdppppdppdo..",
            "opyydppppdyydo..",
            "opyyydpppdyyydo.",
            "oppdppyypdpppdo.",
            "opppdpyypdpppdo.",
            ".opddyppyydpdo..",
            ".oppdyyyyyddo...",
            "..ooppddddppo...",
            "....oooooooo....",
            "................",
        ],
        &[
            ('o', OUT),
            ('p', PUMPK),
            ('d', PUMPK_DK),
            ('y', FLAME_YL),
            ('s', LEAF_DK),
        ],
    );

    // (15,28) MUSHROOM TILE: a forage cluster — one tall tan cap with gills plus two
    // small buttons at its foot (earthy, reads "food find" rather than storybook toadstool)
    tc16(
        s,
        15,
        28,
        &[
            "................",
            "................",
            "....ooooo.......",
            "...otttttto.....",
            "..otlttttdto....",
            "..otttttttdo....",
            "..ogggggggdo....",
            "...oo.cc.oo.....",
            "...oc.cc.co.....",
            "...oc.cc.co.....",
            ".oo.occcco.oo...",
            "obbo.occo.obbo..",
            "oblbo.cc.oblbo..",
            "obbbo.cc.obbbo..",
            ".ooo.occco.ooo..",
            "......oo........",
        ],
        &[
            ('o', OUT),
            ('t', rgb(196, 148, 92)),
            ('l', rgb(228, 188, 132)),
            ('d', rgb(150, 108, 62)),
            ('g', rgb(232, 220, 196)),
            ('c', CREAM),
            ('b', rgb(214, 172, 116)),
        ],
    );

    // (17,28) DRY BUSH: tumbleweed — an airy twig skeleton ball
    tc16(
        s,
        17,
        28,
        &[
            "................",
            "................",
            "................",
            ".....kbbk.......",
            "...kb.k..bk.....",
            "..kb.b.k.k.b....",
            "..b.k.b.k.b.k...",
            ".kb.b.k.b.k.b...",
            ".b.k.b.k.b.k....",
            ".kb.k.b.k.b.k...",
            "..b.b.k.b.k.....",
            "..kb.k.b.k.b....",
            "...kb.k.b.bk....",
            ".....kbbkk......",
            "................",
            "................",
        ],
        &[('b', rgb(168, 138, 92)), ('k', rgb(120, 96, 62))],
    );
}

/* ==============================  furniture (rows 8-10)  ============================== */

/// The 2x2-cell furniture sprites. Grayscale where call sites recolor them (chest,
/// lantern, spawner); true color for the rest.
fn furniture_sprites(s: &mut Sheet) {
    // (0,8) anvil — true color
    tc16(
        s,
        0,
        8,
        &[
            "................",
            "................",
            "................",
            ".oooooooooooo...",
            "oliiiiiiiiiiio..",
            "oiiddddddddddo..",
            ".oo..oddddo.....",
            ".....odddo......",
            "....oddddo......",
            "...odddddddo....",
            "..oliiiiiiiido..",
            "..oidddddddddo..",
            "...oooooooooo...",
            "................",
            "................",
            "................",
        ],
        &[('o', OUT), ('i', IRON_LT), ('l', IRON_LT), ('d', IRON_DK)],
    );

    // (2,8) chest — grayscale (chest / death chest / dungeon chest palettes)
    spr16(
        s,
        2,
        8,
        &[
            "................",
            "................",
            "................",
            "..111111111111..",
            ".13333333333331.",
            ".13333333333331.",
            ".12222222222221.",
            ".11111111111111.",
            ".12222133122221.",
            ".12222133122221.",
            ".12222222222221.",
            ".12222222222221.",
            "..111111111111..",
            "................",
            "................",
            "................",
        ],
    );

    // (4,8) oven — true color: stone dome with a warm mouth
    tc16(
        s,
        4,
        8,
        &[
            "................",
            "................",
            "....oooooooo....",
            "..oolllllllloo..",
            ".ollllllllllllo.",
            ".olmmmmmmmmmmlo.",
            "olmmooooooommmo.",
            "olmoyyyyyyoommo.",
            "olmoyffffyyommo.",
            "olmooffffyoommo.",
            "odmmooooooommdo.",
            "oddmmmmmmmmmddo.",
            ".oddddddddddddo.",
            "..oooooooooooo..",
            "................",
            "................",
        ],
        &[
            ('o', OUT),
            ('l', STONE_LT),
            ('m', STONE_MD),
            ('d', STONE_DK),
            ('y', FLAME_OR),
            ('f', FLAME_YL),
        ],
    );

    // (6,8) furnace — true color: squat stone box, coal fire
    tc16(
        s,
        6,
        8,
        &[
            "................",
            "................",
            "..oooooooooooo..",
            ".ollllllllllllo.",
            ".olmmlmmlmmllmo.",
            ".olmmmmmmmmmmmo.",
            ".ommoooooooommo.",
            ".ommorryyrrommo.",
            ".ommoryyyyrommo.",
            ".ommorryyrrommo.",
            ".odmoooooooomdo.",
            ".oddmmmmmmmmddo.",
            ".odddddddddddo..",
            "..oooooooooooo..",
            "................",
            "................",
        ],
        &[
            ('o', OUT),
            ('l', STONE_LT),
            ('m', STONE_MD),
            ('d', STONE_DK),
            ('r', FLAME_RD),
            ('y', FLAME_YL),
        ],
    );

    // (8,8) workbench — true color: sturdy table, hammer on top
    tc16(
        s,
        8,
        8,
        &[
            "................",
            "................",
            "................",
            "....ii..........",
            "...oiioo........",
            "..ooiioko.......",
            ".ollllokolllllo.", // top edge with hammer resting
            "olwwwwwwwwwwwwlo",
            "owmmmmmmmmmmmmwo",
            ".oowmoooooomwoo.",
            "..owmo....omwo..",
            "..owmo....omwo..",
            "..owmo....omwo..",
            "..oooo....oooo..",
            "................",
            "................",
        ],
        &[
            ('o', OUT),
            ('l', WOOD_LT),
            ('w', WOOD_MD),
            ('m', WOOD_DK),
            ('i', IRON_LT),
            ('k', WOOD_DK),
        ],
    );

    // (10,8) lantern — grayscale: frame 1, metal 2, glowing glass 3
    spr16(
        s,
        10,
        8,
        &[
            "................",
            "................",
            "......111.......",
            ".....1...1......",
            ".....1...1......",
            "....1111111.....",
            "...112222211....",
            "...123333321....",
            "...123333321....",
            "...123333321....",
            "...112333211....",
            "....1111111.....",
            "....1222221.....",
            "................",
            "................",
            "................",
        ],
    );

    // (12,8) enchanter — true color: pedestal with an open tome and sparks
    tc16(
        s,
        12,
        8,
        &[
            "....a.....a.....",
            "..a.....a.......",
            "....oooooooo....",
            "..oopccccccpoo..",
            ".opcccpoopcccpo.",
            ".oppppo..oppppo.",
            "..oooomoomoooo..",
            ".....ommmmo.....",
            ".....ommmmo.....",
            ".....ommmmo.....",
            "....ommmmmmo....",
            "...odmmmmmmdo...",
            "...oddddddddo...",
            "....oooooooo....",
            "................",
            "................",
        ],
        &[
            ('o', OUT),
            ('c', CREAM),
            ('p', MAGIC),
            ('a', MAGIC_LT),
            ('m', STONE_MD),
            ('d', STONE_DK),
        ],
    );

    // (14,8) tnt — true color: red crate, pale band, lit fuse
    tc16(
        s,
        14,
        8,
        &[
            "................",
            "......ky........",
            "......k.........",
            "..oooookoooooo..",
            ".orrrrrrrrrrrro.",
            ".orrrrrrrrrrrro.",
            ".ordrrrrrrdrrro.",
            ".occcccccccccco.",
            ".occocc.occocco.",
            ".occcccccccccco.",
            ".orrrrrrrrrrrro.",
            ".ordrrrrdrrrdro.",
            ".odddddddddddo..",
            "..oooooooooooo..",
            "................",
            "................",
        ],
        &[
            ('o', OUT),
            ('r', FLAME_RD),
            ('d', rgb(140, 38, 30)),
            ('c', CREAM),
            ('k', BARK_DK),
            ('y', FLAME_YL),
        ],
    );

    // (16,8) bed — true color: cream pillow, red blanket, wooden rail
    tc16(
        s,
        16,
        8,
        &[
            "................",
            "................",
            "................",
            "................",
            "................",
            "..oooooooooooo..",
            ".occccorrrrrrro.",
            ".occccorrrrrrro.",
            ".odccdorrrrrrdo.",
            ".orrrrrrrrrrrdo.",
            ".owwwwwwwwwwwwo.",
            "..owo......owo..",
            "..ooo......ooo..",
            "................",
            "................",
            "................",
        ],
        &[
            ('o', OUT),
            ('c', CREAM),
            ('r', RED_CL),
            ('d', rgb(150, 42, 44)),
            ('w', WOOD_MD),
        ],
    );

    // (18,8) loom — true color: frame, warp threads, half-woven cloth
    tc16(
        s,
        18,
        8,
        &[
            "................",
            "................",
            "..oooooooooooo..",
            ".owwwwwwwwwwwwo.",
            ".owoc.c.c.c.owo.",
            ".owoc.c.c.c.owo.",
            ".owoc.c.c.c.owo.",
            ".owoc.c.c.c.owo.",
            ".oworrrrrrrrowo.",
            ".oworrrrrrrrowo.",
            ".owwwwwwwwwwwwo.",
            "..owo......owo..",
            "..ooo......ooo..",
            "................",
            "................",
            "................",
        ],
        &[('o', OUT), ('w', WOOD_MD), ('c', CREAM), ('r', RED_CL)],
    );

    // (20,8) spawner — grayscale (tinted by the caged mob's palette):
    // bars 1, the little captive 2 with shade3 eyes
    spr16(
        s,
        20,
        8,
        &[
            "................",
            "................",
            ".11111111111111.",
            ".1..1..1..1..1..",
            ".1..1..1..1..1..",
            ".1..1222221..1..",
            ".1..1232321..1..",
            ".1..1222221..1..",
            ".1..1222221..1..",
            ".1...22222...1..",
            ".1..1..1..1..1..",
            ".1..1..1..1..1..",
            ".11111111111111.",
            "................",
            "................",
            "................",
        ],
    );
}

/// Row 10: 8x8 furniture item icons (grayscale — the spawner icon inherits the caged
/// mob's palette, chests their chest palette, etc).
#[rustfmt::skip]
fn furniture_icons(s: &mut Sheet) {
    // (0,10) anvil
    item_icon(s, 0, 10, &[
        "........",
        ".333333.",
        ".222222.",
        "...22...",
        "...22...",
        "..2222..",
        ".222222.",
        "........",
    ]);
    // (1,10) chest
    item_icon(s, 1, 10, &[
        "........",
        ".111111.",
        "13333331",
        "12222221",
        "12233221",
        "12222221",
        ".111111.",
        "........",
    ]);
    // (2,10) oven
    item_icon(s, 2, 10, &[
        "........",
        "..1111..",
        ".133331.",
        "13311331",
        "13111131",
        "13311331",
        ".111111.",
        "........",
    ]);
    // (3,10) furnace
    item_icon(s, 3, 10, &[
        "........",
        ".111111.",
        "12222221",
        "12111121",
        "12133121",
        "12111121",
        ".111111.",
        "........",
    ]);
    // (4,10) workbench
    item_icon(s, 4, 10, &[
        "........",
        "........",
        ".111111.",
        "13333331",
        ".21..12.",
        ".21..12.",
        ".11..11.",
        "........",
    ]);
    // (5,10) lantern
    item_icon(s, 5, 10, &[
        "...11...",
        "..1111..",
        ".123321.",
        ".123321.",
        ".112211.",
        "..1111..",
        "........",
        "........",
    ]);
    // (6,10) enchanter (open tome + sparkle)
    item_icon(s, 6, 10, &[
        "..3..3..",
        "........",
        ".111111.",
        "13313331",
        "13311331",
        ".111111.",
        "........",
        "........",
    ]);
    // (7,10) tnt
    item_icon(s, 7, 10, &[
        "....1...",
        ".111111.",
        "12222221",
        "13333331",
        "12222221",
        ".111111.",
        "........",
        "........",
    ]);
    // (8,10) bed
    item_icon(s, 8, 10, &[
        "........",
        "........",
        ".111111.",
        "13322221",
        "13222221",
        ".111111.",
        ".1....1.",
        "........",
    ]);
    // (9,10) loom
    item_icon(s, 9, 10, &[
        "........",
        ".111111.",
        "1.2.2.21",
        "1.2.2.21",
        "13333331",
        ".111111.",
        "........",
        "........",
    ]);
    // (10,10) spawner
    item_icon(s, 10, 10, &[
        "........",
        ".111111.",
        "1.1..1.1",
        "1.1221.1",
        "1.1231.1",
        "1.1..1.1",
        ".111111.",
        "........",
    ]);
}

/* ==============================  mobs (rows 14-23)  ============================== */
/* Every mob is grayscale: shade0 background (transparent via the mob palettes),
 * 1 = outline/dark, 2 = mid (the *dynamic* color: player shirt, mob level tint),
 * 3 = light (skin/highlight). A mob is 4 frames of 2x2 cells starting at its base
 * cell: [down, up, right-step-a, right-step-b]; left is mirrored at draw time, and
 * the down/up walk animation mirrors the (slightly asymmetric) frame. */

/// Draw one 16x16 mob frame at cell (cx, cy).
fn frame16(s: &mut Sheet, cx: i32, cy: i32, rows: &[&str]) {
    spr16(s, cx, cy, rows);
}

// IMPORTANT animation contract (see `compile_mob_sprite_animations`): the second walk
// frame for down/up is this art *horizontally mirrored*, and left is the mirrored right
// frames. Down/up frames MUST therefore be left-right asymmetric (arm swing + one boot
// planted, one extended) or walking shows no animation at all.
//
// Proportions (matching the original Java sprite's discipline): 7-row head, identical
// size in all four directions, sitting directly on the shoulder line — no neck row;
// body as wide as the head; legs 2px wide with a clear stance change between frames.
//
/// Player sets — TRANSCRIBED pixel-for-pixel from the original Java sheet
/// (`icons.png`, removed from the repo after tracing — see git history): walk frames
/// from its cells (0,14), carry from (0,16), suit
/// from (18,20), suit-carry from (18,22) — the same cell coordinates this sheet uses.
/// Shades quantized 0/85/170/255 -> `.`/1/2/3; the call-site palette
/// (`get4(-1, 100, shirt, 532)`) recolors: 1 = hair/outline, 2 = shirt, 3 = skin.
/// This is a trace of the user-owned original, NOT a redesign — do not "improve" the
/// anatomy here; only the palette at call sites may change its look.
fn player_sets(s: &mut Sheet) {
    let walk_down = [
        "................",
        "......1111......",
        ".....111111.....",
        "....11111111....",
        "...113111131....",
        "...113311331....",
        "..12133113311...",
        "..121111111221..",
        "...11111112221..",
        "....1222213321..",
        "....122121331...",
        ".....1221211....",
        ".....122111.....",
        ".....1221.......",
        ".....1111.......",
        "................",
    ];
    let walk_up = [
        "................",
        "......1111......",
        ".....111111.....",
        "....11111111....",
        "....11111111....",
        "...111111111....",
        "..1211111111....",
        "..1211111111....",
        "..12111111221...",
        "...11222222231..",
        "....1222121331..",
        "....112212111...",
        ".....122111.....",
        ".....1221.......",
        ".....1111.......",
        "................",
    ];
    let walk_r1 = [
        "................",
        "......1111......",
        "....1111111.....",
        "...111111311....",
        "..1111111331....",
        "..1111111331....",
        "..1.11111111....",
        ".....111111.....",
        "......1111......",
        "......1221......",
        "......1221......",
        "......1331......",
        "......1111......",
        "......1221......",
        ".......11.......",
        "................",
    ];
    let walk_r2 = [
        "................",
        ".....11111......",
        "...11111111.....",
        "..1111111311....",
        "..1111111331....",
        ".11111111331....",
        "....11111111....",
        "....11111111....",
        "....111111131...",
        "...1322221231...",
        "...132122111....",
        "....1122221.....",
        "....1221221.....",
        "....11111221....",
        ".........111....",
        "................",
    ];
    let carry_down = [
        "...11......11...",
        "..133111111331..",
        "..133111111331..",
        "..121111111121..",
        "..121311113121..",
        "..121331133121..",
        "...1133113311...",
        "....11111111....",
        "....11111121....",
        "....12222221....",
        "....1221221.....",
        ".....122121.....",
        ".....122111.....",
        ".....1221.......",
        ".....1111.......",
        "................",
    ];
    let carry_up = [
        "...11......11...",
        "..133111111331..",
        "..133111111331..",
        "..121111111121..",
        "..121111111121..",
        "..121111111121..",
        "...1111111111...",
        "....11111111....",
        "....11111121....",
        "....12222221....",
        "....1222121.....",
        "....1122121.....",
        ".....122111.....",
        ".....1221.......",
        ".....1111.......",
        "................",
    ];
    let carry_r1 = [
        "................",
        "......1111......",
        "....1113311.....",
        "...111133111....",
        "..1111122131....",
        "..1111122131....",
        "..1.11122111....",
        ".....112211.....",
        "......1111......",
        "......1221......",
        "......1221......",
        "......1221......",
        "......1221......",
        "......1221......",
        ".......11.......",
        "................",
    ];
    let carry_r2 = [
        "................",
        ".....11111......",
        "...11113311.....",
        "..1111133111....",
        "..1111122131....",
        ".11111122131....",
        "....11122111....",
        ".....112211.....",
        "......1111......",
        "......1221......",
        "......12211.....",
        "....1122221.....",
        "....1221221.....",
        "....11111221....",
        ".........111....",
        "................",
    ];
    let suit_down = [
        "................",
        "......1111......",
        ".....111111.....",
        "....11111111....",
        "...113111131....",
        "...113311331....",
        "..12133113311...",
        "..121111111221..",
        "...11111112221..",
        "....1222213321..",
        "....122121331...",
        ".....1221211....",
        ".....122111.....",
        ".....1221.......",
        ".....1111.......",
        "................",
    ];
    let suit_up = [
        "................",
        "......1111......",
        ".....111111.....",
        "....11111111....",
        "....11111111....",
        "...111111111....",
        "..1211111111....",
        "..1211111111....",
        "..12111111221...",
        "...11222222231..",
        "....1222121331..",
        "....112212111...",
        ".....122111.....",
        ".....1221.......",
        ".....1111.......",
        "................",
    ];
    let suit_r1 = [
        "................",
        "......1111......",
        "....1111111.....",
        "...111111311....",
        "..1111111331....",
        "..1111111331....",
        "..1.11111111....",
        ".....111111.....",
        "......1111......",
        "......1221......",
        "......1221......",
        "......1331......",
        "......1111......",
        "......1221......",
        ".......11.......",
        "................",
    ];
    let suit_r2 = [
        "................",
        ".....11111......",
        "...11111111.....",
        "..1111111311....",
        "..1111111331....",
        ".11111111331....",
        "....11111111....",
        "....11111111....",
        "....111111131...",
        "...1322221231...",
        "...132122111....",
        "....1122221.....",
        "....1221221.....",
        "....11111221....",
        ".........111....",
        "................",
    ];
    let suit_carry_down = [
        "................",
        "...11.1111.11...",
        "..133111111331..",
        "..133111111331..",
        "..121311113121..",
        "..121331133121..",
        "..121331133121..",
        "..11111111111...",
        "....11111121....",
        "....12222221....",
        "....12212221....",
        ".....122121.....",
        ".....122111.....",
        ".....1221.......",
        ".....1111.......",
        "................",
    ];
    let suit_carry_up = [
        "................",
        "...11.1111.11...",
        "..133111111331..",
        "..133111111331..",
        "..121111111121..",
        "..121111111121..",
        "..121111111121..",
        "...1111111111...",
        "....11111121....",
        "....12222221....",
        "....1222121.....",
        "....1122121.....",
        ".....122111.....",
        ".....1221.......",
        ".....1111.......",
        "................",
    ];
    let suit_carry_r1 = [
        "......1111......",
        "......1331......",
        "....1113311.....",
        "...111122111....",
        "..1111122131....",
        "..1111122131....",
        "..1.11122111....",
        ".....111111.....",
        "......1111......",
        "......1221......",
        "......1221......",
        "......1221......",
        "......1221......",
        "......1221......",
        ".......11.......",
        "................",
    ];
    let suit_carry_r2 = [
        "......1111......",
        ".....11331......",
        "...11113311.....",
        "..1111122111....",
        "..1111122131....",
        ".11111122131....",
        "....11122111....",
        "....11122111....",
        "....1111111.....",
        "......1221......",
        "......12211.....",
        "....1122221.....",
        "....1221221.....",
        "....11111221....",
        ".........111....",
        "................",
    ];
    frame16(s, 0, 14, &walk_down);
    frame16(s, 2, 14, &walk_up);
    frame16(s, 4, 14, &walk_r1);
    frame16(s, 6, 14, &walk_r2);
    frame16(s, 0, 16, &carry_down);
    frame16(s, 2, 16, &carry_up);
    frame16(s, 4, 16, &carry_r1);
    frame16(s, 6, 16, &carry_r2);
    frame16(s, 18, 20, &suit_down);
    frame16(s, 20, 20, &suit_up);
    frame16(s, 22, 20, &suit_r1);
    frame16(s, 24, 20, &suit_r2);
    frame16(s, 18, 22, &suit_carry_down);
    frame16(s, 20, 22, &suit_carry_up);
    frame16(s, 22, 22, &suit_carry_r1);
    frame16(s, 24, 22, &suit_carry_r2);
}

/// Marsh Lurker (8,14): a low amphibian ambusher — periscope eye stalks over a wide
/// flat body, splayed webbed limbs. Outline/speckles shade1, hide shade2 (level tint),
/// eyes/brow shade3.
fn marsh_lurker(s: &mut Sheet) {
    let down = [
        "................",
        "...11.....11....",
        "..1331...1331...",
        "..1331...1331...",
        "...111111111....",
        "..13333333331...",
        ".1322222222231..",
        ".1222222222221..",
        ".1221221221221..",
        ".1222222222221..",
        "11222222222211..",
        "1211222222112...",
        ".11.12222211....",
        "...112....211...",
        "...11......11...",
        "................",
    ];
    let up = [
        "................",
        "...11.....11....",
        "..1221...1221...",
        "..1221...1221...",
        "...111111111....",
        "..12222222221...",
        ".1223322332221..",
        ".1222222222221..",
        ".1222332233221..",
        ".1222222222221..",
        "11222222222211..",
        "1211222222112...",
        ".11.12222211....",
        "...112....211...",
        "...11......11...",
        "................",
    ];
    let right = [
        "...........11...",
        "..........1331..",
        "..........1331..",
        "........11111111",
        ".......122222231",
        "..111..122222111",
        ".13221112222211.",
        "..1122222222221.",
        "...122222222211.",
        "...1122222221...",
        "....11221122....",
        "....12....12....",
        "...112....112...",
        "...11......11...",
        "................",
        "................",
    ];
    let right2 = [
        "...........11...",
        "..........1331..",
        "..........1331..",
        "........11111111",
        ".......122222231",
        "..111..122222111",
        ".13221112222211.",
        "..1122222222221.",
        "...122222222211.",
        "...1122222221...",
        "....11221122....",
        "...12......12...",
        "..112......112..",
        "..11........11..",
        "................",
        "................",
    ];
    frame16(s, 8, 14, &down);
    frame16(s, 10, 14, &up);
    frame16(s, 12, 14, &right);
    frame16(s, 14, 14, &right2);
}

/// Pig frames (16..23,14..15) — TRANSCRIBED pixel-for-pixel from the
/// original Java sheet (`icons.png`, removed after tracing — see git history; same
/// cell coordinates), like the player
/// sets. Recognizability beats originality: do not redraw, only palettes at call
/// sites may restyle. Frames: [down, up, right-a, right-b]; left + second down/up
/// frames are mirrored at draw time.
fn pig(s: &mut Sheet) {
    // down
    frame16(
        s,
        16,
        14,
        &[
            "................",
            "................",
            "................",
            "......111.......",
            ".....13331......",
            "....1123211.....",
            "...131333131....",
            "...133111331....",
            "...133333331....",
            "...133333331....",
            "...113333311....",
            "...131111131....",
            "...131...111....",
            "...111..........",
            "................",
            "................",
        ],
    );

    // up
    frame16(
        s,
        18,
        14,
        &[
            "................",
            "................",
            "................",
            "......111.......",
            ".....13331......",
            "....1111111.....",
            "...133333331....",
            "...133313331....",
            "...133113331....",
            "...133333331....",
            "...113333311....",
            "...131111131....",
            "...131...111....",
            "...111..........",
            "................",
            "................",
        ],
    );

    // right_a
    frame16(
        s,
        20,
        14,
        &[
            "................",
            "................",
            "................",
            "................",
            "....11111111....",
            "...133333333111.",
            "...1333333331331",
            "..11333333331321",
            ".131333333331331",
            ".111333333331111",
            "...1333333331...",
            "....13111131....",
            "....111..131....",
            ".........111....",
            "................",
            "................",
        ],
    );

    // right_b
    frame16(
        s,
        22,
        14,
        &[
            "................",
            "................",
            "................",
            "................",
            "....11111111....",
            "...133333333111.",
            "...1333333331331",
            "..11333333331321",
            ".131333333331331",
            ".111333333331111",
            "...1333333331...",
            "....13111131....",
            "....131..111....",
            "....111.........",
            "................",
            "................",
        ],
    );
}

/// Knight frames (24..31,14..15) — TRANSCRIBED pixel-for-pixel from the
/// original Java sheet (`icons.png`, removed after tracing — see git history; same
/// cell coordinates), like the player
/// sets. Recognizability beats originality: do not redraw, only palettes at call
/// sites may restyle. Frames: [down, up, right-a, right-b]; left + second down/up
/// frames are mirrored at draw time.
fn knight(s: &mut Sheet) {
    // down
    frame16(
        s,
        24,
        14,
        &[
            "................",
            "......1111......",
            ".....111111.....",
            "....11222211....",
            "...112222221....",
            "...1123333211...",
            "..12122332211...",
            "..121122221221..",
            "...11111113331..",
            "....1222233333..",
            "....1221233333..",
            ".....12212333...",
            ".....1221113....",
            ".....1221.......",
            ".....1111.......",
            "................",
        ],
    );

    // up
    frame16(
        s,
        26,
        14,
        &[
            "................",
            "......1111......",
            ".....122221.....",
            "....12222221....",
            "....12222221....",
            "...112222221....",
            "..1212222221....",
            "..1211222211....",
            "..12111111223...",
            "...11222222233..",
            "....1222121333..",
            "....112212133...",
            ".....1221113....",
            ".....1221.......",
            ".....1111.......",
            "................",
        ],
    );

    // right_a
    frame16(
        s,
        28,
        14,
        &[
            "................",
            "......1111......",
            ".....112221.....",
            "....11222221....",
            "....12222331....",
            "....12222231....",
            "....12222221....",
            ".....112211.....",
            "......1111......",
            "......3333......",
            ".....333333.....",
            ".....333333.....",
            "......3333......",
            "......1331......",
            ".......11.......",
            "................",
        ],
    );

    // right_b
    frame16(
        s,
        30,
        14,
        &[
            "................",
            "......1111......",
            ".....112221.....",
            "....11222221....",
            "....12222331....",
            "....12222231....",
            "....11222221....",
            "....11111111....",
            "...3332221231...",
            "..33333221231...",
            "..3333322111....",
            "...33322221.....",
            "...33221221.....",
            "....11111221....",
            ".........111....",
            "................",
        ],
    );
}

/// Feral Hound (8,16): a lean pack hunter — pricked ears, long snout, thin legs,
/// low-slung tail. Outline shade1, coat shade2 (level tint), muzzle/chest shade3.
fn feral_hound(s: &mut Sheet) {
    let down = [
        "...11.....11....",
        "...131...131....",
        "....11...11.....",
        "....1111111.....",
        "...122222221....",
        "...121222121....",
        "...122232221....",
        "....1233321.....",
        "....1231321.....",
        ".....12221......",
        "....1222221.....",
        "...122222221....",
        "...122222221....",
        "....1111111.....",
        "...12.....12....",
        "..11......11....",
    ];
    let up = [
        "...11.....11....",
        "...111...111....",
        "....11...11.....",
        "....1111111.....",
        "...122222221....",
        "...122222221....",
        "...122222221....",
        "....1222221.....",
        "....1222221.....",
        ".....12221......",
        "....1222221.....",
        "...122222221....",
        "...122212221....",
        "....1111111.....",
        "...12.....12....",
        "..11......11....",
    ];
    let right = [
        "................",
        "..........11....",
        "..........131...",
        ".........11111..",
        ".........1212331",
        "..11......122211",
        ".12211122222211.",
        "..112222222221..",
        "...12222222221..",
        "...1222233321...",
        "....11221221....",
        "....12....12....",
        "....12....12....",
        "....11....11....",
        "................",
        "................",
    ];
    let right2 = [
        "................",
        "..........11....",
        "..........131...",
        ".........11111..",
        ".........1212331",
        "..11......122211",
        ".12211122222211.",
        "..112222222221..",
        "...12222222221..",
        "...1222233321...",
        "....11221221....",
        "...12......12...",
        "..12........12..",
        "..11........11..",
        "................",
        "................",
    ];
    frame16(s, 8, 16, &down);
    frame16(s, 10, 16, &up);
    frame16(s, 12, 16, &right);
    frame16(s, 14, 16, &right2);
}

/// Cow frames (16..23,16..17) — TRANSCRIBED pixel-for-pixel from the
/// original Java sheet (`icons.png`, removed after tracing — see git history; same
/// cell coordinates), like the player
/// sets. Recognizability beats originality: do not redraw, only palettes at call
/// sites may restyle. Frames: [down, up, right-a, right-b]; left + second down/up
/// frames are mirrored at draw time.
fn cow(s: &mut Sheet) {
    // down
    frame16(
        s,
        16,
        16,
        &[
            "................",
            "................",
            "....1111111.....",
            "...133333221....",
            "..13332233331...",
            "..13133223131...",
            "..13131113131...",
            "..13113331131...",
            "..13312321331...",
            "..13313331321...",
            "..12313331321...",
            "..12231113231...",
            "..11333333311...",
            "..13111111131...",
            "..131.....111...",
            "..111...........",
        ],
    );

    // up
    frame16(
        s,
        18,
        16,
        &[
            "................",
            "................",
            "....1111111.....",
            "...133333331....",
            "..13323322331...",
            "..13223322231...",
            "..13333333231...",
            "..13233113231...",
            "..13231313331...",
            "..13313313331...",
            "..13331133231...",
            "..13223332331...",
            "..11333333311...",
            "..13111111131...",
            "..131.....111...",
            "..111...........",
        ],
    );

    // right_a
    frame16(
        s,
        20,
        16,
        &[
            "................",
            "................",
            "...11111111.....",
            "..1333333321....",
            "..1323333331.1..",
            "..133323223111..",
            "..1332233331331.",
            ".11322232331321.",
            "131332332231331.",
            "131333322231331.",
            "11132233333111..",
            "..1333332231....",
            "...131111131....",
            "...111...131....",
            ".........111....",
            "................",
        ],
    );

    // right_b
    frame16(
        s,
        22,
        16,
        &[
            "................",
            "................",
            "...11111111.....",
            "..1333333321....",
            "..1323333331.1..",
            "..133323223111..",
            "..1332233331331.",
            ".11322232331321.",
            "131332332231331.",
            "131333322231331.",
            "11132233333111..",
            "..1333332231....",
            "...131111131....",
            "...131...111....",
            "...111..........",
            "................",
        ],
    );
}

/// Stone Golem (0,18): a hulking mine-dweller — massive square shoulders, a small
/// head sunk between them, boulder fists to the ground. Seams/cracks shade1, rock
/// shade2 (level tint), ore glints/eyes shade3.
///
/// Freed cells: the old Slime (0,18 4x2) and Creeper (4,18 6x2) blocks were retired
/// with those mobs; the golem's four frames span (0..7,18..19). Cells (8,18), (9,18),
/// and (8,19) are FREE; (9,19) still holds the spawner fire particle below.
fn stone_golem(s: &mut Sheet) {
    let down = [
        ".....111111.....",
        "....12322321....",
        "....12222221....",
        "..111211221112..",
        ".11222222222211.",
        "1221222222221221",
        "1221222112221221",
        "1221221221221221",
        "1221222222221221",
        "1122122222122211",
        ".112212222122211",
        ".111.122221.111.",
        ".....122221.....",
        "....1221.1221...",
        "....1221.1221...",
        "....111...111...",
    ];
    let up = [
        ".....111111.....",
        "....12222221....",
        "....12222221....",
        "..111222222111..",
        ".11222222222211.",
        "1221221221221221",
        "1221222222221221",
        "1221223322221221",
        "1221222222321221",
        "1122122222122211",
        ".112212222122211",
        ".111.122221.111.",
        ".....122221.....",
        "....1221.1221...",
        "....1221.1221...",
        "....111...111...",
    ];
    let right = [
        "......111111....",
        ".....1212231....",
        ".....1222221....",
        "...111222222111.",
        "..12222222222211",
        "..12222222222121",
        "..12212212222121",
        "..12222222222121",
        "..12223222222121",
        "..11222222221211",
        "...122222221121.",
        "...1112222111...",
        ".....122221.....",
        "....12211221....",
        "....1221.1221...",
        "....111...111...",
    ];
    let right2 = [
        "......111111....",
        ".....1212231....",
        ".....1222221....",
        "...111222222111.",
        "..12222222222211",
        "..12222222222121",
        "..12212212222121",
        "..12222222222121",
        "..12223222222121",
        "..11222222221211",
        "...122222221121.",
        "...1112222111...",
        ".....122221.....",
        "....122112211...",
        "...1221...1221..",
        "...111.....111..",
    ];
    frame16(s, 0, 18, &down);
    frame16(s, 2, 18, &up);
    frame16(s, 4, 18, &right);
    frame16(s, 6, 18, &right2);
}

/// The spawner fire particle at cell (9,19): a pure layered blob (outer 1, mid 2,
/// core 3) drawn in the fire palette at runtime. It used to double as the removed
/// Creeper's push-off foot; the blob stays because `particle::new_fire_particle`
/// references this cell.
fn fire_particle(s: &mut Sheet) {
    let mut c = cell(s, 8, 18);
    c.disc(12, 12, 3, G1);
    c.disc(12, 12, 2, G2);
    c.set(12, 12, G3);
    c.set(11, 12, G3);
    c.set(12, 13, G3);
    c.set(13, 10, G1); // flicker tip
}

/// Night Wisp (0,20): two 16x16 pulse frames of a floating light. Its palette makes
/// shades 0 AND 1 transparent (like the glow worm) — art lives in shades 2-3 only:
/// halo/trails shade2, core shade3.
fn night_wisp(s: &mut Sheet) {
    let calm = [
        "................",
        "................",
        "......2222......",
        ".....222222.....",
        "....22233222....",
        "....22333322....",
        "....22333322....",
        "....22233222....",
        ".....222222.....",
        "......2222......",
        ".....22..22.....",
        "....22....22....",
        "....2......2....",
        ".....2....2.....",
        "................",
        "................",
    ];
    let flare = [
        "................",
        ".....2....2.....",
        "......2222......",
        "....22222222....",
        "...2223333222...",
        "...2233333322...",
        "...2233333322...",
        "...2223333222...",
        "....22222222....",
        "......2222......",
        "....22.22.22....",
        "...22..22..22...",
        "...2...2....2...",
        "....2.......2...",
        "................",
        "................",
    ];
    frame16(s, 0, 20, &calm);
    frame16(s, 2, 20, &flare);
}

/// Sheep frames (10..17,18..19) — TRANSCRIBED pixel-for-pixel from the
/// original Java sheet (`icons.png`, removed after tracing — see git history; same
/// cell coordinates), like the player
/// sets. Recognizability beats originality: do not redraw, only palettes at call
/// sites may restyle. Frames: [down, up, right-a, right-b]; left + second down/up
/// frames are mirrored at draw time.
fn sheep(s: &mut Sheet) {
    // down
    frame16(
        s,
        10,
        18,
        &[
            "................",
            "................",
            "......111.......",
            ".....13331......",
            "...111232111....",
            "..12213331221...",
            "..12213331221...",
            "..12221112221...",
            "..12222222221...",
            "..12222222221...",
            "..12222222221...",
            "..12222222221...",
            "..11222222211...",
            "..13111111131...",
            "..131.....111...",
            "..111...........",
        ],
    );

    // up
    frame16(
        s,
        12,
        18,
        &[
            "................",
            "................",
            "......111.......",
            ".....13331......",
            "...111111111....",
            "..12222222221...",
            "..12222222221...",
            "..12222222221...",
            "..12222222221...",
            "..12222222221...",
            "..12222222221...",
            "..12222222221...",
            "..11222222211...",
            "..13111111131...",
            "..131.....111...",
            "..111...........",
        ],
    );

    // right_a
    frame16(
        s,
        14,
        18,
        &[
            "................",
            "................",
            "................",
            "...11111111.....",
            "..1222222221....",
            "..122222222111..",
            "..1222222221331.",
            "..1222222221321.",
            "..1222222221331.",
            "..1222222221331.",
            "..122222222111..",
            "..1212222211....",
            "...131111131....",
            "...111...131....",
            ".........111....",
            "................",
        ],
    );

    // right_b
    frame16(
        s,
        16,
        18,
        &[
            "................",
            "................",
            "................",
            "...11111111.....",
            "..1222222221....",
            "..122222222111..",
            "..1222222221331.",
            "..1222222221321.",
            "..1222222221331.",
            "..1222222221331.",
            "..122222222111..",
            "..1212222211....",
            "...131111131....",
            "...131...111....",
            "...111..........",
            "................",
        ],
    );
}

/// Snake frames (18..25,18..19) — TRANSCRIBED pixel-for-pixel from the
/// original Java sheet (`icons.png`, removed after tracing — see git history; same
/// cell coordinates), like the player
/// sets. Recognizability beats originality: do not redraw, only palettes at call
/// sites may restyle. Frames: [down, up, right-a, right-b]; left + second down/up
/// frames are mirrored at draw time.
fn snake(s: &mut Sheet) {
    // down
    frame16(
        s,
        18,
        18,
        &[
            "................",
            "...111111111....",
            "..13333333331...",
            "...11111111331..",
            "...........131..",
            "..........1331..",
            "...1111111331...",
            "..1333333331....",
            ".1331111111.....",
            ".131............",
            ".1331.....111...",
            "..133111113331..",
            "...133333333231.",
            "....1111111331..",
            "...........11...",
            "................",
        ],
    );

    // up
    frame16(
        s,
        20,
        18,
        &[
            "................",
            "................",
            "...........11...",
            "...11111111331..",
            "..1333333333231.",
            ".1333111113331..",
            ".1331.....111...",
            ".1331111111.....",
            "..1333333331....",
            "...1111111331...",
            "..........1331..",
            "...11111113331..",
            "..13333333331...",
            "...111111111....",
            "................",
            "................",
        ],
    );

    // right_a
    frame16(
        s,
        22,
        18,
        &[
            "................",
            "...........11...",
            "....1111111331..",
            "...133333333231.",
            "..133111113331..",
            ".1331.....111...",
            ".131............",
            ".1331111111.....",
            "..1333333331....",
            "...1111111331...",
            "..........1331..",
            "...........131..",
            "...11111111331..",
            "..13333333331...",
            "...111111111....",
            "................",
        ],
    );

    // right_b
    frame16(
        s,
        24,
        18,
        &[
            ".........111....",
            "................",
            "...........11...",
            "...11111111331..",
            "..1333333333231.",
            ".1333111113331..",
            ".1331.....111...",
            ".1331111111.....",
            "..1333333331....",
            "...1111111331...",
            "..........1331..",
            "...11111113331..",
            "..13333333331...",
            "...111111111....",
            "................",
            "................",
        ],
    );
}

/// Glow worm (26,19): shades 0 AND 1 are transparent in its palette — art in 2-3 only.
fn glow_worm(s: &mut Sheet) {
    icon8(
        s,
        26,
        19,
        &[
            "........", "..22....", ".2332...", ".23332..", "..2332..", "...22...", "........",
            "........",
        ],
    );
}

/* ==============================  font (rows 30-31)  ============================== */

/// The renderable half of `Font::CHARS` (all text is uppercased before drawing, so the
/// lowercase tail of CHARS maps to cells past this 256x256 sheet and is never used).
/// Glyph cell = `30*32 + index`.
const FONT_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ      0123456789.,!?'\"-+=/\\%()<>:;^@";

/// An original bold pixel font: 2px strokes, 6px-wide glyphs (7px for M/W) in the
/// 8x8 cell, 7 rows tall. '#' = stroke (shade3); background stays shade0.
#[rustfmt::skip]
fn glyph(ch: char) -> Option<[&'static str; 7]> {
    Some(match ch {
        'A' => [".####.", "##..##", "##..##", "######", "##..##", "##..##", "##..##"],
        'B' => ["#####.", "##..##", "##..##", "#####.", "##..##", "##..##", "#####."],
        'C' => [".####.", "##..##", "##....", "##....", "##....", "##..##", ".####."],
        'D' => ["#####.", "##..##", "##..##", "##..##", "##..##", "##..##", "#####."],
        'E' => ["######", "##....", "##....", "#####.", "##....", "##....", "######"],
        'F' => ["######", "##....", "##....", "#####.", "##....", "##....", "##...."],
        'G' => [".####.", "##..##", "##....", "##.###", "##..##", "##..##", ".#####"],
        'H' => ["##..##", "##..##", "##..##", "######", "##..##", "##..##", "##..##"],
        'I' => [".####.", "..##..", "..##..", "..##..", "..##..", "..##..", ".####."],
        'J' => ["..####", "...##.", "...##.", "...##.", "...##.", "##.##.", ".###.."],
        'K' => ["##..##", "##.##.", "####..", "###...", "####..", "##.##.", "##..##"],
        'L' => ["##....", "##....", "##....", "##....", "##....", "##....", "######"],
        'M' => ["##...##", "###.###", "#######", "##.#.##", "##...##", "##...##", "##...##"],
        'N' => ["##..##", "###.##", "###.##", "######", "##.###", "##.###", "##..##"],
        'O' => [".####.", "##..##", "##..##", "##..##", "##..##", "##..##", ".####."],
        'P' => ["#####.", "##..##", "##..##", "#####.", "##....", "##....", "##...."],
        'Q' => [".####.", "##..##", "##..##", "##..##", "##.###", ".####.", "....##"],
        'R' => ["#####.", "##..##", "##..##", "#####.", "####..", "##.##.", "##..##"],
        'S' => [".#####", "##....", "##....", ".####.", "....##", "....##", "#####."],
        'T' => ["######", "..##..", "..##..", "..##..", "..##..", "..##..", "..##.."],
        'U' => ["##..##", "##..##", "##..##", "##..##", "##..##", "##..##", ".####."],
        'V' => ["##..##", "##..##", "##..##", ".####.", ".####.", "..##..", "..##.."],
        'W' => ["##...##", "##...##", "##...##", "##.#.##", "#######", "###.###", "##...##"],
        'X' => ["##..##", "##..##", ".####.", "..##..", ".####.", "##..##", "##..##"],
        'Y' => ["##..##", "##..##", ".####.", "..##..", "..##..", "..##..", "..##.."],
        'Z' => ["######", "....##", "...##.", "..##..", ".##...", "##....", "######"],
        '0' => [".####.", "##..##", "##.###", "###.##", "##..##", "##..##", ".####."],
        '1' => ["..##..", ".###..", "..##..", "..##..", "..##..", "..##..", "######"],
        '2' => [".####.", "##..##", "....##", "...##.", "..##..", ".##...", "######"],
        '3' => [".####.", "##..##", "....##", "..###.", "....##", "##..##", ".####."],
        '4' => ["...##.", "..###.", ".####.", "##.##.", "######", "...##.", "...##."],
        '5' => ["######", "##....", "#####.", "....##", "....##", "##..##", ".####."],
        '6' => [".####.", "##....", "##....", "#####.", "##..##", "##..##", ".####."],
        '7' => ["######", "....##", "...##.", "..##..", ".##...", ".##...", ".##..."],
        '8' => [".####.", "##..##", "##..##", ".####.", "##..##", "##..##", ".####."],
        '9' => [".####.", "##..##", "##..##", ".#####", "....##", "....##", ".####."],
        '.' => ["", "", "", "", "", "..##..", "..##.."],
        ',' => ["", "", "", "", "..##..", "..##..", ".##..."],
        '!' => ["..##..", "..##..", "..##..", "..##..", "..##..", "", "..##.."],
        '?' => [".####.", "##..##", "...##.", "..##..", "..##..", "", "..##.."],
        '\'' => ["..##..", "..##..", ".##...", "", "", "", ""],
        '"' => [".##.##", ".##.##", "", "", "", "", ""],
        '-' => ["", "", "", ".####.", ".####.", "", ""],
        '+' => ["..##..", "..##..", "######", "######", "..##..", "..##..", ""],
        '=' => ["", "######", "######", "", "######", "######", ""],
        '/' => ["....##", "....##", "...##.", "..##..", ".##...", "##....", "##...."],
        '\\' => ["##....", "##....", ".##...", "..##..", "...##.", "....##", "....##"],
        '%' => ["##..##", "##..##", "...##.", "..##..", ".##...", "##..##", "##..##"],
        '(' => ["...##.", "..##..", ".##...", ".##...", ".##...", "..##..", "...##."],
        ')' => [".##...", "..##..", "...##.", "...##.", "...##.", "..##..", ".##..."],
        '<' => ["...##.", "..##..", ".##...", "##....", ".##...", "..##..", "...##."],
        '>' => [".##...", "..##..", "...##.", "....##", "...##.", "..##..", ".##..."],
        ':' => ["", "..##..", "..##..", "", "..##..", "..##..", ""],
        ';' => ["", "..##..", "..##..", "", "..##..", "..##..", ".##..."],
        '^' => ["..##..", ".####.", "##..##", "", "", "", ""],
        '@' => [".####.", "##..##", "##.###", "##.###", "##.##.", "##....", ".####."],
        _ => return None, // the six spaces: empty (invisible) cells
    })
}

fn font(s: &mut Sheet) {
    for (i, ch) in FONT_CHARS.chars().enumerate() {
        let pos = 30 * 32 + i as i32;
        let (cx, cy) = (pos % 32, pos / 32);
        let mut c = cell(s, cx, cy);
        c.rect(0, 0, 8, 8, G0); // glyph backing box (colored by some callers)
        if let Some(rows) = glyph(ch) {
            c.pat(0, 0, &rows, &[('#', G3)]);
        }
    }
}

/* ==========================  title logo (rows 6-7)  ========================== */

/// The full sheet-art title wordmark, warm-red gradient with a 1px drop shadow.
/// True color: palettes are ignored. Two strips on rows 6..=7:
///
/// - cells (0..14, 6..7), 15x2:  "DOOM"        — art 116px wide incl. shadow +
///   full-width underline (the 15th cell exists so the drop shadow isn't clipped).
/// - cells (15..31, 6..7), 17x2: "FOSSICKERS"  — the kicker line, art 130px wide
///   incl. shadow, half the stroke weight of DOOM at the same cap height.
///
/// Both words are centered within their own strip, so the blit loops in
/// `title_display.rs` / `splash_menu.rs` just center each strip on screen.
fn logo(s: &mut Sheet) {
    let hi = rgb(240, 110, 70);
    let md = RED_CL;
    let dk = rgb(128, 28, 32);

    // Narrow 5-wide variants of the letters in "FOSSICKERS" (the shared font glyphs
    // are 6 wide, which packs the ten-letter kicker too tight at 2x). Same 2px-stroke
    // style, 7 rows tall.
    #[rustfmt::skip]
    let kicker_glyph = |ch: char| -> [&'static str; 7] {
        match ch {
            'F' => ["#####", "##...", "##...", "####.", "##...", "##...", "##..."],
            'O' => [".###.", "##.##", "##.##", "##.##", "##.##", "##.##", ".###."],
            'S' => [".####", "##...", "##...", ".###.", "...##", "...##", "####."],
            'I' => ["####", ".##.", ".##.", ".##.", ".##.", ".##.", "####"],
            'C' => [".####", "##...", "##...", "##...", "##...", "##...", ".####"],
            'K' => ["##.##", "##.##", "####.", "###..", "####.", "##.##", "##.##"],
            'E' => ["#####", "##...", "##...", "####.", "##...", "##...", "#####"],
            'R' => ["####.", "##.##", "##.##", "####.", "###..", "##.##", "##.##"],
            _ => unreachable!("kicker letters only"),
        }
    };

    // draw `word` with glyphs stretched (sx, sy), top edge at y_top, centered in a
    // `field_w`-px-wide strip (accounting for the +1px shadow), gradient banded over
    // the word's own height; pass 0 = drop shadow, pass 1 = fill
    let draw_word = |c: &mut C,
                     word: &str,
                     glyph_of: &dyn Fn(char) -> [&'static str; 7],
                     sx: i32,
                     sy: i32,
                     y_top: i32,
                     gap: i32,
                     field_w: i32| {
        let widths: Vec<i32> = word
            .chars()
            .map(|ch| glyph_of(ch).iter().map(|r| r.len() as i32).max().unwrap())
            .collect();
        let total: i32 = widths.iter().map(|w| w * sx).sum::<i32>() + (word.len() as i32 - 1) * gap;
        let x0 = (field_w - total - 1) / 2; // -1: the drop shadow adds a column
        let height = 7 * sy; // font glyphs are 7 rows tall
        for pass in 0..2 {
            let mut lx = x0;
            for (li, ch) in word.chars().enumerate() {
                let rows = glyph_of(ch);
                for (ry, row) in rows.iter().enumerate() {
                    for (rx, g) in row.chars().enumerate() {
                        if g != '#' {
                            continue;
                        }
                        for dy in 0..sy {
                            for dx in 0..sx {
                                let x = lx + rx as i32 * sx + dx;
                                let y = y_top + ry as i32 * sy + dy;
                                if pass == 0 {
                                    c.set(x + 1, y + 1, OUT);
                                } else {
                                    let band = (y - y_top) * 3 / height.max(1);
                                    let ink = match band {
                                        0 => hi,
                                        1 => md,
                                        _ => dk,
                                    };
                                    c.set(x, y, ink);
                                }
                            }
                        }
                    }
                }
                lx += widths[li] * sx + gap;
            }
        }
    };

    // "DOOM" — the hero word, 8px strokes, in a 15-cell (120px) strip
    let font_glyph = |ch: char| glyph(ch).expect("logo letters exist");
    let mut c = cell(s, 0, 6);
    draw_word(&mut c, "DOOM", &font_glyph, 4, 2, 0, 4, 120);

    // full-width underline under it
    c.hline(3, 15, 114, dk);
    c.set(2, 15, md);
    c.set(117, 15, md);

    // "FOSSICKERS" — the kicker, 4px strokes at the same cap height, in a 17-cell
    // (136px) strip; slightly wider than DOOM so the lockup reads top-heavy
    let mut k = cell(s, 15, 6);
    draw_word(&mut k, "FOSSICKERS", &kicker_glyph, 2, 2, 0, 2, 136);
}

/// Cell (30,30): `Sprite::missing_texture` (drawn flat magenta by its palette).
fn missing_texture(s: &mut Sheet) {
    let mut c = cell(s, 30, 30);
    c.rect(0, 0, 8, 8, G1);
    c.dither(0, 0, 8, 8, 0, G2);
}

/* ==============================  main  ============================== */

fn main() {
    let mut s = Sheet::new();

    // terrain
    dots_cells(&mut s);
    rock_connector(&mut s);
    grass_connector(&mut s);
    water_connector(&mut s);
    grass_texture(&mut s);
    sand_texture(&mut s);
    snow_texture(&mut s);
    dirt_texture(&mut s);
    stone_texture(&mut s);
    wool_cell(&mut s);
    cloud_full_cells(&mut s);
    farm_cell(&mut s);
    footprint_cell(&mut s);
    mud_cells(&mut s);
    ore_cells(&mut s);
    quicksand_cells(&mut s);
    stairs_cells(&mut s);
    blank_cell(&mut s);
    cactus_cells(&mut s);
    wheat_cells(&mut s);
    sapling_cell(&mut s);
    torch_cell(&mut s);
    floor_cells(&mut s);
    tree_cells(&mut s);
    door_cells(&mut s);
    wall_cells(&mut s);
    gravestone_cells(&mut s);
    pumpkin_cells(&mut s);
    tall_grass_cells(&mut s);
    flora_cells(&mut s);

    // items + UI
    items_row4(&mut s);
    items_row5(&mut s);
    food_icons(&mut s);
    ui_row12(&mut s);
    ui_row13(&mut s);
    splash_cells(&mut s);

    // furniture
    furniture_sprites(&mut s);
    furniture_icons(&mut s);

    // mobs
    player_sets(&mut s);
    marsh_lurker(&mut s);
    pig(&mut s);
    knight(&mut s);
    feral_hound(&mut s);
    cow(&mut s);
    stone_golem(&mut s);
    fire_particle(&mut s);
    night_wisp(&mut s);
    sheep(&mut s);
    snake(&mut s);
    glow_worm(&mut s);

    // text
    font(&mut s);
    logo(&mut s);
    missing_texture(&mut s);

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sprites.png");
    s.save(&path);
    println!("wrote {}", path.display());
}
