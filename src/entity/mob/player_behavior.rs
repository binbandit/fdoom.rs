//! Behavior of `fdoom.entity.mob.Player` — tick/attack/render/hurt/death.
//! TODO(port:entity-behavior): full port in progress.

use crate::core::game::Game;
use crate::entity::{Direction, Entity};
use crate::gfx::Screen;

pub fn tick(g: &mut Game, e: &mut Entity) {
    let _ = (g, e); // TODO(port:entity-behavior)
}

pub fn render(g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let _ = (g, screen, e); // TODO(port:entity-behavior)
}

/// Java `Player.hurt(damage, attackDir)` via a mob attacker.
pub fn hurt_by_mob(g: &mut Game, player: &mut Entity, attacker: &mut Entity, damage: i32, attack_dir: Direction) {
    let _ = attacker;
    do_hurt(g, player, damage, attack_dir);
}

/// Java `Player.doHurt(damage, attackDir)`.
pub fn do_hurt(g: &mut Game, player: &mut Entity, damage: i32, attack_dir: Direction) {
    let _ = (g, player, damage, attack_dir); // TODO(port:entity-behavior)
}

/// Java `Player.die()`.
pub fn die(g: &mut Game, e: &mut Entity) {
    crate::entity::behavior::remove_entity(g, e); // TODO(port:entity-behavior)
}

/// Java `Player.getLightRadius()`.
pub fn get_light_radius(e: &Entity) -> i32 {
    let _ = e;
    5 // TODO(port:entity-behavior): furniture-in-hand radius
}

/// Java `Player.pickupItem(itemEntity)`.
pub fn pickup_item(g: &mut Game, player: &mut Entity, item_entity: &mut Entity) {
    let _ = (g, player, item_entity); // TODO(port:entity-behavior)
}
