//! Port of `fdoom.entity.ItemEntity` — a dropped item bouncing on the ground.

use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::item::Item;
use crate::rng::Rng;

#[derive(Debug, Clone)]
pub struct ItemEntityData {
    pub item: Item,
    pub life_time: i32,
    // x, y, and z accelerations
    pub xa: f64,
    pub ya: f64,
    pub za: f64,
    // x, y, and z coordinates in double precision
    pub xx: f64,
    pub yy: f64,
    pub zz: f64,
    pub time: i32,
    pub picked_up: bool,
}

/// Java `new ItemEntity(item, x, y)`.
pub fn new(item: Item, x: i32, y: i32, random: &mut Rng) -> Entity {
    let mut c = EntityCommon::new(2, 2);
    c.x = x;
    c.y = y;
    // JAVA constructor order: the accelerations draw from the RNG before lifeTime does.
    let xa = random.next_gaussian() * 0.3;
    let ya = random.next_gaussian() * 0.2;
    let za = random.next_float() as f64 * 0.7 + 1.0;
    let data = ItemEntityData {
        item,
        // JAVA: min 600 ticks, max 669 ticks
        life_time: 60 * 10 + random.next_int_bound(70),
        xa,
        ya,
        za,
        xx: x as f64,
        yy: y as f64,
        zz: 2.0,
        time: 0,
        picked_up: false,
    };
    Entity::new(c, EntityKind::ItemEntity(data))
}

/// Java `new ItemEntity(item, x, y, zz, lifetime, time, xa, ya, za)` (used by Load).
#[allow(clippy::too_many_arguments)]
pub fn with_motion(
    item: Item,
    x: i32,
    y: i32,
    zz: f64,
    lifetime: i32,
    time: i32,
    xa: f64,
    ya: f64,
    za: f64,
    random: &mut Rng,
) -> Entity {
    let mut e = new(item, x, y, random);
    if let EntityKind::ItemEntity(data) = &mut e.kind {
        data.life_time = lifetime;
        data.time = time;
        data.zz = zz;
        data.xa = xa;
        data.ya = ya;
        data.za = za;
    }
    e
}

/// Java `getData()`.
pub fn get_data(data: &ItemEntityData) -> String {
    [
        data.item.get_data(),
        format_double(data.zz),
        data.life_time.to_string(),
        data.time.to_string(),
        format_double(data.xa),
        format_double(data.ya),
        format_double(data.za),
    ]
    .join(":")
}

/// Java's `Double.toString` — always includes a decimal point.
pub fn format_double(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e7 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}
