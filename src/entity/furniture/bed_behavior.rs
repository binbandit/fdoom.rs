//! Behavior of `fdoom.entity.furniture.Bed`. TODO(port:entity-behavior)

use crate::core::game::Game;
use crate::entity::Entity;

/// Java `Bed.use(player)`.
pub fn use_furniture(g: &mut Game, e: &mut Entity, player: &mut Entity) -> bool {
    let _ = (g, e, player); // TODO(port:entity-behavior)
    false
}

/// Java `Bed.sleeping()`.
pub fn sleeping(g: &Game) -> bool {
    g.bed_state.players_awake == 0
}

/// Java `Bed.inBed(player)`.
pub fn in_bed(g: &Game, player_eid: i32) -> bool {
    g.bed_state.sleeping_players.contains_key(&player_eid)
}

/// Java `Bed.restorePlayers()`.
pub fn restore_players(g: &mut Game) {
    let _ = g; // TODO(port:entity-behavior)
}

/// Java `Bed.removePlayers()`.
pub fn remove_players(g: &mut Game) {
    g.bed_state.sleeping_players.clear();
}
