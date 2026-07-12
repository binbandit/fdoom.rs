//! Port of `fdoom.screen.Menu` (including `Menu.Builder`).

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::gfx::{Dimension, Insets, Point, Rectangle, Screen, color, font, sprite_sheet};

use super::entry::{self, EntryHandle};
use super::rel_pos::RelPos;

pub struct Menu {
    /// True when a display explicitly chose frame colors (book pages etc.) — those
    /// render the classic opaque fill instead of the smoked-glass panel.
    custom_frame_fill: bool,
    entries: Vec<EntryHandle>,

    spacing: i32,
    bounds: Rectangle,
    entry_bounds: Rectangle,
    entry_pos: RelPos,

    title: String,
    title_color: i32,
    title_loc: Point,
    draw_vertically: bool,

    has_frame: bool,
    frame_fill_color: i32,
    frame_edge_color: i32,

    selectable: bool,
    pub should_render: bool,

    display_length: i32,
    padding: i32,
    wrap: bool,

    // menu selection vars
    selection: i32,
    disp_selection: i32,
    offset: i32,
}

impl Menu {
    fn empty() -> Menu {
        Menu {
            entries: Vec::new(),
            spacing: 0,
            bounds: Rectangle::default(),
            entry_bounds: Rectangle::default(),
            entry_pos: RelPos::Center,
            title: String::new(),
            title_color: 0,
            title_loc: Point::default(),
            draw_vertically: false,
            has_frame: false,
            frame_fill_color: 0,
            frame_edge_color: 0,
            custom_frame_fill: false,
            selectable: false,
            should_render: true,
            display_length: 0,
            padding: 0,
            wrap: false,
            selection: 0,
            disp_selection: 0,
            offset: 0,
        }
    }

    /// Java `init()`.
    fn init(&mut self) {
        if self.entries.is_empty() {
            self.selection = 0;
            self.disp_selection = 0;
            self.offset = 0;
            return;
        }

        self.selection = self.selection.min(self.entries.len() as i32 - 1).max(0);

        if !self.entries[self.selection as usize]
            .borrow()
            .is_selectable()
        {
            let prev_sel = self.selection;
            loop {
                self.selection += 1;
                if self.selection < 0 {
                    self.selection = self.entries.len() as i32 - 1;
                }
                self.selection %= self.entries.len() as i32;
                if self.entries[self.selection as usize]
                    .borrow()
                    .is_selectable()
                    || self.selection == prev_sel
                {
                    break;
                }
            }
        }

        self.disp_selection = self.selection;
        self.disp_selection = self.disp_selection.min(self.display_length - 1).max(0);

        self.do_scroll();
    }

    pub fn set_selection(&mut self, mut idx: i32) {
        if idx >= self.entries.len() as i32 {
            idx = self.entries.len() as i32 - 1;
        }
        if idx < 0 {
            idx = 0;
        }
        self.selection = idx;
        self.do_scroll();
    }

    pub fn get_selection(&self) -> i32 {
        self.selection
    }

    pub fn get_disp_selection(&self) -> i32 {
        self.disp_selection
    }

    pub fn get_entries(&self) -> &[EntryHandle] {
        &self.entries
    }

    /// Java `getCurEntry()`.
    pub fn get_cur_entry(&self) -> Option<EntryHandle> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.entries[self.selection as usize].clone())
        }
    }

    pub fn get_num_options(&self) -> i32 {
        self.entries.len() as i32
    }

    pub fn get_bounds(&self) -> Rectangle {
        self.bounds
    }

    pub fn get_title(&self) -> &str {
        &self.title
    }

    pub fn is_selectable(&self) -> bool {
        self.selectable
    }

    pub fn should_render(&self) -> bool {
        self.should_render
    }

    pub fn translate(&mut self, xoff: i32, yoff: i32) {
        self.bounds.translate(xoff, yoff);
        self.entry_bounds.translate(xoff, yoff);
        self.title_loc.translate(xoff, yoff);
    }

    /// Java `tick(input)`.
    pub fn tick(&mut self, g: &mut Game) {
        if !self.selectable || self.entries.is_empty() {
            return;
        }

        let prev_sel = self.selection;
        let mut selection = self.selection;
        // text-entry rows capture letters, so navigate with the physical arrows only
        let typing = self
            .get_cur_entry()
            .map(|e| e.borrow().captures_typing())
            .unwrap_or(false);
        let (up, down) = if typing {
            (
                g.input.get_physical_key("UP"),
                g.input.get_physical_key("DOWN"),
            )
        } else {
            (g.input.get_key("up"), g.input.get_key("down"))
        };
        if up.clicked {
            selection -= 1;
        }
        if down.clicked {
            selection += 1;
        }

        let delta = selection - prev_sel;

        if delta == 0 {
            // only ticks the entry on a frame where the selection cursor has not moved
            let entry = self.entries[self.selection as usize].clone();
            entry.borrow_mut().tick(g);
            return;
        } else {
            g.play_sound(Sound::Select);
        }

        let len = self.entries.len() as i32;
        selection = prev_sel;
        loop {
            selection += delta;
            if selection < 0 {
                selection = len - 1;
            }
            selection %= len;
            if self.entries[selection as usize].borrow().is_selectable() || selection == prev_sel {
                break;
            }
        }

        // update offset and selection displayed
        self.disp_selection += selection - prev_sel;
        if self.disp_selection < 0 {
            self.disp_selection = 0;
        }
        if self.disp_selection >= self.display_length {
            self.disp_selection = self.display_length - 1;
        }

        self.selection = selection;
        self.do_scroll();
    }

    fn do_scroll(&mut self) {
        // check if dispSelection is past padding point, and if so, bring it back in
        self.disp_selection = self.selection - self.offset;
        let mut offset = self.offset;
        let num_entries = self.entries.len() as i32;

        // for scrolling up
        while (self.disp_selection < self.padding
            || !self.wrap && offset + self.display_length > num_entries)
            && (self.wrap || offset > 0)
        {
            offset -= 1;
            self.disp_selection += 1;
        }

        // for scrolling down
        while (self.display_length - self.disp_selection <= self.padding
            || !self.wrap && offset < 0)
            && (self.wrap || offset + self.display_length < num_entries)
        {
            offset += 1;
            self.disp_selection -= 1;
        }

        // only useful when wrap is true
        if offset < 0 {
            offset += num_entries;
        }
        if offset > 0 {
            offset %= num_entries;
        }

        self.offset = offset;
    }

    /// Java `render(screen)`.
    pub fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        self.render_frame(screen);

        // render the title
        if !self.title.is_empty() {
            if self.draw_vertically {
                for (i, ch) in self.title.chars().enumerate() {
                    font::draw(
                        &ch.to_string(),
                        screen,
                        self.title_loc.x,
                        self.title_loc.y + i as i32 * font::text_height(),
                        self.title_color,
                    );
                }
            } else {
                font::draw(
                    &self.title,
                    screen,
                    self.title_loc.x,
                    self.title_loc.y,
                    self.title_color,
                );
            }
        }

        // render the options
        let mut y = self.entry_bounds.top();
        let num_entries = self.entries.len() as i32;
        let special = self.wrap && num_entries < self.display_length;
        if special {
            let diff = self.display_length - num_entries; // account for this many entry heights
            let extra = diff * (entry::entry_height() + self.spacing) / 2;
            y += extra;
        }
        let end = if self.wrap {
            self.offset + self.display_length
        } else {
            (self.offset + self.display_length).min(num_entries)
        };
        for i in self.offset..end {
            if special && i - self.offset >= num_entries {
                break;
            }

            let idx = (i % num_entries) as usize;
            let entry = self.entries[idx].clone();
            let mut entry = entry.borrow_mut();

            if !entry.is_blank_entry() {
                let width = entry.get_width(g);
                let pos = self.entry_pos.position_rect_in(
                    Dimension::new(width, entry::entry_height()),
                    &Rectangle::new(
                        self.entry_bounds.left(),
                        y,
                        self.entry_bounds.width(),
                        entry::entry_height(),
                        Rectangle::CORNER_DIMS,
                    ),
                );
                let selected = idx as i32 == self.selection;
                entry.render(screen, g, pos.x, pos.y, selected);
                if selected && entry.is_selectable() {
                    // draw the arrows
                    font::draw(
                        "> ",
                        screen,
                        pos.x - font::text_width("> "),
                        y,
                        entry::COL_SLCT,
                    );
                    font::draw(" <", screen, pos.x + width, y, entry::COL_SLCT);
                }
            }

            y += entry::entry_height() + self.spacing;
        }
    }

    pub fn update_selected_entry(&mut self, new_entry: EntryHandle) {
        self.update_entry(self.selection, new_entry);
    }

    pub fn update_entry(&mut self, idx: i32, new_entry: EntryHandle) {
        if idx >= 0 && (idx as usize) < self.entries.len() {
            self.entries[idx as usize] = new_entry;
        }
    }

    pub fn remove_selected_entry(&mut self) {
        self.entries.remove(self.selection as usize);

        if self.selection >= self.entries.len() as i32 {
            self.selection = self.entries.len() as i32 - 1;
        }
        if self.selection < 0 {
            self.selection = 0;
        }

        self.do_scroll();
    }

    pub fn set_frame_colors(&mut self, fill_col: i32, edge_stroke_col: i32, edge_fill_col: i32) {
        self.custom_frame_fill = true;
        self.frame_fill_color = color::get(fill_col, fill_col);
        self.frame_edge_color = color::get4(-1, edge_stroke_col, fill_col, edge_fill_col);
        let title_cols = color::separate_encoded_sprite_readable(self.title_color);
        self.title_color = color::get(
            fill_col,
            if title_cols[3] < 0 {
                550
            } else {
                title_cols[3]
            },
        );
    }

    pub fn set_frame_colors_from(&mut self, model: &Menu) {
        self.custom_frame_fill = model.custom_frame_fill;
        self.frame_fill_color = model.frame_fill_color;
        self.frame_edge_color = model.frame_edge_color;
        self.title_color = model.title_color;
    }

    fn render_frame(&self, screen: &mut Screen) {
        if !self.has_frame {
            return;
        }

        let bottom = self.bounds.bottom() - sprite_sheet::BOX_WIDTH;
        let right = self.bounds.right() - sprite_sheet::BOX_WIDTH;

        // smoked-glass panel: darken what's behind instead of a flat opaque fill.
        // Displays that explicitly picked a fill (book pages = light paper with black
        // text) keep the classic opaque look — glass would make them unreadable.
        if !self.custom_frame_fill {
            screen.darken_rect_screen(
                self.bounds.left(),
                self.bounds.top(),
                self.bounds.width(),
                self.bounds.height(),
                185,
            );
        }

        let mut y = self.bounds.top();
        while y <= bottom {
            let mut x = self.bounds.left();
            while x <= right {
                let xend = x == self.bounds.left() || x == right;
                let yend = y == self.bounds.top() || y == bottom;
                let spriteoffset = if xend && yend {
                    0
                } else if yend {
                    1
                } else {
                    2
                };
                let mirrors = (if x == right { 1 } else { 0 }) + (if y == bottom { 2 } else { 0 });
                if xend || yend {
                    screen.render(x, y, spriteoffset + 13 * 32, self.frame_edge_color, mirrors);
                } else if self.custom_frame_fill {
                    screen.render(x, y, 2 + 13 * 32, self.frame_fill_color, 1);
                }

                if x < right && x + sprite_sheet::BOX_WIDTH > right {
                    x = right - sprite_sheet::BOX_WIDTH;
                }
                x += sprite_sheet::BOX_WIDTH;
            }
            if y < bottom && y + sprite_sheet::BOX_WIDTH > bottom {
                y = bottom - sprite_sheet::BOX_WIDTH;
            }
            y += sprite_sheet::BOX_WIDTH;
        }
    }
}

/// Java `Menu.Builder`.
pub struct MenuBuilder {
    menu: Menu,

    set_selectable: bool,
    padding: f32,

    title_pos: RelPos,
    full_title_color: bool,
    set_title_color: bool,
    title_col: i32,
    frame_fill_col: i32,
    frame_edge_stroke: i32,
    frame_edge_fill: i32,

    anchor: Point,
    menu_pos: RelPos,
    menu_size: Option<Dimension>,
}

impl MenuBuilder {
    pub fn new(
        has_frame: bool,
        entry_spacing: i32,
        entry_pos: RelPos,
        entries: Vec<EntryHandle>,
    ) -> MenuBuilder {
        let mut menu = Menu::empty();
        menu.entries = entries;
        menu.has_frame = has_frame;
        menu.spacing = entry_spacing;
        menu.entry_pos = entry_pos;
        MenuBuilder {
            menu,
            set_selectable: false,
            padding: 1.0,
            title_pos: RelPos::Top,
            full_title_color: false,
            set_title_color: false,
            title_col: 550,
            // dark slate edges over the smoked-glass fill (was the classic blue 5/1/445)
            frame_fill_col: 111,
            frame_edge_stroke: 0,
            frame_edge_fill: 333,
            anchor: Point::new(0, 0),
            menu_pos: RelPos::Center,
            menu_size: None,
        }
    }

    pub fn set_entries(mut self, entries: Vec<EntryHandle>) -> Self {
        self.menu.entries = entries;
        self
    }

    pub fn set_positioning(mut self, anchor: Point, menu_pos: RelPos) -> Self {
        self.anchor = anchor;
        self.menu_pos = menu_pos;
        self
    }

    pub fn set_size(mut self, width: i32, height: i32) -> Self {
        self.menu_size = Some(Dimension::new(width, height));
        self
    }

    pub fn set_menu_size(mut self, d: Option<Dimension>) -> Self {
        self.menu_size = d;
        self
    }

    pub fn set_bounds(mut self, rect: Rectangle) -> Self {
        self.menu_size = Some(rect.size());
        self.anchor = rect.center();
        self.menu_pos = RelPos::Center;
        self
    }

    pub fn set_display_length(mut self, num_entries: i32) -> Self {
        self.menu.display_length = num_entries;
        self
    }

    pub fn set_title_pos(mut self, rp: RelPos) -> Self {
        self.title_pos = rp;
        self
    }

    pub fn set_title(mut self, title: &str) -> Self {
        self.menu.title = title.to_string();
        self
    }

    pub fn set_title_color(mut self, title: &str, color: i32, full_color: bool) -> Self {
        self.menu.title = title.to_string();
        self.full_title_color = full_color;
        self.set_title_color = true;
        if full_color {
            self.menu.title_color = color;
        } else {
            self.title_col = color;
        }
        self
    }

    pub fn set_frame(mut self, has_frame: bool) -> Self {
        self.menu.has_frame = has_frame;
        self
    }

    pub fn set_frame_colors(mut self, fill_col: i32, edge_stroke: i32, edge_fill: i32) -> Self {
        self.menu.has_frame = true;
        self.frame_fill_col = fill_col;
        self.frame_edge_stroke = edge_stroke;
        self.frame_edge_fill = edge_fill;
        self
    }

    pub fn set_scroll_policies(mut self, padding: f32, wrap: bool) -> Self {
        self.padding = padding;
        self.menu.wrap = wrap;
        self
    }

    pub fn set_should_render(mut self, render: bool) -> Self {
        self.menu.should_render = render;
        self
    }

    pub fn set_selectable(mut self, selectable: bool) -> Self {
        self.set_selectable = true;
        self.menu.selectable = selectable;
        self
    }

    pub fn set_selection(mut self, sel: i32) -> Self {
        self.menu.selection = sel;
        self
    }

    pub fn set_selection_disp(mut self, sel: i32, disp_sel: i32) -> Self {
        self.menu.selection = sel;
        self.menu.disp_selection = disp_sel;
        self
    }

    /// Java `createMenu()` — consumes the builder (Java copied it first; entry handles
    /// stay shared either way, so rebuild a Builder when a menu must be recreated).
    pub fn create_menu(mut self, g: &Game) -> Menu {
        if self.anchor == Point::new(0, 0) && self.menu_pos == RelPos::Center {
            self.anchor = Point::new(g.screen_size.0 / 2, g.screen_size.1 / 2);
        }
        let menu = &mut self.menu;
        menu.title = g.localization.get_localized(&menu.title);

        // set default selectability
        if !self.set_selectable {
            for entry in &menu.entries {
                menu.selectable = menu.selectable || entry.borrow().is_selectable();
                if menu.selectable {
                    break;
                }
            }
        }

        // check the centering of the title, and find the dimensions of the title's display space
        menu.draw_vertically = self.title_pos == RelPos::Left || self.title_pos == RelPos::Right;

        let title_dim = if menu.draw_vertically {
            Dimension::new(font::text_height() * 2, font::text_width(&menu.title))
        } else {
            Dimension::new(font::text_width(&menu.title), font::text_height() * 2)
        };

        let mut border;
        if menu.has_frame {
            border = Insets::uniform(sprite_sheet::BOX_WIDTH); // add frame insets
        } else {
            border = Insets::default();

            // add title insets
            if !menu.title.is_empty() && self.title_pos != RelPos::Center {
                let c = self.title_pos;
                let space = sprite_sheet::BOX_WIDTH * 2;
                if c.y_index() == 0 {
                    border.top = space;
                } else if c.y_index() == 2 {
                    border.bottom = space;
                } else if c.x_index() == 0 {
                    border.left = space; // must be center left
                } else if c.x_index() == 2 {
                    border.right = space; // must be center right
                }
            }
        }

        if menu.is_selectable() {
            // add spacing for selection cursors
            border.left += sprite_sheet::BOX_WIDTH * 2;
            border.right += sprite_sheet::BOX_WIDTH * 2;
        }

        if menu.wrap && menu.display_length > 0 {
            menu.display_length = menu.display_length.min(menu.entries.len() as i32);
        }

        // I have anchor and menu's relative position to it, and may or may not have size.
        let entry_size;
        let menu_size;

        if let Some(set_size) = self.menu_size {
            // menuSize was set manually
            menu_size = set_size;
            entry_size = border.subtract_from_dim(menu_size);
        } else {
            let mut width = title_dim.width;
            for entry in &menu.entries {
                let entry_ref = entry.borrow();
                let mut entry_width = entry_ref.get_width(g);
                if menu.is_selectable() && !entry_ref.is_selectable() {
                    entry_width = 0.max(entry_width - sprite_sheet::BOX_WIDTH * 4);
                }
                width = width.max(entry_width);
            }

            if menu.display_length > 0 {
                // has been set; use to determine entry bounds
                let height =
                    (entry::entry_height() + menu.spacing) * menu.display_length - menu.spacing;
                entry_size = Dimension::new(width, height);
            } else {
                // no set size; just keep going to the edges of the screen
                let mut max_height;
                if self.menu_pos.y_index() == 0 {
                    max_height = self.anchor.y;
                } else if self.menu_pos.y_index() == 2 {
                    max_height = g.screen_size.1 - self.anchor.y;
                } else {
                    max_height = self.anchor.y.min(g.screen_size.1 - self.anchor.y) * 2;
                }

                max_height -= border.top + border.bottom; // reserve border space

                let entry_height = menu.spacing + entry::entry_height();
                let total_height = entry_height * menu.entries.len() as i32 - menu.spacing;
                max_height =
                    ((max_height + menu.spacing) / entry_height) * entry_height - menu.spacing;

                entry_size = Dimension::new(width, max_height.min(total_height));
            }

            menu_size = border.add_to_dim(entry_size);
        }

        // set default max display length (needs size first)
        if menu.display_length <= 0 && !menu.entries.is_empty() {
            menu.display_length =
                (entry_size.height + menu.spacing) / (entry::entry_height() + menu.spacing);
        }

        // based on the menu centering, and the anchor, determine the upper-left point from
        // which to draw the menu
        menu.bounds = self.menu_pos.position_rect(menu_size, self.anchor);
        menu.entry_bounds = border.subtract_from_rect(&menu.bounds);
        menu.title_loc = self.title_pos.position_rect_in(title_dim, &menu.bounds);

        if self.title_pos.x_index() == 0 && self.title_pos.y_index() != 1 {
            menu.title_loc.x += sprite_sheet::BOX_WIDTH;
        }
        if self.title_pos.x_index() == 2 && self.title_pos.y_index() != 1 {
            menu.title_loc.x -= sprite_sheet::BOX_WIDTH;
        }

        // set the menu title color
        if !menu.title.is_empty() {
            if self.full_title_color {
                menu.title_color = self.title_col;
            } else {
                if !self.set_title_color {
                    self.title_col = if menu.has_frame { 550 } else { 555 };
                }
                // make it match the frame color, or be transparent
                menu.title_color = color::get(
                    if menu.has_frame {
                        self.frame_fill_col
                    } else {
                        -1
                    },
                    self.title_col,
                );
            }
        }

        // set the menu frame colors
        if menu.has_frame {
            menu.frame_fill_color = color::get(self.frame_fill_col, self.frame_fill_col);
            menu.frame_edge_color = color::get4(
                -1,
                self.frame_edge_stroke,
                self.frame_fill_col,
                self.frame_edge_fill,
            );
        }

        let padding = self.padding.clamp(0.0, 1.0);
        menu.padding = (padding * menu.display_length as f32 / 2.0).floor() as i32;

        // done setting defaults/values; return the new menu
        self.menu.init();
        self.menu
    }
}
