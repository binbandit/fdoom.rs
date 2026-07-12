//! Map screen: shared biome palette + chunk-based reveal logic.

use std::collections::HashSet;

use fdoom::gfx::biome_palette::{BIOME_LEGEND, biome_color};
use fdoom::level::chunk::chunk_coord;
use fdoom::screen::map_menu::MapMenu;
use fdoom::testutil::TestWorld;

/// Every biome has its own color. `biome_color`'s exhaustive match guarantees the
/// palette is total; the legend (which the tools iterate) must cover all 11 variants
/// and never map two biomes to one color.
#[test]
fn every_biome_has_a_distinct_color() {
    assert_eq!(BIOME_LEGEND.len(), 11);
    let mut seen = HashSet::new();
    for (b, name) in BIOME_LEGEND {
        assert!(
            seen.insert(biome_color(b)),
            "duplicate palette color: {name}"
        );
    }
}

#[test]
fn reveal_marks_walked_chunks_and_hides_far_ones() {
    let mut tw = TestWorld::infinite().build();
    tw.tick_n(8); // let the spawn halo stream in
    let lvl = tw.g.current_level;
    let (ptx, pty) = tw.player_tile();
    let (pcx, pcy) = (chunk_coord(ptx), chunk_coord(pty));

    let revealed = MapMenu::revealed_chunks(&tw.g, lvl);
    assert!(
        revealed.contains(&(pcx, pcy)),
        "the chunk under the player must be revealed"
    );
    assert!(
        !revealed.contains(&(pcx + 100, pcy)),
        "a chunk 100 chunks away must stay hidden"
    );

    // walk into fresh territory: three chunks east, then let streaming catch up
    let far = (pcx + 3, pcy);
    assert!(!revealed.contains(&far), "not yet visited => hidden");
    tw.teleport(ptx + 3 * 64, pty);
    tw.tick_n(8);
    let revealed = MapMenu::revealed_chunks(&tw.g, lvl);
    assert!(
        revealed.contains(&far),
        "a freshly-walked chunk must be revealed"
    );
}
