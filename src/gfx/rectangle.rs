//! Port of `fdoom.gfx.Rectangle`.

use super::{Dimension, Point};
use crate::screen::rel_pos::RelPos;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rectangle {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Rectangle {
    pub const CORNER_DIMS: i32 = 0;
    pub const CORNERS: i32 = 1;
    pub const CENTER_DIMS: i32 = 2;

    /// Java `new Rectangle(x, y, x1, y1, type)`.
    pub fn new(x: i32, y: i32, x1: i32, y1: i32, mut rect_type: i32) -> Rectangle {
        if !(0..=2).contains(&rect_type) {
            rect_type = 0;
        }
        let mut r = Rectangle::default();
        if rect_type != Self::CENTER_DIMS {
            r.x = x;
            r.y = y;
        } else {
            r.x = x - x1 / 2;
            r.y = y - y1 / 2;
        }
        if rect_type != Self::CORNERS {
            r.w = x1;
            r.h = y1;
        } else {
            r.w = x1 - x;
            r.h = y1 - y;
        }
        r
    }

    /// Java `new Rectangle(Point, Dimension)` / `new Rectangle(isCenter, Point, Dimension)`.
    pub fn from_point(is_center: bool, p: Point, d: Dimension) -> Rectangle {
        Rectangle::new(
            p.x,
            p.y,
            d.width,
            d.height,
            if is_center { Self::CENTER_DIMS } else { Self::CORNER_DIMS },
        )
    }

    pub fn left(&self) -> i32 {
        self.x
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn top(&self) -> i32 {
        self.y
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn width(&self) -> i32 {
        self.w
    }
    pub fn height(&self) -> i32 {
        self.h
    }

    pub fn center(&self) -> Point {
        Point::new(self.x + self.w / 2, self.y + self.h / 2)
    }
    pub fn size(&self) -> Dimension {
        Dimension::new(self.w, self.h)
    }

    pub fn position(&self, rel_pos: RelPos) -> Point {
        let mut p = Point::new(self.x, self.y);
        p.x += rel_pos.x_index() * self.w / 2;
        p.y += rel_pos.y_index() * self.h / 2;
        p
    }

    pub fn intersects(&self, other: &Rectangle) -> bool {
        !(self.left() > other.right()
            || other.left() > self.right()
            || self.bottom() < other.top()
            || other.bottom() < self.top())
    }

    pub fn set_position(&mut self, x: i32, y: i32, rel_pos: RelPos) {
        self.x = x - rel_pos.x_index() * self.w / 2;
        self.y = y - rel_pos.y_index() * self.h / 2;
    }

    pub fn set_position_point(&mut self, p: Point, rel_pos: RelPos) {
        self.set_position(p.x, p.y, rel_pos);
    }

    pub fn translate(&mut self, xoff: i32, yoff: i32) {
        self.x += xoff;
        self.y += yoff;
    }

    pub fn set_size(&mut self, width: i32, height: i32, anchor: RelPos) {
        let p = self.position(anchor);
        self.w = width;
        self.h = height;
        self.set_position_point(p, anchor);
    }
}
