//! Port of `fdoom.screen.InventoryMenu`.
//!
//! Java subclassed `Menu`, overriding `tick` (drop-one/drop-stack handling) and
//! `removeSelectedEntry` (kept in sync with the Inventory). Menus aren't polymorphic in
//! the port, so this module builds the plain `Menu` (`new`/`rebuilt`) and exposes the
//! tick override's extra key handling (`tick_drops`), which `PlayerInvDisplay` and
//! `ContainerDisplay` call at the point where Java's `Display.tick` would have run
//! `menus[selection].tick(input)`.

use crate::core::game::Game;
use crate::entity::Entity;
use crate::gfx::{Screen, sprite_sheet};
use crate::item::Inventory;
use crate::level;

use super::entry::handle;
use super::entry::item_entry::ItemEntry;
use super::item_list_menu;
use super::menu::Menu;

/// The holder's inventory (Java `inv`; every call site passed `holder.getInventory()`).
pub(super) fn inventory_of(holder: &Entity) -> &Inventory {
    if holder.is_player() {
        &holder.player().inventory
    } else {
        &holder
            .chest()
            .expect("inventory holder must be a player or chest")
            .inventory
    }
}

pub(super) fn inventory_of_mut(holder: &mut Entity) -> &mut Inventory {
    if holder.is_player() {
        &mut holder.player_mut().inventory
    } else {
        &mut holder
            .chest_mut()
            .expect("inventory holder must be a player or chest")
            .inventory
    }
}

/// Java `new InventoryMenu(holder, inv, title)`.
pub fn new(g: &Game, holder: &Entity, title: &str) -> Menu {
    let entries = ItemEntry::use_items(inventory_of(holder).items());
    item_list_menu::new(g, entries, title)
}

/// Java `new InventoryMenu(model)` — rebuilds from the live inventory, keeping the
/// selection (used by ContainerDisplay.update).
pub fn rebuilt(g: &Game, holder: &Entity, title: &str, selection: i32) -> Menu {
    let mut menu = new(g, holder, title);
    menu.set_selection(selection);
    menu
}

/// QOL: the selected item's row (icon + name, including its stack count), pinned to the
/// panel's bottom border — the counterpart of the title overlapping the top border.
pub fn render_selected_info(menu: &Menu, screen: &mut Screen, g: &mut Game) {
    let Some(entry) = menu.get_cur_entry() else {
        return;
    };
    let b = menu.get_bounds();
    let x = b.left() + sprite_sheet::BOX_WIDTH;
    let y = b.bottom() - sprite_sheet::BOX_WIDTH;
    // smoked-glass backing over the border sprites so the line reads clearly
    screen.darken_rect_screen(
        b.left() + 2,
        y - 1,
        b.width() - 4,
        sprite_sheet::BOX_WIDTH + 1,
        185,
    );
    entry.borrow_mut().render(screen, g, x, y, true);
}

/// The tail of Java `InventoryMenu.tick(input)` (after `super.tick`): drop one item or a
/// whole stack onto the holder's level. `allow_drop_one` is Java's
/// `!(Game.getMenu() instanceof ContainerDisplay)`.
pub fn tick_drops(g: &mut Game, menu: &mut Menu, holder_eid: i32, allow_drop_one: bool) {
    let drop_one = g.input.get_key("drop-one").clicked && allow_drop_one;

    if !(menu.get_num_options() > 0 && (drop_one || g.input.get_key("drop-stack").clicked)) {
        return;
    }

    let creative = g.is_mode("creative");
    let sel = menu.get_selection();

    let mut updated_entry_item = None;
    let mut remove_entry = false;
    let (hx, hy, hlvl, drop);
    {
        let Some(holder) = g.entities.get_mut(holder_eid) else {
            return; // JAVA: `if(entry == null) return;` (take-out reentrancy; see PORTING.md)
        };
        let holder_is_player = holder.is_player();
        hx = holder.c.x;
        hy = holder.c.y;
        hlvl = holder.c.level;

        let inv = inventory_of_mut(holder);
        let inv_item = inv.get_mut(sel);
        let mut d = inv_item.clone();

        if drop_one && d.is_stackable() && d.count() > 1 {
            // just drop one from the stack
            d.set_count(1);
            inv_item.set_count(inv_item.count() - 1);
            // JAVA: the entry shared the inventory's Item object; refresh the clone.
            updated_entry_item = Some(inv_item.clone());
        } else {
            // drop the whole item.
            if !creative || !holder_is_player {
                // JAVA: removeSelectedEntry() — inv.remove(getSelection()) + the menu.
                inv.remove(sel);
                remove_entry = true;
            }
        }
        drop = d;
    }

    if let Some(item) = updated_entry_item {
        menu.update_selected_entry(handle(ItemEntry::new(item)));
    }
    if remove_entry {
        menu.remove_selected_entry();
    }

    if let Some(lvl) = hlvl {
        // JAVA: the isValidClient dropItem packet is dead — holder.getLevel().dropItem.
        level::drop_item(g, lvl, hx, hy, drop);
    }
}
