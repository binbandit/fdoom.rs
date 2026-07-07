//! Rare world events (docs/ROADMAP.md "Rare world events") — original post-port content,
//! no Java counterpart.
//!
//! The schedule is a *pure* function of `(world_seed, day_number)`: nothing about which
//! day hosts which event is stored or saved. Each day rolls at most one event (roughly
//! one day in [`EVENT_DAY_ODDS`]) via the same SplitMix64 avalanche the infinite terrain
//! uses, so the calendar is reproducible per seed. `day_number` is the only stateful
//! input: [`EventState`] counts day-clock wraps (`tick_count` falling back toward 0 —
//! both the natural `set_time` midnight wrap and the in-bed day skip drop it that way),
//! starting from 0 each session.
//!
//! Implemented events:
//! - **Hollow Night** — announced by an unnatural stillness at dusk; while the night
//!   lasts, every gravestone random-tick rolls a crumble chance far above the normal
//!   once-per-night odds (see `level::tile::grave_stone`). The following seven days are
//!   a "quiet week": grave decay is suppressed entirely, derived purely from the
//!   schedule ([`grave_decay_suppressed`]) — surviving the night needs no bookkeeping.
//! - **Aurora** — shimmering lights announced at dusk. v1's world-wide mechanical
//!   effect: mob spawning pauses for the night (one gate in `level::try_spawn`).
//!   [`mobs_docile`] is exposed for the eventual "hostiles ignore the player" hook,
//!   which needs entity-side wiring that is out of scope here.

use crate::core::game::Game;
use crate::core::updater::Time;
use crate::level::infinite_gen;

/// One day in this many is an event day.
pub const EVENT_DAY_ODDS: u64 = 6;

/// Days of grave-decay suppression after a Hollow Night.
pub const QUIET_WEEK_DAYS: i32 = 7;

/// Hash salt separating the event calendar from every terrain-generation stream.
const EVENT_SALT: u64 = 0x0E7E17;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldEvent {
    HollowNight,
    Aurora,
}

/// The one stateful piece: the session's day counter plus the previous-tick snapshots
/// used to detect day wraps and time-of-day transitions. Deliberately not saved —
/// the schedule itself is pure, and a reload simply restarts the calendar at day 0.
#[derive(Debug, Default, Clone)]
pub struct EventState {
    /// Days completed since the session started (0 = the first day, which never hosts
    /// an event, so fresh worlds always get a calm first night).
    pub day_number: i32,
    prev_tick_count: i32,
    prev_time: Option<Time>,
}

/// The event hosted by `day` for `world_seed`, if any. Pure; at most one event per day
/// by construction. Day 0 (and anything earlier) is always calm.
pub fn event_for_day(world_seed: i64, day: i32) -> Option<WorldEvent> {
    if day <= 0 {
        return None;
    }
    let h = infinite_gen::hash(world_seed, EVENT_SALT, day, 0);
    if h % EVENT_DAY_ODDS != 0 {
        return None;
    }
    Some(if (h >> 8) % 2 == 0 {
        WorldEvent::HollowNight
    } else {
        WorldEvent::Aurora
    })
}

/// Today's event, if any.
pub fn current_event(g: &Game) -> Option<WorldEvent> {
    event_for_day(g.world_seed, g.events.day_number)
}

/// True during the night of a Hollow Night day — gravestone crumbling accelerates.
pub fn hollow_night_active(g: &Game) -> bool {
    g.get_time() == Time::Night && current_event(g) == Some(WorldEvent::HollowNight)
}

/// True during the night of an Aurora day.
pub fn aurora_active(g: &Game) -> bool {
    g.get_time() == Time::Night && current_event(g) == Some(WorldEvent::Aurora)
}

/// Aurora nights calm the world's hostiles. v1 consumes this as a spawn pause
/// (`level::try_spawn`); the target-acquisition hook is future entity-side work.
pub fn mobs_docile(g: &Game) -> bool {
    aurora_active(g)
}

/// Quiet week: no grave decay if a Hollow Night fell within the last
/// [`QUIET_WEEK_DAYS`] days. Pure derivation from the schedule — see
/// [`grave_decay_suppressed_on`].
pub fn grave_decay_suppressed(g: &Game) -> bool {
    grave_decay_suppressed_on(g.world_seed, g.events.day_number)
}

/// Pure form of [`grave_decay_suppressed`]: was any of the `QUIET_WEEK_DAYS` days
/// before `day` a Hollow Night?
pub fn grave_decay_suppressed_on(world_seed: i64, day: i32) -> bool {
    (1..=QUIET_WEEK_DAYS)
        .any(|back| event_for_day(world_seed, day - back) == Some(WorldEvent::HollowNight))
}

/// Per-tick scheduler hook (called once from `Game::tick`, right after the day clock
/// advances): counts day wraps and fires the dusk/dawn notification cues on time-of-day
/// transitions. While a menu is open the day clock is frozen, so no cues fire.
pub fn tick(g: &mut Game) {
    // A backwards jump of the day clock is a day boundary: `set_time` wraps past
    // DAY_LENGTH to 0, and sleeping through midnight resets `tick_count` directly.
    if g.tick_count < g.events.prev_tick_count {
        g.events.day_number += 1;
    }
    g.events.prev_tick_count = g.tick_count;

    let time = g.get_time();
    if g.events.prev_time == Some(time) {
        return;
    }
    g.events.prev_time = Some(time);

    match time {
        Time::Evening => match current_event(g) {
            Some(WorldEvent::HollowNight) => g.notify_all("The evening is unnaturally still..."),
            Some(WorldEvent::Aurora) => g.notify_all("Pale lights shimmer at the horizon..."),
            None => {}
        },
        Time::Night => {
            if current_event(g) == Some(WorldEvent::Aurora) {
                g.notify_all("An aurora ripples across the sky.");
            }
        }
        Time::Morning => match event_for_day(g.world_seed, g.events.day_number - 1) {
            Some(WorldEvent::HollowNight) => g.notify_all("Dawn breaks. The graves lie quiet."),
            Some(WorldEvent::Aurora) => g.notify_all("The aurora fades with the dawn."),
            None => {}
        },
        Time::Day => {}
    }
}
