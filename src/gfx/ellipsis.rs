//! Port of `fdoom.gfx.Ellipsis` — the animated "..." used on loading/waiting screens.
//!
//! Java modeled this as two small class hierarchies (Ellipsis kinds x DotUpdater kinds);
//! here both are enums. `update_and_get` takes the current tick count because Java's
//! `TickUpdater` read `Updater.tickCount` statically.

use crate::core::updater::NORM_SPEED;

#[derive(Debug, Clone)]
enum Kind {
    /// Java `SequentialEllipsis` — one dot walks: ".  ", " . ", "  .".
    Sequential,
    /// Java `SmoothEllipsis` — dots fill up then empty: ".", "..", "...", "..", ...
    Smooth { dots: [char; 3] },
}

#[derive(Debug, Clone)]
enum Method {
    /// Java `TickUpdater` — advances with game ticks.
    Tick { last_tick: i32, started: bool },
    /// Java `TimeUpdater` — advances with wall-clock milliseconds.
    Time { last_time: std::time::Instant },
    /// Java `CallUpdater` — advances once per call.
    Call,
}

#[derive(Debug, Clone)]
pub struct Ellipsis {
    kind: Kind,
    method: Method,
    interval_count: i32,
    cur_interval: i32,
    count_per_interval: i32,
    counter: i32,
}

impl Ellipsis {
    /// Java `new SmoothEllipsis(new TickUpdater())`.
    pub fn smooth_tick() -> Ellipsis {
        Ellipsis::new(
            Kind::Smooth {
                dots: [' ', ' ', ' '],
            },
            Method::Tick {
                last_tick: 0,
                started: false,
            },
            NORM_SPEED,
        )
    }

    /// Java `new SmoothEllipsis()` (TimeUpdater, 750ms per cycle).
    pub fn smooth_time() -> Ellipsis {
        Ellipsis::new(
            Kind::Smooth {
                dots: [' ', ' ', ' '],
            },
            Method::Time {
                last_time: std::time::Instant::now(),
            },
            750,
        )
    }

    /// Java `new SequentialEllipsis()` (CallUpdater, normSpeed*2/3 calls per cycle).
    pub fn sequential_call() -> Ellipsis {
        Ellipsis::new(Kind::Sequential, Method::Call, NORM_SPEED * 2 / 3)
    }

    fn new(kind: Kind, method: Method, count_per_cycle: i32) -> Ellipsis {
        let interval_count = match &kind {
            Kind::Sequential => 3,
            Kind::Smooth { dots } => dots.len() as i32 * 2,
        };
        let count_per_interval =
            1.max((count_per_cycle as f32 / interval_count as f32).round() as i32);
        Ellipsis {
            kind,
            method,
            interval_count,
            cur_interval: 0,
            count_per_interval,
            counter: 0,
        }
    }

    /// Java `updateAndGet()`; `tick_count` feeds the Tick method (others ignore it).
    pub fn update_and_get(&mut self, tick_count: i32) -> String {
        let amt = match &mut self.method {
            Method::Tick { last_tick, started } => {
                if !*started {
                    *started = true;
                    *last_tick = tick_count;
                }
                let passed = tick_count - *last_tick;
                *last_tick = tick_count;
                passed
            }
            Method::Time { last_time } => {
                let now = std::time::Instant::now();
                let diff_millis = now.duration_since(*last_time).as_millis() as i32;
                *last_time = now;
                diff_millis
            }
            Method::Call => 1,
        };
        self.inc_counter(amt);
        self.get()
    }

    fn inc_counter(&mut self, amt: i32) {
        self.counter += amt;
        let intervals = self.counter / self.count_per_interval;
        if intervals > 0 {
            self.inc_interval(intervals);
            self.counter -= intervals * self.count_per_interval;
        }
    }

    fn inc_interval(&mut self, amt: i32) {
        // Smooth updates its dot chars on every interval passed (Java nextInterval callback).
        let interval_count = self.interval_count;
        if let Kind::Smooth { dots } = &mut self.kind {
            for i in self.cur_interval + 1..=self.cur_interval + amt {
                let interval = i % interval_count;
                let epos = (interval % dots.len() as i32) as usize;
                let set = if interval < interval_count / 2 {
                    '.'
                } else {
                    ' '
                };
                dots[epos] = set;
            }
        }
        self.cur_interval += amt;
        self.cur_interval %= self.interval_count;
    }

    fn get(&self) -> String {
        match &self.kind {
            Kind::Sequential => {
                let mut dots = String::new();
                for i in 0..self.interval_count {
                    dots.push(if self.cur_interval == i { '.' } else { ' ' });
                }
                dots
            }
            Kind::Smooth { dots } => dots.iter().collect(),
        }
    }
}
