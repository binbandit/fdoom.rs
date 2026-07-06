//! Port of `fdoom.entity.Arrow` and `fdoom.entity.Spark`.

use crate::entity::{Direction, Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::rng::Rng;

#[derive(Debug, Clone)]
pub struct ArrowData {
    pub dir: Direction,
    pub damage: i32,
    /// eid of the owning mob (Java held a Mob reference).
    pub owner: i32,
    pub speed: i32,
}

/// Java `new Arrow(owner, x, y, dir, dmg)`.
pub fn new_arrow(owner_eid: i32, x: i32, y: i32, dir: Direction, dmg: i32) -> Entity {
    let mut c = EntityCommon::new(dir.x().abs() + 1, dir.y().abs() + 1);
    c.x = x;
    c.y = y;
    c.col = color::get4(-1, 111, 222, 430);

    let speed = if dmg > 3 {
        8
    } else if dmg >= 0 {
        7
    } else {
        6
    };

    Entity::new(
        c,
        EntityKind::Arrow(ArrowData {
            dir,
            damage: dmg,
            owner: owner_eid,
            speed,
        }),
    )
}

#[derive(Debug, Clone)]
pub struct SparkData {
    pub life_time: i32,
    // x and y acceleration
    pub xa: f64,
    pub ya: f64,
    // x and y positions
    pub xx: f64,
    pub yy: f64,
    pub time: i32,
    /// eid of the AirWizard that created this spark.
    pub owner: i32,
}

/// Java `new Spark(owner, xa, ya)`.
pub fn new_spark(
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
    let data = SparkData {
        // JAVA: max time = 629 ticks, min time = 600 ticks
        life_time: 60 * 10 + random.next_int_bound(30),
        xa,
        ya,
        xx: owner_x as f64,
        yy: owner_y as f64,
        time: 0,
        owner: owner_eid,
    };
    Entity::new(c, EntityKind::Spark(data))
}
