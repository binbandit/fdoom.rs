//! Port of `fdoom.entity.mob` — the inheritance-layer data structs. Shared behavior
//! functions (`mob_tick` etc.) live here too once ported; leaf behaviors live in the
//! per-mob files.

pub mod cow;
pub mod deer;
pub mod feral_hound;
pub mod ghost;
pub mod glow_worm;
pub mod knight;
pub mod marsh_lurker;
pub mod night_wisp;
pub mod pig;
pub mod player;
pub mod player_behavior;
pub mod sheep;
pub mod snake;
pub mod stone_golem;
pub mod zombie;

use crate::entity::Direction;
use crate::gfx::sprite::MobAnims;

/// Movement personality consumed by the shared MobAi/EnemyMob layers
/// (`behavior::mobai_tick_base` / `enemy_mob_tick_base`). `Classic` is the untouched
/// original walk — zombies, knights, and every passive mob keep it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MovementStyle {
    /// The original MobAi walk, byte-for-byte.
    #[default]
    Classic,
    /// Orbit the target at ~4 tiles, with a periodic straight lunge (Feral Hound).
    Circle,
    /// Wide sinusoidal drift while moving (Night Wisp).
    Curve,
    /// Hold still ~2 s, then a fast burst of movement (Marsh Lurker).
    FreezeBurst,
    /// Tight S-curve side-offsets while moving (the snake family).
    Slither,
    /// Gentle vertical bob layered on the drift (Ghost).
    SineFloat,
}

/// Fields of the Java `Mob` base class.
#[derive(Debug, Clone)]
pub struct MobData {
    /// All the mob's sprites, by direction then walk animation state.
    pub sprites: &'static MobAnims,
    pub walk_dist: i32,
    pub dir: Direction,
    /// Delay after being hurt that prevents further damage for a short time.
    pub hurt_time: i32,
    pub x_knockback: i32,
    pub y_knockback: i32,
    pub health: i32,
    pub max_health: i32,
    pub walk_time: i32,
    pub speed: i32,
    /// Incremented whenever tick() is called; effectively the age in ticks.
    pub tick_time: i32,
}

impl MobData {
    /// Java `Mob(sprites, health)`; the Java constructor also set xr=4, yr=3 on Entity.
    pub fn new(sprites: &'static MobAnims, health: i32) -> MobData {
        MobData {
            sprites,
            walk_dist: 0,
            dir: Direction::Down,
            hurt_time: 0,
            x_knockback: 0,
            y_knockback: 0,
            health,
            max_health: health,
            walk_time: 1,
            speed: 1,
            tick_time: 0,
        }
    }
}

/// Fields of the Java `MobAi` class.
#[derive(Debug, Clone)]
pub struct MobAiData {
    pub mob: MobData,
    pub random_walk_time: i32,
    pub random_walk_chance: i32,
    pub random_walk_duration: i32,
    pub xa: i32,
    pub ya: i32,
    pub lifetime: i32,
    pub age: i32,
    pub slowtick: bool,
    /// How this mob carries itself while moving (see [`MovementStyle`]).
    pub movement_style: MovementStyle,
}

impl MobAiData {
    /// Java `MobAi(sprites, maxHealth, lifetime, rwTime, rwChance)`.
    pub fn new(
        sprites: &'static MobAnims,
        max_health: i32,
        lifetime: i32,
        rw_time: i32,
        rw_chance: i32,
    ) -> MobAiData {
        let mut mob = MobData::new(sprites, max_health);
        mob.walk_time = 2;
        MobAiData {
            mob,
            random_walk_time: 0,
            random_walk_chance: rw_chance,
            random_walk_duration: rw_time,
            xa: 0,
            ya: 0,
            lifetime,
            age: 0,
            slowtick: false,
            movement_style: MovementStyle::Classic,
        }
    }
}

/// Fields of the Java `EnemyMob` class.
#[derive(Debug, Clone)]
pub struct EnemyMobData {
    pub ai: MobAiData,
    pub lvl: i32,
    pub lvlcols: Vec<i32>,
    pub detect_dist: i32,
}

impl EnemyMobData {
    /// Java `EnemyMob(lvl, sprites, lvlcols, health, isFactor, detectDist, lifetime, rwTime, rwChance)`.
    /// `diff_idx` is `Settings.getIdx("diff")`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        lvl: i32,
        sprites: &'static MobAnims,
        lvlcols: &[i32],
        health: i32,
        is_factor: bool,
        detect_dist: i32,
        lifetime: i32,
        rw_time: i32,
        rw_chance: i32,
        diff_idx: i32,
    ) -> (EnemyMobData, i32) {
        let max_health = if is_factor {
            (if lvl == 0 { 1 } else { lvl * lvl }) * health * (2f64.powi(diff_idx) as i32)
        } else {
            health
        };
        let ai = MobAiData::new(sprites, max_health, lifetime, rw_time, rw_chance);
        let lvl = if lvl == 0 { 1 } else { lvl };
        let col = lvlcols[(lvl - 1) as usize];
        (
            EnemyMobData {
                ai,
                lvl,
                lvlcols: lvlcols.to_vec(),
                detect_dist,
            },
            col, // Java set `this.col` (EntityCommon) from lvlcols
        )
    }

    /// Java 8-arg constructor (lifetime = 60 * normSpeed).
    #[allow(clippy::too_many_arguments)]
    pub fn with_default_lifetime(
        lvl: i32,
        sprites: &'static MobAnims,
        lvlcols: &[i32],
        health: i32,
        is_factor: bool,
        detect_dist: i32,
        rw_time: i32,
        rw_chance: i32,
        diff_idx: i32,
    ) -> (EnemyMobData, i32) {
        Self::new(
            lvl,
            sprites,
            lvlcols,
            health,
            is_factor,
            detect_dist,
            60 * crate::core::updater::NORM_SPEED,
            rw_time,
            rw_chance,
            diff_idx,
        )
    }

    /// Java 5-arg constructor (isFactor=true, rwTime=60, rwChance=200).
    pub fn simple(
        lvl: i32,
        sprites: &'static MobAnims,
        lvlcols: &[i32],
        health: i32,
        detect_dist: i32,
        diff_idx: i32,
    ) -> (EnemyMobData, i32) {
        Self::with_default_lifetime(
            lvl,
            sprites,
            lvlcols,
            health,
            true,
            detect_dist,
            60,
            200,
            diff_idx,
        )
    }
}

/// Fields of the Java `PassiveMob` class.
#[derive(Debug, Clone)]
pub struct PassiveMobData {
    pub ai: MobAiData,
    pub color: i32,
}

impl PassiveMobData {
    /// Java `PassiveMob(sprites, color, healthFactor)`; returns (data, col).
    pub fn new(
        sprites: &'static MobAnims,
        color: i32,
        health_factor: i32,
        diff_idx: i32,
    ) -> (PassiveMobData, i32) {
        let ai = MobAiData::new(
            sprites,
            5 + health_factor * diff_idx,
            5 * 60 * crate::core::updater::NORM_SPEED,
            45,
            40,
        );
        (PassiveMobData { ai, color }, color)
    }
}
