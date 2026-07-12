//! Dug pits and chasms render as ragged organic holes, not squares: tile corners stay
//! untouched dirt, the interior darkens, edge midpoints (unlike corners) can be part
//! of the hole, and every render is a pure deterministic f(seed, x, y).

use fdoom::gfx::screen;
use fdoom::level::tile::{depth, dispatch};
use fdoom::testutil::{TestWorld, renderer, save_png, verify_path};

/// Screen pixel where the probed tile's top-left corner lands.
const PX: i32 = 32;

/// Render only the tile registered as `def_name` at `(tx, ty)` through the real
/// dispatch and return its 16x16 pixel patch (row-major).
fn patch(
    tw: &mut TestWorld,
    scr: &mut fdoom::gfx::Screen,
    tx: i32,
    ty: i32,
    def_name: &str,
) -> Vec<i32> {
    let def = tw.g.tiles.get(def_name);
    let lvl = tw.g.current_level;
    scr.set_offset(tx * 16 - PX, ty * 16 - PX);
    dispatch::render(&mut tw.g, scr, &def, lvl, tx, ty);
    scr.set_offset(0, 0);
    let mut out = Vec::with_capacity(256);
    for y in 0..16 {
        for x in 0..16 {
            out.push(scr.pixels[((PX + x) + (PX + y) * screen::W) as usize]);
        }
    }
    out
}

fn at(p: &[i32], x: usize, y: usize) -> i32 {
    p[x + y * 16]
}

fn luma(p: i32) -> i32 {
    ((p >> 16) & 0xFF) + ((p >> 8) & 0xFF) + (p & 0xFF)
}

const CORNERS: [(usize, usize); 4] = [(0, 0), (15, 0), (0, 15), (15, 15)];
const EDGE_MIDS: [(usize, usize); 4] = [(0, 7), (0, 8), (15, 7), (15, 8)];

#[test]
fn pit_stages_are_ragged_not_square() {
    let mut tw = TestWorld::infinite().seed(777).name("hole_pit").build();
    let mut r = renderer();
    let lvl = tw.g.current_level;
    let (ptx, pty) = tw.player_tile();
    let (tx, ty) = (ptx + 2, pty);

    let base = patch(&mut tw, &mut r.screen, tx, ty, "dirt");
    tw.place_at("Dug Pit", tx, ty);

    for stage in 0..=depth::MAX_STAGE {
        tw.g.level_mut(lvl).set_data(tx, ty, stage);
        let pit = patch(&mut tw, &mut r.screen, tx, ty, "Dug Pit");

        // the square is gone: corners are untouched dirt...
        for &(cx, cy) in &CORNERS {
            assert_eq!(
                at(&pit, cx, cy),
                at(&base, cx, cy),
                "stage {stage}: corner ({cx},{cy}) must stay plain dirt"
            );
        }
        // ...while the middle of the tile is a darkened depression
        for &(cx, cy) in &[(7usize, 7usize), (8, 8)] {
            assert!(
                luma(at(&pit, cx, cy)) < luma(at(&base, cx, cy)),
                "stage {stage}: center ({cx},{cy}) must be darker than dirt"
            );
        }
        // determinism: rendering the same tile again is pixel-identical
        let again = patch(&mut tw, &mut r.screen, tx, ty, "Dug Pit");
        assert_eq!(
            pit, again,
            "stage {stage}: pit render must be deterministic"
        );
    }

    // at full depth the lip is widest: at least one tile-edge midpoint is part of the
    // hole (darkened) even though every corner is not — corners differ from edge mids
    tw.g.level_mut(lvl).set_data(tx, ty, depth::MAX_STAGE);
    let pit = patch(&mut tw, &mut r.screen, tx, ty, "Dug Pit");
    assert!(
        EDGE_MIDS
            .iter()
            .any(|&(x, y)| luma(at(&pit, x, y)) < luma(at(&base, x, y))),
        "max-stage pit: some edge midpoint must be darkened while corners are not"
    );
}

#[test]
fn chasm_is_a_ragged_opening_not_a_square() {
    let mut tw = TestWorld::infinite().seed(4242).name("hole_chasm").build();
    let mut r = renderer();
    let (ptx, pty) = tw.player_tile();
    let (tx, ty) = (ptx + 2, pty);

    let base = patch(&mut tw, &mut r.screen, tx, ty, "dirt");
    tw.place_at("Chasm", tx, ty);
    let ch = patch(&mut tw, &mut r.screen, tx, ty, "Chasm");

    // corners survive as plain dirt (no square-in-square frame)
    for &(cx, cy) in &CORNERS {
        assert_eq!(
            at(&ch, cx, cy),
            at(&base, cx, cy),
            "chasm corner ({cx},{cy}) must stay plain dirt"
        );
    }
    // the middle is a pitch-black drop
    assert_eq!(at(&ch, 7, 7), 0, "chasm center must be black");
    assert_eq!(at(&ch, 8, 8), 0, "chasm center must be black");
    // the opening reaches an edge midpoint the corners never reach
    assert!(
        EDGE_MIDS
            .iter()
            .any(|&(x, y)| luma(at(&ch, x, y)) < luma(at(&base, x, y))),
        "chasm: some edge midpoint must be darkened while corners are not"
    );
    // the rim is shaded, not binary: some pixels sit between plain dirt and black
    let base_max = base.iter().map(|&p| luma(p)).max().unwrap();
    assert!(
        ch.iter().any(|&p| luma(p) > 0 && luma(p) < base_max / 2),
        "chasm rim must grade between dirt and black"
    );
    // determinism
    let again = patch(&mut tw, &mut r.screen, tx, ty, "Chasm");
    assert_eq!(ch, again, "chasm render must be deterministic");
}

/// Neighboring holes pick different outlines (hash of position, not one stamp).
#[test]
fn neighboring_holes_differ() {
    let mut tw = TestWorld::infinite().seed(99).name("hole_vary").build();
    let mut r = renderer();
    let lvl = tw.g.current_level;
    let (ptx, pty) = tw.player_tile();

    let mut shapes: Vec<Vec<bool>> = Vec::new();
    for i in 0..4 {
        let (tx, ty) = (ptx - 3 + i * 2, pty - 2);
        let base = patch(&mut tw, &mut r.screen, tx, ty, "dirt");
        tw.place_at("Dug Pit", tx, ty);
        tw.g.level_mut(lvl).set_data(tx, ty, depth::MAX_STAGE);
        let pit = patch(&mut tw, &mut r.screen, tx, ty, "Dug Pit");
        // reduce to a "was this pixel darkened" mask so dirt texture differences
        // between positions don't count as shape differences
        shapes.push(
            pit.iter()
                .zip(&base)
                .map(|(&a, &b)| luma(a) < luma(b))
                .collect(),
        );
    }
    assert!(
        shapes.windows(2).any(|w| w[0] != w[1]) || shapes[0] != shapes[3],
        "adjacent pits must not all share one outline"
    );
}

/// Visual check material: a field of pits at every stage plus chasms and a deep-water
/// inlet, dumped at 1x and 6x into target/verify for a human to actually look at.
#[test]
fn field_screenshot_for_eyeballing() {
    let mut tw = TestWorld::infinite()
        .seed(20260711)
        .name("hole_field")
        .build();
    tw.tick_n(8); // stream chunks around spawn
    // pin the clock to midday so the shot is judged in flat daylight (settling ticks
    // advance time, so pin, settle, pin again — the visuals.rs pattern)
    let noon = fdoom::core::updater::DAY_LENGTH / 3;
    tw.g.set_time(noon);
    tw.tick_n(2);
    tw.g.set_time(noon);
    let lvl = tw.g.current_level;
    let (ptx, pty) = tw.player_tile();

    // row of pits stage 0..2 (twice, to show outline variety) above the player
    for i in 0..6 {
        let (tx, ty) = (ptx - 3 + i, pty - 3);
        tw.place_at("Dug Pit", tx, ty);
        tw.g.level_mut(lvl)
            .set_data(tx, ty, i % (depth::MAX_STAGE + 1));
    }
    // row of chasms below that
    for i in 0..4 {
        tw.place_at("Chasm", ptx - 2 + i, pty - 1);
    }
    // a small deep-water inlet ringed by shallow water, to check the ragged fringe
    for dy in 0..3 {
        for dx in 0..5 {
            let name = if dy == 1 && (1..=3).contains(&dx) {
                "Deep Water"
            } else {
                "water"
            };
            tw.place_at(name, ptx - 2 + dx, pty + 2 + dy);
        }
    }

    let path = tw.screenshot("hole_render_field.png");
    assert!(path.exists());
    let pixels = tw.render();
    save_png(
        verify_path("hole_render_field_6x.png"),
        &pixels,
        screen::W as usize,
        screen::H as usize,
        6,
    );
}
