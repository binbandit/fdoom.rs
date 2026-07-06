//! Port of `fdoom.entity.furniture.Spawner`.

use crate::entity::{Entity, EntityKind};
use crate::gfx::Sprite;
use crate::java_random::JavaRandom;

use super::{FurnitureData, furniture_common};

pub const ACTIVE_RADIUS: i32 = 8 * 16;
pub const MIN_SPAWN_INTERVAL: i32 = 200;
pub const MAX_SPAWN_INTERVAL: i32 = 500;
/// 1 in this chance of calling trySpawn every interval.
pub const MIN_MOB_SPAWN_CHANCE: i32 = 10;

#[derive(Debug, Clone)]
pub struct SpawnerData {
    pub furniture: FurnitureData,
    /// The mob this spawner spawns (a template entity, exactly like Java's `MobAi mob`).
    pub mob: Box<Entity>,
    pub health: i32,
    pub lvl: i32,
    pub max_mob_level: i32,
    pub spawn_tick: i32,
}

/// The Java mob-class simple name, for the "<Mob> Spawner" furniture name.
fn mob_class_name(mob: &Entity) -> &'static str {
    match &mob.kind {
        EntityKind::Cow(_) => "Cow",
        EntityKind::Pig(_) => "Pig",
        EntityKind::Sheep(_) => "Sheep",
        EntityKind::GlowWorm(_) => "GlowWorm",
        EntityKind::Zombie(_) => "Zombie",
        EntityKind::Slime(_) => "Slime",
        EntityKind::Creeper(_) => "Creeper",
        EntityKind::Skeleton(_) => "Skeleton",
        EntityKind::Snake(_) => "Snake",
        EntityKind::Knight(_) => "Knight",
        EntityKind::AirWizard(_) => "AirWizard",
        _ => "Mob",
    }
}

/// Java `MobAi.getMaxLevel()` for the given mob template.
pub fn max_mob_level(mob: &Entity) -> i32 {
    match &mob.kind {
        EntityKind::Zombie(m) => m.enemy.lvlcols.len() as i32,
        EntityKind::Slime(m) => m.enemy.lvlcols.len() as i32,
        EntityKind::Creeper(m) => m.enemy.lvlcols.len() as i32,
        EntityKind::Skeleton(m) => m.enemy.lvlcols.len() as i32,
        EntityKind::Snake(m) => m.enemy.lvlcols.len() as i32,
        EntityKind::Knight(_) => 5, // JAVA: Knight overrides getMaxLevel() to 5
        EntityKind::AirWizard(_) => 2, // JAVA: AirWizard overrides getMaxLevel() to 2
        _ => 1,                     // passive mobs
    }
}

/// Java `new Spawner(m)`.
pub fn new(mob: Entity, random: &mut JavaRandom) -> Entity {
    let name = format!("{} Spawner", mob_class_name(&mob));
    let sprite = Sprite::new(20, 8, 2, 2, mob.c.col, 0);
    let furniture = FurnitureData::new(&name, sprite);

    let lvl = match mob.enemy_mob() {
        Some(e) => e.lvl,
        None => 1,
    };
    let max_lvl = max_mob_level(&mob);

    let c = furniture_common(furniture.sprite.color, 7, 2);
    let spawn_tick =
        random.next_int_bound(MAX_SPAWN_INTERVAL - MIN_SPAWN_INTERVAL + 1) + MIN_SPAWN_INTERVAL;
    Entity::new(
        c,
        EntityKind::Spawner(SpawnerData {
            furniture,
            mob: Box::new(mob),
            health: 100,
            lvl,
            max_mob_level: max_lvl,
            spawn_tick,
        }),
    )
}
