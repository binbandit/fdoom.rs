//! Port of `fdoom.gfx.Insets`.

use super::{Dimension, Rectangle};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Insets {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Insets {
    pub fn uniform(dist: i32) -> Insets {
        Insets::new(dist, dist, dist, dist)
    }

    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Insets {
        Insets {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn add_to_rect(&self, r: &Rectangle) -> Rectangle {
        Rectangle::new(
            r.left() - self.left,
            r.top() - self.top,
            r.right() + self.right,
            r.bottom() + self.bottom,
            Rectangle::CORNERS,
        )
    }

    pub fn subtract_from_rect(&self, r: &Rectangle) -> Rectangle {
        Rectangle::new(
            r.left() + self.left,
            r.top() + self.top,
            r.right() - self.right,
            r.bottom() - self.bottom,
            Rectangle::CORNERS,
        )
    }

    pub fn add_to_dim(&self, d: Dimension) -> Dimension {
        Dimension::new(
            d.width + self.left + self.right,
            d.height + self.top + self.bottom,
        )
    }

    pub fn subtract_from_dim(&self, d: Dimension) -> Dimension {
        Dimension::new(
            d.width - self.left - self.right,
            d.height - self.top - self.bottom,
        )
    }

    pub fn add_insets(&self, s: &Insets) -> Insets {
        Insets::new(
            self.left + s.left,
            self.top + s.top,
            self.right + s.right,
            self.bottom + s.bottom,
        )
    }

    pub fn subtract_insets(&self, s: &Insets) -> Insets {
        Insets::new(
            self.left - s.left,
            self.top - s.top,
            self.right - s.right,
            self.bottom - s.bottom,
        )
    }
}
