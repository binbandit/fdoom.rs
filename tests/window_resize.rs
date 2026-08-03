//! The window can be dragged smaller than the logical screen (which clamps at
//! 288x192) — every one of those sizes used to crash the blit.
use fdoom::platform::{blit_scaled, logical_size_for_window};

/// Drive the real blit for a window size the way `redraw` does.
fn blit_window(win_w: i32, win_h: i32) {
    let (scale, lw, lh) = logical_size_for_window(win_w, win_h);
    let pixels = vec![0x336699i32; (lw * lh) as usize];
    let mut buffer = vec![0u32; (win_w.max(0) * win_h.max(0)) as usize];
    let (ww, hh) = (lw * scale, lh * scale);
    let (xo, yo) = ((win_w - ww) / 2, (win_h - hh) / 2);
    blit_scaled(&mut buffer, win_w, win_h, &pixels, lw, scale, xo, yo);
}

#[test]
fn any_window_size_blits_without_panicking() {
    // 1x1 is the window's declared minimum inner size, so every size from there up
    // is reachable by dragging an edge.
    for w in [1, 2, 17, 100, 287, 288, 289, 431, 640, 864, 1153, 1920] {
        for h in [1, 2, 17, 100, 191, 192, 193, 337, 400, 576, 801, 1080] {
            blit_window(w, h);
        }
    }
}

#[test]
fn a_window_smaller_than_the_logical_screen_is_cropped_not_corrupted() {
    // 200x150 < 288x192: the logical screen clamps up, so offsets go negative.
    let (scale, lw, lh) = logical_size_for_window(200, 150);
    assert_eq!((scale, lw, lh), (1, 288, 192), "logical size clamps up");
    let pixels = vec![0x00FF00i32; (lw * lh) as usize];
    let mut buffer = vec![0xDEADBEEFu32; (200 * 150) as usize];
    let (xo, yo) = ((200 - lw) / 2, (150 - lh) / 2);
    assert!(xo < 0 && yo < 0, "offsets are negative for a small window");
    blit_scaled(&mut buffer, 200, 150, &pixels, lw, scale, xo, yo);
    // the whole window is covered by cropped content — nothing left unwritten,
    // nothing written past the end
    assert!(
        buffer.iter().all(|p| *p == 0x00FF00),
        "every visible pixel comes from the framebuffer"
    );
}

#[test]
fn a_window_larger_than_the_logical_screen_letterboxes() {
    let win = (900, 700);
    let (scale, lw, lh) = logical_size_for_window(win.0, win.1);
    let pixels = vec![0x00FF00i32; (lw * lh) as usize];
    let mut buffer = vec![0u32; (win.0 * win.1) as usize];
    let (ww, hh) = (lw * scale, lh * scale);
    let (xo, yo) = ((win.0 - ww) / 2, (win.1 - hh) / 2);
    blit_scaled(&mut buffer, win.0, win.1, &pixels, lw, scale, xo, yo);
    let lit = buffer.iter().filter(|p| **p == 0x00FF00).count() as i32;
    assert_eq!(lit, ww * hh, "exactly the scaled image is drawn, centered");
}
