//! The survival screen — one panel on E with four tabs: PACK / WEAR / CRAFT / SELF
//! (docs/UI_REDESIGN.md §3). Lane L2 ships the shell plus the PACK and SELF panes.
//! WEAR is a read-only summary until L3 lands wear slots; CRAFT hosts the existing
//! personal recipe list (with a cost card in the detail column) until L4 restyles
//! crafting and adds station context.

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::core::temperature;
use crate::core::updater::NORM_SPEED;
use crate::entity::Entity;
use crate::entity::mob::player::PlayerData;
use crate::gfx::{Rectangle, Screen, color, font};
use crate::item::{Item, ItemKind, PotionType, Recipe, registry};
use crate::level::{self, infinite_gen};

use super::display::{Display, DisplayBase};
use super::entry::recipe_entry::RecipeEntry;
use super::menu::{Menu, MenuBuilder};
use super::rel_pos::RelPos;

/* ------------------------------- geometry (the spec) ------------------------------- */
// Every coordinate below comes from the mockups (`target/verify/ui_mock/mock_*.png`),
// which were composed at real screen coordinates.

const PANEL_X: i32 = 8;
const PANEL_Y: i32 = 8;
const PANEL_W: i32 = 272;
const PANEL_H: i32 = 176;

const TAB_Y: i32 = 13;
const UNDERLINE_Y: i32 = 22;

const BODY_Y: i32 = 28;
const BODY_BOTTOM: i32 = 166;
const LIST_X: i32 = 12;
const LIST_RIGHT: i32 = 146;
const DIVIDER_X: i32 = 148;
const DETAIL_X: i32 = 154;
const DETAIL_RIGHT: i32 = 276;

const ROW_H: i32 = 10;
const MAX_ROWS: i32 = (BODY_BOTTOM - BODY_Y) / ROW_H; // 13

const LEGEND_Y: i32 = 170;

/// Category headers, dim gold (reads as a label, not a row).
const COL_HEADER: i32 = color::get(-1, 431);
/// The active tab's underline (raw RGB — drawn as a pixel fill).
const GOLD_RGB: i32 = 0xE0C84A;
const DIVIDER_RGB: i32 = 0x4A4A4A;
const SCROLLBAR_RGB: i32 = 0x9A9A9A;

/// Fill a screen-space rect with a literal RGB color (bounds-clipped). Same shape as
/// the renderer's private helper — the gauges here need flat fills, not sprite cells.
fn fill_rect(screen: &mut Screen, x: i32, y: i32, w: i32, h: i32, rgb: i32) {
    use crate::gfx::screen::{H, W};
    for yy in y.max(0)..(y + h).min(H) {
        let row = (yy * W) as usize;
        for xx in x.max(0)..(x + w).min(W) {
            screen.pixels[row + xx as usize] = rgb;
        }
    }
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
fn bare_name(g: &Game, item: &Item) -> String {
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

/// One plain-words line for the detail card: what the item is for and how it's used
/// today (the instant-equip flow is L3's; this stays truthful until then).
fn info_line(item: &Item) -> String {
    match &item.kind {
        ItemKind::Tool { .. } => "A TOOL. HOLD IT TO USE IT.".to_string(),
        ItemKind::Food { heal, .. } => format!("EAT WHILE HELD: +{heal} FOOD."),
        ItemKind::Medical { heal, .. } => format!("USE WHILE HELD: +{heal} HEALTH."),
        ItemKind::Armor { .. } => "HOLD IT, THEN USE IT ON OPEN GROUND TO WEAR.".to_string(),
        ItemKind::Clothing { .. } => "USE WHILE HELD TO DYE YOUR SHIRT.".to_string(),
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

    // CRAFT (the personal recipe list; stations stay on the old display until L4)
    recipes: Vec<Rc<RefCell<Recipe>>>,
    craft_menu: Menu,
}

impl SurvivalDisplay {
    /// Opens on PACK — the E key's screen.
    pub fn new(g: &Game, player: &Entity) -> SurvivalDisplay {
        Self::on_tab(g, player, Tab::Pack)
    }

    /// Opens on a specific tab (Z lands on CRAFT, P lands on SELF).
    pub fn on_tab(g: &Game, player: &Entity, tab: Tab) -> SurvivalDisplay {
        let inventory = &player.player().inventory;

        let mut recipes: Vec<Rc<RefCell<Recipe>>> = g
            .recipes
            .craft
            .clone()
            .into_iter()
            .map(|r| Rc::new(RefCell::new(r)))
            .collect();
        let craft_menu = Self::build_craft_menu(g, &mut recipes, inventory, 0);

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
            recipes,
            craft_menu,
        };
        display.rebuild_pack(inventory.items());
        display
    }

    pub fn current_tab(&self) -> Tab {
        self.tab
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
    ) -> Menu {
        for r in recipes.iter() {
            r.borrow_mut().check_can_craft(g, inventory);
        }
        // craftable recipes first, original order otherwise (same as recipe_menu)
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
            BODY_Y,
            LIST_RIGHT - LIST_X,
            BODY_BOTTOM - BODY_Y,
            Rectangle::CORNER_DIMS,
        ))
        .set_display_length(n.min(MAX_ROWS))
        .set_selectable(true)
        .set_scroll_policies(1.0, false)
        .set_selection(selection)
        .create_menu(g)
    }

    /* --------------------------------- pack state --------------------------------- */

    fn rebuild_pack(&mut self, items: &[Item]) {
        let prev_sel = self.pack_sel;
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

    /// ENTER — HOLD IT: the selected item becomes the held item, and the screen
    /// closes so it can be used. The previously held item goes back into the pack
    /// (never silently dropped).
    fn hold_selected(&mut self, g: &mut Game) {
        let Some(idx) = self.selected_inv_idx() else {
            return;
        };
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

    /// Q / SHIFT-Q — drop one / drop the stack (same rules as `inventory_menu::
    /// tick_drops`, remapped through the categorized rows).
    fn drop_selected(&mut self, g: &mut Game, drop_one: bool) {
        let Some(sel) = self.selected_inv_idx() else {
            return;
        };
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
                fill_rect(screen, x - 1, UNDERLINE_Y, w + 2, 1, GOLD_RGB);
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

    fn render_pack(&self, screen: &mut Screen, g: &mut Game) {
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
        let end = ((self.pack_off as i32 + MAX_ROWS) as usize).min(self.pack_rows.len());
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
                    font::draw(&bare_name(g, &item), screen, LIST_X + 17, y, col);
                    if item.count() > 1 {
                        let count = item.count().min(999).to_string();
                        let cx = LIST_RIGHT - 6 - font::text_width(&count);
                        font::draw(&count, screen, cx, y, col);
                    }
                }
            }
        }

        // 1px scrollbar on the divider when the list overflows
        let rows = self.pack_rows.len() as i32;
        if rows > MAX_ROWS {
            let body_h = BODY_BOTTOM - BODY_Y;
            let bar_h = (body_h * MAX_ROWS / rows).max(8);
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
        font::draw(&name, screen, DETAIL_X + 14, BODY_Y + 6, color::WHITE);

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

        // action legend (bottom of the card)
        font::draw("ENTER", screen, DETAIL_X, BODY_BOTTOM - 26, color::WHITE);
        font::draw(
            "HOLD IT",
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

        font::draw("WORN", screen, LIST_X + 2, BODY_Y + 2, COL_HEADER);
        match &pd.cur_armor {
            Some(a) => {
                a.sprite.render(screen, LIST_X + 8, BODY_Y + 12);
                font::draw(
                    &bare_name(g, a),
                    screen,
                    LIST_X + 17,
                    BODY_Y + 12,
                    color::WHITE,
                );
            }
            None => font::draw("NOTHING", screen, LIST_X + 8, BODY_Y + 12, color::DARK_GRAY),
        }
        font::draw(
            &format!(
                "ARMOR {}/{}",
                pd.armor,
                crate::entity::mob::player::MAX_ARMOR
            ),
            screen,
            LIST_X + 8,
            BODY_Y + 24,
            color::GRAY,
        );

        font::draw("HELD", screen, LIST_X + 2, BODY_Y + 42, COL_HEADER);
        match &pd.active_item {
            Some(a) => {
                a.sprite.render(screen, LIST_X + 8, BODY_Y + 52);
                font::draw(
                    &bare_name(g, a),
                    screen,
                    LIST_X + 17,
                    BODY_Y + 52,
                    color::WHITE,
                );
            }
            None => font::draw(
                "EMPTY HANDS",
                screen,
                LIST_X + 8,
                BODY_Y + 52,
                color::DARK_GRAY,
            ),
        }

        // read-only until L3's wear slots land; the hint states today's real flow
        let mut y = BODY_Y + 4;
        for line in font::get_lines(
            "WEAR SLOTS ARRIVE WITH A LATER UPDATE.",
            DETAIL_RIGHT - DETAIL_X,
            40,
            1,
        ) {
            font::draw(&line, screen, DETAIL_X, y, color::DARK_GRAY);
            y += font::text_height() + 1;
        }
        y += 6;
        for line in font::get_lines(
            "TO WEAR ARMOR NOW: HOLD IT, FACE OPEN GROUND, AND USE IT.",
            DETAIL_RIGHT - DETAIL_X,
            60,
            1,
        ) {
            font::draw(&line, screen, DETAIL_X, y, color::GRAY);
            y += font::text_height() + 1;
        }
    }

    fn render_craft(&mut self, screen: &mut Screen, g: &mut Game) {
        self.render_divider(screen);

        if self.recipes.is_empty() {
            return;
        }
        self.craft_menu.render(screen, g);

        // cost card for the selected recipe (the old HAVE/COST satellite boxes'
        // content, folded into the detail column)
        let recipe = self.recipes[self.craft_menu.get_selection() as usize].clone();
        let recipe = recipe.borrow();
        let product = recipe.get_product(g);
        product.sprite.render(screen, DETAIL_X + 2, BODY_Y + 6);
        font::draw(
            &bare_name(g, &product),
            screen,
            DETAIL_X + 14,
            BODY_Y + 6,
            color::WHITE,
        );

        let Some(player) = g.entities.get(self.player_eid) else {
            return;
        };
        let inventory = &player.player().inventory;

        let mut y = BODY_Y + 24;
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
            font::draw(
                &format!("{}/{} {}", amount, have, bare_name(g, &cost)),
                screen,
                DETAIL_X,
                y,
                col,
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
                font::draw(line, screen, x, y, color::WHITE);
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
            g.play_sound(Sound::Select);
            return; // the switch consumes the frame's input
        }

        match self.tab {
            Tab::Pack => self.tick_pack(g),
            Tab::Craft => self.tick_craft(g),
            Tab::Wear | Tab::SelfPane => {}
        }
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        // deepen the frame's default 185 glass to the survival panel's 200 spec
        screen.darken_rect_screen(PANEL_X, PANEL_Y, PANEL_W, PANEL_H, 55);
        self.shell_menu.render(screen, g);
        self.render_tabs(screen);
        font::draw_centered("< > SWITCH TAB   ESC CLOSE", screen, LEGEND_Y, color::GRAY);

        match self.tab {
            Tab::Pack => self.render_pack(screen, g),
            Tab::Wear => self.render_wear(screen, g),
            Tab::Craft => self.render_craft(screen, g),
            Tab::SelfPane => self.render_self(screen, g),
        }
    }
}
