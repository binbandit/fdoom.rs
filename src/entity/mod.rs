//! Port of the `fdoom.entity` package.
//!
//! Java's `Entity` class hierarchy becomes `EntityCommon` (the base fields) + the
//! `EntityKind` enum (one variant per concrete Java class), with inheritance layers as
//! nested data structs (see PORTING.md). All live entities are stored in one
//! `EntityArena` keyed by eid; ticking uses the take-out pattern (`Game::with_entity`).

pub mod behavior;
pub mod direction;
pub mod fireflies;
pub mod furniture;
pub mod item_entity;
pub mod item_entity_behavior;
pub mod mob;
pub mod particle;
pub mod particle_behavior;
pub mod projectile;
pub mod projectile_behavior;

use crate::gfx::Rectangle;
use crate::rng::Rng;

pub use direction::Direction;
pub use furniture::FurnitureData;
pub use item_entity::ItemEntityData;
pub use mob::MobData;
pub use particle::{ParticleData, TextParticleData};
pub use projectile::{ArrowData, ZapData};

/// The fields of the Java `Entity` base class.
#[derive(Debug, Clone)]
pub struct EntityCommon {
    /// Entity coordinates are per pixel; each tile is 16x16 entity pixels.
    pub x: i32,
    pub y: i32,
    /// x, y radius of the entity (collision box half-size).
    pub xr: i32,
    pub yr: i32,
    pub removed: bool,
    /// Index into `g.levels` (Java: the `level` reference).
    pub level: Option<usize>,
    /// Current color.
    pub col: i32,
    pub eid: i32,
}

impl EntityCommon {
    pub fn new(xr: i32, yr: i32) -> EntityCommon {
        EntityCommon {
            x: 0,
            y: 0,
            xr,
            yr,
            removed: true,
            level: None,
            col: 0,
            eid: -1,
        }
    }

    /// Java `getBounds()`.
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(
            self.x,
            self.y,
            self.xr * 2,
            self.yr * 2,
            Rectangle::CENTER_DIMS,
        )
    }

    /// Java `isTouching(area)`.
    pub fn is_touching(&self, area: &Rectangle) -> bool {
        area.intersects(&self.bounds())
    }
}

#[derive(Debug, Clone)]
pub enum EntityKind {
    Player(Box<mob::player::PlayerData>),
    // passive mobs
    Cow(mob::cow::CowData),
    Pig(mob::pig::PigData),
    Sheep(mob::sheep::SheepData),
    GlowWorm(mob::glow_worm::GlowWormData),
    // enemy mobs
    Zombie(mob::zombie::ZombieData),
    Snake(mob::snake::SnakeData),
    Knight(mob::knight::KnightData),
    MarshLurker(mob::marsh_lurker::MarshLurkerData),
    FeralHound(mob::feral_hound::FeralHoundData),
    StoneGolem(mob::stone_golem::StoneGolemData),
    NightWisp(mob::night_wisp::NightWispData),
    Ghost(mob::ghost::GhostData),
    // free-floating things
    ItemEntity(ItemEntityData),
    Arrow(ArrowData),
    Zap(ZapData),
    /// Ambient glow-speck swarm — not a mob (no health/collision, never mob-capped).
    Fireflies(fireflies::FirefliesData),
    // particles
    Particle(ParticleData),
    TextParticle(TextParticleData),
    // furniture
    Furniture(FurnitureData),
    Campfire(furniture::campfire::CampfireData),
    Chest(furniture::chest::ChestData),
    DeathChest(furniture::death_chest::DeathChestData),
    DungeonChest(furniture::dungeon_chest::DungeonChestData),
    Bed(furniture::bed::BedData),
    Crafter(furniture::crafter::CrafterData),
    Lantern(furniture::lantern::LanternData),
    Spawner(furniture::spawner::SpawnerData),
    Tnt(furniture::tnt::TntData),
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub c: EntityCommon,
    pub kind: EntityKind,
}

impl Entity {
    pub fn new(c: EntityCommon, kind: EntityKind) -> Entity {
        Entity { c, kind }
    }

    /* ----- Java `instanceof` predicates ----- */

    pub fn is_player(&self) -> bool {
        matches!(self.kind, EntityKind::Player(_))
    }

    /// Java `instanceof Mob`.
    pub fn is_mob(&self) -> bool {
        self.mob().is_some()
    }

    /// Java `instanceof MobAi`.
    pub fn is_mob_ai(&self) -> bool {
        self.mob_ai().is_some()
    }

    /// Java `instanceof EnemyMob`.
    pub fn is_enemy_mob(&self) -> bool {
        self.enemy_mob().is_some()
    }

    /// Java `instanceof Furniture`.
    pub fn is_furniture(&self) -> bool {
        self.furniture().is_some()
    }

    /// Java `instanceof Particle`.
    pub fn is_particle(&self) -> bool {
        matches!(
            self.kind,
            EntityKind::Particle(_) | EntityKind::TextParticle(_)
        )
    }

    /// Java `instanceof Chest`.
    pub fn is_chest(&self) -> bool {
        matches!(
            self.kind,
            EntityKind::Chest(_) | EntityKind::DeathChest(_) | EntityKind::DungeonChest(_)
        )
    }

    /* ----- inheritance-layer accessors (Java upcasts) ----- */

    /// The `Mob` layer of this entity, if it is one.
    pub fn mob(&self) -> Option<&MobData> {
        Some(match &self.kind {
            EntityKind::Player(p) => &p.mob,
            EntityKind::Cow(m) => &m.passive.ai.mob,
            EntityKind::Pig(m) => &m.passive.ai.mob,
            EntityKind::Sheep(m) => &m.passive.ai.mob,
            EntityKind::GlowWorm(m) => &m.passive.ai.mob,
            EntityKind::Zombie(m) => &m.enemy.ai.mob,
            EntityKind::Snake(m) => &m.enemy.ai.mob,
            EntityKind::Knight(m) => &m.enemy.ai.mob,
            EntityKind::MarshLurker(m) => &m.enemy.ai.mob,
            EntityKind::FeralHound(m) => &m.enemy.ai.mob,
            EntityKind::StoneGolem(m) => &m.enemy.ai.mob,
            EntityKind::NightWisp(m) => &m.enemy.ai.mob,
            EntityKind::Ghost(m) => &m.enemy.ai.mob,
            _ => return None,
        })
    }

    pub fn mob_mut(&mut self) -> Option<&mut MobData> {
        Some(match &mut self.kind {
            EntityKind::Player(p) => &mut p.mob,
            EntityKind::Cow(m) => &mut m.passive.ai.mob,
            EntityKind::Pig(m) => &mut m.passive.ai.mob,
            EntityKind::Sheep(m) => &mut m.passive.ai.mob,
            EntityKind::GlowWorm(m) => &mut m.passive.ai.mob,
            EntityKind::Zombie(m) => &mut m.enemy.ai.mob,
            EntityKind::Snake(m) => &mut m.enemy.ai.mob,
            EntityKind::Knight(m) => &mut m.enemy.ai.mob,
            EntityKind::MarshLurker(m) => &mut m.enemy.ai.mob,
            EntityKind::FeralHound(m) => &mut m.enemy.ai.mob,
            EntityKind::StoneGolem(m) => &mut m.enemy.ai.mob,
            EntityKind::NightWisp(m) => &mut m.enemy.ai.mob,
            EntityKind::Ghost(m) => &mut m.enemy.ai.mob,
            _ => return None,
        })
    }

    /// The `MobAi` layer of this entity, if it is one.
    pub fn mob_ai(&self) -> Option<&mob::MobAiData> {
        Some(match &self.kind {
            EntityKind::Cow(m) => &m.passive.ai,
            EntityKind::Pig(m) => &m.passive.ai,
            EntityKind::Sheep(m) => &m.passive.ai,
            EntityKind::GlowWorm(m) => &m.passive.ai,
            EntityKind::Zombie(m) => &m.enemy.ai,
            EntityKind::Snake(m) => &m.enemy.ai,
            EntityKind::Knight(m) => &m.enemy.ai,
            EntityKind::MarshLurker(m) => &m.enemy.ai,
            EntityKind::FeralHound(m) => &m.enemy.ai,
            EntityKind::StoneGolem(m) => &m.enemy.ai,
            EntityKind::NightWisp(m) => &m.enemy.ai,
            EntityKind::Ghost(m) => &m.enemy.ai,
            _ => return None,
        })
    }

    pub fn mob_ai_mut(&mut self) -> Option<&mut mob::MobAiData> {
        Some(match &mut self.kind {
            EntityKind::Cow(m) => &mut m.passive.ai,
            EntityKind::Pig(m) => &mut m.passive.ai,
            EntityKind::Sheep(m) => &mut m.passive.ai,
            EntityKind::GlowWorm(m) => &mut m.passive.ai,
            EntityKind::Zombie(m) => &mut m.enemy.ai,
            EntityKind::Snake(m) => &mut m.enemy.ai,
            EntityKind::Knight(m) => &mut m.enemy.ai,
            EntityKind::MarshLurker(m) => &mut m.enemy.ai,
            EntityKind::FeralHound(m) => &mut m.enemy.ai,
            EntityKind::StoneGolem(m) => &mut m.enemy.ai,
            EntityKind::NightWisp(m) => &mut m.enemy.ai,
            EntityKind::Ghost(m) => &mut m.enemy.ai,
            _ => return None,
        })
    }

    /// The `EnemyMob` layer of this entity, if it is one.
    pub fn enemy_mob(&self) -> Option<&mob::EnemyMobData> {
        Some(match &self.kind {
            EntityKind::Zombie(m) => &m.enemy,
            EntityKind::Snake(m) => &m.enemy,
            EntityKind::Knight(m) => &m.enemy,
            EntityKind::MarshLurker(m) => &m.enemy,
            EntityKind::FeralHound(m) => &m.enemy,
            EntityKind::StoneGolem(m) => &m.enemy,
            EntityKind::NightWisp(m) => &m.enemy,
            EntityKind::Ghost(m) => &m.enemy,
            _ => return None,
        })
    }

    pub fn enemy_mob_mut(&mut self) -> Option<&mut mob::EnemyMobData> {
        Some(match &mut self.kind {
            EntityKind::Zombie(m) => &mut m.enemy,
            EntityKind::Snake(m) => &mut m.enemy,
            EntityKind::Knight(m) => &mut m.enemy,
            EntityKind::MarshLurker(m) => &mut m.enemy,
            EntityKind::FeralHound(m) => &mut m.enemy,
            EntityKind::StoneGolem(m) => &mut m.enemy,
            EntityKind::NightWisp(m) => &mut m.enemy,
            EntityKind::Ghost(m) => &mut m.enemy,
            _ => return None,
        })
    }

    /// The `PassiveMob` layer of this entity, if it is one.
    pub fn passive_mob(&self) -> Option<&mob::PassiveMobData> {
        Some(match &self.kind {
            EntityKind::Cow(m) => &m.passive,
            EntityKind::Pig(m) => &m.passive,
            EntityKind::Sheep(m) => &m.passive,
            EntityKind::GlowWorm(m) => &m.passive,
            _ => return None,
        })
    }

    /// The `Furniture` layer of this entity, if it is one.
    pub fn furniture(&self) -> Option<&FurnitureData> {
        Some(match &self.kind {
            EntityKind::Furniture(f) => f,
            EntityKind::Campfire(cf) => &cf.furniture,
            EntityKind::Chest(c) => &c.furniture,
            EntityKind::DeathChest(c) => &c.chest.furniture,
            EntityKind::DungeonChest(c) => &c.chest.furniture,
            EntityKind::Bed(b) => &b.furniture,
            EntityKind::Crafter(c) => &c.furniture,
            EntityKind::Lantern(l) => &l.furniture,
            EntityKind::Spawner(s) => &s.furniture,
            EntityKind::Tnt(t) => &t.furniture,
            _ => return None,
        })
    }

    pub fn furniture_mut(&mut self) -> Option<&mut FurnitureData> {
        Some(match &mut self.kind {
            EntityKind::Furniture(f) => f,
            EntityKind::Campfire(cf) => &mut cf.furniture,
            EntityKind::Chest(c) => &mut c.furniture,
            EntityKind::DeathChest(c) => &mut c.chest.furniture,
            EntityKind::DungeonChest(c) => &mut c.chest.furniture,
            EntityKind::Bed(b) => &mut b.furniture,
            EntityKind::Crafter(c) => &mut c.furniture,
            EntityKind::Lantern(l) => &mut l.furniture,
            EntityKind::Spawner(s) => &mut s.furniture,
            EntityKind::Tnt(t) => &mut t.furniture,
            _ => return None,
        })
    }

    /// The `Chest` layer of this entity, if it is one.
    pub fn chest(&self) -> Option<&furniture::chest::ChestData> {
        Some(match &self.kind {
            EntityKind::Chest(c) => c,
            EntityKind::DeathChest(c) => &c.chest,
            EntityKind::DungeonChest(c) => &c.chest,
            _ => return None,
        })
    }

    pub fn chest_mut(&mut self) -> Option<&mut furniture::chest::ChestData> {
        Some(match &mut self.kind {
            EntityKind::Chest(c) => c,
            EntityKind::DeathChest(c) => &mut c.chest,
            EntityKind::DungeonChest(c) => &mut c.chest,
            _ => return None,
        })
    }

    /// Java `Player` downcast.
    pub fn player(&self) -> &mob::player::PlayerData {
        match &self.kind {
            EntityKind::Player(p) => p,
            _ => panic!("entity is not the player"),
        }
    }

    pub fn player_mut(&mut self) -> &mut mob::player::PlayerData {
        match &mut self.kind {
            EntityKind::Player(p) => p,
            _ => panic!("entity is not the player"),
        }
    }
}

/// The single arena holding all live entities (replaces the per-level Java entity sets;
/// see PORTING.md).
#[derive(Default)]
pub struct EntityArena {
    map: std::collections::HashMap<i32, Entity>,
}

impl EntityArena {
    pub fn clear(&mut self) {
        self.map.clear();
    }

    pub fn contains(&self, eid: i32) -> bool {
        self.map.contains_key(&eid)
    }

    pub fn get(&self, eid: i32) -> Option<&Entity> {
        self.map.get(&eid)
    }

    pub fn get_mut(&mut self, eid: i32) -> Option<&mut Entity> {
        self.map.get_mut(&eid)
    }

    /// Take an entity out for the take-out tick pattern.
    pub fn take(&mut self, eid: i32) -> Option<Entity> {
        self.map.remove(&eid)
    }

    pub fn put_back(&mut self, e: Entity) {
        self.map.insert(e.c.eid, e);
    }

    /// Insert an entity, assigning a unique eid if it has none
    /// (Java `Network.generateUniqueEntityId`).
    pub fn insert(&mut self, mut e: Entity, random: &mut Rng) -> i32 {
        if e.c.eid < 0 {
            e.c.eid = self.generate_unique_entity_id(random);
        }
        let eid = e.c.eid;
        self.map.insert(eid, e);
        eid
    }

    fn generate_unique_entity_id(&self, random: &mut Rng) -> i32 {
        loop {
            let eid = random.next_int();
            // JAVA: ids must be positive; 0 is reserved for the main player.
            if eid > 0 && !self.map.contains_key(&eid) {
                return eid;
            }
        }
    }

    /// Remove an entity permanently.
    pub fn delete(&mut self, eid: i32) {
        self.map.remove(&eid);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.map.values()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Entity> {
        self.map.values_mut()
    }

    /// Java `Level.getEntityArray()` — all (non-removed) entity ids on a level.
    pub fn ids_on_level(&self, level: usize) -> Vec<i32> {
        self.map
            .values()
            .filter(|e| e.c.level == Some(level))
            .map(|e| e.c.eid)
            .collect()
    }

    pub fn entities_on_level(&self, level: usize) -> impl Iterator<Item = &Entity> {
        self.map.values().filter(move |e| e.c.level == Some(level))
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
