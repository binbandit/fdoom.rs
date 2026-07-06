//! Behavior of the Java `Furniture` base class (tick/render/push/take/use dispatch).

use crate::core::game::Game;
use crate::entity::{Direction, Entity, EntityKind, behavior};
use crate::gfx::Screen;
use crate::item::{ItemKind, registry};

/// Java `Furniture.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    let push_dir = {
        let Some(f) = e.furniture_mut() else { return };
        let d = f.push_dir;
        f.push_dir = Direction::None;
        if f.push_time > 0 {
            f.push_time -= 1;
        } else {
            f.multi_push_time = 0;
        }
        d
    };
    behavior::entity_move(g, e, push_dir.x(), push_dir.y());
}

/// Java `Furniture.render(screen)`.
pub fn render(_g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let Some(f) = e.furniture() else { return };
    f.sprite.render(screen, e.c.x - 8, e.c.y - 8);
}

/// Java `Furniture.tryPush(player)`.
pub fn try_push(_g: &mut Game, e: &mut Entity, player: &mut Entity) {
    let player_dir = player.mob().map(|m| m.dir).unwrap_or(Direction::None);
    let Some(f) = e.furniture_mut() else { return };
    if f.push_time == 0 {
        f.push_dir = player_dir; // set pushDir to the player's dir
        f.push_time = 10;
        f.multi_push_time = 10;
    }
}

/// Java `Furniture.take(player)` — used by the power glove to pick up furniture.
pub fn take(g: &mut Game, e: &mut Entity, player: &mut Entity) {
    behavior::remove_entity(g, e); // remove this from the world

    let creative = g.is_mode("creative");
    let pd = player.player_mut();
    if !creative {
        if let Some(active) = pd.active_item.take() {
            if !matches!(active.kind, ItemKind::PowerGlove) {
                pd.inventory.add_at(0, active);
            }
        }
    }
    // make this furniture the player's current item
    pd.active_item = Some(registry::new_furniture_item(e.clone()));
}

/// Java `Furniture.use(player)` dispatch — called when the player presses MENU nearby.
/// Returns true if the furniture handled it.
pub fn use_furniture(g: &mut Game, e: &mut Entity, player: &mut Entity) -> bool {
    match &e.kind {
        EntityKind::DungeonChest(_) => super::dungeon_chest_behavior::use_furniture(g, e, player),
        EntityKind::Chest(_) | EntityKind::DeathChest(_) => {
            super::chest_behavior::use_furniture(g, e, player)
        }
        EntityKind::Crafter(_) => super::crafter_behavior::use_furniture(g, e, player),
        EntityKind::Bed(_) => super::bed_behavior::use_furniture(g, e, player),
        EntityKind::Spawner(_) => super::spawner_behavior::use_furniture(g, e, player),
        _ => false,
    }
}
