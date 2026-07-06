//! Port of `fdoom.gfx.Dimension`.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dimension {
    pub width: i32,
    pub height: i32,
}

impl Dimension {
    pub fn new(width: i32, height: i32) -> Dimension {
        Dimension { width, height }
    }
}

impl std::fmt::Display for Dimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}
