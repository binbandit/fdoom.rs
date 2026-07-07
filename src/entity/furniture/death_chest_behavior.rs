//! Behavior of `fdoom.entity.furniture.DeathChest`.

use crate::core::game::Game;
use crate::core::updater::NORM_SPEED;
use crate::entity::{Entity, EntityKind, behavior};
use crate::gfx::{Screen, color, font};

/// Java `DeathChest.tick()` — expiry countdown + red flash color.
pub fn tick(g: &mut Game, e: &mut Entity) {
    super::behavior::tick(g, e);

    let inv_empty = e
        .chest()
        .map(|c| c.inventory.inv_size() == 0)
        .unwrap_or(false);
    if inv_empty {
        behavior::remove_entity(g, e);
    }

    let time = {
        let EntityKind::DeathChest(d) = &mut e.kind else {
            return;
        };
        if d.time < 30 * NORM_SPEED {
            // if there is less than 30 seconds left...
            d.redtick += if d.reverse { -1 } else { 1 }; // inc/dec-rement redtick, changing the red shading.

            // set the chest color based on redtick's value
            let expcol = 100 * (d.redtick / 5 + 1);
            d.chest.furniture.sprite.color = color::get4(-1, expcol, expcol + 100, expcol + 200);

            // these two statements keep the red color oscillating.
            if d.redtick > 13 {
                d.reverse = true;
            }
            if d.redtick < 0 {
                d.reverse = false;
            }
        }

        if d.time > 0 {
            d.time -= 1; // decrement the time if it is not already zero.
        }
        d.time
    };

    if time == 0 {
        // remove the death chest when the time expires, spilling all the contents.
        super::chest_behavior::die(g, e);
    }
}

/// Java `DeathChest.render(screen)` — chest + remaining-time text.
pub fn render(g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    super::behavior::render(g, screen, e);
    let EntityKind::DeathChest(d) = &e.kind else {
        return;
    };
    let time_string = format!("{}S", d.time / NORM_SPEED);
    font::draw(
        &time_string,
        screen,
        e.c.x - font::text_width(&time_string) / 2,
        e.c.y - font::text_height() - e.c.bounds().height() / 2,
        color::WHITE,
    );
}

/// Java `DeathChest.touchedBy(other)` — a player walking into it retrieves the items.
pub fn touched_by(g: &mut Game, e: &mut Entity, by: &mut Entity) {
    if by.is_player() {
        let inv = e.chest().map(|c| c.inventory.clone()).unwrap_or_default();
        by.player_mut().inventory.add_all(&inv);
        behavior::remove_entity(g, e);
        g.notifications.push("Death chest retrieved!".to_string());
    }
}

/// Java `DeathChest.take(player)` — can't grab a death chest.
pub fn take(_g: &mut Game, _e: &mut Entity, _player: &mut Entity) {}
