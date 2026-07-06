//! Port of `fdoom.screen.BookDisplay` — the paged book reader.

use crate::core::game::Game;
use crate::gfx::{Point, color, font, sprite_sheet};

use super::display::{Display, DisplayBase};
use super::entry::{StringEntry, handle};
use super::menu::MenuBuilder;
use super::rel_pos::RelPos;

// null characters "\0" denote page breaks.
const DEFAULT_BOOK: &str = " \n \0There is nothing of use here.\0 \0Still nothing... :P";

const SPACING: i32 = 3;
const MIN_X: i32 = 15;
const MAX_X: i32 = 15 + 8 * 32;
const MIN_Y: i32 = 8 * 5;
const MAX_Y: i32 = 8 * 5 + 8 * 16;

pub struct BookDisplay {
    base: DisplayBase,
    lines: Vec<Vec<String>>,
    page: i32,
    has_title: bool,
    show_page_count: bool,
    page_offset: i32,
}

impl BookDisplay {
    /// Java `new BookDisplay(book)`.
    pub fn new(g: &Game, book: &str) -> BookDisplay {
        Self::with_title(g, Some(book), false)
    }

    /// Java `new BookDisplay(book, hasTitle)` (book == null → the default book).
    pub fn with_title(g: &Game, book: Option<&str>, has_title: bool) -> BookDisplay {
        let page = 0;
        let (book, has_title) = match book {
            Some(b) => (b, has_title),
            None => (DEFAULT_BOOK, false),
        };
        let book = g.localization.get_localized(book);

        let mut pages: Vec<Vec<String>> = Vec::new();
        for content in book.split('\0') {
            let mut remainder = vec![content.to_string()];
            while !remainder[remainder.len() - 1].is_empty() {
                remainder = font::get_lines_keep(
                    &remainder[remainder.len() - 1],
                    MAX_X - MIN_X,
                    MAX_Y - MIN_Y,
                    SPACING,
                    true,
                );
                // removes the last element of remainder, which is the leftover.
                pages.push(remainder[..remainder.len() - 1].to_vec());
            }
        }

        let lines = pages;

        let show_page_count = has_title || lines.len() != 1;
        let page_offset: i32 = if show_page_count { 1 } else { 0 };

        // Java reused one Menu.Builder; the Rust builder is consumed, so rebuild it.
        let make_builder = || {
            MenuBuilder::new(true, SPACING, RelPos::Center, Vec::new())
                .set_frame_colors(554, 1, 554)
        };

        let page_count_menu = make_builder() // the small rect for the title
            .set_positioning(Point::new(crate::gfx::screen::W / 2, 0), RelPos::Bottom)
            .set_entries(StringEntry::use_lines_color(
                color::BLACK,
                &["Page".to_string(), if has_title { "Title".to_string() } else { format!("1/{}", lines.len()) }],
            ))
            .set_selection(1)
            .create_menu(g);
        let page_count_bottom = page_count_menu.get_bounds().bottom();

        let mut menus: Vec<super::menu::Menu> = Vec::new();
        if show_page_count {
            menus.push(page_count_menu);
        }
        for page_lines in &lines {
            menus.push(
                make_builder()
                    .set_positioning(
                        Point::new(crate::gfx::screen::W / 2, page_count_bottom + SPACING),
                        RelPos::Bottom,
                    )
                    .set_size(
                        MAX_X - MIN_X + sprite_sheet::BOX_WIDTH * 2,
                        MAX_Y - MIN_Y + sprite_sheet::BOX_WIDTH * 2,
                    )
                    .set_should_render(false)
                    .set_entries(StringEntry::use_lines_color(color::BLACK, page_lines))
                    .create_menu(g),
            );
        }

        menus[(page + page_offset) as usize].should_render = true;

        BookDisplay {
            base: DisplayBase::new(false, true, menus),
            lines,
            page,
            has_title,
            show_page_count,
            page_offset,
        }
    }

    fn turn_page(&mut self, dir: i32) {
        if self.page + dir >= 0 && self.page + dir < self.lines.len() as i32 {
            self.base.menus[(self.page + self.page_offset) as usize].should_render = false;
            self.page += dir;
            if self.show_page_count {
                let text = if self.page == 0 && self.has_title {
                    "Title".to_string()
                } else {
                    format!("{}/{}", self.page + 1, self.lines.len())
                };
                self.base.menus[0]
                    .update_selected_entry(handle(StringEntry::with_color(&text, color::BLACK)));
            }
            self.base.menus[(self.page + self.page_offset) as usize].should_render = true;
        }
    }
}

impl Display for BookDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        if g.input.get_key("menu").clicked || g.input.get_key("exit").clicked {
            g.exit_menu(); // this is what closes the book
        }
        if g.input.get_key("left").clicked {
            self.turn_page(-1); // this is what turns the page back
        }
        if g.input.get_key("right").clicked {
            self.turn_page(1); // this is what turns the page forward
        }
    }
}
