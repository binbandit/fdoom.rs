//! Port of `fdoom.screen.RelPos` ("relative position").

use crate::core::my_utils::clamp;
use crate::gfx::{Dimension, Point, Rectangle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RelPos {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl RelPos {
    pub const VALUES: [RelPos; 9] = [
        RelPos::TopLeft,
        RelPos::Top,
        RelPos::TopRight,
        RelPos::Left,
        RelPos::Center,
        RelPos::Right,
        RelPos::BottomLeft,
        RelPos::Bottom,
        RelPos::BottomRight,
    ];

    pub fn ordinal(self) -> i32 {
        self as i32
    }

    pub fn x_index(self) -> i32 {
        self.ordinal() % 3
    }

    pub fn y_index(self) -> i32 {
        self.ordinal() / 3
    }

    pub fn get_pos(x_index: i32, y_index: i32) -> RelPos {
        Self::VALUES[(clamp(x_index, 0, 2) + clamp(y_index, 0, 2) * 3) as usize]
    }

    pub fn get_opposite(self) -> RelPos {
        let nx = -(self.x_index() - 1) + 1;
        let ny = -(self.y_index() - 1) + 1;
        Self::get_pos(nx, ny)
    }

    /// Java `positionRect(Dimension, Point)` — positions a rect of the given size around
    /// the anchor point (the doubled-size bounds trick aligns it to a point).
    pub fn position_rect_point(self, rect_size: Dimension, anchor: Point) -> Point {
        let bounds = Rectangle::new(
            anchor.x,
            anchor.y,
            rect_size.width * 2,
            rect_size.height * 2,
            Rectangle::CENTER_DIMS,
        );
        self.position_rect_in(rect_size, &bounds)
    }

    /// Java `positionRect(Dimension, Point, Rectangle dummy)`.
    pub fn position_rect(self, rect_size: Dimension, anchor: Point) -> Rectangle {
        let pos = self.position_rect_point(rect_size, anchor);
        let mut dummy = Rectangle::default();
        dummy.set_size(rect_size.width, rect_size.height, RelPos::TopLeft);
        dummy.set_position_point(pos, RelPos::TopLeft);
        dummy
    }

    /// Java `positionRect(Dimension, Rectangle container)` — top-left corner of a rect of
    /// the given size at this relative position within the container.
    pub fn position_rect_in(self, rect_size: Dimension, container: &Rectangle) -> Point {
        let mut tlcorner = container.center();
        tlcorner.x +=
            ((self.x_index() - 1) * container.width() / 2) - (self.x_index() * rect_size.width / 2);
        tlcorner.y += ((self.y_index() - 1) * container.height() / 2)
            - (self.y_index() * rect_size.height / 2);
        tlcorner
    }

    /// Java `positionRect(Dimension, Rectangle container, Rectangle dummy)`.
    pub fn position_rect_in_container(
        self,
        rect_size: Dimension,
        container: &Rectangle,
    ) -> Rectangle {
        let pos = self.position_rect_in(rect_size, container);
        let mut dummy = Rectangle::default();
        dummy.set_size(rect_size.width, rect_size.height, RelPos::TopLeft);
        dummy.set_position_point(pos, RelPos::TopLeft);
        dummy
    }
}
