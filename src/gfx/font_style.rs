//! Port of `fdoom.gfx.FontStyle`.

use super::font;
use super::screen::{self, Screen};
use super::{color, Dimension, Point, Rectangle};
use crate::screen::rel_pos::RelPos;

/// x and y offsets for each binary value in the "shadow location byte"; the values
/// progress in a circle (N, NE, E, SE, S, SW, W, NW).
const SHADOW_POS_MAP: [i32; 16] = [0, 1, 1, 1, 0, -1, -1, -1, -1, -1, 0, 1, 1, 1, 0, -1];

#[derive(Debug, Clone)]
pub struct FontStyle {
    main_color: i32,
    shadow_color: i32,
    shadow_type: String,
    anchor: Point,
    /// aligns the complete block of text with the anchor
    rel_text_pos: RelPos,
    /// paragraph only: alignment of each line within the paragraph bounds
    rel_line_pos: RelPos,
    configured_para: Option<Vec<String>>,
    para_bounds: Rectangle,
    pad_x: i32,
    pad_y: i32,
}

impl Default for FontStyle {
    fn default() -> Self {
        FontStyle::new(color::WHITE)
    }
}

impl FontStyle {
    pub fn new(main_color: i32) -> FontStyle {
        FontStyle {
            main_color,
            shadow_color: color::get(-1, -1),
            shadow_type: String::new(),
            anchor: Point::new(screen::W / 2, screen::H / 2),
            rel_text_pos: RelPos::Center,
            rel_line_pos: RelPos::Center,
            configured_para: None,
            para_bounds: Rectangle::default(),
            pad_x: 0,
            pad_y: 0,
        }
    }

    /// Java `draw(msg, screen)`.
    pub fn draw(&self, msg: &str, screen: &mut Screen) {
        let mut size = Dimension::new(font::text_width(msg), font::text_height());

        let mut text_bounds = self.rel_text_pos.position_rect(size, self.anchor);

        if self.pad_x != 0 || self.pad_y != 0 {
            size.width += self.pad_x;
            size.height += self.pad_y;
            let text_box = self.rel_text_pos.position_rect(size, self.anchor);

            let inner_size = text_bounds.size();
            text_bounds = self.rel_line_pos.position_rect_in_container(inner_size, &text_box);
        }

        let x_pos = text_bounds.left();
        let y_pos = text_bounds.top();

        // shadow
        let sides: Vec<char> = self.shadow_type.chars().collect();
        for i in 0..8.min(sides.len()) {
            if sides[i] == '1' {
                font::draw(msg, screen, x_pos + SHADOW_POS_MAP[i], y_pos + SHADOW_POS_MAP[i + 8], self.shadow_color);
            }
        }

        // the main drawing of the text
        font::draw(msg, screen, x_pos, y_pos, self.main_color);
    }

    /// Java `configureForParagraph(para, spacing)`.
    pub fn configure_for_paragraph(&mut self, para: &[String], spacing: i32) {
        self.configured_para = Some(para.to_vec());
        let size = Dimension::new(
            font::text_width_para(para),
            para.len() as i32 * (font::text_height() + spacing),
        );
        self.para_bounds = self.rel_text_pos.position_rect(size, self.anchor);
    }

    /// Java `setupParagraphLine(para, line, spacing)`.
    pub fn setup_paragraph_line(&mut self, para: &[String], line: i32, spacing: i32) {
        if line < 0 || line as usize >= para.len() {
            eprintln!("FontStyle: index {line} is invalid; can't draw line.");
            return;
        }

        if self.configured_para.as_deref() != Some(para) {
            self.configure_for_paragraph(para, spacing);
        }

        let mut text_area = self.para_bounds;
        text_area.set_size(text_area.width(), font::text_height() + spacing, RelPos::TopLeft);
        text_area.translate(0, line * text_area.height());

        // for the relpos to put the rect in the correct pos, the anchor is fetched with the opposite relpos
        self.anchor = text_area.position(self.rel_text_pos.get_opposite());

        self.pad_x = self.para_bounds.width() - font::text_width(&para[line as usize]);
        self.pad_y = spacing;
    }

    /// Java `drawParagraphLine(para, line, spacing, screen)`.
    pub fn draw_paragraph_line(&mut self, para: &[String], line: i32, spacing: i32, screen: &mut Screen) {
        self.setup_paragraph_line(para, line, spacing);
        self.draw(&para[line as usize], screen);
        self.pad_x = 0;
        self.pad_y = 0;
    }

    /* -- Builder-style modifiers, as in Java (usable both chained and on &mut). -- */

    pub fn set_color(mut self, col: i32) -> Self {
        self.main_color = col;
        self
    }

    pub fn set_x_pos(self, pos: i32) -> Self {
        self.set_x_pos_align(pos, true)
    }

    pub fn set_x_pos_align(mut self, pos: i32, reset_alignment: bool) -> Self {
        self.anchor.x = pos;
        if reset_alignment {
            self.rel_text_pos = RelPos::get_pos(RelPos::Right.x_index(), self.rel_text_pos.y_index());
            self.rel_line_pos = RelPos::get_pos(RelPos::Left.x_index(), self.rel_line_pos.y_index());
        }
        self
    }

    pub fn set_y_pos(self, pos: i32) -> Self {
        self.set_y_pos_align(pos, true)
    }

    pub fn set_y_pos_align(mut self, pos: i32, reset_alignment: bool) -> Self {
        self.anchor.y = pos;
        if reset_alignment {
            self.rel_text_pos = RelPos::get_pos(self.rel_text_pos.x_index(), RelPos::Bottom.y_index());
            self.rel_line_pos = RelPos::get_pos(self.rel_line_pos.x_index(), RelPos::Top.y_index());
        }
        self
    }

    pub fn set_anchor(mut self, x: i32, y: i32) -> Self {
        self.anchor = Point::new(x, y);
        self
    }

    /// Java `setRelTextPos(relPos)` — also sets the line pos to the opposite.
    pub fn set_rel_text_pos(self, rel_pos: RelPos) -> Self {
        self.set_rel_text_pos_both(rel_pos, true)
    }

    pub fn set_rel_text_pos_both(mut self, rel_pos: RelPos, set_both: bool) -> Self {
        self.rel_text_pos = rel_pos;
        if set_both {
            self.rel_line_pos = self.rel_text_pos.get_opposite();
        }
        self
    }

    pub fn set_rel_line_pos(mut self, rel_pos: RelPos) -> Self {
        self.rel_line_pos = rel_pos;
        self
    }

    /// Java `setShadowType(color, full)` — outline preset or single standard shadow.
    pub fn set_shadow_type(self, color: i32, full: bool) -> Self {
        let shadow_type = if full { "10101010" } else { "00010000" };
        self.set_shadow_type_custom(color, shadow_type)
    }

    pub fn set_shadow_type_custom(mut self, color: i32, shadow_type: &str) -> Self {
        self.shadow_color = color;
        self.shadow_type = shadow_type.to_string();
        self
    }

    pub fn get_color(&self) -> i32 {
        self.main_color
    }
}
