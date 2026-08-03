//! The 8px cell grid every sprite is addressed on, plus the sprite map for
//! monolithic atlas sheets.
//!
//! Split sprite files carry their own footprint, so directory and canvas modes never
//! need this table. Sheet mode does: a stitched atlas is one PNG with no metadata, so
//! `G`-snapping and the header label read the layout from [`SPRITE_MAP`].

/// Sprite cell edge (must match `sprite_sheet::BOX_WIDTH`).
pub(crate) const CELL: i32 = 8;

/// Region map of the classic 256x256 atlas layout (row = 8px cell row), so the
/// sheet-mode browser can label unmapped cells. Dir-mode files are labeled by their
/// folder instead.
pub(crate) fn artgen_region(row: i32) -> &'static str {
    match row {
        0..=3 => "TERRAIN",
        4..=5 => "ITEMS",
        6..=7 => "TITLE LOGO",
        8..=10 => "FURNITURE",
        11..=13 => "UI + GRAVES",
        14..=19 => "MOBS",
        20..=21 => "MOBS + FIRE FX",
        22..=23 => "MOBS",
        24..=25 => "DECOR",
        26..=29 => "FLORA",
        30..=31 => "FONT",
        _ => "?",
    }
}

/// Sprite map for atlas sheets: `(cx, cy, w, h, uw, uh, name)` in 8px cells. An
/// entry is a sprite or a strip of same-size sprites; `uw x uh` is the footprint of
/// one sprite inside it (`G` snaps the window to the unit under the cursor). Mirrors
/// the manifest / `tests/artgen_sheet.rs` inventory — including odd-origin blocks
/// (graves at x 15/17/19..., decor flora at (15,26), ...).
pub(crate) type SpriteSpan = (i32, i32, i32, i32, i32, i32, &'static str);
pub(crate) const SPRITE_MAP: &[SpriteSpan] = &[
    (0, 0, 4, 1, 4, 1, "TERRAIN DOTS TILE"),
    (22, 0, 4, 1, 4, 1, "GRASS TUFT TILE"),
    (26, 0, 4, 1, 4, 1, "SAND RIPPLE TILE"),
    (13, 3, 4, 1, 4, 1, "SNOW DRIFT TILE"),
    (21, 3, 4, 1, 4, 1, "DIRT CLOD TILE"),
    (25, 3, 4, 1, 4, 1, "STONE PLATE TILE"),
    (24, 1, 2, 2, 2, 2, "MUD BLOCK"),
    (4, 0, 3, 3, 3, 3, "ROCK SPARSE BLOB"),
    (7, 0, 2, 2, 2, 2, "ROCK SIDES"),
    (9, 0, 2, 2, 2, 2, "TREE OUTER PIECES"),
    (11, 0, 3, 3, 3, 3, "GRASS SPARSE BLOB"),
    (14, 0, 3, 3, 3, 3, "WATER SPARSE BLOB"),
    (17, 1, 2, 2, 2, 2, "ORE NUB"),
    (22, 1, 2, 2, 2, 2, "QUICKSAND"),
    (0, 2, 2, 2, 2, 2, "STAIRS DOWN"),
    (2, 2, 2, 2, 2, 2, "STAIRS UP"),
    (8, 2, 2, 2, 2, 2, "CACTUS"),
    (19, 2, 2, 2, 2, 2, "FLOOR / LAVA BRICK"),
    (4, 3, 4, 1, 1, 1, "WHEAT STAGE"),
    (0, 4, 32, 1, 1, 1, "ITEM ICON"),
    (0, 5, 32, 1, 1, 1, "ITEM ICON"),
    (0, 6, 15, 2, 15, 2, "TITLE: DOOM STRIP"),
    (16, 6, 15, 2, 15, 2, "TITLE: KICKER STRIP"),
    (0, 8, 22, 2, 2, 2, "FURNITURE"),
    (22, 8, 2, 2, 2, 2, "PUMPKIN"),
    (26, 8, 2, 2, 2, 2, "TALL GRASS: TALL"),
    (30, 8, 2, 2, 2, 2, "TALL GRASS: MEDIUM"),
    (28, 9, 2, 1, 2, 1, "TALL GRASS: SMALL"),
    (0, 10, 18, 1, 1, 1, "FURNITURE / FOOD ICON"),
    (11, 11, 2, 2, 2, 2, "GRAVE: SLAB"),
    (13, 11, 2, 2, 2, 2, "GRAVE: RUBBLE"),
    (15, 11, 2, 2, 2, 2, "GRAVE: ROUNDED"),
    (17, 11, 2, 2, 2, 2, "GRAVE: STONE CROSS"),
    (19, 11, 2, 2, 2, 2, "GRAVE: CRACKED SLAB"),
    (21, 11, 2, 2, 2, 2, "GRAVE: RUBBLE B"),
    (23, 11, 2, 2, 2, 2, "GRAVE: WOODEN CROSS"),
    (25, 11, 2, 2, 2, 2, "GRAVE: BROKEN CROSS"),
    (0, 12, 7, 1, 1, 1, "HUD ICON"),
    (0, 13, 9, 1, 1, 1, "UI FRAME / FX"),
    (0, 14, 8, 2, 2, 2, "PLAYER/ZOMBIE WALK FRAMES"),
    (8, 14, 8, 2, 2, 2, "MARSH LURKER FRAMES"),
    (16, 14, 8, 2, 2, 2, "PIG FRAMES"),
    (24, 14, 8, 2, 2, 2, "KNIGHT FRAMES"),
    (0, 16, 8, 2, 2, 2, "PLAYER CARRY FRAMES"),
    (8, 16, 8, 2, 2, 2, "FERAL HOUND FRAMES"),
    (16, 16, 8, 2, 2, 2, "COW FRAMES"),
    (0, 18, 8, 2, 2, 2, "STONE GOLEM FRAMES"),
    (10, 18, 8, 2, 2, 2, "SHEEP FRAMES"),
    (18, 18, 8, 2, 2, 2, "SNAKE FRAMES"),
    (8, 18, 2, 1, 1, 1, "SMOKE PUFF"),
    (0, 20, 4, 2, 2, 2, "NIGHT WISP FRAMES"),
    (4, 20, 2, 2, 2, 2, "RATTLER COIL"),
    (6, 20, 4, 2, 2, 2, "GHOST PULSE FRAMES"),
    (12, 20, 6, 2, 2, 2, "CAMPFIRE"),
    (10, 21, 2, 1, 1, 1, "TILE-FIRE OVERLAY"),
    (18, 20, 8, 2, 2, 2, "PLAYER SUIT FRAMES"),
    (18, 22, 8, 2, 2, 2, "SUIT CARRY FRAMES"),
    (0, 24, 2, 2, 2, 2, "OPEN DOOR"),
    (2, 24, 2, 2, 2, 2, "CLOSED DOOR"),
    (4, 22, 3, 3, 3, 3, "WOOD WALL SPARSE"),
    (7, 22, 2, 2, 2, 2, "WOOD WALL SIDES"),
    (4, 25, 3, 3, 3, 3, "STONE WALL SPARSE"),
    (7, 24, 2, 2, 2, 2, "STONE WALL SIDES"),
    (0, 26, 4, 3, 2, 3, "PINE / DEAD TREE SET"),
    (7, 26, 8, 3, 2, 3, "TREE SPECIES SET"),
    (15, 26, 16, 2, 2, 2, "DECOR FLORA"),
    (15, 28, 4, 2, 2, 2, "MUSHROOM / DRY BUSH"),
    (19, 28, 12, 2, 2, 2, "TREE VARIANT B"),
    (0, 30, 32, 2, 1, 1, "FONT GLYPH"),
];

/// The most specific (smallest) sprite-map entry containing cell `(ccx, ccy)`.
pub(crate) fn sprite_at(ccx: i32, ccy: i32) -> Option<&'static SpriteSpan> {
    SPRITE_MAP
        .iter()
        .filter(|&&(cx, cy, w, h, ..)| (cx..cx + w).contains(&ccx) && (cy..cy + h).contains(&ccy))
        .min_by_key(|&&(_, _, w, h, ..)| w * h)
}

/// Origin of the sprite *unit* containing cell `(ccx, ccy)` inside `span`. Strips
/// hold several same-size sprites, so this walks whole `uw x uh` steps from the
/// span's origin — never an even-cell snap, which would halve odd-origin blocks.
pub(crate) fn unit_origin(span: &SpriteSpan, ccx: i32, ccy: i32) -> (i32, i32, i32, i32) {
    let &(cx, cy, _, _, uw, uh, _) = span;
    (
        cx + ((ccx - cx) / uw) * uw,
        cy + ((ccy - cy) / uh) * uh,
        uw,
        uh,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Overlapping entries resolve to the smallest, so a sprite inside a strip wins
    /// over the strip's own bounding box.
    #[test]
    fn sprite_at_picks_the_most_specific_entry() {
        // (8,18) is inside both the SNAKE/SHEEP rows and the 2x1 SMOKE PUFF
        assert_eq!(sprite_at(8, 18).map(|s| s.6), Some("SMOKE PUFF"));
        assert_eq!(sprite_at(0, 0).map(|s| s.6), Some("TERRAIN DOTS TILE"));
        assert_eq!(sprite_at(200, 200), None, "off the sheet");
    }

    /// The odd-origin regression: graves start at cell x 15/17/19, so hovering the
    /// right half must still resolve to the block's true origin.
    #[test]
    fn unit_origin_keeps_odd_origins_whole() {
        let graves = sprite_at(16, 11).expect("graves mapped");
        assert_eq!(graves.6, "GRAVE: ROUNDED");
        assert_eq!(unit_origin(graves, 16, 11), (15, 11, 2, 2));

        // decor flora: a 16-cell-wide strip of 2x2 units starting at odd x 15
        let flora = sprite_at(16, 27).expect("flora mapped");
        assert_eq!(unit_origin(flora, 16, 27), (15, 26, 2, 2));

        // tree species: 2x3 units from (7,26) — the second unit starts at x 9
        let trees = sprite_at(10, 28).expect("trees mapped");
        assert_eq!(unit_origin(trees, 10, 28), (9, 26, 2, 3));
    }

    /// Every mapped unit tiles its span exactly — a strip whose unit size does not
    /// divide it would snap the window off the end of the sprite.
    #[test]
    fn every_span_is_a_whole_number_of_units() {
        for &(cx, cy, w, h, uw, uh, name) in SPRITE_MAP {
            assert!(uw > 0 && uh > 0, "{name}: zero-size unit");
            assert_eq!(w % uw, 0, "{name}: {w} wide is not a multiple of {uw}");
            assert_eq!(h % uh, 0, "{name}: {h} tall is not a multiple of {uh}");
            assert!(cx >= 0 && cy >= 0, "{name}: negative origin");
        }
    }

    /// Rows are labelled across the whole classic sheet, with a fallback past it.
    #[test]
    fn regions_cover_the_classic_sheet() {
        for row in 0..32 {
            assert_ne!(artgen_region(row), "?", "row {row} unlabelled");
        }
        assert_eq!(artgen_region(99), "?");
    }
}
