//! Port of `fdoom.core.Updater` — the tick constants and time-of-day model. The `tick()`
//! logic itself lives on `Game` (see `core::game`), since all the Java statics are fields
//! there.

/// Ticks per second (Java `Updater.normSpeed`).
pub const NORM_SPEED: i32 = 60;

/// Length of a game day in ticks (Java `Updater.dayLength`).
pub const DAY_LENGTH: i32 = 64800;

/// When the player "wakes up" in the morning (Java `Updater.sleepEndTime`).
pub const SLEEP_END_TIME: i32 = DAY_LENGTH / 8;

/// When the player is allowed to sleep (Java `Updater.sleepStartTime`).
pub const SLEEP_START_TIME: i32 = DAY_LENGTH / 2 + DAY_LENGTH / 8;

/// Java `Updater.Time`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Time {
    Morning,
    Day,
    Evening,
    Night,
}

impl Time {
    pub const VALUES: [Time; 4] = [Time::Morning, Time::Day, Time::Evening, Time::Night];

    /// Java `Time.tickTime`.
    pub fn tick_time(self) -> i32 {
        match self {
            Time::Morning => 0,
            Time::Day => DAY_LENGTH / 4,
            Time::Evening => DAY_LENGTH / 2,
            Time::Night => DAY_LENGTH / 4 * 3,
        }
    }

    pub fn ordinal(self) -> i32 {
        self as i32
    }
}

impl std::fmt::Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
