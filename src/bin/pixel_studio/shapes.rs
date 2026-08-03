//! Brush geometry: the point sets the line and rectangle tools stamp.
//!
//! Pure `(x, y)` generation in window-relative coordinates — clipping and painting
//! happen in `studio::paint`. The same functions produce the drag-preview ghost and
//! the committed stroke, so what the artist sees is exactly what lands.

/// Bresenham line, inclusive.
pub(crate) fn line_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    let mut pts = Vec::new();
    loop {
        pts.push((x, y));
        if x == x1 && y == y1 {
            return pts;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

pub(crate) fn rect_points(x0: i32, y0: i32, x1: i32, y1: i32, fill: bool) -> Vec<(i32, i32)> {
    let (ax, bx) = (x0.min(x1), x0.max(x1));
    let (ay, by) = (y0.min(y1), y0.max(y1));
    let mut pts = Vec::new();
    for y in ay..=by {
        for x in ax..=bx {
            if fill || x == ax || x == bx || y == ay || y == by {
                pts.push((x, y));
            }
        }
    }
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A line always includes both endpoints, even a zero-length one.
    #[test]
    fn lines_are_inclusive_at_both_ends() {
        assert_eq!(line_points(2, 3, 2, 3), vec![(2, 3)]);
        let pts = line_points(0, 0, 3, 0);
        assert_eq!(pts, vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
        let pts = line_points(0, 0, 0, -2);
        assert_eq!(pts, vec![(0, 0), (0, -1), (0, -2)]);
    }

    /// A 45-degree line steps one pixel per axis per point, in either direction.
    #[test]
    fn diagonals_step_evenly_and_reverse_symmetrically() {
        assert_eq!(
            line_points(0, 0, 3, 3),
            vec![(0, 0), (1, 1), (2, 2), (3, 3)]
        );
        let fwd = line_points(0, 0, 7, 3);
        let mut back = line_points(7, 3, 0, 0);
        back.reverse();
        assert_eq!(fwd.len(), back.len(), "same pixel count in both directions");
        assert_eq!(fwd.first(), back.first());
        assert_eq!(fwd.last(), back.last());
    }

    /// Rect outlines are hollow; fills are the whole area. Corners are not doubled.
    #[test]
    fn rects_outline_and_fill() {
        let fill = rect_points(0, 0, 2, 2, true);
        assert_eq!(fill.len(), 9);
        let outline = rect_points(0, 0, 2, 2, false);
        assert_eq!(outline.len(), 8, "3x3 minus the centre");
        assert!(!outline.contains(&(1, 1)));
        let mut sorted = outline.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), outline.len(), "no duplicated corners");
    }

    /// Dragging in any direction gives the same rect — anchor and cursor commute.
    #[test]
    fn rects_normalise_their_corners() {
        for fill in [false, true] {
            let a = rect_points(5, 7, 1, 2, fill);
            let b = rect_points(1, 2, 5, 7, fill);
            assert_eq!(a, b);
        }
        assert_eq!(rect_points(4, 4, 4, 4, false), vec![(4, 4)], "1x1 drag");
    }
}
