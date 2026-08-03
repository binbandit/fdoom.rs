//! Undo/redo: a bounded pair of pixel-rect snapshots.
//!
//! Every edit records the *previous* contents of the rect it is about to touch —
//! usually the edit window, the whole image for a nudge — together with the view that
//! was showing at the time, so undoing a change also takes you back to where you made
//! it. Undo and redo are symmetric: popping one stack snapshots the current pixels
//! onto the other before restoring, which makes redo just undo in the other
//! direction. A fresh edit invalidates the redo branch.

use crate::color::Rgba;
use crate::image::Image;

use super::Studio;

/// How many edits are recoverable. Snapshots are whole rects, so this is a memory
/// bound as much as a usability one.
pub(crate) const UNDO_LEVELS: usize = 64;

/// Undo record: a pixel rect (usually the edit window; the whole image for nudges)
/// plus the view to restore alongside it.
pub(crate) struct Snap {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) px: Vec<Rgba>,
    pub(crate) view: (i32, i32, i32, i32), // bx, by, view_w, view_h
}

impl Snap {
    /// Snapshot `(x, y, w, h)` of `img`, clamped to the image so an oversized or
    /// off-edge rect records what exists rather than panicking.
    pub(crate) fn capture(
        img: &Image,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        view: (i32, i32, i32, i32),
    ) -> Snap {
        let x = x.clamp(0, img.w);
        let y = y.clamp(0, img.h);
        let w = w.min(img.w - x).max(0);
        let h = h.min(img.h - y).max(0);
        let mut px = Vec::with_capacity((w * h) as usize);
        for yy in y..y + h {
            for xx in x..x + w {
                px.push(img.pixel(xx, yy));
            }
        }
        Snap {
            x,
            y,
            w,
            h,
            px,
            view,
        }
    }
}

/// The undo and redo stacks, kept together so the invariants between them (a new
/// edit clears redo; the depth cap) live in one place.
#[derive(Default)]
pub(crate) struct History {
    pub(crate) undo: Vec<Snap>,
    pub(crate) redo: Vec<Snap>,
}

impl History {
    /// Record an edit. This is a new branch, so anything that was redoable is gone.
    pub(crate) fn push(&mut self, snap: Snap) {
        self.undo.push(snap);
        self.redo.clear();
        if self.undo.len() > UNDO_LEVELS {
            self.undo.remove(0);
        }
    }

    /// Drop both stacks — used whenever the document underneath them is replaced.
    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

impl Studio {
    pub(crate) fn push_undo_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let view = (self.bx, self.by, self.view_w, self.view_h);
        let snap = Snap::capture(&self.img, x, y, w, h, view);
        self.history.push(snap);
    }

    pub(crate) fn push_undo_block(&mut self) {
        let (bx, by, bw, bh) = self.block_rect();
        self.push_undo_rect(bx, by, bw, bh);
    }

    /// Snapshot the current pixels of `s`'s rect (for the opposite stack).
    fn counter_snap(&self, s: &Snap) -> Snap {
        let view = (self.bx, self.by, self.view_w, self.view_h);
        Snap::capture(&self.img, s.x, s.y, s.w, s.h, view)
    }

    fn apply_snap(&mut self, s: &Snap) {
        for y in 0..s.h {
            for x in 0..s.w {
                self.put(s.x + x, s.y + y, s.px[(x + y * s.w) as usize]);
            }
        }
        (self.bx, self.by, self.view_w, self.view_h) = s.view;
        self.clamp_pan();
        self.dirty = true;
    }

    pub(crate) fn undo_pop(&mut self) {
        let Some(s) = self.history.undo.pop() else {
            self.status = "NOTHING TO UNDO".into();
            return;
        };
        let counter = self.counter_snap(&s);
        self.history.redo.push(counter);
        self.apply_snap(&s);
        self.status = format!("UNDO ({} LEFT, Y REDOES)", self.history.undo.len());
    }

    pub(crate) fn redo_pop(&mut self) {
        let Some(s) = self.history.redo.pop() else {
            self.status = "NOTHING TO REDO".into();
            return;
        };
        let counter = self.counter_snap(&s);
        self.history.undo.push(counter);
        self.apply_snap(&s);
        self.status = format!("REDO ({} LEFT)", self.history.redo.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: (i32, i32, i32, i32) = (0, 0, 16, 16);

    fn ramp(w: i32, h: i32) -> Image {
        let mut img = Image::blank(w, h);
        for y in 0..h {
            for x in 0..w {
                img.px[(x + y * w) as usize] = [x as u8, y as u8, 0, 255];
            }
        }
        img
    }

    fn restore(img: &mut Image, s: &Snap) {
        for y in 0..s.h {
            for x in 0..s.w {
                let i = ((s.x + x) + (s.y + y) * img.w) as usize;
                img.px[i] = s.px[(x + y * s.w) as usize];
            }
        }
    }

    /// A snapshot captures exactly its rect, and restoring it undoes any edit made
    /// inside that rect — pixel for pixel.
    #[test]
    fn capture_and_restore_round_trips_a_rect() {
        let mut img = ramp(16, 16);
        let before = img.px.clone();
        let snap = Snap::capture(&img, 4, 4, 8, 8, VIEW);
        assert_eq!(snap.px.len(), 64);
        assert_eq!(snap.px[0], [4, 4, 0, 255], "records the rect's own origin");

        for y in 4..12 {
            for x in 4..12 {
                img.px[(x + y * 16) as usize] = [9, 9, 9, 255];
            }
        }
        assert_ne!(img.px, before);
        restore(&mut img, &snap);
        assert_eq!(img.px, before, "restore is exact");
    }

    /// An oversized or off-edge rect clamps to what exists instead of panicking —
    /// the nudge path hands in the whole image, and `G` can leave a window hanging
    /// off the edge of a smaller file.
    #[test]
    fn capture_clamps_to_the_image() {
        let img = ramp(8, 8);
        let s = Snap::capture(&img, 6, 6, 99, 99, VIEW);
        assert_eq!((s.x, s.y, s.w, s.h), (6, 6, 2, 2));
        assert_eq!(s.px.len(), 4);

        let s = Snap::capture(&img, 99, 99, 4, 4, VIEW);
        assert_eq!((s.w, s.h), (0, 0), "fully outside captures nothing");
        assert!(s.px.is_empty());
    }

    /// The view rides along with the pixels, so undo returns you to where the edit
    /// was made rather than wherever you have since browsed to.
    #[test]
    fn snapshots_carry_their_view() {
        let img = ramp(8, 8);
        let s = Snap::capture(&img, 0, 0, 8, 8, (24, 32, 8, 16));
        assert_eq!(s.view, (24, 32, 8, 16));
    }

    /// The stack is capped: past the limit the oldest edit falls off, and the most
    /// recent `UNDO_LEVELS` remain in order.
    #[test]
    fn history_caps_at_the_undo_limit() {
        let img = ramp(8, 8);
        let mut h = History::default();
        for i in 0..UNDO_LEVELS + 10 {
            h.push(Snap::capture(&img, 0, 0, 1, 1, (i as i32, 0, 8, 8)));
        }
        assert_eq!(h.undo.len(), UNDO_LEVELS, "never grows past the cap");
        assert_eq!(h.undo[0].view.0, 10, "the oldest edits fell off the bottom");
        assert_eq!(h.undo.last().unwrap().view.0, (UNDO_LEVELS + 9) as i32);
    }

    /// A fresh edit invalidates the redo branch — you cannot redo onto a history
    /// that no longer leads there.
    #[test]
    fn a_fresh_edit_clears_the_redo_branch() {
        let img = ramp(8, 8);
        let mut h = History::default();
        h.redo.push(Snap::capture(&img, 0, 0, 1, 1, VIEW));
        h.redo.push(Snap::capture(&img, 0, 0, 1, 1, VIEW));
        assert_eq!(h.redo.len(), 2);

        h.push(Snap::capture(&img, 0, 0, 1, 1, VIEW));
        assert!(h.redo.is_empty(), "redo branch dropped");
        assert_eq!(h.undo.len(), 1);

        h.clear();
        assert!(h.undo.is_empty() && h.redo.is_empty());
    }
}
