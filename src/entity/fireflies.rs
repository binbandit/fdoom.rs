//! Fireflies — an original fdoom.rs ambient swarm entity (no Java counterpart, and
//! not a mob: no health, no collision, never counted against the mob cap). One entity
//! renders 4-8 drifting glow specks, cheap by construction: speck positions are pure
//! functions of the swarm's `time` + per-speck phases, so there is no per-speck state.
//!
//! Life cycle: spawns at dusk near trees or marsh water (`level::try_spawn`), wanders
//! in slow curvy loops, then settles ("roosts") on a nearby tree tile — the specks
//! glow over the canopy. Spooked (player within ~3 tiles, or the roost tree is hit /
//! felled), the swarm bursts into fast scatter flight and regroups after ~10 s. Fades
//! at dawn. Registers as a tiny light emitter (radius 2, like the glow worm).

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::rng::Rng;

/// Sheet cell (10,20): a 2-pixel glow speck (true-color — warm through the grade).
const SPECK_POS: i32 = 10 + 20 * 32;
/// Nominal swarm color (`c.col` cosmetics only; the speck cell is true-color).
pub const SPECK_COL: i32 = color::get4(-1, -1, 230, 550);

/// Spook radius (~3 tiles), in px.
const SPOOK_DIST: i32 = 3 * 16;
/// Scatter duration before regrouping (~10 s).
const SCATTER_TICKS: i32 = 10 * crate::core::updater::NORM_SPEED;
/// Wander time before the swarm looks for a tree to roost on.
const SETTLE_TICKS: i32 = 10 * crate::core::updater::NORM_SPEED;
/// How far the wandering swarm strays from its anchor (px).
const WANDER_RANGE: i32 = 56;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireflyState {
    /// Free flight around `home`; `settle` counts down to roost-hunting.
    Wander { settle: i32 },
    /// Settled on the tree tile at `home`; `tree_data` snapshots the tile's data so a
    /// hit on the tree (damage counter change) spooks the swarm.
    Roost { tree_data: i32 },
    /// Burst scatter; regroups when `left` runs out.
    Scatter { left: i32 },
}

#[derive(Debug, Clone)]
pub struct FirefliesData {
    /// 4..=8 rendered specks.
    pub count: i32,
    /// Per-swarm phase seed (decorrelates neighboring swarms).
    pub seed: i32,
    pub time: i32,
    pub state: FireflyState,
    /// Anchor point (px); the roost tree center while roosting.
    pub home: (i32, i32),
    /// Current drift direction.
    pub dx: i32,
    pub dy: i32,
}

/// Weather gate stub for the weather wave: fireflies stay in on rainy nights.
/// TODO(weather agent): consult the live weather state here.
pub fn weather_allows(_g: &Game) -> bool {
    true
}

pub fn new(random: &mut Rng) -> Entity {
    let mut c = EntityCommon::new(4, 3);
    c.col = SPECK_COL;
    Entity::new(
        c,
        EntityKind::Fireflies(FirefliesData {
            count: 4 + random.next_int_bound(5),
            seed: random.next_int_bound(0x10000),
            time: 0,
            state: FireflyState::Wander {
                settle: SETTLE_TICKS,
            },
            home: (0, 0),
            dx: 0,
            dy: 0,
        }),
    )
}

fn is_tree(g: &Game, lvl: usize, xt: i32, yt: i32) -> bool {
    use crate::level::tile::TileKind;
    matches!(
        g.tile_at(lvl, xt, yt).kind,
        TileKind::Tree | TileKind::TreeSpecies { .. } | TileKind::SnowTree
    )
}

pub fn tick(g: &mut Game, e: &mut Entity) {
    use crate::core::updater::Time;

    let Some(lvl) = e.c.level else { return };

    // dawn (or weather, once it lands) disperses the swarm
    let time_of_day = g.get_time();
    if !(time_of_day == Time::Night || time_of_day == Time::Evening) || !weather_allows(g) {
        crate::entity::behavior::remove_entity(g, e);
        return;
    }

    // first tick after placement: anchor where the spawner put us
    if let EntityKind::Fireflies(d) = &mut e.kind {
        if d.time == 0 {
            d.home = (e.c.x, e.c.y);
        }
        d.time += 1;
    }

    // the closest player, for the spook check and scatter direction
    let player_at = crate::entity::behavior::get_closest_player(g, e)
        .and_then(|pid| g.entities.get(pid).map(|p| (p.c.x, p.c.y)));
    let spooked = player_at
        .map(|(px, py)| {
            let (xd, yd) = (px - e.c.x, py - e.c.y);
            xd * xd + yd * yd < SPOOK_DIST * SPOOK_DIST
        })
        .unwrap_or(false);

    let (x, y) = (e.c.x, e.c.y);
    let EntityKind::Fireflies(d) = &mut e.kind else {
        return;
    };

    match d.state {
        FireflyState::Wander { settle } => {
            if spooked {
                d.state = FireflyState::Scatter {
                    left: SCATTER_TICKS,
                };
            } else {
                // slow curvy drift: re-roll every 90 ticks, steer home past the leash
                if d.time % 90 == 1 {
                    d.dx = g.random.next_int_bound(3) - 1;
                    d.dy = g.random.next_int_bound(3) - 1;
                }
                if (x - d.home.0).abs() > WANDER_RANGE {
                    d.dx = (d.home.0 - x).signum();
                }
                if (y - d.home.1).abs() > WANDER_RANGE {
                    d.dy = (d.home.1 - y).signum();
                }
                if d.time % 3 == 0 {
                    e.c.x += d.dx;
                    e.c.y += d.dy;
                }
                if settle <= 1 {
                    // look for a tree to roost on
                    let (xt, yt) = (x >> 4, y >> 4);
                    let mut found = None;
                    'scan: for r in 0i32..=3 {
                        for dy in -r..=r {
                            for dx in -r..=r {
                                if dx.abs().max(dy.abs()) == r && is_tree(g, lvl, xt + dx, yt + dy)
                                {
                                    found = Some((xt + dx, yt + dy));
                                    break 'scan;
                                }
                            }
                        }
                    }
                    if let Some((tx, ty)) = found {
                        e.c.x = tx * 16 + 8;
                        e.c.y = ty * 16 + 6; // hover over the canopy
                        d.home = (e.c.x, e.c.y);
                        d.state = FireflyState::Roost {
                            tree_data: g.level(lvl).get_data(tx, ty),
                        };
                    } else {
                        d.state = FireflyState::Wander {
                            settle: SETTLE_TICKS / 2,
                        };
                    }
                } else {
                    d.state = FireflyState::Wander { settle: settle - 1 };
                }
            }
        }
        FireflyState::Roost { tree_data } => {
            let (tx, ty) = (d.home.0 >> 4, d.home.1 >> 4);
            let tree_gone = !is_tree(g, lvl, tx, ty);
            let tree_hit = g.level(lvl).get_data(tx, ty) != tree_data;
            if spooked || tree_gone || tree_hit {
                d.state = FireflyState::Scatter {
                    left: SCATTER_TICKS,
                };
            }
        }
        FireflyState::Scatter { left } => {
            // burst flight away from the player, curving hard
            if let Some((px, py)) = player_at {
                d.dx = (x - px).signum();
                d.dy = (y - py).signum();
            }
            if d.time % 2 == 0 {
                e.c.x += d.dx;
                e.c.y += d.dy + tri(d.time + d.seed, 24, 1);
            }
            if left <= 1 {
                d.home = (e.c.x, e.c.y);
                d.state = FireflyState::Wander {
                    settle: SETTLE_TICKS,
                };
            } else {
                d.state = FireflyState::Scatter { left: left - 1 };
            }
        }
    }
}

/// Integer triangle wave: period `period`, range `-amp..=amp`.
fn tri(t: i32, period: i32, amp: i32) -> i32 {
    let p = t.rem_euclid(period.max(2));
    let half = (period / 2).max(1);
    let v = if p < half { p } else { period - p }; // 0..half..0
    (v - half / 2) * 2 * amp / half
}

/// The swarm: each speck is a pure function of `(time, seed, i)` — Lissajous-ish
/// loops with per-speck phase and period, occasional blink-outs.
pub fn render(_g: &mut Game, screen: &mut crate::gfx::Screen, e: &mut Entity) {
    let EntityKind::Fireflies(d) = &e.kind else {
        return;
    };
    let (ax, ay, speed) = match d.state {
        FireflyState::Wander { .. } => (12, 8, 1),
        FireflyState::Roost { .. } => (7, 5, 1),
        FireflyState::Scatter { .. } => (20, 14, 3),
    };
    for i in 0..d.count {
        let phase = d.seed + i * 977;
        let t = d.time * speed + phase;
        // blink: each speck goes dark on its own cadence
        if ((t >> 3) + i * 5) % 7 == 0 {
            continue;
        }
        let px = e.c.x + tri(t, 96 + i * 14, ax);
        let py = e.c.y + tri(t + 31, 64 + i * 10, ay);
        screen.render(px - 4, py - 4, SPECK_POS, SPECK_COL, i & 3);
    }
}
