//! Behavior of the scavenge containers ([`super::scav_container`]): one-time rummage
//! that spills the seeded finds, then an emptied color state (dungeon-chest pattern).

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Entity, EntityKind};
use crate::gfx::Screen;

use super::scav_container::ScavKind;

/// The first use rummages: every held item spills onto the floor at the container
/// (the pickup magnet does the rest) and the container flips to its emptied look.
/// Later uses find nothing — the loot is strictly one-time.
pub fn use_furniture(g: &mut Game, e: &mut Entity, _player: &mut Entity) -> bool {
    let (kind, searched) = match &e.kind {
        EntityKind::ScavContainer(sc) => (sc.kind, sc.searched),
        _ => return false,
    };

    if searched {
        g.notifications.push("Nothing but dust.".to_string());
        return true;
    }

    let items = match &mut e.kind {
        EntityKind::ScavContainer(sc) => {
            sc.searched = true;
            sc.chest.furniture.sprite = kind.sprite(true);
            let items = sc.chest.inventory.items().to_vec();
            sc.chest.inventory.clear_inv();
            items
        }
        _ => return false,
    };
    e.c.col = kind.col(true);

    let flavor = match kind {
        ScavKind::Crate => "You pry the crate open...",
        ScavKind::Barrel => "You tip the barrel over...",
        ScavKind::Cupboard => "You rifle through the cupboard...",
    };
    g.notifications.push(flavor.to_string());
    g.play_sound(Sound::Craft);

    if let Some(lvl) = e.c.level {
        for item in items {
            crate::level::drop_item(g, lvl, e.c.x, e.c.y, item);
        }
    }
    true
}

/// Refreshes the color from the searched state first (dungeon-chest pattern), so a
/// loaded or glove-carried container always renders its true state.
pub fn render(g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let col = match &e.kind {
        EntityKind::ScavContainer(sc) => sc.kind.col(sc.searched),
        _ => return,
    };
    if let EntityKind::ScavContainer(sc) = &mut e.kind {
        sc.chest.furniture.sprite.color = col;
    }
    e.c.col = col;
    super::behavior::render(g, screen, e);
}
