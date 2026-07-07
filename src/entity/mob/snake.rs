//! The snake family. Port lineage: `fdoom.entity.mob.Snake` became four variants in
//! the mob-life wave — one `EntityKind::Snake` with a [`SnakeVariant`] tag, so the
//! shared enemy layers and the save format stay simple. Save-name compatibility: the
//! Cave Serpent (the mines/dungeon snake, where the classic Snake spawned) still
//! saves/loads as `"Snake"`; the surface variants get their own names.
//!
//! - **Grass Snake** — small, green, harmless plains/forest ambience; flees the player.
//! - **Adder** — marsh/savanna; its bite drains 2 stamina on top of the damage.
//! - **Rattler** — desert; spawns coiled and still, rattles a warning when the player
//!   comes within 4 tiles, strikes for 2x snake damage at close range, then slithers.
//! - **Cave Serpent** — mines/dungeon; the classic snake, bulked up: more health, a
//!   heavier bite, dark palette.
//!
//! All variants move with `MovementStyle::Slither` (S-curve side offsets).

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, Sprite, compile_mob_sprite_animations};

use super::{EnemyMobData, MovementStyle};

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(18, 18));

/// The Rattler's coiled pose — a single 16x16 frame (artgen cell (4,20)).
static COILED: LazyLock<Sprite> = LazyLock::new(|| Sprite::mob(4, 20, 2, 2, 0));

/// Grass Snake: fresh spring greens.
pub const GRASS_COLS: [i32; 5] = [
    color::get4(-1, 20, 141, 353),
    color::get4(-1, 20, 141, 453),
    color::get4(-1, 30, 252, 464),
    color::get4(-1, 121, 232, 554),
    color::get4(-1, 0, 131, 343),
];

/// Adder: olive and bog-brown.
pub const ADDER_COLS: [i32; 5] = [
    color::get4(-1, 110, 321, 542),
    color::get4(-1, 110, 331, 553),
    color::get4(-1, 100, 221, 442),
    color::get4(-1, 210, 432, 554),
    color::get4(-1, 0, 211, 433),
];

/// Rattler: sun-bleached desert tans.
pub const RATTLER_COLS: [i32; 5] = [
    color::get4(-1, 210, 432, 553),
    color::get4(-1, 220, 442, 554),
    color::get4(-1, 110, 322, 543),
    color::get4(-1, 100, 431, 552),
    color::get4(-1, 0, 321, 542),
];

/// Cave Serpent: deep-mine blue-blacks (the classic Snake's slot, restyled dark).
pub const CAVE_COLS: [i32; 5] = [
    color::get4(-1, 0, 112, 334),
    color::get4(-1, 0, 122, 344),
    color::get4(-1, 0, 213, 435),
    color::get4(-1, 0, 313, 535),
    color::get4(-1, 0, 322, 544),
];

/// Rattle warning range (tiles -> px).
const RATTLE_RANGE: i32 = 4 * 16;
/// Uncoil-and-strike range (~1.5 tiles).
const STRIKE_RANGE: i32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnakeVariant {
    Grass,
    Adder,
    Rattler,
    Cave,
}

impl SnakeVariant {
    /// Save-format class name (`"Snake"` kept for the Cave Serpent — save compat).
    pub fn class_name(self) -> &'static str {
        match self {
            SnakeVariant::Grass => "GrassSnake",
            SnakeVariant::Adder => "Adder",
            SnakeVariant::Rattler => "Rattler",
            SnakeVariant::Cave => "Snake",
        }
    }

    fn lvlcols(self) -> &'static [i32; 5] {
        match self {
            SnakeVariant::Grass => &GRASS_COLS,
            SnakeVariant::Adder => &ADDER_COLS,
            SnakeVariant::Rattler => &RATTLER_COLS,
            SnakeVariant::Cave => &CAVE_COLS,
        }
    }

    /// Health factor fed to the `EnemyMobData` formula (`lvl² * health * 2^diff`).
    fn health_factor(self, lvl: i32) -> i32 {
        match self {
            SnakeVariant::Grass => 2, // a couple of hits
            SnakeVariant::Adder => 6,
            SnakeVariant::Rattler => 7,
            // the bulked-up classic (the old Snake used 7/8)
            SnakeVariant::Cave => {
                if lvl > 1 {
                    12
                } else {
                    10
                }
            }
        }
    }

    fn detect_dist(self) -> i32 {
        match self {
            SnakeVariant::Grass => 80, // flee radius
            SnakeVariant::Adder | SnakeVariant::Rattler | SnakeVariant::Cave => 100,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnakeData {
    pub enemy: EnemyMobData,
    pub variant: SnakeVariant,
    /// Rattler: sitting coiled and still (its spawn state).
    pub coiled: bool,
    /// Rattler: already gave its dry-rattle warning.
    pub rattled: bool,
    /// Rattler: the next touch is the 2x uncoil strike.
    pub strike_primed: bool,
}

/// Java `new Snake(lvl)` — now builds the Cave Serpent (save name "Snake").
pub fn new(g: &Game, lvl: i32) -> Entity {
    new_variant(g, SnakeVariant::Cave, lvl)
}

pub fn new_variant(g: &Game, variant: SnakeVariant, lvl: i32) -> Entity {
    let cols = variant.lvlcols();
    // FIX: clamp to the lvlcols range — Java indexed lvlcols[lvl-1] unchecked and an
    // out-of-range level (e.g. from a hand-edited save) crashed the game.
    let lvl = lvl.clamp(1, cols.len() as i32);
    let diff_idx = g.settings.get_idx("diff");
    let (mut enemy, col) = EnemyMobData::simple(
        lvl,
        &SPRITES,
        cols,
        variant.health_factor(lvl),
        variant.detect_dist(),
        diff_idx,
    );
    enemy.ai.movement_style = MovementStyle::Slither;
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    let coiled = variant == SnakeVariant::Rattler;
    Entity::new(
        c,
        EntityKind::Snake(SnakeData {
            enemy,
            variant,
            coiled,
            rattled: false,
            strike_primed: false,
        }),
    )
}

/// The base touch damage every biting variant scales from: Java Snake's
/// `lvl + diffIdx`.
fn base_damage(g: &Game, this_e: &Entity) -> i32 {
    let lvl = this_e.enemy_mob().map(|em| em.lvl).unwrap_or(1);
    lvl + g.settings.get_idx("diff")
}

fn variant_of(e: &Entity) -> SnakeVariant {
    match &e.kind {
        EntityKind::Snake(d) => d.variant,
        _ => SnakeVariant::Cave,
    }
}

pub fn tick(g: &mut Game, e: &mut Entity) {
    let variant = variant_of(e);

    // Coiled Rattler: no wandering, no chasing — just the Mob base (health, knockback,
    // hurt flash) plus the proximity watch.
    let coiled = matches!(&e.kind, EntityKind::Snake(d) if d.coiled);
    if coiled {
        if !crate::entity::behavior::mob_tick_base(g, e) {
            return;
        }
        if let Some(ai) = e.mob_ai_mut() {
            ai.xa = 0;
            ai.ya = 0;
        }

        let Some(pid) = crate::entity::behavior::get_closest_player(g, e) else {
            return;
        };
        let Some((px, py)) = g.entities.get(pid).map(|p| (p.c.x, p.c.y)) else {
            return;
        };
        let (xd, yd) = (px - e.c.x, py - e.c.y);
        let d2 = xd * xd + yd * yd;

        // the dry-rattle warning, once, when the player wanders within 4 tiles
        let needs_rattle = d2 < RATTLE_RANGE * RATTLE_RANGE
            && matches!(&e.kind, EntityKind::Snake(d) if !d.rattled);
        if needs_rattle {
            g.notify_all("A dry rattle rises from the sand...");
            g.play_sound(crate::core::io::sound::Sound::MonsterHurt);
            if let EntityKind::Snake(d) = &mut e.kind {
                d.rattled = true;
            }
        }

        // adjacent: uncoil and strike (the primed touch hits for 2x), then slither
        if d2 < STRIKE_RANGE * STRIKE_RANGE {
            if let EntityKind::Snake(d) = &mut e.kind {
                d.coiled = false;
                d.strike_primed = true;
            }
            if let Some(ai) = e.mob_ai_mut() {
                ai.xa = xd.signum();
                ai.ya = yd.signum();
            }
        }
        return;
    }

    if !crate::entity::behavior::enemy_mob_tick_base(g, e) {
        return;
    }

    // Grass Snake: harmless, so the chase acceleration the enemy layer just set gets
    // inverted — it flees instead of hunting.
    if variant == SnakeVariant::Grass {
        if let Some(pid) = crate::entity::behavior::get_closest_player(g, e) {
            if let Some((px, py)) = g.entities.get(pid).map(|p| (p.c.x, p.c.y)) {
                let (xd, yd) = (px - e.c.x, py - e.c.y);
                let detect = e.enemy_mob().map(|em| em.detect_dist).unwrap_or(80);
                if xd * xd + yd * yd < detect * detect {
                    if let Some(ai) = e.mob_ai_mut() {
                        ai.xa = -xd.signum();
                        ai.ya = -yd.signum();
                    }
                }
            }
        }
    }
}

/// Custom render only for the Rattler's coiled pose; everything else is the shared
/// enemy render (which also handles the tall-grass stealth clip).
pub fn render(g: &mut Game, screen: &mut crate::gfx::Screen, e: &mut Entity) {
    let coiled = matches!(&e.kind, EntityKind::Snake(d) if d.coiled);
    if !coiled {
        crate::entity::behavior::enemy_mob_render(g, screen, e);
        return;
    }
    if let Some(em) = e.enemy_mob() {
        e.c.col = em.lvlcols[(em.lvl - 1) as usize];
    }
    let col = if e.mob().map(|m| m.hurt_time).unwrap_or(0) > 0 {
        color::WHITE
    } else {
        e.c.col
    };
    COILED.render_color(screen, e.c.x - 8, e.c.y - 11, col);
}

/// Java `Snake.touchedBy(entity)` — damage is `lvl + diffIdx` (not EnemyMob's
/// `lvl * (hard ? 2 : 1)`), and it does NOT call super. Variant twists: the Grass
/// Snake never bites, the Adder's bite also drains stamina, a primed Rattler strike
/// doubles the damage, and the Cave Serpent bites harder.
pub fn touched_by(g: &mut Game, this_e: &mut Entity, by: &mut Entity) {
    if !by.is_player() {
        return;
    }
    let variant = variant_of(this_e);
    if variant == SnakeVariant::Grass {
        return; // harmless — it only wants to get away
    }

    let mut damage = base_damage(g, this_e);
    if variant == SnakeVariant::Cave {
        damage += 2;
    }
    if let EntityKind::Snake(d) = &mut this_e.kind {
        if d.strike_primed {
            damage *= 2;
            d.strike_primed = false;
        }
    }

    // the Adder's venom saps the legs: 2 stamina, gated by the same hurt-cooldown
    // window as the damage so a lingering touch doesn't drain per-tick
    let drains = variant == SnakeVariant::Adder
        && by.mob().map(|m| m.hurt_time).unwrap_or(1) == 0
        && !g.is_mode("creative");

    let attack_dir = crate::entity::behavior::get_attack_dir(this_e, by);
    super::player_behavior::hurt_by_mob(g, by, this_e, damage, attack_dir);

    if drains {
        let pd = by.player_mut();
        pd.stamina = (pd.stamina - 2).max(0);
    }
}

/// Java `Snake.die()` — scale + rare key, tuned down for the harmless Grass Snake.
pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::entity::behavior::mobai_drop_items;
    use crate::item::registry;

    let variant = variant_of(e);
    if variant == SnakeVariant::Grass {
        // ambience, not loot: a rare single scale, nothing else
        if g.random.next_int_bound(4) == 0 {
            let scale = registry::get(g, "scale");
            mobai_drop_items(g, e, 1, 1, &[scale]);
        }
        crate::entity::behavior::enemy_mob_die(g, e);
        return;
    }

    let num = if g.settings.get("diff").as_str() == "Hard" {
        1
    } else {
        0
    };
    let scale = registry::get(g, "scale");
    mobai_drop_items(g, e, num, num + 1, &[scale]);

    let lvl = e.enemy_mob().map(|em| em.lvl).unwrap_or(1);
    let diff_idx = g.settings.get_idx("diff");
    if g.random.next_int_bound(30 / lvl / (diff_idx + 1)) == 0 {
        let key = registry::get(g, "key");
        mobai_drop_items(g, e, 1, 1, &[key]);
    }

    crate::entity::behavior::enemy_mob_die(g, e);
}
