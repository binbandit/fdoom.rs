//! Port of `fdoom.entity.Arrow` and `fdoom.entity.Spark`.

use crate::entity::{Direction, Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::rng::Rng;

/// Post-port: render/behavior style of an `Arrow`-shaped straight-line projectile.
/// `Arrow` is the Java original; the rest are the survival thrown/launched weapons that
/// reuse its flight code (see `projectile_behavior::arrow_tick`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileStyle {
    Arrow,
    Spear,
    Knife,
    Pellet,
}

#[derive(Debug, Clone)]
pub struct ArrowData {
    pub dir: Direction,
    pub damage: i32,
    /// eid of the owning mob (Java held a Mob reference).
    pub owner: i32,
    pub speed: i32,
    /// Post-port: which weapon fired this (sprite + landing behavior).
    pub style: ProjectileStyle,
    /// Post-port: remaining flight ticks; `< 0` = unlimited (plain arrows/bolts).
    /// At 0 the projectile lands (drops `payload` if any, then despawns).
    pub range_ticks: i32,
    /// Post-port: item data string (`Item::get_data`) dropped where the projectile
    /// lands — the thrown spear/knife that can be picked back up. Not persisted in
    /// saves (a mid-flight save reloads it as a plain arrow; flight lasts under a
    /// second, an accepted loss).
    pub payload: Option<String>,
}

/// Java `new Arrow(owner, x, y, dir, dmg)`.
pub fn new_arrow(owner_eid: i32, x: i32, y: i32, dir: Direction, dmg: i32) -> Entity {
    let speed = if dmg > 3 {
        8
    } else if dmg >= 0 {
        7
    } else {
        6
    };
    new_projectile(
        owner_eid,
        x,
        y,
        dir,
        dmg,
        speed,
        ProjectileStyle::Arrow,
        -1,
        None,
        color::get4(-1, 111, 222, 430),
    )
}

/// Post-port: a thrown/launched projectile reusing the Arrow flight path — spears,
/// throwing knives, slingshot pellets.
#[allow(clippy::too_many_arguments)]
pub fn new_thrown(
    owner_eid: i32,
    x: i32,
    y: i32,
    dir: Direction,
    dmg: i32,
    style: ProjectileStyle,
    range_ticks: i32,
    payload: Option<String>,
) -> Entity {
    let (speed, col) = match style {
        ProjectileStyle::Spear => (7, color::get4(-1, 100, 321, 444)),
        ProjectileStyle::Knife => (8, color::get4(-1, 111, 444, 555)),
        ProjectileStyle::Pellet => (7, color::get4(-1, 111, 333, 555)),
        ProjectileStyle::Arrow => (7, color::get4(-1, 111, 222, 430)),
    };
    new_projectile(
        owner_eid,
        x,
        y,
        dir,
        dmg,
        speed,
        style,
        range_ticks,
        payload,
        col,
    )
}

#[allow(clippy::too_many_arguments)]
fn new_projectile(
    owner_eid: i32,
    x: i32,
    y: i32,
    dir: Direction,
    dmg: i32,
    speed: i32,
    style: ProjectileStyle,
    range_ticks: i32,
    payload: Option<String>,
    col: i32,
) -> Entity {
    let mut c = EntityCommon::new(dir.x().abs() + 1, dir.y().abs() + 1);
    c.x = x;
    c.y = y;
    c.col = col;

    Entity::new(
        c,
        EntityKind::Arrow(ArrowData {
            dir,
            damage: dmg,
            owner: owner_eid,
            speed,
            style,
            range_ticks,
            payload,
        }),
    )
}

/// The Night Wisp's zap bolt — adapted from the removed AirWizard's `Spark`
/// (Java `fdoom.entity.Spark`): the same free-floating double-precision motion that
/// ignores tiles entirely, re-owned and shortened to a ranged-attack bolt.
#[derive(Debug, Clone)]
pub struct ZapData {
    pub life_time: i32,
    // x and y velocity (Java Spark called these accelerations)
    pub xa: f64,
    pub ya: f64,
    // x and y positions
    pub xx: f64,
    pub yy: f64,
    pub time: i32,
    /// eid of the Night Wisp that fired this zap.
    pub owner: i32,
}

/// Adapted Java `new Spark(owner, xa, ya)` — shorter-lived (a bolt, not a swarm).
pub fn new_zap(
    owner_eid: i32,
    owner_x: i32,
    owner_y: i32,
    xa: f64,
    ya: f64,
    random: &mut Rng,
) -> Entity {
    let mut c = EntityCommon::new(0, 0);
    c.x = owner_x;
    c.y = owner_y;
    let data = ZapData {
        // ~2.5-3s at 1.5 px/tick = a few tiles of range (Spark lived a full 600+)
        life_time: 60 * 2 + random.next_int_bound(60),
        xa,
        ya,
        xx: owner_x as f64,
        yy: owner_y as f64,
        time: 0,
        owner: owner_eid,
    };
    Entity::new(c, EntityKind::Zap(data))
}
