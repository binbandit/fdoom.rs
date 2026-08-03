//! Text/panel anchoring across screen sizes.
//!
//! The framebuffer is 288x192 at minimum but grows with the window (up to 640x400,
//! see `platform::logical_size_for_window`). Every screen here must center on the
//! *live* framebuffer, keep its panels inside it, and keep world-derived strings
//! (world names, item names, key names, book text) inside their panel.

use std::sync::{Arc, OnceLock};

use fdoom::core::game::Game;
use fdoom::gfx::sprite_sheet::SpriteSheet;
use fdoom::gfx::{Rectangle, Screen, color, font};
use fdoom::screen::display::Display;

/// The three sizes under test: classic, a mid window, and the platform's max.
const SIZES: [(i32, i32); 3] = [(288, 192), (384, 240), (640, 400)];

fn sheet() -> Arc<SpriteSheet> {
    static SHEET: OnceLock<Arc<SpriteSheet>> = OnceLock::new();
    SHEET
        .get_or_init(|| Arc::new(fdoom::assets::sprite_sheet()))
        .clone()
}

fn game(tag: &str) -> Game {
    fdoom::testutil::bare_game(&format!("textlayout_{tag}"))
}

/// Build a display *at* the given framebuffer size (displays lay their menus out in
/// their constructor, so `screen_size` has to be live first) and render one frame.
fn render_at(
    g: &mut Game,
    w: i32,
    h: i32,
    build: impl FnOnce(&mut Game) -> Box<dyn Display>,
) -> (Screen, Box<dyn Display>) {
    g.screen_size = (w, h);
    g.has_gui = true;
    let mut d = build(g);
    d.init(g);
    let mut screen = Screen::with_size(w, h, sheet());
    screen.clear(0);
    d.render(&mut screen, g);
    (screen, d)
}

fn screen_rect(w: i32, h: i32) -> Rectangle {
    Rectangle::new(0, 0, w, h, Rectangle::CORNER_DIMS)
}

fn contains(outer: &Rectangle, inner: &Rectangle) -> bool {
    inner.left() >= outer.left()
        && inner.top() >= outer.top()
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

/// Every menu a display owns must sit inside the framebuffer.
fn panel_problems(d: &dyn Display, w: i32, h: i32, what: &str) -> Vec<String> {
    let sr = screen_rect(w, h);
    let mut out = Vec::new();
    for (i, m) in d.base().menus.iter().enumerate() {
        if !m.should_render() {
            continue;
        }
        let b = m.get_bounds();
        if !contains(&sr, &b) {
            out.push(format!(
                "{what} @ {w}x{h}: menu {i} bounds ({},{})..({},{}) escape the screen",
                b.left(),
                b.top(),
                b.right(),
                b.bottom()
            ));
        }
    }
    out
}

fn assert_panels_inside(d: &dyn Display, w: i32, h: i32, what: &str) {
    let p = panel_problems(d, w, h, what);
    assert!(p.is_empty(), "{}", p.join("\n"));
}

/// Horizontal span of every pixel that is not the given background color.
fn drawn_x_span(screen: &Screen, bg: i32) -> Option<(i32, i32)> {
    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    for y in 0..screen.h {
        for x in 0..screen.w {
            if screen.pixels[(x + y * screen.w) as usize] != bg {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
    }
    (lo <= hi).then_some((lo, hi))
}

fn save(screen: &Screen, name: &str) {
    let path = fdoom::testutil::verify_path(name);
    // upscaled so the 8px font is legible when a human looks at these
    let scale = if screen.w <= 288 { 3 } else { 2 };
    fdoom::testutil::save_png(
        &path,
        &screen.pixels,
        screen.w as usize,
        screen.h as usize,
        scale,
    );
    println!("wrote {}", path.display());
}

/* ------------------------------------------------------------------ *
 * baseline capture (used to byte-compare the classic render)
 * ------------------------------------------------------------------ */

fn baseline_dir() -> std::path::PathBuf {
    let dir = std::path::PathBuf::from("target/verify/baseline_now");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn record(screen: &Screen, name: &str) {
    let bytes: Vec<u8> = screen.pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
    std::fs::write(baseline_dir().join(format!("{name}.bin")), bytes).unwrap();
}

/* ------------------------------------------------------------------ *
 * the screens under test
 * ------------------------------------------------------------------ */

fn seed_worlds(g: &Game, names: &[&str]) {
    let saves = g.game_dir.join("saves");
    for n in names {
        let dir = saves.join(n);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("Game{}", fdoom::saveload::save::EXTENSION)),
            "2.2.0,100,0,0,0\n",
        )
        .unwrap();
    }
}

const LONG_WORLD: &str = "AN ABSURDLY LONG WORLD NAME THAT NOBODY SHOULD TYPE";

/// Renders every affected screen at all three sizes, asserts each screen keeps its
/// panels on-screen, and writes the PNGs a human reviews in `target/verify/`.
#[test]
fn every_screen_keeps_its_panels_on_screen() {
    for (w, h) in SIZES {
        let tag = format!("{w}x{h}");

        // --- book page ---
        let mut g = game(&format!("book_{tag}"));
        let (s, d) = render_at(&mut g, w, h, |g| {
            Box::new(fdoom::screen::book_display::BookDisplay::with_title(
                g,
                Some(fdoom::assets::PROSPECTORS_NOTE_TXT),
                false,
            ))
        });
        assert_panels_inside(&*d, w, h, "book");
        save(&s, &format!("textlayout_book_{tag}.png"));
        if (w, h) == (288, 192) {
            record(&s, "book");
        }

        // --- key bindings ---
        let mut g = game(&format!("keys_{tag}"));
        let (s, d) = render_at(&mut g, w, h, |g| {
            Box::new(fdoom::screen::key_input_display::KeyInputDisplay::new(g))
        });
        assert_panels_inside(&*d, w, h, "keys");
        save(&s, &format!("textlayout_keys_{tag}.png"));
        if (w, h) == (288, 192) {
            record(&s, "keys");
        }

        // --- world select ---
        let mut g = game(&format!("worlds_{tag}"));
        seed_worlds(&g, &["alpha", "beta", LONG_WORLD]);
        let (s, d) = render_at(&mut g, w, h, |_g| {
            Box::new(fdoom::screen::world_select::WorldSelectDisplay::new())
        });
        assert_panels_inside(&*d, w, h, "world select");
        save(&s, &format!("textlayout_worlds_{tag}.png"));
        if (w, h) == (288, 192) {
            record(&s, "worlds");
        }

        // --- world select, short names only (control: nothing here overflowed
        // before, so this render must be byte-identical across the refactor) ---
        let mut g = game(&format!("worldsshort_{tag}"));
        seed_worlds(&g, &["alpha", "beta"]);
        let (s, d) = render_at(&mut g, w, h, |_g| {
            Box::new(fdoom::screen::world_select::WorldSelectDisplay::new())
        });
        assert_panels_inside(&*d, w, h, "world select (short)");
        save(&s, &format!("textlayout_worlds_short_{tag}.png"));
        if (w, h) == (288, 192) {
            record(&s, "worlds_short");
        }

        // --- splash ---
        let mut g = game(&format!("splash_{tag}"));
        let (mut s, mut d) = render_at(&mut g, w, h, |_g| {
            Box::new(fdoom::screen::splash_menu::SplashMenu::new())
        });
        // let it run past both reveal beats
        for _ in 0..70 {
            d.tick(&mut g);
        }
        s.clear(0);
        d.render(&mut s, &mut g);
        save(&s, &format!("textlayout_splash_{tag}.png"));
        if (w, h) == (288, 192) {
            record(&s, "splash");
        }

        // --- item list ---
        let mut g = game(&format!("items_{tag}"));
        let (s, d) = render_at(&mut g, w, h, |g| Box::new(item_list(g)));
        assert_panels_inside(&*d, w, h, "item list");
        save(&s, &format!("textlayout_items_{tag}.png"));
        if (w, h) == (288, 192) {
            record(&s, "items");
        }
    }
}

/// A bare `Display` wrapper around an `item_list_menu` (the module has no in-game
/// caller yet, but it is the shared item-list configuration).
struct ItemList(fdoom::screen::DisplayBase);

impl Display for ItemList {
    fn base(&self) -> &fdoom::screen::DisplayBase {
        &self.0
    }
    fn base_mut(&mut self) -> &mut fdoom::screen::DisplayBase {
        &mut self.0
    }
}

fn item_list(g: &Game) -> ItemList {
    let items: Vec<fdoom::item::Item> = [
        "wood_5",
        "stone_12",
        "raw pork_2",
        "gold lantern_1",
        "iron pickaxe",
    ]
    .iter()
    .map(|n| fdoom::item::registry::get(g, n))
    .collect();
    let entries = fdoom::screen::entry::item_entry::ItemEntry::use_items(&items);
    let menu = fdoom::screen::item_list_menu::new(g, entries, "Inventory");
    ItemList(fdoom::screen::DisplayBase::new(true, true, vec![menu]))
}

/* ------------------------------------------------------------------ *
 * centering
 * ------------------------------------------------------------------ */

#[test]
fn centered_text_centers_on_the_live_framebuffer() {
    for (w, h) in SIZES {
        let mut screen = Screen::with_size(w, h, sheet());
        screen.clear(0);
        font::draw_centered("CENTERED", &mut screen, h / 2, color::WHITE);
        let (lo, hi) = drawn_x_span(&screen, 0).expect("something was drawn");
        let mid = (lo + hi + 1) / 2;
        assert!(
            (mid - w / 2).abs() <= 4,
            "centered text midpoint {mid} should be within 4px of {} at {w}x{h} (span {lo}..{hi})",
            w / 2
        );
    }
}

/// A paragraph block (the `FontStyle` path with a per-line anchor) must center on the
/// live screen too — `configure_for_paragraph` used to bake the classic midpoint.
#[test]
fn centered_paragraphs_center_on_the_live_framebuffer() {
    for (w, h) in SIZES {
        let mut screen = Screen::with_size(w, h, sheet());
        screen.clear(0);
        let lines: Vec<String> = ["FIRST LINE".to_string(), "A MUCH LONGER SECOND".to_string()]
            .into_iter()
            .collect();
        let mut style = fdoom::gfx::FontStyle::new(color::WHITE);
        font::draw_paragraph(&lines, &mut screen, &mut style, 1);
        let (lo, hi) = drawn_x_span(&screen, 0).expect("something was drawn");
        let mid = (lo + hi + 1) / 2;
        assert!(
            (mid - w / 2).abs() <= 4,
            "paragraph midpoint {mid} should be within 4px of {} at {w}x{h}",
            w / 2
        );
    }
}

/// Only the axis a caller pins should stop following the screen; the other one still
/// centers. `set_y_pos` (used by `draw_centered`) must leave x screen-centered.
#[test]
fn pinning_one_axis_leaves_the_other_screen_centered() {
    for (w, h) in SIZES {
        let mut screen = Screen::with_size(w, h, sheet());
        screen.clear(0);
        fdoom::gfx::FontStyle::new(color::WHITE)
            .set_y_pos(24)
            .draw("PINNED Y", &mut screen);
        let (lo, hi) = drawn_x_span(&screen, 0).expect("something was drawn");
        assert!(
            ((lo + hi + 1) / 2 - w / 2).abs() <= 4,
            "x should still center at {w}x{h} (span {lo}..{hi})"
        );

        screen.clear(0);
        fdoom::gfx::FontStyle::new(color::WHITE)
            .set_x_pos(40)
            .draw("PINNED X", &mut screen);
        let rows: Vec<i32> = (0..screen.h)
            .filter(|y| (0..screen.w).any(|x| screen.pixels[(x + y * screen.w) as usize] != 0))
            .collect();
        let mid_y = (rows[0] + rows[rows.len() - 1] + 1) / 2;
        assert!(
            (mid_y - h / 2).abs() <= 5,
            "y should still center at {w}x{h} (rows {}..{})",
            rows[0],
            rows[rows.len() - 1]
        );
    }
}

/* ------------------------------------------------------------------ *
 * world/player-supplied strings must not escape their panel
 * ------------------------------------------------------------------ */

/// A world name long enough to overrun the screen gets ellipsized, at every size.
#[test]
fn long_world_names_ellipsize_instead_of_escaping_the_panel() {
    for (w, h) in SIZES {
        let mut g = game(&format!("longworld_{w}x{h}"));
        seed_worlds(&g, &[LONG_WORLD]);
        let (_s, d) = render_at(&mut g, w, h, |_g| {
            Box::new(fdoom::screen::world_select::WorldSelectDisplay::new())
        });
        assert_panels_inside(&*d, w, h, "world select (long name)");

        let row = d.base().menus[0].get_entries()[0]
            .borrow()
            .to_display_string(&g);
        assert!(
            font::text_width(&row) <= w,
            "world row {row:?} ({}px) must fit {w}px at {w}x{h}",
            font::text_width(&row)
        );
        // at 288 the name genuinely cannot fit, so it must be visibly truncated
        if w == 288 {
            assert!(row.ends_with(".."), "expected an ellipsis, got {row:?}");
        }
    }
}

/// Every key row fits the screen at every size, and the mapping column is never cut.
#[test]
fn key_binding_rows_fit_the_screen() {
    for (w, h) in SIZES {
        let mut g = game(&format!("keyrows_{w}x{h}"));
        let (_s, d) = render_at(&mut g, w, h, |g| {
            Box::new(fdoom::screen::key_input_display::KeyInputDisplay::new(g))
        });
        assert_panels_inside(&*d, w, h, "keys");

        for e in d.base().menus[0].get_entries() {
            let row = e.borrow().to_display_string(&g);
            let width = font::text_width(&row);
            assert!(
                width <= w,
                "key row {row:?} is {width}px wide, past the {w}px screen"
            );
            // the row is padded to the full usable width, so the mapping ends flush
            // against the right gutter — that is what the cursors need room for
            assert!(
                width <= w - 32,
                "key row {row:?} ({width}px) leaves no room for the selection cursors at {w}x{h}"
            );
        }
    }
}

/// A book page must wrap to the page box and never spill outside the paper panel.
#[test]
fn book_pages_wrap_inside_the_page_panel() {
    // a single unbroken word longer than the page is the pathological case
    let brutal = format!("{}\0short page", "SUPERCALIFRAGILISTIC".repeat(6));
    for (w, h) in SIZES {
        let mut g = game(&format!("bookwrap_{w}x{h}"));
        let (_s, d) = render_at(&mut g, w, h, |g| {
            Box::new(fdoom::screen::book_display::BookDisplay::with_title(
                g,
                Some(&brutal),
                false,
            ))
        });
        assert_panels_inside(&*d, w, h, "book (long word)");

        // page menus start after the page-count menu; check every rendered line
        for m in d.base().menus.iter() {
            let inner = m.get_bounds();
            for e in m.get_entries() {
                let line = e.borrow().to_display_string(&g);
                assert!(
                    font::text_width(&line) <= inner.width(),
                    "book line {line:?} ({}px) escapes its {}px panel at {w}x{h}",
                    font::text_width(&line),
                    inner.width()
                );
            }
        }
    }
}

/// Book pages are light paper with dark text. A builder that picks frame colours
/// must therefore render an OPAQUE panel — it used to fall through to the
/// smoked-glass darkening, leaving near-black glyphs on a near-black page, so the
/// four field-notes journals shipped unreadable.
#[test]
fn book_pages_are_readable_paper_not_smoked_glass() {
    let mut g = game("book_fill");
    let (screen, _d) = render_at(&mut g, 288, 192, |g| {
        Box::new(fdoom::screen::book_display::BookDisplay::with_title(
            g,
            Some(fdoom::assets::PROSPECTORS_NOTE_TXT),
            false,
        ))
    });

    // sample the page interior, well inside the frame
    let (w, h) = (288i32, 192i32);
    let (mut bright, mut total) = (0, 0);
    for y in (h / 3)..(h * 2 / 3) {
        for x in (w / 3)..(w * 2 / 3) {
            let p = screen.pixels[(y * w + x) as usize];
            let lum = ((p >> 16) & 0xff) + ((p >> 8) & 0xff) + (p & 0xff);
            if lum > 300 {
                bright += 1;
            }
            total += 1;
        }
    }
    let frac = bright as f32 / total as f32;
    assert!(
        frac > 0.5,
        "book page interior should be light paper; only {:.0}% of sampled pixels are bright",
        frac * 100.0
    );
}
