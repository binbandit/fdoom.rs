//! Fixed geometry of the studio's internal 960x720 frame, its chrome colours, and
//! the pane-scroll maths shared by the renderer and hit-testing.
//!
//! Everything here is a compile-time measurement of the UI shell: where the panes
//! sit, which slots the palette banks occupy, what the chrome is painted with. The
//! frame is a fixed-size buffer that `app` blits to the window scaled and centered,
//! so these numbers never depend on the real window size.

use std::time::Duration;

pub(crate) const VIEW_W: i32 = 960;
pub(crate) const VIEW_H: i32 = 720;

pub(crate) const PANE_X: i32 = 8;
pub(crate) const PANE_Y: i32 = 56;
pub(crate) const PANE_W: i32 = 512; // sheet browser: 256 sheet px at 2x; dir mode: file list
pub(crate) const PANE_H: i32 = 512;
pub(crate) const ROW_H: i32 = 12; // file-list line height

pub(crate) const RX: i32 = 536; // right pane origin
pub(crate) const CANVAS_Y: i32 = 56;
pub(crate) const CANVAS_MAX: i32 = 384; // canvas viewport is CANVAS_MAX x CANVAS_MAX
pub(crate) const PAL_A_Y: i32 = 450;
pub(crate) const PAL_B_Y: i32 = 480;
pub(crate) const RECENT_Y: i32 = 520;
pub(crate) const RGB_Y: i32 = 542;
pub(crate) const PREVIEW_Y: i32 = 568;
pub(crate) const SWATCH_X: i32 = RX + 88;

pub(crate) const BG: u32 = 0x14181C;
pub(crate) const PANEL: u32 = 0x0C0F13;
pub(crate) const GRID: u32 = 0x262C34;
pub(crate) const GRID_MAJOR: u32 = 0x3E4854;
pub(crate) const ACCENT: u32 = 0xFFD24A;
pub(crate) const TXT: i32 = 555; // readable-color text values for draw_text
pub(crate) const TXT_DIM: i32 = 333;
pub(crate) const TXT_WARN: i32 = 540;

/// Game walk cadence: mobs flip frames on `walk_dist >> 3` — about every 8 ticks at
/// 60 tps, so ~133 ms per animation frame.
pub(crate) const ANIM_FRAME: Duration = Duration::from_millis(133);

/// Sheet-pane scroll for sheets larger than the 256px view: keep the window visible.
pub(crate) fn clamp_scroll(sel_px: i32, block: i32, dim: i32, view: i32) -> i32 {
    if dim <= view {
        0
    } else {
        (sel_px + block / 2 - view / 2).clamp(0, dim - view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sheet that fits the pane never scrolls, whatever the window is doing.
    #[test]
    fn small_sheets_never_scroll() {
        assert_eq!(clamp_scroll(0, 16, 256, 256), 0);
        assert_eq!(clamp_scroll(200, 16, 100, 256), 0);
    }

    /// A bigger sheet centres the window and clamps at both edges, so the selection
    /// is always inside the visible pane.
    #[test]
    fn large_sheets_centre_the_window_and_clamp() {
        // window at the very start: clamped to the top-left
        assert_eq!(clamp_scroll(0, 16, 512, 256), 0);
        // window in the middle: centred (sel + block/2 - view/2)
        assert_eq!(clamp_scroll(256, 16, 512, 256), 256 + 8 - 128);
        // window at the far edge: clamped so the pane stays full
        assert_eq!(clamp_scroll(500, 16, 512, 256), 256);
    }
}
