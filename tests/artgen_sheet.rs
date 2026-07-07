//! Sanity checks for the generated sprite sheet (`assets/sprites.png`, produced by
//! `cargo run --bin artgen`): every cell the code references must contain art, and each
//! cell must be in the right pixel mode (palette grayscale vs true color).
//!
//! The inventory below was built by auditing every `Sprite::*` constructor, raw
//! `screen.render(.., pos, ..)` call, the font (`30*32 + index`) and the title logo
//! (`x + (y+6)*32`). Update it when call sites change.

use fdoom::gfx::sprite_sheet::{SheetPixel, SpriteSheet};

fn sheet() -> SpriteSheet {
    SpriteSheet::from_png(fdoom::assets::SPRITES_PNG)
}

/// All pixels of cell `pos = cx + cy*32`.
fn cell_pixels(s: &SpriteSheet, cx: i32, cy: i32) -> Vec<SheetPixel> {
    let mut v = Vec::with_capacity(64);
    for y in 0..8 {
        for x in 0..8 {
            v.push(s.pixels[((cy * 8 + y) * s.width + cx * 8 + x) as usize]);
        }
    }
    v
}

fn non_empty(s: &SpriteSheet, cx: i32, cy: i32) -> bool {
    cell_pixels(s, cx, cy)
        .iter()
        .any(|p| !matches!(p, SheetPixel::Transparent))
}

fn pure_grayscale(s: &SpriteSheet, cx: i32, cy: i32) -> bool {
    cell_pixels(s, cx, cy)
        .iter()
        .all(|p| matches!(p, SheetPixel::Palette(_)))
}

fn has_true_color(s: &SpriteSheet, cx: i32, cy: i32) -> bool {
    cell_pixels(s, cx, cy)
        .iter()
        .any(|p| matches!(p, SheetPixel::Rgb(_)))
}

/// Every (cx, cy, w, h) cell block referenced by game code.
const INVENTORY: &[(i32, i32, i32, i32, &str)] = &[
    // -- terrain --
    (0, 0, 4, 1, "dots texture cells (Sprite::dots/random_dots)"),
    (22, 0, 4, 1, "grass tuft texture (Sprite::dots_at)"),
    (26, 0, 4, 1, "sand ripple texture (Sprite::dots_at)"),
    (13, 3, 4, 1, "snow drift texture (Sprite::dots_at)"),
    (21, 3, 4, 1, "dirt clod texture (Sprite::dots_at)"),
    (25, 3, 4, 1, "stone plate texture (Sprite::dots_at)"),
    (24, 1, 2, 2, "mud block"),
    (4, 0, 3, 3, "rock/hard-rock/cloud sparse blob"),
    (7, 0, 2, 2, "rock/cloud sides block"),
    (9, 0, 2, 2, "tree outer pieces"),
    (10, 1, 1, 1, "tree canopy fill"),
    (10, 2, 1, 1, "tree canopy fill + bark"),
    (10, 3, 1, 1, "tree bottom-right piece"),
    (11, 0, 3, 3, "grass/sand/snow sparse blob"),
    (14, 0, 3, 3, "water/lava/hole/exploded sparse blob"),
    (17, 0, 1, 1, "wool"),
    (18, 0, 3, 1, "cloud interior cells"),
    (2, 1, 1, 1, "farmland"),
    (3, 1, 1, 1, "sand/snow footprint"),
    (17, 1, 2, 2, "ore nub / cloud cactus"),
    (22, 1, 2, 2, "quicksand"),
    (0, 2, 2, 2, "stairs down"),
    (2, 2, 2, 2, "stairs up"),
    (7, 2, 1, 1, "Sprite::blank fill"),
    (8, 2, 2, 2, "cactus"),
    (4, 3, 4, 1, "wheat growth stages"),
    (11, 3, 1, 1, "sapling"),
    (12, 3, 1, 1, "torch tile"),
    (19, 2, 2, 2, "floor / lava brick"),
    // -- items --
    (0, 4, 29, 1, "item icons row 4"),
    (0, 5, 8, 1, "tools row 5"),
    (
        8,
        5,
        4,
        1,
        "reserved crafting icons (fiber/stick/cord/sharp stone)",
    ),
    (13, 5, 4, 1, "flight arrows"),
    (20, 5, 2, 1, "stick + grass fibers"),
    (22, 5, 4, 1, "weapon icons (spear/crossbow/knife/slingshot)"),
    // -- logo + furniture + decor --
    (0, 6, 15, 2, "title logo (DOOM strip)"),
    // the kicker strip is 17 cells wide but its first/last cells are transparent
    // margin (the word is centered within the strip)
    (16, 6, 15, 2, "title kicker (FOSSICKERS strip)"),
    (0, 8, 22, 2, "furniture sprites (anvil..spawner)"),
    (22, 8, 2, 2, "pumpkin"),
    (26, 8, 2, 2, "tall grass: tall stage"),
    (28, 9, 2, 1, "tall grass: small stage (ground row only)"),
    (30, 8, 2, 2, "tall grass: medium stage"),
    (0, 10, 11, 1, "furniture item icons"),
    (11, 10, 7, 1, "forage/food icons"),
    (0, 11, 1, 1, "splash cell A"),
    (3, 11, 1, 1, "splash cell B"),
    (11, 11, 2, 2, "grave stone (slab)"),
    (13, 11, 2, 2, "broken grave stone (rubble)"),
    (15, 11, 2, 2, "grave: rounded headstone"),
    (17, 11, 2, 2, "grave: stone cross"),
    (19, 11, 2, 2, "grave: cracked slab"),
    (21, 11, 2, 2, "grave: rubble variant"),
    (23, 11, 2, 2, "grave: wooden cross"),
    (25, 11, 2, 2, "grave: broken wooden cross"),
    // -- UI --
    (0, 12, 4, 1, "heart/stamina/hunger/armor icons"),
    (5, 12, 1, 1, "smash particle"),
    (6, 12, 1, 1, "clothing item"),
    (0, 13, 3, 1, "menu frame pieces"),
    (5, 13, 4, 1, "swim ripple + slashes + zap"),
    // -- mobs --
    (0, 14, 8, 2, "player/zombie walk frames"),
    (8, 14, 8, 2, "marsh lurker frames"),
    (16, 14, 8, 2, "pig frames"),
    (24, 14, 8, 2, "knight frames"),
    (0, 16, 8, 2, "player carry frames"),
    (8, 16, 8, 2, "feral hound frames"),
    (16, 16, 8, 2, "cow frames"),
    (0, 18, 8, 2, "stone golem frames"),
    (9, 19, 1, 1, "spawner fire particle"),
    (10, 18, 8, 2, "sheep frames"),
    (18, 18, 8, 2, "snake frames"),
    (26, 19, 1, 1, "glow worm"),
    (0, 20, 4, 2, "night wisp frames"),
    (4, 20, 2, 2, "rattler coiled pose"),
    (6, 20, 4, 2, "ghost pulse frames"),
    (10, 20, 1, 1, "firefly glow speck"),
    (11, 20, 1, 1, "grass-stealth eye glints"),
    // -- fire wave --
    (8, 18, 2, 1, "smoke puff + wisp"),
    (8, 19, 1, 1, "campfire item icon"),
    (12, 20, 6, 2, "campfire lit A/B + ember"),
    (10, 21, 2, 1, "tile-fire overlay frames"),
    (18, 20, 8, 2, "player suit frames"),
    (18, 22, 8, 2, "player suit carry frames"),
    // -- structures --
    (0, 24, 2, 2, "open door"),
    (2, 24, 2, 2, "closed door"),
    (4, 22, 3, 3, "wood wall sparse (center = full tile)"),
    (7, 22, 2, 2, "wood wall sides"),
    (4, 25, 3, 3, "stone wall sparse"),
    (7, 24, 2, 2, "stone wall sides"),
    (30, 30, 1, 1, "missing texture"),
    // -- flora (rows 26-29) --
    (0, 26, 4, 3, "pine + dead tree species sets"),
    (7, 26, 8, 3, "willow/palm/flat-crown/snow-pine species sets"),
    (15, 26, 16, 2, "decor flora (berry bushes..jack-o-lantern)"),
    (15, 28, 4, 2, "mushroom + dry bush"),
    (19, 28, 12, 2, "species tree shape variants B"),
];

/// `Font::CHARS` prefix that is actually renderable (text is uppercased before drawing).
const FONT_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ      0123456789.,!?'\"-+=/\\%()<>:;^@";

#[test]
fn sheet_is_256x256() {
    let s = sheet();
    assert_eq!((s.width, s.height), (256, 256));
}

#[test]
fn every_referenced_cell_has_art() {
    let s = sheet();
    let mut missing = Vec::new();
    for &(cx, cy, w, h, what) in INVENTORY {
        for dy in 0..h {
            for dx in 0..w {
                if !non_empty(&s, cx + dx, cy + dy) {
                    missing.push(format!("({},{}) {}", cx + dx, cy + dy, what));
                }
            }
        }
    }
    assert!(missing.is_empty(), "empty referenced cells: {missing:#?}");
}

#[test]
fn font_glyphs_are_palette_grayscale() {
    let s = sheet();
    for (i, ch) in FONT_CHARS.chars().enumerate() {
        let pos = 30 * 32 + i as i32;
        let (cx, cy) = (pos % 32, pos / 32);
        assert!(
            pure_grayscale(&s, cx, cy),
            "font glyph {ch:?} at ({cx},{cy}) must be pure grayscale"
        );
        if ch != ' ' {
            // strokes exist: at least one non-shade-0 pixel
            assert!(
                cell_pixels(&s, cx, cy)
                    .iter()
                    .any(|p| matches!(p, SheetPixel::Palette(sh) if *sh > 0)),
                "font glyph {ch:?} at ({cx},{cy}) has no strokes"
            );
        }
    }
}

#[test]
fn palette_cells_stay_grayscale() {
    let s = sheet();
    // cells recolored through meaningful palettes at draw time must never contain
    // true-color pixels
    let must_be_gray: &[(i32, i32, i32, i32, &str)] = &[
        (0, 0, 4, 1, "dots"),
        (22, 0, 8, 1, "grass/sand textures"),
        (13, 3, 4, 1, "snow texture"),
        (21, 3, 8, 1, "dirt/stone textures"),
        (24, 1, 2, 2, "mud block"),
        (4, 0, 3, 3, "rock sparse"),
        (7, 0, 2, 2, "rock sides"),
        (11, 0, 6, 3, "grass/water sparse"),
        (17, 0, 1, 1, "wool"),
        (17, 1, 2, 2, "ore nub"),
        (0, 2, 4, 2, "stairs"),
        (19, 2, 2, 2, "floor/lava brick"),
        (4, 3, 4, 1, "wheat"),
        (0, 4, 29, 1, "item icons"),
        (0, 5, 8, 1, "tools"),
        (8, 5, 4, 1, "reserved crafting icons"),
        (13, 5, 4, 1, "arrows"),
        (22, 5, 4, 1, "weapon icons"),
        (11, 10, 7, 1, "food icons"),
        (2, 8, 2, 2, "chest"),
        (10, 8, 2, 2, "lantern"),
        (20, 8, 2, 2, "spawner"),
        (0, 10, 11, 1, "furniture icons"),
        (0, 12, 4, 1, "HUD icons"),
        (0, 13, 3, 1, "frame"),
        (5, 13, 4, 1, "ripple/slash/zap"),
        (0, 14, 32, 2, "mob row 14"),
        (0, 16, 24, 2, "mob row 16"),
        (0, 18, 26, 2, "mob row 18"),
        (0, 20, 4, 2, "night wisp"),
        (4, 20, 6, 2, "mob-life cells (rattler coil + ghost frames)"),
        (8, 18, 2, 1, "smoke cells"),
        (8, 19, 1, 1, "campfire icon"),
        (18, 20, 8, 2, "suit"),
        (18, 22, 8, 2, "suit carry"),
        (0, 24, 4, 2, "doors"),
        (4, 22, 5, 3, "wood wall"),
        (4, 25, 3, 3, "stone wall"),
        (7, 24, 2, 2, "stone wall sides"),
    ];
    for &(cx, cy, w, h, what) in must_be_gray {
        for dy in 0..h {
            for dx in 0..w {
                let all_gray = cell_pixels(&s, cx + dx, cy + dy)
                    .iter()
                    .all(|p| !matches!(p, SheetPixel::Rgb(_)));
                assert!(
                    all_gray,
                    "{what} cell ({},{}) must not contain true-color pixels",
                    cx + dx,
                    cy + dy
                );
            }
        }
    }
}

#[test]
fn scenery_cells_are_true_color() {
    let s = sheet();
    for &(cx, cy, what) in &[
        (10, 1, "tree canopy"),
        (8, 2, "cactus"),
        (12, 3, "torch"),
        (22, 8, "pumpkin"),
        (0, 6, "title logo"),
        (14, 8, "tnt"),
        (11, 11, "grave stone"),
        (22, 1, "quicksand"),
        (16, 6, "FOSSICKERS kicker"),
        (0, 26, "pine tree"),
        (15, 26, "berry bush"),
        (23, 11, "wooden cross grave"),
        (15, 28, "mushroom tile"),
        (19, 28, "pine variant B"),
        (12, 20, "campfire lit"),
        (16, 20, "campfire ember"),
        (10, 21, "tile-fire overlay"),
    ] {
        assert!(
            has_true_color(&s, cx, cy),
            "{what} at ({cx},{cy}) should contain true-color pixels"
        );
    }
}
