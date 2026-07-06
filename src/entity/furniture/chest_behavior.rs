//! Behavior of `fdoom.entity.furniture.Chest` (+ the DeathChest `use` override, which the
//! use dispatch routes here).

use crate::core::game::Game;
use crate::entity::{Entity, EntityKind, behavior};

/// Java `Chest.use(player)` — opens the container display.
pub fn use_furniture(g: &mut Game, e: &mut Entity, player: &mut Entity) -> bool {
    // JAVA: DeathChest.use(player) overrides to false — can't open it, just walk into it.
    if matches!(e.kind, EntityKind::DeathChest(_)) {
        return false;
    }
    // JAVA: Game.setMenu(new ContainerDisplay(player, this));
    g.set_menu(crate::screen::container_display::ContainerDisplay::new(
        g, player, e,
    ));
    true
}

/// Java `Chest.die()` — spills the contents, then `super.die()`.
pub fn die(g: &mut Game, e: &mut Entity) {
    if let Some(lvl) = e.c.level {
        let items = e
            .chest()
            .map(|c| c.inventory.items().to_vec())
            .unwrap_or_default();
        // JAVA: level.dropItem(x, y, items.toArray(...))
        for item in items {
            crate::level::drop_item(g, lvl, e.c.x, e.c.y, item);
        }
    }
    behavior::remove_entity(g, e); // JAVA: super.die() (Entity.die → remove)
}
