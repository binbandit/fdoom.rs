//! Rare world events (docs/ROADMAP.md "Rare world events") — original post-port content,
//! no Java counterpart.
//!
//! The schedule is a *pure* function of `(world_seed, day_number)` (plus the real-world
//! [`Season`], see below): nothing about which day hosts which event is stored or saved.
//! Each day rolls at most one event (roughly one day in [`EVENT_DAY_ODDS`]) via the same
//! SplitMix64 avalanche the infinite terrain uses, so the calendar is reproducible per
//! seed. `day_number` is the only stateful input: [`EventState`] counts day-clock wraps
//! (`tick_count` falling back toward 0 — both the natural `set_time` midnight wrap and
//! the in-bed day skip drop it that way), starting from 0 each session.
//!
//! Implemented events:
//! - **Hollow Night** — announced by an unnatural stillness at dusk; while the night
//!   lasts, every gravestone random-tick rolls a crumble chance far above the normal
//!   once-per-night odds (see `level::tile::grave_stone`). The following seven days are
//!   a "quiet week": grave decay is suppressed entirely, derived purely from the
//!   schedule ([`grave_decay_suppressed`]) — surviving the night needs no bookkeeping.
//! - **Aurora** — shimmering lights announced at dusk. v1's world-wide mechanical
//!   effect: mob spawning pauses for the night (the [`spawn_passes`] gate in
//!   `level::try_spawn`). [`mobs_docile`] is exposed for the eventual "hostiles ignore
//!   the player" hook, which needs entity-side wiring that is out of scope here.
//! - **Ember Rain** — the sky glows warm at dusk; through the night, every
//!   [`EMBER_CRATER_PERIOD`] day-clock ticks a small smoldering crater lands somewhere
//!   in the loaded surface area around the player: a pocket of rock, a vein or two of
//!   iron (rarely gold), and a lava speck at the point of impact. The specks cool to
//!   rock the moment the event is over ("cooled" is derived purely from the schedule:
//!   cooled = not an Ember Rain night); only the speck coordinates are session state.
//! - **Whisper Fog** (flavor v1) — fog banks roll over the marshes for the night. Mob
//!   spawn pressure doubles while the player stands in a Marsh biome (the second pass
//!   of [`spawn_passes`]), and whispers reach the player every [`WHISPER_PERIOD`] ticks.
//! - **The Caravan** — the only *day* event: wheel-ruts announced at dawn. Until dusk,
//!   walking near a worn trail (any dirt within [`CARAVAN_TRAIL_RADIUS`] tiles — trails
//!   wear dirt into the ground, see `level::structures_gen`) drops a bonus supply item
//!   by the player every [`CARAVAN_DROP_PERIOD`] ticks, picked by hash from
//!   [`CARAVAN_GOODS`].
//!
//! **Seasons** ([`Season`]): a thin real-calendar layer over presentation and spawn
//! selection only — world generation never sees it. Halloween (Oct 24-31) doubles how
//! often *night* events land (an extra roll that never yields a Caravan) and whispers
//! "The veil is thin tonight..." at dusk; Christmas (Dec 20-27) greets each dawn and
//! suppresses Hollow Night entirely. The date is injectable for tests
//! (`EventState::date_override`) or via env `FDOOM_DATE=MM-DD`.

use crate::core::game::Game;
use crate::core::updater::Time;
use crate::level::chunk;
use crate::level::infinite_gen;

/// One day in this many is an event day.
pub const EVENT_DAY_ODDS: u64 = 6;

/// Days of grave-decay suppression after a Hollow Night.
pub const QUIET_WEEK_DAYS: i32 = 7;

/// Ember Rain: day-clock ticks between crater impacts.
pub const EMBER_CRATER_PERIOD: i32 = 200;

/// Whisper Fog: day-clock ticks between whispers.
pub const WHISPER_PERIOD: i32 = 2000;

/// The Caravan: day-clock ticks between supply drops (while near a trail).
pub const CARAVAN_DROP_PERIOD: i32 = 500;

/// The Caravan: how close (in tiles) a worn-trail tile must be to count as
/// "walking the trail".
pub const CARAVAN_TRAIL_RADIUS: i32 = 8;

/// The Caravan's supply crates, picked by hash per drop.
pub const CARAVAN_GOODS: [&str; 4] = ["Torch", "Wood", "Stone", "Bread"];

/// Hash salt separating the event calendar from every terrain-generation stream.
const EVENT_SALT: u64 = 0x0E7E17;

/// Hash salt for the Caravan's per-drop supply pick.
const CARAVAN_SALT: u64 = 0x0CAB00;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldEvent {
    HollowNight,
    Aurora,
    EmberRain,
    WhisperFog,
    Caravan,
}

/// Kind table for the daily roll; order is part of the schedule (changing it reshuffles
/// every world's calendar).
const KINDS: [WorldEvent; 5] = [
    WorldEvent::HollowNight,
    WorldEvent::Aurora,
    WorldEvent::EmberRain,
    WorldEvent::WhisperFog,
    WorldEvent::Caravan,
];

impl WorldEvent {
    /// Everything but the Caravan plays out overnight.
    pub fn is_night(self) -> bool {
        self != WorldEvent::Caravan
    }
}

/// Real-calendar season window (presentation/spawn selection only — never world gen).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    #[default]
    None,
    /// Oct 24-31: night events roll twice as often; the veil-cue at dusk.
    Halloween,
    /// Dec 20-27: festive dawn cue; Hollow Night is suppressed.
    Christmas,
}

/// The season table, keyed on (month, day). Pure — the single source of truth for
/// the seasonal windows.
pub fn season_for(month: u32, day: u32) -> Season {
    match (month, day) {
        (10, 24..=31) => Season::Halloween,
        (12, 20..=27) => Season::Christmas,
        _ => Season::None,
    }
}

/// Today's season. Priority: explicit override (tests) > env `FDOOM_DATE=MM-DD` >
/// the system clock (UTC).
pub fn season_now(date_override: Option<(u32, u32)>) -> Season {
    let (m, d) = date_override
        .or_else(env_date)
        .unwrap_or_else(today_utc_month_day);
    season_for(m, d)
}

/// `FDOOM_DATE=MM-DD` — mock the calendar date without touching the code.
fn env_date() -> Option<(u32, u32)> {
    let s = std::env::var("FDOOM_DATE").ok()?;
    let (m, d) = s.split_once('-')?;
    Some((m.trim().parse().ok()?, d.trim().parse().ok()?))
}

fn today_utc_month_day() -> (u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    civil_month_day((secs / 86_400) as i64)
}

/// Gregorian (month, day) for a days-since-1970-01-01 count (Howard Hinnant's
/// `civil_from_days`, year discarded). Pure; exposed for the seasonal-table tests.
pub fn civil_month_day(days: i64) -> (u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (m, d)
}

/// The one stateful piece: the session's day counter plus the previous-tick snapshots
/// used to detect day wraps and time-of-day transitions. Deliberately not saved —
/// the schedule itself is pure, and a reload simply restarts the calendar at day 0.
#[derive(Debug, Default, Clone)]
pub struct EventState {
    /// Days completed since the session started (0 = the first day, which never hosts
    /// an event, so fresh worlds always get a calm first night).
    pub day_number: i32,
    /// Cached real-calendar season; refreshed at session start and on every day wrap.
    pub season: Season,
    /// Mocked (month, day) for tests; takes precedence over the clock and `FDOOM_DATE`.
    pub date_override: Option<(u32, u32)>,
    prev_tick_count: i32,
    prev_time: Option<Time>,
    /// Lava specks stamped by tonight's Ember Rain, cooled to rock once the event is
    /// over. Session-local by design: cooling is derived from the schedule, not saved.
    ember_lava: Vec<(usize, i32, i32)>,
}

/// The event hosted by `day` for `world_seed` in `season`, if any. Pure; at most one
/// event per day by construction. Day 0 (and anything earlier) is always calm.
///
/// The base roll (one day in [`EVENT_DAY_ODDS`]) picks uniformly among all five kinds.
/// Halloween adds a second, equally likely roll on otherwise-calm days that only night
/// events can win — night events land exactly twice as often, the Caravan is untouched.
/// Christmas turns every would-be Hollow Night into a calm day.
pub fn event_for_day_in_season(world_seed: i64, day: i32, season: Season) -> Option<WorldEvent> {
    if day <= 0 {
        return None;
    }
    let h = infinite_gen::hash(world_seed, EVENT_SALT, day, 0);
    let kind = KINDS[((h >> 8) % KINDS.len() as u64) as usize];
    let rolled = h % EVENT_DAY_ODDS == 0
        // the veil is thin: an equally likely bonus roll only night events can win
        || (season == Season::Halloween && h % EVENT_DAY_ODDS == 3 && kind.is_night());
    match rolled.then_some(kind) {
        Some(WorldEvent::HollowNight) if season == Season::Christmas => None,
        e => e,
    }
}

/// [`event_for_day_in_season`] outside any seasonal window — the base calendar.
pub fn event_for_day(world_seed: i64, day: i32) -> Option<WorldEvent> {
    event_for_day_in_season(world_seed, day, Season::None)
}

/// Today's event, if any.
pub fn current_event(g: &Game) -> Option<WorldEvent> {
    event_for_day_in_season(g.world_seed, g.events.day_number, g.events.season)
}

/// True during the night of a Hollow Night day — gravestone crumbling accelerates.
pub fn hollow_night_active(g: &Game) -> bool {
    g.get_time() == Time::Night && current_event(g) == Some(WorldEvent::HollowNight)
}

/// True during the night of an Aurora day.
pub fn aurora_active(g: &Game) -> bool {
    g.get_time() == Time::Night && current_event(g) == Some(WorldEvent::Aurora)
}

/// True during the night of an Ember Rain day — craters are falling.
pub fn ember_rain_active(g: &Game) -> bool {
    g.get_time() == Time::Night && current_event(g) == Some(WorldEvent::EmberRain)
}

/// True during the night of a Whisper Fog day.
pub fn whisper_fog_active(g: &Game) -> bool {
    g.get_time() == Time::Night && current_event(g) == Some(WorldEvent::WhisperFog)
}

/// True through the daylight hours (dawn to dusk) of a Caravan day.
pub fn caravan_active(g: &Game) -> bool {
    matches!(g.get_time(), Time::Morning | Time::Day)
        && current_event(g) == Some(WorldEvent::Caravan)
}

/// Aurora nights calm the world's hostiles. v1 consumes this as a spawn pause
/// (`level::try_spawn`); the target-acquisition hook is future entity-side work.
pub fn mobs_docile(g: &Game) -> bool {
    aurora_active(g)
}

/// The world-events gate for `level::try_spawn`: how many spawn passes this call gets.
/// 0 — Aurora pauses spawning for the night; 2 — Whisper Fog doubles spawn pressure
/// while the player stands in the fog (a Marsh biome); 1 — everything else.
pub fn spawn_passes(g: &Game, lvl: usize) -> u32 {
    if aurora_active(g) {
        return 0;
    }
    if whisper_fog_active(g) && player_in_marsh(g, lvl) {
        return 2;
    }
    1
}

/// Is the player standing in Whisper Fog country on `lvl`? Marsh biome on infinite
/// surfaces; classic finite surfaces (no biome field) fall back to standing on mud.
fn player_in_marsh(g: &Game, lvl: usize) -> bool {
    let Some(p) = g.try_player() else {
        return false;
    };
    if p.c.level != Some(lvl) {
        return false;
    }
    let Some(level) = g.levels.get(lvl).and_then(|l| l.as_ref()) else {
        return false;
    };
    if level.depth != 0 {
        return false;
    }
    let (xt, yt) = (p.c.x >> 4, p.c.y >> 4);
    if level.is_infinite() {
        infinite_gen::biome_at(g.world_seed, xt, yt) == infinite_gen::Biome::Marsh
    } else {
        g.tile_at(lvl, xt, yt).name == "MUD"
    }
}

/// Quiet week: no grave decay if a Hollow Night fell within the last
/// [`QUIET_WEEK_DAYS`] days. Pure derivation from the schedule — see
/// [`grave_decay_suppressed_on`].
pub fn grave_decay_suppressed(g: &Game) -> bool {
    grave_decay_suppressed_in_season(g.world_seed, g.events.day_number, g.events.season)
}

/// Pure form of [`grave_decay_suppressed`] on the base (no-season) calendar.
pub fn grave_decay_suppressed_on(world_seed: i64, day: i32) -> bool {
    grave_decay_suppressed_in_season(world_seed, day, Season::None)
}

/// Was any of the `QUIET_WEEK_DAYS` days before `day` a Hollow Night?
pub fn grave_decay_suppressed_in_season(world_seed: i64, day: i32, season: Season) -> bool {
    (1..=QUIET_WEEK_DAYS).any(|back| {
        event_for_day_in_season(world_seed, day - back, season) == Some(WorldEvent::HollowNight)
    })
}

/// Per-tick scheduler hook (called once from `Game::tick`, right after the day clock
/// advances): counts day wraps, runs the active event's per-tick mechanics, and fires
/// the dusk/dawn notification cues on time-of-day transitions. While a menu is open the
/// day clock is frozen — no cues fire and no mechanics run (`clock_advanced`).
pub fn tick(g: &mut Game) {
    // A backwards jump of the day clock is a day boundary: `set_time` wraps past
    // DAY_LENGTH to 0, and sleeping through midnight resets `tick_count` directly.
    let clock_advanced = g.tick_count != g.events.prev_tick_count;
    if g.tick_count < g.events.prev_tick_count {
        g.events.day_number += 1;
        // the real calendar can roll over mid-session too
        g.events.season = season_now(g.events.date_override);
    }
    g.events.prev_tick_count = g.tick_count;
    if g.events.prev_time.is_none() {
        // first tick of the session: pick up the calendar season
        g.events.season = season_now(g.events.date_override);
    }

    // per-tick event mechanics, keyed to the day clock so menus pause them with time
    if clock_advanced {
        if ember_rain_active(g) && g.tick_count % EMBER_CRATER_PERIOD == 0 {
            ember_impact(g);
        }
        if whisper_fog_active(g) && g.tick_count % WHISPER_PERIOD == 0 {
            g.push_warning("You hear whispers in the fog...");
        }
        if caravan_active(g) && g.tick_count % CARAVAN_DROP_PERIOD == 0 {
            caravan_drop(g);
        }
    }
    // Ember specks cool the moment the schedule says the rain is over (dawn, or a
    // day-skip in bed) — pure derivation, no cooling bookkeeping.
    if !g.events.ember_lava.is_empty() && !ember_rain_active(g) {
        cool_ember_lava(g);
    }

    let time = g.get_time();
    if g.events.prev_time == Some(time) {
        return;
    }
    g.events.prev_time = Some(time);

    match time {
        Time::Evening => {
            if g.events.season == Season::Halloween {
                g.push_warning("The veil is thin tonight...");
            }
            match current_event(g) {
                Some(WorldEvent::HollowNight) => {
                    g.push_warning("The evening is unnaturally still...")
                }
                Some(WorldEvent::Aurora) => g.push_warning("Pale lights shimmer at the horizon..."),
                Some(WorldEvent::EmberRain) => g.push_warning("The sky glows warm..."),
                Some(WorldEvent::WhisperFog) => {
                    g.push_warning("A pale fog gathers over the marshes...")
                }
                Some(WorldEvent::Caravan) | None => {}
            }
        }
        Time::Night => match current_event(g) {
            Some(WorldEvent::Aurora) => g.push_warning("An aurora ripples across the sky."),
            Some(WorldEvent::EmberRain) => g.push_warning("Embers streak down from the sky!"),
            _ => {}
        },
        Time::Morning => {
            if g.events.season == Season::Christmas {
                g.push_warning("The air feels festive.");
            }
            match event_for_day_in_season(g.world_seed, g.events.day_number - 1, g.events.season) {
                Some(WorldEvent::HollowNight) => {
                    g.push_warning("Dawn breaks. The graves lie quiet.")
                }
                Some(WorldEvent::Aurora) => g.push_warning("The aurora fades with the dawn."),
                Some(WorldEvent::EmberRain) => {
                    g.push_warning("The fallen embers have cooled to stone.")
                }
                Some(WorldEvent::WhisperFog) => g.push_warning("The fog lifts with the sun."),
                Some(WorldEvent::Caravan) | None => {}
            }
            if current_event(g) == Some(WorldEvent::Caravan) {
                g.push_warning("Fresh wheel-ruts mark the old trails...");
            }
        }
        Time::Day => {}
    }
}

/* ------------------------------- event mechanics ------------------------------------ */

/// The player's (level, tile x, tile y) when they stand on a surface layer.
fn player_surface_tile(g: &Game) -> Option<(usize, i32, i32)> {
    let p = g.try_player()?;
    let lvl = p.c.level?;
    let level = g.levels.get(lvl)?.as_ref()?;
    (level.depth == 0).then_some((lvl, p.c.x >> 4, p.c.y >> 4))
}

/// One Ember Rain impact: pick a loaded surface spot near the player and stamp a small
/// smoldering crater — 2-3 rock, 1-2 ore (iron, rarely gold), lava speck at the center.
/// Incidental randomness (`g.random`), like every other in-world roll.
fn ember_impact(g: &mut Game) {
    let Some((lvl, px, py)) = player_surface_tile(g) else {
        return;
    };
    // Anywhere within +-(one loaded chunk span) of the player: always inside the loaded
    // ring on infinite layers, and simply "nearby" on finite ones.
    let span = chunk::CHUNK_SIZE * chunk::LOAD_RADIUS;
    let cx = px - span / 2 + g.random.next_int_bound(span);
    let cy = py - span / 2 + g.random.next_int_bound(span);
    if g.level(lvl).tile_id(cx, cy).is_none() {
        return; // unloaded chunk / out of bounds: the meteor lands unseen
    }
    let center = g.tile_at(lvl, cx, cy);
    if center.name == "WATER" || center.name == "DEEP WATER" {
        return; // fizzles into the water
    }

    let rock = g.tiles.get("rock");
    let lava = g.tiles.get("lava");
    let iron = g.tiles.get("iron ore");
    let gold = g.tiles.get("gold ore");
    const RING: [(i32, i32); 8] = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];
    let rot = g.random.next_int_bound(8);
    let n_rock = 2 + g.random.next_int_bound(2); // 2-3 tile rock pocket
    let n_ore = 1 + g.random.next_int_bound(2); // 1-2 ore veins
    for i in 0..n_rock + n_ore {
        let (dx, dy) = RING[((rot + i) % 8) as usize];
        let (x, y) = (cx + dx, cy + dy);
        if g.level(lvl).tile_id(x, y).is_none() {
            continue;
        }
        let t = if i < n_rock {
            rock.clone()
        } else if g.random.next_int_bound(8) == 0 {
            gold.clone()
        } else {
            iron.clone()
        };
        g.set_tile_default(lvl, x, y, &t);
    }
    g.set_tile_default(lvl, cx, cy, &lava);
    g.events.ember_lava.push((lvl, cx, cy));
}

/// Cool tonight's lava specks to rock. Specks in unloaded chunks are kept and retried —
/// they cool the moment their chunk streams back in this session.
fn cool_ember_lava(g: &mut Game) {
    let specks = std::mem::take(&mut g.events.ember_lava);
    let rock = g.tiles.get("rock");
    for (lvl, x, y) in specks {
        let loaded = g
            .levels
            .get(lvl)
            .and_then(|l| l.as_ref())
            .is_some_and(|l| l.tile_id(x, y).is_some());
        if !loaded {
            g.events.ember_lava.push((lvl, x, y));
            continue;
        }
        if g.tile_at(lvl, x, y).name == "LAVA" {
            g.set_tile_default(lvl, x, y, &rock);
        }
    }
}

/// One Caravan supply drop: if the player walks within [`CARAVAN_TRAIL_RADIUS`] tiles
/// of a worn trail (dirt), a bonus item — hash-picked from [`CARAVAN_GOODS`] — lands
/// beside them.
fn caravan_drop(g: &mut Game) {
    let Some((lvl, xt, yt)) = player_surface_tile(g) else {
        return;
    };
    if !near_trail(g, lvl, xt, yt) {
        return;
    }
    let h = infinite_gen::hash(
        g.world_seed,
        CARAVAN_SALT,
        g.events.day_number,
        g.tick_count,
    );
    let item = crate::item::registry::get(g, CARAVAN_GOODS[(h % 4) as usize]);
    let (px, py) = {
        let p = g.try_player().expect("checked above");
        (p.c.x, p.c.y)
    };
    crate::level::drop_item(g, lvl, px, py, item);
    g.notify_all("You find supplies dropped along the old trail.");
}

/// Any worn-trail tile (dirt — trails wear dirt into the ground) within
/// [`CARAVAN_TRAIL_RADIUS`] tiles of `(xt, yt)`?
fn near_trail(g: &Game, lvl: usize, xt: i32, yt: i32) -> bool {
    let r = CARAVAN_TRAIL_RADIUS;
    for dy in -r..=r {
        for dx in -r..=r {
            if g.tile_at(lvl, xt + dx, yt + dy).name == "DIRT" {
                return true;
            }
        }
    }
    false
}
