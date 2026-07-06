//! Item use behaviors: Java `Item.interact` / `interactOn` and subclasses.
//! TODO(port:item-behavior): full port pending.

use crate::core::game::Game;
use crate::entity::{Direction, Entity};
use crate::item::Item;

/// Java `item.interact(player, entity, attackDir)` (PowerGlove picks up furniture).
pub fn item_interact_entity(g: &mut Game, item: &mut Item, player: &mut Entity, entity: &mut Entity, attack_dir: Direction) -> bool {
    let _ = (g, item, player, entity, attack_dir); // TODO(port:item-behavior)
    false
}

/// Java `item.interactOn(tile, level, xt, yt, player, attackDir)`.
#[allow(clippy::too_many_arguments)]
pub fn item_interact_on_tile(g: &mut Game, item: &mut Item, lvl: usize, xt: i32, yt: i32, player: &mut Entity, attack_dir: Direction) -> bool {
    let _ = (g, item, lvl, xt, yt, player, attack_dir); // TODO(port:item-behavior)
    false
}
