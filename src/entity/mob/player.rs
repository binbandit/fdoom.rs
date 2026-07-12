//! Port of `fdoom.entity.mob.Player` — data + constructor. The (large) tick/attack/render
//! behavior lives in the player behavior functions.
//!
//! Java's `Player` held the `InputHandler`; here input is reached through `g.input`.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Direction, Entity, EntityCommon, EntityKind};
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};
use crate::item::{Inventory, Item, PotionType};

use super::MobData;

pub const PLAYER_HURT_TIME: i32 = 30;
pub const INTERACT_DIST: i32 = 12;
pub const ATTACK_DIST: i32 = 20;

/// Java `mtm` — time given to increase multiplier before it goes back to 1.
pub const MTM: i32 = 300;
pub const MAX_MULTIPLIER: i32 = 50;

pub const MAX_STAT: i32 = 10;
pub const MAX_HEALTH: i32 = MAX_STAT;
pub const MAX_STAMINA: i32 = MAX_STAT;
pub const MAX_HUNGER: i32 = MAX_STAT;
pub const MAX_ARMOR: i32 = 100;

pub const MAX_STAMINA_RECHARGE: i32 = 10;
pub const MAX_HUNGER_TICKS: i32 = 400;
/// hungerStamCnt required to lose a burger, by difficulty.
pub const MAX_HUNGER_STAMS: [i32; 3] = [10, 7, 5];
/// ticks before decrementing stamHungerTicks, by difficulty.
pub const HUNGER_TICK_COUNT: [i32; 3] = [120, 30, 10];
/// steps before decrementing stamHungerTicks, by difficulty.
pub const HUNGER_STEP_COUNT: [i32; 3] = [8, 3, 1];
/// min hearts required for hunger to hurt you, by difficulty.
pub const MIN_STARVE_HEALTH: [i32; 3] = [5, 3, 0];

pub static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(0, 14));
pub static CARRY_SPRITES: LazyLock<MobAnims> =
    LazyLock::new(|| compile_mob_sprite_animations(0, 16));
pub static SUIT_SPRITES: LazyLock<MobAnims> =
    LazyLock::new(|| compile_mob_sprite_animations(18, 20));
pub static CARRY_SUIT_SPRITES: LazyLock<MobAnims> =
    LazyLock::new(|| compile_mob_sprite_animations(18, 22));

#[derive(Debug, Clone)]
pub struct PlayerData {
    pub mob: MobData,

    /// the number of coordinate squares to move; each tile is 16x16.
    pub move_speed: f64,
    score: i32,

    pub multiplier_time: i32,
    multiplier: i32,

    /// spawn position, in tile coordinates; saved from the first spawn.
    pub spawnx: i32,
    pub spawny: i32,

    pub skinon: bool,

    pub inventory: Inventory,

    pub active_item: Option<Item>,
    pub attack_item: Option<Item>,
    pub prev_item: Option<Item>,

    pub attack_time: i32,
    pub attack_dir: Direction,
    /// Melee-swing sweep countdown (render-only juice): while positive, an extra
    /// slash arc detaches and travels into the facing tile. Set by `attack()`,
    /// ticked down alongside `attack_time`; never saved.
    pub swing_flash: i32,

    pub on_stair_delay: i32,

    pub hunger: i32,
    pub stamina: i32,
    pub armor: i32,
    pub armor_damage_buffer: i32,
    /// the color/type of armor being worn (Java `curArmor: ArmorItem`).
    pub cur_armor: Option<Item>,

    pub stamina_recharge: i32,
    pub stamina_recharge_delay: i32,

    pub hunger_stam_cnt: i32,
    pub stam_hunger_ticks: i32,
    pub step_count: i32,
    pub hunger_charge_delay: i32,
    pub hunger_starve_delay: i32,

    pub potioneffects: HashMap<PotionType, i32>,
    pub showpotioneffects: bool,
    pub cooldowninfo: i32,
    pub regentick: i32,

    pub shirt_color: i32,

    /// First-day onboarding cues (session-only, never saved). `grass_cue_delay`
    /// counts down to the tall-grass hint — armed only for brand-new worlds
    /// (world.rs), 0 = disarmed. The flags one-shot the fiber and cord hints.
    pub grass_cue_delay: i32,
    pub fiber_cue_done: bool,
    pub cord_cue_done: bool,

    /// Temperature wave (session-only, never saved — body temperature is recomputed
    /// from the world): the last band that fired a cue, and the spacing between cues.
    pub temp_prev_band: i32,
    pub temp_cue_cooldown: i32,
}

impl PlayerData {
    /// Java `getScore()`.
    pub fn get_score(&self) -> i32 {
        self.score
    }

    /// Java `setScore(score)`.
    pub fn set_score(&mut self, score: i32) {
        self.score = score;
    }

    /// Java `addScore(points)` — applies the multiplier.
    pub fn add_score(&mut self, points: i32, score_mode: bool) {
        self.score += points * self.get_multiplier(score_mode);
    }

    /// Java `getMultiplier()`.
    pub fn get_multiplier(&self, score_mode: bool) -> i32 {
        if score_mode { self.multiplier } else { 1 }
    }

    /// Java `resetMultiplier()`.
    pub fn reset_multiplier(&mut self) {
        self.multiplier = 1;
        self.multiplier_time = MTM;
    }

    /// Java `addMultiplier(value)`.
    pub fn add_multiplier(&mut self, value: i32, score_mode: bool) {
        if !score_mode {
            return;
        }
        self.multiplier = MAX_MULTIPLIER.min(self.multiplier + value);
        self.multiplier_time = self.multiplier_time.max(MTM - 5);
    }

    /// Java `tickMultiplier()` (the ISONLINE branch is always false in this build).
    pub fn tick_multiplier(&mut self, paused: bool) {
        if !paused && self.multiplier > 1 {
            if self.multiplier_time != 0 {
                self.multiplier_time -= 1;
            }
            if self.multiplier_time <= 0 {
                self.reset_multiplier();
            }
        }
    }

    pub fn set_multiplier(&mut self, mult: i32) {
        self.multiplier = mult;
    }

    /// Java `getDebugHunger()`.
    pub fn get_debug_hunger(&self) -> String {
        format!("{}_{}", self.hunger_stam_cnt, self.stam_hunger_ticks)
    }
}

/// Java `new Player(previousInstance, input)`. Copies the spawn point from the previous
/// player, if any. Creative-mode inventory fill is done by the caller-side world code.
pub fn new(g: &Game, previous: Option<&PlayerData>) -> Entity {
    let mut c = EntityCommon::new(4, 3);
    c.x = 24;
    c.y = 24;

    let mob = MobData::new(&SPRITES, MAX_HEALTH);
    let diff_idx = g.settings.get_idx("diff");

    let mut data = PlayerData {
        mob,
        move_speed: 1.0,
        score: 0,
        multiplier_time: MTM,
        multiplier: 1,
        spawnx: 0,
        spawny: 0,
        skinon: false,
        inventory: Inventory::new_player(),
        active_item: None,
        attack_item: None,
        prev_item: None,
        attack_time: 0,
        attack_dir: Direction::Down, // matches the initial facing direction
        swing_flash: 0,
        on_stair_delay: 0,
        hunger: MAX_HUNGER,
        stamina: MAX_STAMINA,
        armor: 0,
        armor_damage_buffer: 0,
        cur_armor: None,
        stamina_recharge: 0,
        stamina_recharge_delay: 0,
        hunger_stam_cnt: MAX_HUNGER_STAMS[diff_idx as usize],
        stam_hunger_ticks: MAX_HUNGER_TICKS,
        step_count: 0,
        hunger_charge_delay: 0,
        hunger_starve_delay: 0,
        potioneffects: HashMap::new(),
        showpotioneffects: true,
        cooldowninfo: 0,
        regentick: 0,
        shirt_color: 110,
        grass_cue_delay: 0,
        fiber_cue_done: false,
        cord_cue_done: false,
        temp_prev_band: 0,
        temp_cue_cooldown: 0,
    };

    if let Some(prev) = previous {
        data.spawnx = prev.spawnx;
        data.spawny = prev.spawny;
    }

    Entity::new(c, EntityKind::Player(Box::new(data)))
}
