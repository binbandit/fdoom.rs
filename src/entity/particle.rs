//! Port of `fdoom.entity.particle` — `Particle` (plus its Fire/Smash flavors, which only
//! differ in constructor arguments) and `TextParticle`.

use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::{FontStyle, Sprite, color};
use crate::rng::Rng;

#[derive(Debug, Clone)]
pub struct ParticleData {
    pub time: i32,
    pub lifetime: i32,
    pub sprite: Sprite,
    /// Fire wave: purely-visual drift, a function of `time` (the entity's own
    /// x/y never move). `rise` = pixels climbed per tick; `sway` = horizontal
    /// sine amplitude in pixels; `phase` offsets the sine so puffs desynchronize.
    pub rise: f32,
    pub sway: f32,
    pub phase: f32,
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
            rise: 0.0,
            sway: 0.0,
            phase: 0.0,
        }),
    )
}

/// Fire wave: a gray smoke puff that rises with a lazy side-to-side sway. `thin`
/// picks the wispy cell (low campfire fuel) over the fat puff.
pub fn new_smoke_particle(x: i32, y: i32, thin: bool, random: &mut Rng) -> Entity {
    let (cell_x, palette) = if thin {
        (9, color::get4(-1, -1, -1, 333))
    } else {
        (8, color::get4(-1, -1, 222, 333))
    };
    let mut e = new_particle(
        x,
        y,
        1,
        50 + random.next_int_bound(30),
        Sprite::new1x1(cell_x, 18, palette),
    );
    if let EntityKind::Particle(p) = &mut e.kind {
        p.rise = 0.4;
        p.sway = if thin { 1.5 } else { 2.5 };
        p.phase = random.next_float() * std::f32::consts::TAU;
    }
    e
}

/// Attack-impact feedback: a small, short-lived puff of the struck material (stone
/// chips, leaf flecks, dust) — the thin smoke wisp cell under a material-tinted
/// palette. Whiffs spawn none; the caller picks the tint (see the player attack path).
/// `(x, y)` is the puff's *center* (particle sprites draw from their top-left, so the
/// 8x8 cell is offset here — see the smash particle's tile-corner convention).
pub fn new_material_puff(x: i32, y: i32, palette: i32, random: &mut Rng) -> Entity {
    let mut e = new_particle(
        x - 4,
        y - 4,
        1,
        10 + random.next_int_bound(6),
        Sprite::new1x1(9, 18, palette),
    );
    if let EntityKind::Particle(p) = &mut e.kind {
        p.rise = 0.3;
        p.sway = 1.0;
        p.phase = random.next_float() * std::f32::consts::TAU;
    }
    e
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
pub fn new_text_particle(msg: &str, x: i32, y: i32, col: i32, random: &mut Rng) -> Entity {
    let mut c = EntityCommon::new(msg.chars().count() as i32, 1);
    c.x = x;
    c.y = y;
    let style = FontStyle::new(col).set_shadow_type(color::BLACK, false);
    let data = TextParticleData {
        particle: ParticleData {
            time: 0,
            lifetime: 60,
            sprite: Sprite::missing_texture(1, 1),
            rise: 0.0,
            sway: 0.0,
            phase: 0.0,
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
