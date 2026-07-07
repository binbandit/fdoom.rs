//! Behavior of the Campfire furniture (fire wave): fuel burn-down, smoke, flame
//! animation, the rest-by-the-fire stamina bonus, wood refueling, mushroom cooking,
//! and the occasional stray spark that ignites adjacent flammable tiles.

use crate::core::game::Game;
use crate::entity::{Direction, Entity, EntityKind, behavior};
use crate::gfx::Screen;
use crate::item::{Item, ItemKind};
use crate::level::tile::fire;

use super::campfire::{FUEL_PER_WOOD, MAX_FUEL, REST_RADIUS_TILES, ember_sprite, lit_sprite};

/// Smoke cadence while lit (ticks between puffs).
const SMOKE_INTERVAL: i32 = 20;
/// Below one Wood worth of fuel the fire is "low": thin smoke wisps, dimmer story.
const LOW_FUEL: i32 = FUEL_PER_WOOD;
/// 1-in-this per tick: a spark lands on one random neighboring tile (which only
/// matters if that tile is flammable — keep tinder clear of the fire ring).
const SPARK_ODDS: i32 = 500;

/// Campfire tick: base push handling, then fuel burn-down + smoke + stray sparks.
pub fn tick(g: &mut Game, e: &mut Entity) {
    super::behavior::tick(g, e);

    let (was_lit, now_lit, low) = {
        let EntityKind::Campfire(cf) = &mut e.kind else {
            return;
        };
        let was_lit = cf.fuel > 0;
        if was_lit {
            cf.fuel -= 1;
            if cf.fuel == 0 {
                cf.furniture.sprite = ember_sprite();
            }
        }
        (was_lit, cf.fuel > 0, cf.fuel < LOW_FUEL)
    };
    if was_lit && !now_lit {
        g.notify_all("The campfire dies to embers");
    }
    if !now_lit {
        return;
    }
    let Some(lvl) = e.c.level else { return };

    // smoke: a puff every SMOKE_INTERVAL ticks, thin wisps when the fuel runs low
    if g.tick_count % SMOKE_INTERVAL == 0 {
        let jx = g.random.next_int_bound(7) - 3;
        let smoke =
            crate::entity::particle::new_smoke_particle(e.c.x + jx, e.c.y - 8, low, &mut g.random);
        g.level_mut(lvl).add(smoke, lvl);
    }

    // stray sparks: rarely, one random neighboring tile catches (if flammable)
    if g.random.next_int_bound(SPARK_ODDS) == 0 {
        let (xt, yt) = (e.c.x >> 4, e.c.y >> 4);
        let d = g.random.next_int_bound(8);
        let (dx, dy) = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ][d as usize];
        fire::ignite(g, lvl, xt + dx, yt + dy);
    }
}

/// Campfire render: two-frame flame while lit, the stored ember sprite otherwise.
pub fn render(g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let EntityKind::Campfire(cf) = &e.kind else {
        return;
    };
    if cf.fuel > 0 {
        let sprite = if (g.tick_count >> 4) & 1 == 0 {
            lit_sprite()
        } else {
            super::campfire::lit_sprite_b()
        };
        sprite.render(screen, e.c.x - 8, e.c.y - 8);
    } else {
        super::behavior::render(g, screen, e);
    }
}

/// Campfire `interact` (attack key while facing it):
/// - holding Wood: feed the fire (relights an ember; capped at [`MAX_FUEL`]);
/// - holding a Mushroom over a lit fire: roast it (1 -> 1 Cooked Mushroom);
/// - empty-handed: read the fuel state.
pub fn interact(
    g: &mut Game,
    e: &mut Entity,
    player: &mut Entity,
    item: &mut Option<Item>,
    _attack_dir: Direction,
) -> bool {
    let EntityKind::Campfire(cf) = &mut e.kind else {
        return false;
    };
    let creative = g.is_mode("creative");

    match item {
        Some(it)
            if matches!(it.kind, ItemKind::Stackable { .. })
                && it.get_name().eq_ignore_ascii_case("Wood") =>
        {
            if cf.fuel >= MAX_FUEL {
                g.notify_all("The fire is piled high already");
                return true;
            }
            let was_ember = cf.fuel == 0;
            cf.fuel = (cf.fuel + FUEL_PER_WOOD).min(MAX_FUEL);
            cf.furniture.sprite = lit_sprite();
            consume_one(item, creative);
            g.notify_all(if was_ember {
                "The embers flare back to life"
            } else {
                "The fire crackles higher"
            });
            true
        }
        Some(it) if cf.fuel > 0 && it.get_name().eq_ignore_ascii_case("Mushroom") => {
            consume_one(item, creative);
            let cooked = crate::item::registry::get(g, "Cooked Mushroom");
            player.player_mut().inventory.add(cooked);
            g.notify_all("The mushroom roasts slowly over the fire...");
            true
        }
        None => {
            let msg = if cf.fuel == 0 {
                "Only cold embers remain"
            } else if cf.fuel < LOW_FUEL {
                "The fire is burning low..."
            } else if cf.fuel >= MAX_FUEL - FUEL_PER_WOOD {
                "The fire roars, piled with wood"
            } else {
                "The fire burns steadily"
            };
            g.notify_all(msg);
            true
        }
        _ => false,
    }
}

/// One item off the held stack (the `StackableItem.interactOn` convention).
fn consume_one(item: &mut Option<Item>, creative: bool) {
    if creative {
        return;
    }
    if let Some(it) = item {
        if let Some(count) = it.count_mut() {
            *count -= 1;
        }
        if it.is_depleted() {
            *item = None;
        }
    }
}

/// Is `e` (a player, usually) within resting range of any lit campfire on its level?
/// Read by the player's stamina recharge for the 2x rest bonus.
pub fn near_lit_campfire(g: &Game, e: &Entity) -> bool {
    let Some(lvl) = e.c.level else { return false };
    g.entities.entities_on_level(lvl).any(|o| {
        matches!(&o.kind, EntityKind::Campfire(cf) if cf.fuel > 0)
            && behavior::is_within(o, REST_RADIUS_TILES, e)
    })
}
