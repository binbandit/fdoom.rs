//! Port of `fdoom.entity.particle` — `Particle` (plus its Fire/Smash flavors, which only
//! differ in constructor arguments) and `TextParticle`.

use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::{FontStyle, Sprite, color};
use crate::java_random::JavaRandom;

#[derive(Debug, Clone)]
pub struct ParticleData {
    pub time: i32,
    pub lifetime: i32,
    pub sprite: Sprite,
}

/// Java `new Particle(x, y, xr, lifetime, sprite)`.
pub fn new_particle(x: i32, y: i32, xr: i32, lifetime: i32, sprite: Sprite) -> Entity {
    let mut c = EntityCommon::new(xr, 1);
    c.x = x;
    c.y = y;
    Entity::new(
        c,
        EntityKind::Particle(ParticleData {
            time: 0,
            lifetime,
            sprite,
        }),
    )
}

/// Java `new FireParticle(x, y)` — used by Spawners when they spawn an entity.
pub fn new_fire_particle(x: i32, y: i32) -> Entity {
    new_particle(
        x,
        y,
        1,
        30,
        Sprite::new1x1(9, 19, color::get4(-1, 520, 550, 500)),
    )
}

/// Java `new SmashParticle(x, y)`. (The monsterHurt sound is played by the caller-side
/// helper in the behavior code, as the Java constructor did.)
pub fn new_smash_particle(x: i32, y: i32) -> Entity {
    let mirrors: [Vec<i32>; 2] = [vec![2, 3], vec![0, 1]];
    new_particle(
        x,
        y,
        1,
        10,
        Sprite::with_mirrors(5, 12, 2, 2, color::WHITE, true, &mirrors),
    )
}

#[derive(Debug, Clone)]
pub struct TextParticleData {
    pub particle: ParticleData,
    pub msg: String,
    // x, y, z acceleration
    pub xa: f64,
    pub ya: f64,
    pub za: f64,
    // x, y, z coordinates
    pub xx: f64,
    pub yy: f64,
    pub zz: f64,
    pub style: FontStyle,
}

/// Java `new TextParticle(msg, x, y, col)`.
pub fn new_text_particle(msg: &str, x: i32, y: i32, col: i32, random: &mut JavaRandom) -> Entity {
    let mut c = EntityCommon::new(msg.chars().count() as i32, 1);
    c.x = x;
    c.y = y;
    let style = FontStyle::new(col).set_shadow_type(color::BLACK, false);
    let data = TextParticleData {
        particle: ParticleData {
            time: 0,
            lifetime: 60,
            sprite: Sprite::missing_texture(1, 1),
        },
        msg: msg.to_string(),
        xx: x as f64,
        yy: y as f64,
        zz: 2.0,
        xa: random.next_gaussian() * 0.3,
        ya: random.next_gaussian() * 0.2,
        za: random.next_float() as f64 * 0.7 + 2.0,
        style,
    };
    Entity::new(c, EntityKind::TextParticle(data))
}
