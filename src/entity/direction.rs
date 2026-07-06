//! Port of `fdoom.entity.Direction`.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    None,
    Down,
    Up,
    Left,
    Right,
}

impl Direction {
    pub const VALUES: [Direction; 5] =
        [Direction::None, Direction::Down, Direction::Up, Direction::Left, Direction::Right];

    pub fn x(self) -> i32 {
        match self {
            Direction::Left => -1,
            Direction::Right => 1,
            _ => 0,
        }
    }

    pub fn y(self) -> i32 {
        match self {
            Direction::Down => 1,
            Direction::Up => -1,
            _ => 0,
        }
    }

    /// Java `getDirection(xd, yd)`.
    pub fn get_direction(xd: i32, yd: i32) -> Direction {
        if xd == 0 && yd == 0 {
            return Direction::None; // the attack was from the same entity, probably
        }
        if xd.abs() > yd.abs() {
            // the x distance is more prominent than the y distance
            if xd < 0 { Direction::Left } else { Direction::Right }
        } else if yd < 0 {
            Direction::Up
        } else {
            Direction::Down
        }
    }

    /// Java `getDirection(dir)` — from the `getDir()` index (-1..3).
    pub fn from_dir(dir: i32) -> Direction {
        Self::VALUES[(dir + 1) as usize]
    }

    /// Java `getDir()` — ordinal minus one (None = -1, Down = 0, ...).
    pub fn get_dir(self) -> i32 {
        self.ordinal() - 1
    }

    pub fn ordinal(self) -> i32 {
        self as i32
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Java enum toString: uppercase names
        let name = match self {
            Direction::None => "NONE",
            Direction::Down => "DOWN",
            Direction::Up => "UP",
            Direction::Left => "LEFT",
            Direction::Right => "RIGHT",
        };
        write!(f, "{name}")
    }
}
