//! The survival screen — one panel on E with four tabs: PACK / WEAR / CRAFT / SELF
//! (docs/UI_REDESIGN.md §3). L2 shipped the shell plus the PACK and SELF panes; L3
//! landed WEAR as real equip slots (HEAD / BODY / HELD, CHARM reserved): ENTER on a
//! wearable in PACK or on a WEAR slot equips/unequips instantly — no world
//! interaction, no silent failure. CRAFT hosts the personal recipe list with a cost
//! card in the detail column; L4 added station context — using an oven/furnace/
//! anvil/workbench/enchanter/loom opens this same screen on CRAFT with the
//! station's recipe set and its name as a sub-header, so PACK stays one tab away
//! while you shuffle materials at a bench (fixes UI_REDESIGN J11).

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::core::temperature;
use crate::core::updater::NORM_SPEED;
use crate::entity::Entity;
use crate::entity::mob::player::{MAX_ARMOR, PlayerData, SPRITES, WearSlot, wear_slot_for};
use crate::gfx::screen::fill_rect;
use crate::gfx::{Rectangle, Screen, color, font};
use crate::item::{Inventory, Item, ItemKind, PotionType, Recipe, registry};
use crate::level::{self, infinite_gen};

use super::display::{Display, DisplayBase};
use super::entry::recipe_entry::RecipeEntry;
use super::menu::{Menu, MenuBuilder};
use super::rel_pos::RelPos;

/* ------------------------------- geometry (the spec) ------------------------------- */
// Every coordinate below comes from the mockups (`target/verify/ui_mock/mock_*.png`),
// which were composed at real screen coordinates.

// (pub(crate): the container variant in `container_display` is the same shell.)
pub(crate) const PANEL_X: i32 = 8;
pub(crate) const PANEL_Y: i32 = 8;
pub(crate) const PANEL_W: i32 = 272;
pub(crate) const PANEL_H: i32 = 176;

pub(crate) const TAB_Y: i32 = 13;
const UNDERLINE_Y: i32 = 22;

pub(crate) const BODY_Y: i32 = 28;
pub(crate) const BODY_BOTTOM: i32 = 166;
pub(crate) const LIST_X: i32 = 12;
const LIST_RIGHT: i32 = 146;
const DIVIDER_X: i32 = 148;
const DETAIL_X: i32 = 154;
pub(crate) const DETAIL_RIGHT: i32 = 276;

pub(crate) const ROW_H: i32 = 10;
const MAX_ROWS: i32 = (BODY_BOTTOM - BODY_Y) / ROW_H; // 13

pub(crate) const LEGEND_Y: i32 = 170;

/// Runtime survival/container shell geometry. Classic dimensions intentionally map
/// byte-for-byte to the original constants; taller panels expose more list rows.
#[allow(dead_code)] // fields are consumed incrementally by the individual pane renderers.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Layout {
    pub panel_x: i32,
    pub panel_y: i32,
    pub panel_w: i32,
    pub panel_h: i32,
    pub body_y: i32,
    pub body_bottom: i32,
    pub list_x: i32,
    pub list_right: i32,
    pub divider_x: i32,
    pub detail_x: i32,
    pub detail_right: i32,
    pub row_h: i32,
    pub max_rows: i32,
}

impl Layout {
    pub(crate) fn new(w: i32, h: i32) -> Self {
        let panel_w = (w - 16).clamp(PANEL_W, 336);
        let panel_h = (h - 16).clamp(PANEL_H, 224);
        let panel_x = (w - panel_w) / 2;
        let panel_y = (h - panel_h) / 2;
        let body_y = panel_y + 20;
        let body_bottom = panel_y + panel_h - 18;
        let divider_x = panel_x + panel_w / 2 + 4;
        let row_h = if panel_h > 200 { 11 } else { ROW_H };
        Self {
            panel_x,
            panel_y,
            panel_w,
            panel_h,
            body_y,
            body_bottom,
            list_x: panel_x + 4,
            list_right: divider_x - 2,
            divider_x,
            detail_x: divider_x + 6,
            detail_right: panel_x + panel_w - 4,
            row_h,
            max_rows: ((body_bottom - body_y) / row_h).max(1),
        }
    }
}

/// Category headers, dim gold (reads as a label, not a row).
const COL_HEADER: i32 = color::get(-1, 431);
/// Warmth/shade stat lines, the mock's indigo (readable on the dark glass —
/// `color::BLUE` is too deep there).
const COL_WARMTH: i32 = color::get(-1, 225);
/// The active tab's underline (raw RGB — drawn as a pixel fill).
pub(crate) const GOLD_RGB: i32 = 0xE0C84A;
pub(crate) const DIVIDER_RGB: i32 = 0x4A4A4A;
pub(crate) const SCROLLBAR_RGB: i32 = 0x9A9A9A;

/// The pixel width left for a list entry starting at `x` before the craft pane's
/// divider — recipe entries clip their text here (overflow rule; see font::draw_fit).
pub(crate) fn list_clip_width(x: i32) -> i32 {
    (DIVIDER_X - 2 - x).max(8)
}

/* ------------------------------------- tabs ------------------------------------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Pack,
    Wear,
    Craft,
    SelfPane,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Pack, Tab::Wear, Tab::Craft, Tab::SelfPane];

    fn label(self) -> &'static str {
        match self {
            Tab::Pack => "PACK",
            Tab::Wear => "WEAR",
            Tab::Craft => "CRAFT",
            Tab::SelfPane => "SELF",
        }
    }
}

/* --------------------------------- the pack model --------------------------------- */

#[derive(Clone, Copy)]
enum PackRow {
    Header(&'static str),
    /// Index into the player's inventory.
    Item(i32),
}

const CATEGORY_LABELS: [&str; 4] = ["TOOLS", "MATERIALS", "FOOD", "GEAR"];

/// Which category an item files under, as an index into [`CATEGORY_LABELS`].
/// Derived from `ItemKind` — no data changes (UI_REDESIGN §3.1).
fn category_of(item: &Item) -> usize {
    match item.kind {
        ItemKind::Tool { .. } | ItemKind::PowerGlove | ItemKind::Bucket { .. } => 0,
        ItemKind::Food { .. } | ItemKind::Medical { .. } | ItemKind::Potion { .. } => 2,
        ItemKind::Armor { .. } | ItemKind::Clothing { .. } => 3,
        _ => 1, // materials and placeables
    }
}

/// The item's name without the baked-in stack count ("30 PLANK" -> "PLANK"); counts
/// get their own right-aligned column on the PACK pane (fixes UI_REDESIGN J7).
pub(crate) fn bare_name(g: &Game, item: &Item) -> String {
    let full = item.get_display_name(g);
    let full = full.trim();
    if item.is_stackable() {
        match full.split_once(' ') {
            Some((_, name)) => name.to_string(),
            None => full.to_string(),
        }
    } else {
        full.to_string()
    }
}

/// One plain-words line for the detail card: what the item is for and how it's used.
fn info_line(item: &Item) -> String {
    match &item.kind {
        ItemKind::Tool { .. } => "A TOOL. HOLD IT TO USE IT.".to_string(),
        ItemKind::Food { heal, .. } => format!("EAT WHILE HELD: +{heal} FOOD."),
        ItemKind::Medical { heal, .. } => format!("USE WHILE HELD: +{heal} HEALTH."),
        ItemKind::Armor { .. } => "PROTECTIVE GEAR. ENTER WEARS IT.".to_string(),
        ItemKind::Clothing { .. } => "SHIRT DYE. ENTER APPLIES IT.".to_string(),
        ItemKind::Potion { ptype, .. } => {
            format!(
                "DRINK WHILE HELD: {} EFFECT.",
                ptype.enum_name().to_uppercase()
            )
        }
        ItemKind::TileItem { .. } | ItemKind::Torch { .. } => {
            "PLACE IT ON FITTING GROUND.".to_string()
        }
        ItemKind::Bucket { .. } => "SCOOPS AND POURS LIQUIDS.".to_string(),
        ItemKind::Furniture { .. } => "PLACE IT IN THE WORLD.".to_string(),
        ItemKind::Book { .. } => "READ IT WHILE HELD.".to_string(),
        _ => "CRAFTING MATERIAL.".to_string(),
    }
}

/* --------------------------------- the wear model --------------------------------- */
// UI_REDESIGN §3.2: the slot list is HEAD / BODY / HELD / CHARM (CHARM reserved,
// ships disabled). Slot classes come from `player::wear_slot_for`; clothing is not
// a slot — it stays an instant shirt dye offered from the pack.

/// The navigable wear rows (CHARM is rendered but never selectable).
const WEAR_ROWS: i32 = 3;
const WEAR_HEAD: usize = 0;
const WEAR_BODY: usize = 1;
const WEAR_HELD: usize = 2;

fn wear_row_slot(row: usize) -> Option<WearSlot> {
    match row {
        WEAR_HEAD => Some(WearSlot::Head),
        WEAR_BODY => Some(WearSlot::Body),
        _ => None,
    }
}

/// Split one unit off a pack stack (or take the whole item when it's the last one).
fn take_one(inv: &mut Inventory, idx: i32) -> Item {
    let item = inv.get_mut(idx);
    if item.is_stackable() && item.count() > 1 {
        let mut one = item.clone();
        one.set_count(1);
        let left = item.count() - 1;
        item.set_count(left);
        one
    } else {
        inv.remove(idx)
    }
}

/// The FITS-ON list's one-line effect. The by-name warmth/shade values mirror
/// `core::temperature`'s COAT_SHIFT / HAT_SHIFT band shifts.
fn wear_effect_line(item: &Item) -> String {
    match &item.kind {
        ItemKind::Armor { armor, .. } => match item.get_name() {
            "Fur Coat" => "+2 WARMTH".to_string(),
            "Straw Hat" => "+1 SHADE".to_string(),
            _ => format!("{} HITS", (armor * MAX_ARMOR as f32) as i32),
        },
        ItemKind::Clothing { .. } => "DYES SHIRT".to_string(),
        _ => String::new(),
    }
}

/// Does this pack item belong on the FITS ON list for a slot? BODY also lists
/// clothing (dye is applied from PACK, but the list shows the option).
fn fits_slot(item: &Item, slot: WearSlot) -> bool {
    wear_slot_for(item) == Some(slot)
        || (slot == WearSlot::Body && matches!(item.kind, ItemKind::Clothing { .. }))
}

/* ------------------------------------ helpers ------------------------------------ */

/// "DeepOcean" -> "DEEP OCEAN" (word-splits camel-case enum names for display).
fn spaced_upper(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

/// The SELF pane's location line: biome + layer on the infinite surface, the mine
/// layer name underground.
fn location_line(g: &Game, e: &Entity) -> String {
    let Some(lvl) = e.c.level else {
        return String::new();
    };
    let Some(level) = g.levels.get(lvl).and_then(|l| l.as_ref()) else {
        return String::new();
    };
    if level.depth == 0 && level.is_infinite() {
        let (xt, yt) = (e.c.x >> 4, e.c.y >> 4);
        let biome = format!("{:?}", infinite_gen::biome_at(g.world_seed, xt, yt));
        format!("{}, SURFACE", spaced_upper(&biome))
    } else if level.depth < 0 {
        format!(
            "{}, B{}",
            level::get_level_name(level.depth).to_uppercase(),
            -level.depth
        )
    } else {
        "SURFACE".to_string()
    }
}

/// The SELF pane's effect rows ("SWIM 1:24"), ordinal-sorted for a stable order.
/// Public so tests can pin the pane's content without scraping pixels.
pub fn effect_lines(pd: &PlayerData) -> Vec<String> {
    let mut effects: Vec<(PotionType, i32)> = pd
        .potioneffects
        .iter()
        .map(|(t, ticks)| (*t, *ticks))
        .collect();
    effects.sort_by_key(|(t, _)| t.ordinal());
    effects
        .iter()
        .map(|(t, ticks)| {
            let secs = ticks / NORM_SPEED;
            format!(
                "{} {}:{:02}",
                t.enum_name().to_uppercase(),
                secs / 60,
                secs % 60
            )
        })
        .collect()
}

/// Warmth-gauge cell colors, one per band step (Freezing -3 .. Scorching +3) —
/// the temperature dot's steady colors (`core::renderer`), comfort a muted green.
fn band_cell_rgb(steps: i32) -> i32 {
    match steps {
        i32::MIN..=-3 => 0x2B4FF0,
        -2 => 0x3E6FE0,
        -1 => 0x5E8FD4,
        0 => 0x6E7B6E,
        1 => 0xD9A85A,
        2 => 0xE07E33,
        _ => 0xE0491F,
    }
}

/* ------------------------------------ the display ------------------------------------ */

pub struct SurvivalDisplay {
    base: DisplayBase,
    tab: Tab,
    player_eid: i32,

    /// The glass panel + 9-slice edge, as an empty framed menu (house styling).
    shell_menu: Menu,

    // PACK
    pack_rows: Vec<PackRow>,
    pack_sel: usize,
    pack_off: usize,
    /// Inventory length the rows were built against — `sync_pack` rebuilds on any
    /// mismatch, so no mutation path (drop, eat, craft, equip, external) can leave
    /// a row indexing past the live inventory (the survival-screen crash class).
    pack_inv_len: i32,

    // WEAR: the selected slot row (HEAD / BODY / HELD)
    wear_sel: usize,

    // CRAFT: the personal recipe list, or a station's set when opened at one
    // (UI_REDESIGN §3.5 — the station's name renders as a sub-header and the
    // list starts one row lower to make room for it)
    recipes: Vec<Rc<RefCell<Recipe>>>,
    craft_menu: Menu,
    station: Option<String>,
    /// THE BENCH's module rack (`Module::VALUES` order), when opened at a bench:
    /// filled sockets light up, empty ones show a dim `?` with a fit hint.
    bench_rack: Option<[bool; 3]>,
    craft_y0: i32,
}

impl SurvivalDisplay {
    /// Opens on PACK — the E key's screen.
    pub fn new(g: &Game, player: &Entity) -> SurvivalDisplay {
        Self::on_tab(g, player, Tab::Pack)
    }

    /// Opens on a specific tab (Z lands on CRAFT, P lands on SELF).
    pub fn on_tab(g: &Game, player: &Entity, tab: Tab) -> SurvivalDisplay {
        Self::build(g, player, tab, None, None, g.recipes.craft.clone())
    }

    /// Opens on CRAFT with a station's recipe set and its name as a sub-header —
    /// what using an oven/furnace/anvil/workbench/enchanter/loom does. The other
    /// tabs stay live, so PACK is reachable without stepping away from the station.
    pub fn at_station(
        g: &Game,
        player: &Entity,
        station: &str,
        recipes: Vec<Recipe>,
    ) -> SurvivalDisplay {
        Self::build(
            g,
            player,
            Tab::Craft,
            Some(station.to_uppercase()),
            None,
            recipes,
        )
    }

    /// Opens at THE BENCH: station context plus the module rack across the top
    /// (`mock_bench.png` — filled modules lit, empty sockets dim with a hint).
    pub fn at_bench(
        g: &Game,
        player: &Entity,
        recipes: Vec<Recipe>,
        fitted: [bool; 3],
    ) -> SurvivalDisplay {
        Self::build(
            g,
            player,
            Tab::Craft,
            Some("THE BENCH".to_string()),
            Some(fitted),
            recipes,
        )
    }

    fn build(
        g: &Game,
        player: &Entity,
        tab: Tab,
        station: Option<String>,
        bench_rack: Option<[bool; 3]>,
        recipes: Vec<Recipe>,
    ) -> SurvivalDisplay {
        let inventory = &player.player().inventory;
        // the bench rack needs a second header row under the station name
        let header_h = match (&station, &bench_rack) {
            (Some(_), Some(_)) => 36, // name + rack + fit hint
            (Some(_), None) => 12,
            _ => 0,
        };
        let craft_y0 = BODY_Y + header_h;

        let mut recipes: Vec<Rc<RefCell<Recipe>>> = recipes
            .into_iter()
            .map(|r| Rc::new(RefCell::new(r)))
            .collect();
        let craft_menu = Self::build_craft_menu(g, &mut recipes, inventory, 0, craft_y0);

        let shell_menu = MenuBuilder::new(true, 0, RelPos::Center, Vec::new())
            .set_bounds(Rectangle::new(
                PANEL_X,
                PANEL_Y,
                PANEL_W,
                PANEL_H,
                Rectangle::CORNER_DIMS,
            ))
            .set_selectable(false)
            .create_menu(g);

        let mut display = SurvivalDisplay {
            base: DisplayBase::new(false, true, Vec::new()),
            tab,
            player_eid: player.c.eid,
            shell_menu,
            pack_rows: Vec::new(),
            pack_sel: 0,
            pack_off: 0,
            pack_inv_len: 0,
            wear_sel: 0,
            recipes,
            craft_menu,
            station,
            bench_rack,
            craft_y0,
        };
        display.rebuild_pack(inventory.items());
        display
    }

    pub fn current_tab(&self) -> Tab {
        self.tab
    }

    /// The station sub-header, when opened at one (None on the personal screen) —
    /// public for tests.
    pub fn station_label(&self) -> Option<&str> {
        self.station.as_deref()
    }

    /// The CRAFT list's product names, in menu order — public for tests.
    pub fn craft_product_names(&self, g: &Game) -> Vec<String> {
        self.recipes
            .iter()
            .map(|r| bare_name(g, &r.borrow().get_product(g)))
            .collect()
    }

    /// PACK rows as text (headers, then bare item names) — public for tests.
    pub fn pack_row_labels(&self, g: &Game) -> Vec<String> {
        self.pack_rows
            .iter()
            .map(|row| match row {
                PackRow::Header(h) => h.to_string(),
                PackRow::Item(i) => g
                    .entities
                    .get(self.player_eid)
                    .map(|p| bare_name(g, p.player().inventory.get(*i)))
                    .unwrap_or_default(),
            })
            .collect()
    }

    fn build_craft_menu(
        g: &Game,
        recipes: &mut [Rc<RefCell<Recipe>>],
        inventory: &crate::item::Inventory,
        selection: i32,
        y0: i32,
    ) -> Menu {
        for r in recipes.iter() {
            r.borrow_mut().check_can_craft(g, inventory);
        }
        // craftable recipes first, original order otherwise
        recipes.sort_by_key(|r| !r.borrow().get_can_craft());
        let entries = RecipeEntry::use_recipes(g, recipes);
        let n = entries.len() as i32;
        MenuBuilder::new(
            false,
            ROW_H - super::entry::entry_height(),
            RelPos::Left,
            entries,
        )
        .set_bounds(Rectangle::new(
            LIST_X,
            y0,
            LIST_RIGHT - LIST_X,
            BODY_BOTTOM - y0,
            Rectangle::CORNER_DIMS,
        ))
        .set_display_length(n.min((BODY_BOTTOM - y0) / ROW_H))
        .set_selectable(true)
        .set_scroll_policies(1.0, false)
        .set_selection(selection)
        .create_menu(g)
    }

    /* --------------------------------- pack state --------------------------------- */

    fn rebuild_pack(&mut self, items: &[Item]) {
        let prev_sel = self.pack_sel;
        self.pack_inv_len = items.len() as i32;
        self.pack_rows.clear();
        for (cat, label) in CATEGORY_LABELS.iter().enumerate() {
            let mut wrote_header = false;
            for (i, item) in items.iter().enumerate() {
                if category_of(item) == cat {
                    if !wrote_header {
                        self.pack_rows.push(PackRow::Header(label));
                        wrote_header = true;
                    }
                    self.pack_rows.push(PackRow::Item(i as i32));
                }
            }
        }

        // reselect the item row nearest the old position (at/after it, else before)
        let item_rows: Vec<usize> = self.item_row_indices();
        self.pack_sel = item_rows
            .iter()
            .find(|&&r| r >= prev_sel)
            .or_else(|| item_rows.last())
            .copied()
            .unwrap_or(0);
        let max_off = (self.pack_rows.len() as i32 - MAX_ROWS).max(0) as usize;
        self.pack_off = self.pack_off.min(max_off);
        if !item_rows.is_empty() {
            self.scroll_pack();
        }
    }

    fn rebuild_pack_from_arena(&mut self, g: &Game) {
        let items: Vec<Item> = g
            .entities
            .get(self.player_eid)
            .map(|p| p.player().inventory.items().to_vec())
            .unwrap_or_default();
        self.rebuild_pack(&items);
    }

    /// Self-heal before any read of the row list: if the live inventory length no
    /// longer matches what the rows were built against, rebuild. A same-length swap
    /// keeps every index in bounds, so this cheap check is sufficient for safety.
    fn sync_pack(&mut self, g: &Game) {
        let live = g
            .entities
            .get(self.player_eid)
            .map(|p| p.player().inventory.inv_size())
            .unwrap_or(0);
        if live != self.pack_inv_len {
            self.rebuild_pack_from_arena(g);
        }
    }

    fn item_row_indices(&self) -> Vec<usize> {
        self.pack_rows
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r, PackRow::Item(_)))
            .map(|(i, _)| i)
            .collect()
    }

    fn selected_inv_idx(&self) -> Option<i32> {
        match self.pack_rows.get(self.pack_sel) {
            Some(PackRow::Item(i)) => Some(*i),
            _ => None,
        }
    }

    fn scroll_pack(&mut self) {
        let sel = self.pack_sel as i32;
        let mut off = self.pack_off as i32;
        if sel < off {
            off = sel;
        }
        if sel >= off + MAX_ROWS {
            off = sel - MAX_ROWS + 1;
        }
        // reveal a category header sitting directly above the top row
        if off > 0
            && off == sel
            && matches!(
                self.pack_rows.get((off - 1) as usize),
                Some(PackRow::Header(_))
            )
        {
            off -= 1;
        }
        self.pack_off = off.max(0) as usize;
    }

    /* --------------------------------- pack input --------------------------------- */

    fn tick_pack(&mut self, g: &mut Game) {
        self.sync_pack(g);
        let item_rows = self.item_row_indices();
        if item_rows.is_empty() {
            return;
        }

        let cur = item_rows
            .iter()
            .position(|&r| r == self.pack_sel)
            .unwrap_or(0) as i32;
        let mut next = cur;
        if g.input.get_key("up").clicked {
            next -= 1;
        }
        if g.input.get_key("down").clicked {
            next += 1;
        }
        if next != cur {
            // wrap at the ends (UI_REDESIGN §3 key map)
            let n = item_rows.len() as i32;
            self.pack_sel = item_rows[next.rem_euclid(n) as usize];
            self.scroll_pack();
            g.play_sound(Sound::Select);
        }

        if g.input.get_key("select").clicked || g.input.get_key("attack").clicked {
            self.hold_selected(g);
            return;
        }

        let drop_one = g.input.get_key("drop-one").clicked;
        if drop_one || g.input.get_key("drop-stack").clicked {
            self.drop_selected(g, drop_one);
        }
    }

    /// ENTER on a pack row. Wearables equip instantly and clothing dyes instantly
    /// (UI_REDESIGN §3.2 — no more attack-the-ground ritual), both keeping the
    /// screen open so the result is visible. Everything else is HOLD IT: the item
    /// becomes the held item and the screen closes so it can be used. Displaced
    /// items always go back into the pack (never silently dropped).
    fn hold_selected(&mut self, g: &mut Game) {
        let Some(idx) = self.selected_inv_idx() else {
            return;
        };
        // stale-row guard: if the inventory shrank under this row list (any path
        // that missed a rebuild), resync instead of indexing out of bounds
        let inv_len = g
            .entities
            .get(self.player_eid)
            .map(|p| p.player().inventory.inv_size())
            .unwrap_or(0);
        if idx >= inv_len {
            self.rebuild_pack_from_arena(g);
            return;
        }
        enum Act {
            Wear,
            Dye(i32),
            /// A legacy bench-shaped station in the pack breaks down into its
            /// bench module (your grandfathered anvil becomes the VICE).
            BreakDown(crate::entity::furniture::crafter::Module),
            Hold,
        }
        let act = match g
            .entities
            .get(self.player_eid)
            .map(|p| &p.player().inventory.get(idx).kind)
        {
            Some(ItemKind::Armor { .. }) => Act::Wear,
            Some(ItemKind::Clothing { player_col, .. }) => Act::Dye(*player_col),
            Some(ItemKind::Furniture { furniture, .. }) => {
                use crate::entity::furniture::crafter::Module;
                let legacy = if let crate::entity::EntityKind::Crafter(c) = &furniture.kind {
                    Module::VALUES
                        .iter()
                        .copied()
                        .find(|m| m.legacy_station() == c.crafter_type)
                } else {
                    None
                };
                match legacy {
                    Some(m) => Act::BreakDown(m),
                    None => Act::Hold,
                }
            }
            Some(_) => Act::Hold,
            None => return,
        };
        match act {
            Act::Wear => self.equip_from_pack(g, idx),
            Act::Dye(col) => self.dye_from_pack(g, idx, col),
            Act::BreakDown(m) => {
                let module = registry::get(g, m.item_name());
                if let Some(player) = g.entities.get_mut(self.player_eid) {
                    let pd = player.player_mut();
                    let station = pd.inventory.remove(idx);
                    pd.inventory.add_at(idx, module);
                    let note = format!(
                        "The {} breaks down into its {}.",
                        station.get_name(),
                        m.item_name()
                    );
                    g.push_ambient(&note);
                    g.play_sound(Sound::Craft);
                }
                self.rebuild_pack_from_arena(g);
            }
            Act::Hold => {
                if let Some(player) = g.entities.get_mut(self.player_eid) {
                    let pd = player.player_mut();
                    let item = pd.inventory.remove(idx);
                    if let Some(prev) = pd.active_item.take() {
                        pd.inventory.add_at(0, prev);
                    }
                    pd.active_item = Some(item);
                }
                g.clear_menu();
            }
        }
    }

    /// Instant equip from the pack: one unit off the selected stack goes onto its
    /// slot; whatever that slot wore returns to the pack. No stamina toll, no fail
    /// state — the WEAR pane is the primary flow (the legacy use-to-wear ritual in
    /// `item::interact` keeps its classic cost).
    fn equip_from_pack(&mut self, g: &mut Game, idx: i32) {
        if let Some(player) = g.entities.get_mut(self.player_eid) {
            let pd = player.player_mut();
            let item = take_one(&mut pd.inventory, idx);
            if let Some(prev) = pd.equip(item) {
                pd.inventory.add_at(0, prev);
            }
        }
        g.play_sound(Sound::Craft);
        self.rebuild_pack_from_arena(g);
    }

    /// Instant shirt dye from the pack (the same rules as the classic
    /// `ClothingItem.interactOn`: no-op on the current color, creative keeps the dye).
    fn dye_from_pack(&mut self, g: &mut Game, idx: i32, player_col: i32) {
        let creative = g.is_mode("creative");
        let mut dyed = false;
        if let Some(player) = g.entities.get_mut(self.player_eid) {
            let pd = player.player_mut();
            if pd.shirt_color != player_col {
                pd.shirt_color = player_col;
                dyed = true;
                if !creative {
                    let item = pd.inventory.get_mut(idx);
                    if item.count() > 1 {
                        let left = item.count() - 1;
                        item.set_count(left);
                    } else {
                        pd.inventory.remove(idx);
                    }
                }
            }
        }
        if dyed {
            g.play_sound(Sound::Craft);
            self.rebuild_pack_from_arena(g);
        }
    }

    /// Q / SHIFT-Q — drop one / drop the stack (the classic inventory drop rules,
    /// remapped through the categorized rows).
    fn drop_selected(&mut self, g: &mut Game, drop_one: bool) {
        let Some(sel) = self.selected_inv_idx() else {
            return;
        };
        // same stale-row guard as hold_selected
        let inv_len = g
            .entities
            .get(self.player_eid)
            .map(|p| p.player().inventory.inv_size())
            .unwrap_or(0);
        if sel >= inv_len {
            self.rebuild_pack_from_arena(g);
            return;
        }
        let creative = g.is_mode("creative");
        let (hx, hy, hlvl, drop);
        {
            let Some(player) = g.entities.get_mut(self.player_eid) else {
                return;
            };
            hx = player.c.x;
            hy = player.c.y;
            hlvl = player.c.level;

            let inv = &mut player.player_mut().inventory;
            let inv_item = inv.get_mut(sel);
            let mut d = inv_item.clone();
            if drop_one && d.is_stackable() && d.count() > 1 {
                d.set_count(1);
                inv_item.set_count(inv_item.count() - 1);
            } else if !creative {
                // creative keeps the original and drops a copy (house container rule)
                inv.remove(sel);
            }
            drop = d;
        }
        if let Some(lvl) = hlvl {
            level::drop_item(g, lvl, hx, hy, drop);
        }
        self.rebuild_pack_from_arena(g);
    }

    /* --------------------------------- wear input --------------------------------- */

    /// UP/DOWN pick a slot; ENTER equips/unequips it instantly; Q takes off.
    /// ENTER on an empty HEAD/BODY wears the first fitting pack item (choosing
    /// among several is PACK's job — ENTER there equips too); on the HELD row both
    /// keys stow the held item back into the pack.
    fn tick_wear(&mut self, g: &mut Game) {
        let mut next = self.wear_sel as i32;
        if g.input.get_key("up").clicked {
            next -= 1;
        }
        if g.input.get_key("down").clicked {
            next += 1;
        }
        if next != self.wear_sel as i32 {
            self.wear_sel = next.rem_euclid(WEAR_ROWS) as usize;
            g.play_sound(Sound::Select);
        }

        let enter = g.input.get_key("select").clicked || g.input.get_key("attack").clicked;
        let take_off = g.input.get_key("drop-one").clicked || g.input.get_key("drop-stack").clicked;
        if !enter && !take_off {
            return;
        }

        let mut acted = false;
        if let Some(player) = g.entities.get_mut(self.player_eid) {
            let pd = player.player_mut();
            match wear_row_slot(self.wear_sel) {
                None => {
                    // HELD: stow the held item (the pack-stash idiom — never lost)
                    if let Some(held) = pd.active_item.take() {
                        pd.inventory.add_at(0, held);
                        acted = true;
                    }
                }
                Some(slot) => {
                    if pd.worn(slot).is_some() {
                        if let Some(prev) = pd.unequip(slot) {
                            pd.inventory.add_at(0, prev);
                            acted = true;
                        }
                    } else if enter {
                        let fit = (0..pd.inventory.inv_size())
                            .find(|&j| wear_slot_for(pd.inventory.get(j)) == Some(slot));
                        if let Some(j) = fit {
                            let item = take_one(&mut pd.inventory, j);
                            if let Some(prev) = pd.equip(item) {
                                pd.inventory.add_at(0, prev);
                            }
                            acted = true;
                        }
                    }
                }
            }
        }
        if acted {
            g.play_sound(Sound::Craft);
            self.rebuild_pack_from_arena(g);
        }
    }

    /* --------------------------------- craft input --------------------------------- */

    fn tick_craft(&mut self, g: &mut Game) {
        self.craft_menu.tick(g);

        if !(g.input.get_key("select").clicked || g.input.get_key("attack").clicked) {
            return;
        }
        if self.recipes.is_empty() {
            return;
        }
        let recipe = self.recipes[self.craft_menu.get_selection() as usize].clone();
        if !recipe.borrow().get_can_craft() {
            return;
        }
        let Some(mut player) = g.entities.take(self.player_eid) else {
            return;
        };
        let crafted;
        {
            let inventory = &mut player.player_mut().inventory;
            crafted = recipe.borrow().craft(g, inventory);
            for r in &self.recipes {
                r.borrow_mut().check_can_craft(g, inventory);
            }
        }
        // First-day thread, cue 3: the first cord points at the crude-tool lashing
        // (same cue the old CraftingDisplay fired).
        if crafted && recipe.borrow().product_name().eq_ignore_ascii_case("cord") {
            let pd = player.player_mut();
            if !pd.cord_cue_done {
                pd.cord_cue_done = true;
                g.push_cue("A sharp stone and a stick would make a tool.");
            }
        }
        g.entities.put_back(player);
        self.rebuild_pack_from_arena(g);
    }

    /* ---------------------------------- rendering ---------------------------------- */

    fn render_tabs(&self, screen: &mut Screen) {
        font::draw("<", screen, PANEL_X + 6, TAB_Y, color::GRAY);
        font::draw(">", screen, PANEL_X + PANEL_W - 14, TAB_Y, color::GRAY);

        let slot_x0 = PANEL_X + 16;
        let slot_w = (PANEL_W - 48) / 4;
        for (i, tab) in Tab::ALL.iter().enumerate() {
            let label = tab.label();
            let w = font::text_width(label);
            let x = slot_x0 + i as i32 * slot_w + (slot_w - w) / 2;
            let active = *tab == self.tab;
            let col = if active {
                color::WHITE
            } else {
                color::DARK_GRAY
            };
            font::draw(label, screen, x, TAB_Y, col);
            if active {
                screen.fill_rect(x - 1, UNDERLINE_Y, w + 2, 1, GOLD_RGB);
            }
        }
    }

    fn render_divider(&self, screen: &mut Screen) {
        fill_rect(
            screen,
            DIVIDER_X,
            BODY_Y,
            1,
            BODY_BOTTOM - BODY_Y,
            DIVIDER_RGB,
        );
    }

    fn render_pack(&mut self, screen: &mut Screen, g: &mut Game) {
        self.sync_pack(g);
        let max_rows = Layout::new(screen.w, screen.h).max_rows;
        self.render_divider(screen);

        if self.item_row_indices().is_empty() {
            // onboarding empty state, carried over from the old inventory panel
            let w = LIST_RIGHT - LIST_X - 4;
            let lines = font::get_lines("Empty - gather something.", w, BODY_BOTTOM - BODY_Y, 1);
            let line_h = font::text_height() + 1;
            let mut y = BODY_Y + (BODY_BOTTOM - BODY_Y - lines.len() as i32 * line_h) / 2;
            for line in &lines {
                let x = LIST_X + (LIST_RIGHT - LIST_X - font::text_width(line)) / 2;
                font::draw(line, screen, x, y, color::DARK_GRAY);
                y += line_h;
            }
            return;
        }

        let sel_item: Option<Item> = g.entities.get(self.player_eid).and_then(|p| {
            self.selected_inv_idx()
                .map(|i| p.player().inventory.get(i).clone())
        });

        // the list
        let end = ((self.pack_off as i32 + max_rows) as usize).min(self.pack_rows.len());
        for (slot, row_idx) in (self.pack_off..end).enumerate() {
            let y = BODY_Y + slot as i32 * ROW_H;
            match self.pack_rows[row_idx] {
                PackRow::Header(label) => {
                    font::draw(label, screen, LIST_X + 2, y, COL_HEADER);
                }
                PackRow::Item(inv_idx) => {
                    let Some(player) = g.entities.get(self.player_eid) else {
                        continue;
                    };
                    let item = player.player().inventory.get(inv_idx).clone();
                    let selected = row_idx == self.pack_sel;
                    if selected {
                        font::draw(">", screen, LIST_X + 2, y, color::YELLOW);
                    }
                    item.sprite.render(screen, LIST_X + 8, y);
                    let col = if selected { color::WHITE } else { color::GRAY };
                    // name clips before the count column so neither ever collides
                    let mut name_w = LIST_RIGHT - 6 - (LIST_X + 17);
                    if item.count() > 1 {
                        let count = item.count().min(999).to_string();
                        let cx = LIST_RIGHT - 6 - font::text_width(&count);
                        font::draw(&count, screen, cx, y, col);
                        name_w = cx - 4 - (LIST_X + 17);
                    }
                    font::draw_fit(&bare_name(g, &item), screen, LIST_X + 17, y, col, name_w);
                }
            }
        }

        // 1px scrollbar on the divider when the list overflows
        let rows = self.pack_rows.len() as i32;
        if rows > max_rows {
            let body_h = BODY_BOTTOM - BODY_Y;
            let bar_h = (body_h * max_rows / rows).max(8);
            let bar_y = BODY_Y + body_h * self.pack_off as i32 / rows;
            fill_rect(screen, DIVIDER_X, bar_y, 1, bar_h, SCROLLBAR_RGB);
        }

        // the detail card
        if let Some(item) = sel_item {
            self.render_item_card(screen, g, &item);
        }
    }

    fn render_item_card(&self, screen: &mut Screen, g: &Game, item: &Item) {
        item.sprite.render(screen, DETAIL_X + 2, BODY_Y + 6);
        let name = bare_name(g, item);
        font::draw_fit(
            &name,
            screen,
            DETAIL_X + 14,
            BODY_Y + 6,
            color::WHITE,
            DETAIL_RIGHT - (DETAIL_X + 14),
        );

        let mut y = BODY_Y + 26;
        match &item.kind {
            ItemKind::Tool { dur, ttype, .. } if ttype.durability() > 0 => {
                font::draw("DURABILITY", screen, DETAIL_X, y, color::DARK_GRAY);
                y += ROW_H;
                let pct = (dur * 100 / ttype.durability()).clamp(0, 100);
                let bar_w = 80;
                let fill_w = bar_w * pct / 100;
                let rgb = if pct > 50 {
                    0x46B750
                } else if pct > 20 {
                    0xD9A825
                } else {
                    0xC03A2B
                };
                fill_rect(screen, DETAIL_X, y + 2, bar_w, 2, 0x303030);
                fill_rect(screen, DETAIL_X, y + 2, fill_w, 2, rgb);
                let pct_text = format!("{pct}%");
                font::draw(
                    &pct_text,
                    screen,
                    DETAIL_RIGHT - font::text_width(&pct_text),
                    y,
                    color::GRAY,
                );
                y += ROW_H + 2;
            }
            _ if item.is_stackable() => {
                font::draw(
                    &format!("COUNT {}", item.count()),
                    screen,
                    DETAIL_X,
                    y,
                    color::GRAY,
                );
                y += ROW_H + 2;
            }
            _ => {}
        }

        for line in font::get_lines(&info_line(item), DETAIL_RIGHT - DETAIL_X, 40, 1) {
            font::draw(&line, screen, DETAIL_X, y, color::DARK_GRAY);
            y += font::text_height() + 1;
        }

        // action legend (bottom of the card) — wearables equip/dye from right here
        let action = match &item.kind {
            ItemKind::Armor { .. } => "WEAR IT",
            ItemKind::Clothing { .. } => "DYE SHIRT",
            _ => "HOLD IT",
        };
        font::draw("ENTER", screen, DETAIL_X, BODY_BOTTOM - 26, color::WHITE);
        font::draw(
            action,
            screen,
            DETAIL_X + 56,
            BODY_BOTTOM - 26,
            color::WHITE,
        );
        font::draw("Q", screen, DETAIL_X, BODY_BOTTOM - 16, color::WHITE);
        font::draw(
            "DROP ONE",
            screen,
            DETAIL_X + 56,
            BODY_BOTTOM - 16,
            color::WHITE,
        );
    }

    fn render_wear(&self, screen: &mut Screen, g: &mut Game) {
        self.render_divider(screen);
        let Some(player) = g.entities.get(self.player_eid) else {
            return;
        };
        let pd = player.player();

        // ---- left: the slot list (mock_wear geometry) ----
        const BOX_X: i32 = LIST_X + 14;
        const SLOT_Y0: i32 = BODY_Y + 5;
        const SLOT_PITCH: i32 = 26;
        const LABEL_X: i32 = BOX_X + 22;

        let slots: [(&str, Option<&Item>, bool); 4] = [
            ("HEAD", pd.worn_head.as_ref(), true),
            ("BODY", pd.cur_armor.as_ref(), true),
            ("HELD", pd.active_item.as_ref(), true),
            ("CHARM", None, false), // reserved slot, ships disabled
        ];
        for (i, (label, item, live)) in slots.iter().enumerate() {
            let y = SLOT_Y0 + i as i32 * SLOT_PITCH;
            let selected = *live && i == self.wear_sel;
            let border = if selected {
                0xFFFFFF
            } else if *live {
                0x8A8A8A
            } else {
                0x4A4A4A
            };
            fill_rect(screen, BOX_X, y, 16, 1, border);
            fill_rect(screen, BOX_X, y + 15, 16, 1, border);
            fill_rect(screen, BOX_X, y, 1, 16, border);
            fill_rect(screen, BOX_X + 15, y, 1, 16, border);
            if selected {
                font::draw(">", screen, LIST_X, y + 5, color::YELLOW);
            }
            font::draw(
                label,
                screen,
                LABEL_X,
                y + 1,
                if *live { COL_HEADER } else { color::DARK_GRAY },
            );
            match item {
                Some(it) => {
                    it.sprite.render(screen, BOX_X + 4, y + 4);
                    let mut name = bare_name(g, it);
                    if it.count() > 1 {
                        name.push_str(&format!(" X{}", it.count()));
                    }
                    // the slot column ends at the divider; long names ellipsize
                    font::draw_fit(
                        &name,
                        screen,
                        LABEL_X,
                        y + 10,
                        color::WHITE,
                        DIVIDER_X - 4 - LABEL_X,
                    );
                }
                None => {
                    font::draw("-", screen, BOX_X + 6, y + 5, color::DARK_GRAY);
                    let empty = if i == WEAR_HELD {
                        "EMPTY HANDS"
                    } else {
                        "NOTHING"
                    };
                    font::draw(empty, screen, LABEL_X, y + 10, color::DARK_GRAY);
                }
            }
        }

        // ---- left: totals ----
        let armor_col = if pd.armor > 0 {
            color::WHITE
        } else {
            color::DARK_GRAY
        };
        // totals sit clear of the HUD's armor badge row (y=158, renderer.rs)
        font::draw(
            &format!("ARMOR {} HITS", pd.armor),
            screen,
            LIST_X + 2,
            BODY_Y + 104,
            armor_col,
        );
        let coat = pd
            .cur_armor
            .as_ref()
            .is_some_and(|a| a.get_name() == "Fur Coat");
        font::draw(
            &format!("WARMTH +{}", if coat { 2 } else { 0 }),
            screen,
            LIST_X + 2,
            BODY_Y + 113,
            COL_WARMTH,
        );
        if pd
            .worn_head
            .as_ref()
            .is_some_and(|a| a.get_name() == "Straw Hat")
        {
            font::draw("SHADE +1", screen, LIST_X + 2, BODY_Y + 122, COL_WARMTH);
        }

        // ---- right: the player portrait (real sprite, palette-correct) ----
        const PORT_X: i32 = 196;
        const PORT_Y: i32 = BODY_Y + 2;
        const PORT_W: i32 = 36;
        const PORT_H: i32 = 28;
        screen.darken_rect_screen(PORT_X, PORT_Y, PORT_W, PORT_H, 60);
        fill_rect(screen, PORT_X, PORT_Y, PORT_W, 1, 0x8A8A8A);
        fill_rect(screen, PORT_X, PORT_Y + PORT_H - 1, PORT_W, 1, 0x8A8A8A);
        fill_rect(screen, PORT_X, PORT_Y, 1, PORT_H, 0x8A8A8A);
        fill_rect(screen, PORT_X + PORT_W - 1, PORT_Y, 1, PORT_H, 0x8A8A8A);
        let shirt = color::get4(-1, 100, pd.shirt_color, 532);
        SPRITES[0][0].render_color(screen, PORT_X + (PORT_W - 16) / 2, PORT_Y + 6, shirt);

        // ---- right: FITS ON <slot> ----
        let slot = wear_row_slot(self.wear_sel);
        let slot_name = ["HEAD", "BODY", "HELD"][self.wear_sel];
        let fits_y = BODY_Y + 40;
        font::draw(
            &format!("FITS ON {slot_name}"),
            screen,
            DETAIL_X,
            fits_y,
            COL_HEADER,
        );

        let Some(slot) = slot else {
            // HELD: the pack is the picker
            font::draw(
                "PICK FROM THE PACK TAB.",
                screen,
                DETAIL_X,
                fits_y + 12,
                color::DARK_GRAY,
            );
            return;
        };

        // the worn item leads the list (marked), then every fitting pack item
        let mut rows: Vec<(&Item, bool)> = Vec::new();
        if let Some(worn) = pd.worn(slot) {
            rows.push((worn, true));
        }
        let inv = &pd.inventory;
        for j in 0..inv.inv_size() {
            let it = inv.get(j);
            if fits_slot(it, slot) {
                rows.push((it, false));
            }
        }
        if rows.is_empty() {
            let mut y = fits_y + 12;
            for line in font::get_lines("NOTHING IN THE PACK FITS.", DETAIL_RIGHT - DETAIL_X, 30, 1)
            {
                font::draw(&line, screen, DETAIL_X, y, color::DARK_GRAY);
                y += font::text_height() + 1;
            }
            return;
        }
        const FIT_PITCH: i32 = 20;
        let max_fit = 4;
        let mut y = fits_y + 12;
        for (it, worn) in rows.iter().take(max_fit) {
            if *worn {
                font::draw(">", screen, DETAIL_X, y, color::YELLOW);
            }
            it.sprite.render(screen, DETAIL_X + 8, y);
            let fit_w = DETAIL_RIGHT - (DETAIL_X + 18);
            font::draw_fit(
                &bare_name(g, it),
                screen,
                DETAIL_X + 18,
                y,
                color::WHITE,
                fit_w,
            );
            let effect = wear_effect_line(it);
            let ecol = if effect.contains("WARMTH") || effect.contains("SHADE") {
                COL_WARMTH
            } else {
                color::GRAY
            };
            font::draw_fit(&effect, screen, DETAIL_X + 18, y + 9, ecol, fit_w);
            y += FIT_PITCH;
        }
        if rows.len() > max_fit {
            font::draw(
                &format!("+{} MORE IN THE PACK", rows.len() - max_fit),
                screen,
                DETAIL_X,
                y,
                color::DARK_GRAY,
            );
        }
    }

    fn render_craft(&mut self, screen: &mut Screen, g: &mut Game) {
        let y0 = self.craft_y0;
        // station context: the station's name as a sub-header over the whole body
        // (mock_bench) — the list and divider start one row lower to make room
        if let Some(name) = &self.station {
            font::draw_centered(name, screen, BODY_Y + 1, color::YELLOW);
        }
        // THE BENCH's module rack: SAW built in, then one socket per module —
        // filled sockets lit, empty ones a dim ? with the next fit hint under it
        if let Some(fitted) = self.bench_rack {
            use crate::entity::furniture::crafter::Module;
            let socket_w = 42; // wide enough for a full 4-char label at 8px
            let rack_w = socket_w * 4;
            let rack_x = PANEL_X + (PANEL_W - rack_w) / 2;
            let rack_y = BODY_Y + 12;
            let labels = ["SAW", "VICE", "SPND", "ASSY"];
            let lit = [true, fitted[0], fitted[1], fitted[2]];
            for i in 0..4 {
                let x = rack_x + i as i32 * socket_w;
                let col = if lit[i] { GOLD_RGB } else { DIVIDER_RGB };
                fill_rect(screen, x, rack_y, socket_w - 4, 1, col);
                fill_rect(screen, x, rack_y + 9, socket_w - 4, 1, col);
                if lit[i] {
                    font::draw_fit(
                        labels[i],
                        screen,
                        x + 1,
                        rack_y + 2,
                        color::WHITE,
                        socket_w - 4,
                    );
                } else {
                    font::draw(
                        "?",
                        screen,
                        x + (socket_w - 12) / 2,
                        rack_y + 2,
                        color::DARK_GRAY,
                    );
                }
            }
            // the hint names the first empty socket ("SPINDLE FITS HERE")
            if let Some(next) = Module::VALUES.iter().enumerate().find(|(i, _)| !fitted[*i]) {
                let hint = format!("HOLD A {} AND USE THE BENCH", next.1.item_name());
                font::draw_centered(&hint, screen, rack_y + 12, color::DARK_GRAY);
            }
        }
        fill_rect(screen, DIVIDER_X, y0, 1, BODY_BOTTOM - y0, DIVIDER_RGB);

        if self.recipes.is_empty() {
            return;
        }
        self.craft_menu.render(screen, g);

        // cost card for the selected recipe (the old HAVE/COST satellite boxes'
        // content, folded into the detail column)
        let recipe = self.recipes[self.craft_menu.get_selection() as usize].clone();
        let recipe = recipe.borrow();
        let product = recipe.get_product(g);
        product.sprite.render(screen, DETAIL_X + 2, y0 + 6);
        font::draw_fit(
            &bare_name(g, &product),
            screen,
            DETAIL_X + 14,
            y0 + 6,
            color::WHITE,
            DETAIL_RIGHT - (DETAIL_X + 14),
        );

        let Some(player) = g.entities.get(self.player_eid) else {
            return;
        };
        let inventory = &player.player().inventory;

        let mut y = y0 + 24;
        font::draw("NEEDS", screen, DETAIL_X, y, color::DARK_GRAY);
        y += ROW_H;
        for (name, amount) in recipe.get_costs() {
            let cost = registry::get(g, name);
            let have = inventory.count(&cost);
            let col = if have >= *amount {
                color::WHITE
            } else {
                color::RED
            };
            font::draw_fit(
                &format!("{}/{} {}", amount, have, bare_name(g, &cost)),
                screen,
                DETAIL_X,
                y,
                col,
                DETAIL_RIGHT - DETAIL_X,
            );
            y += ROW_H;
        }
        y += 4;
        font::draw(
            &format!("YOU HAVE {}", inventory.count(&product)),
            screen,
            DETAIL_X,
            y,
            color::GRAY,
        );

        font::draw("ENTER", screen, DETAIL_X, BODY_BOTTOM - 16, color::WHITE);
        font::draw(
            "CRAFT",
            screen,
            DETAIL_X + 56,
            BODY_BOTTOM - 16,
            color::WHITE,
        );
    }

    fn render_self(&self, screen: &mut Screen, g: &mut Game) {
        let Some(player) = g.entities.get(self.player_eid) else {
            return;
        };
        let pd = player.player();
        let x = 24;

        // day, time of day, place
        let day_line = format!(
            "DAY {} - {}",
            g.events.day_number + 1,
            g.get_time().to_string().to_uppercase()
        );
        font::draw(&day_line, screen, x, BODY_Y + 2, color::WHITE);
        font::draw(
            &location_line(g, player),
            screen,
            x,
            BODY_Y + 12,
            color::DARK_GRAY,
        );

        // numeric meters — the HUD hides full meters, SELF always tells
        let meters = [
            (0, color::get4(-1, 200, 500, 533), "HEALTH", pd.mob.health),
            (1, color::get4(-1, 220, 550, 553), "STAMINA", pd.stamina),
            (2, color::get4(-1, 100, 530, 211), "FOOD", pd.hunger),
        ];
        let mut y = BODY_Y + 26;
        for (tile, col, label, value) in meters {
            screen.render(x, y, tile + 12 * 32, col, 0);
            font::draw(label, screen, x + 10, y, color::WHITE);
            font::draw(
                &format!("{}/{}", value, crate::entity::mob::player::MAX_STAT),
                screen,
                x + 74,
                y,
                color::WHITE,
            );
            y += ROW_H;
        }

        // warmth gauge: the 7 temperature bands as cells with a marker
        let band = temperature::band_for(g, player);
        let steps = band.steps();
        y += 6;
        font::draw("WARMTH", screen, x, y, COL_HEADER);
        y += ROW_H;
        for s in -3..=3 {
            let cx = x + (s + 3) * 14;
            fill_rect(screen, cx, y, 12, 6, band_cell_rgb(s));
            if s == steps {
                fill_rect(screen, cx + 4, y + 7, 4, 2, 0xFFFFFF);
            }
        }
        let word = format!("{band:?}").to_uppercase();
        let word_col = match steps {
            i32::MIN..=-1 => color::get(-1, 345),
            0 => color::GRAY,
            _ => color::get(-1, 530),
        };
        font::draw(&word, screen, x + 7 * 14 + 8, y - 1, word_col);
        y += 12;
        let advice = match steps {
            i32::MIN..=-1 => "WEAR A COAT OR FIND FIRE.",
            0 => "YOU ARE COMFORTABLE.",
            _ => "FIND SHADE OR WATER.",
        };
        font::draw(advice, screen, x, y, color::DARK_GRAY);

        // active effects (absorbs the old P overlay)
        y += 14;
        font::draw("EFFECTS", screen, x, y, COL_HEADER);
        y += ROW_H;
        let lines = effect_lines(pd);
        if lines.is_empty() {
            font::draw("NONE.", screen, x, y, color::DARK_GRAY);
        } else {
            for line in lines.iter().take(2) {
                font::draw_fit(line, screen, x, y, color::WHITE, PANEL_X + PANEL_W - 4 - x);
                y += ROW_H;
            }
            if lines.len() > 2 {
                font::draw(
                    &format!("+{} MORE", lines.len() - 2),
                    screen,
                    x,
                    y,
                    color::DARK_GRAY,
                );
            }
        }
    }
}

impl Display for SurvivalDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        // E, X, and ESC close from any tab
        if g.input.get_key("exit").clicked
            || g.input.get_key("menu").clicked
            || g.input.get_key("inventory").clicked
        {
            g.exit_menu();
            return;
        }
        // Z: jump to CRAFT; on CRAFT it closes (the old craft-menu toggle feel)
        if g.input.get_key("craft").clicked {
            if self.tab == Tab::Craft {
                g.exit_menu();
                return;
            }
            self.tab = Tab::Craft;
            g.play_sound(Sound::Select);
        }
        // P is freed from the old overlay: alias for SELF
        if g.input.get_key("potionEffects").clicked && self.tab != Tab::SelfPane {
            self.tab = Tab::SelfPane;
            g.play_sound(Sound::Select);
        }

        // LEFT/RIGHT switch tabs, wrapping
        let cur = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0) as i32;
        let mut next = cur;
        if g.input.get_key("left").clicked {
            next -= 1;
        }
        if g.input.get_key("right").clicked {
            next += 1;
        }
        if next != cur {
            self.tab = Tab::ALL[next.rem_euclid(4) as usize];
            // crafting/equipping on other tabs mutates the inventory: entering
            // PACK with the old row list would act on stale indices (panic bug —
            // "index out of bounds" holding an item after crafting)
            if self.tab == Tab::Pack {
                self.rebuild_pack_from_arena(g);
            }
            g.play_sound(Sound::Select);
            return; // the switch consumes the frame's input
        }

        match self.tab {
            Tab::Pack => self.tick_pack(g),
            Tab::Wear => self.tick_wear(g),
            Tab::Craft => self.tick_craft(g),
            Tab::SelfPane => {}
        }
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        let layout = Layout::new(screen.w, screen.h);
        // deepen the frame's default 185 glass to the survival panel's 200 spec
        screen.darken_rect_screen(
            layout.panel_x,
            layout.panel_y,
            layout.panel_w,
            layout.panel_h,
            55,
        );
        if layout.panel_w == PANEL_W && layout.panel_h == PANEL_H {
            self.shell_menu.render(screen, g);
        }
        self.render_tabs(screen);
        let legend = match self.tab {
            Tab::Wear => "ENTER WEAR   Q TAKE OFF   ESC CLOSE",
            _ => "< > SWITCH TAB   ESC CLOSE",
        };
        font::draw_centered(legend, screen, LEGEND_Y, color::GRAY);

        match self.tab {
            Tab::Pack => self.render_pack(screen, g),
            Tab::Wear => self.render_wear(screen, g),
            Tab::Craft => self.render_craft(screen, g),
            Tab::SelfPane => self.render_self(screen, g),
        }
    }
}
