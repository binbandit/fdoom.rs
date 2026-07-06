//! Port of `fdoom.gfx.Font`.

use super::color;
use super::font_style::FontStyle;
use super::screen::{self, Screen};
use super::sprite_sheet;

// These are all the characters that will be translated to the screen. (The spaces are important)
const CHARS: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ      0123456789.,!?'\"-+=/\\%()<>:;^@bcdefghijklmnopqrstuvwxyz";

pub fn default_background_color() -> i32 {
    let c = color::rgb(60, 63, 65);
    color::get4(c, c, c, c)
}
pub fn default_border_color() -> i32 {
    color::get4(-1, 1, color::rgb(60, 63, 65), 445)
}
pub fn default_text_color() -> i32 {
    color::get4(-1, 555, 555, 555)
}
pub fn default_title_color() -> i32 {
    color::get4(5, 5, 5, 550)
}

/// Java `Font.draw(msg, screen, x, y, col)`.
pub fn draw(msg: &str, screen: &mut Screen, x: i32, y: i32, col: i32) {
    let msg = msg.to_uppercase();
    for (i, ch) in msg.chars().enumerate() {
        if let Some(ix) = CHARS.chars().position(|c| c == ch) {
            screen.render(x + i as i32 * 8, y, ix as i32 + 30 * 32, col, 0);
        }
    }
}

/// Java `Font.textWidth(String)`.
pub fn text_width(text: &str) -> i32 {
    text.chars().count() as i32 * 8
}

/// Java `Font.textWidth(String[])` — max width over the lines.
pub fn text_width_para(para: &[String]) -> i32 {
    para.iter().map(|s| text_width(s)).max().unwrap_or(0)
}

/// Java `Font.textHeight()`.
pub fn text_height() -> i32 {
    sprite_sheet::BOX_WIDTH
}

/// Java `Font.drawCentered(msg, screen, y, color)`.
pub fn draw_centered(msg: &str, screen: &mut Screen, y: i32, color: i32) {
    FontStyle::new(color).set_y_pos(y).draw(msg, screen);
}

/// Java `Font.drawParagraph(para, screen, style, lineSpacing)` (String overload).
pub fn draw_paragraph_str(
    para: &str,
    screen: &mut Screen,
    style: &mut FontStyle,
    line_spacing: i32,
) {
    let lines = get_lines(para, screen::W, screen::H, line_spacing);
    draw_paragraph(&lines, screen, style, line_spacing);
}

/// Java `Font.drawParagraph(lines, screen, style, lineSpacing)` — the one all others call.
pub fn draw_paragraph(
    lines: &[String],
    screen: &mut Screen,
    style: &mut FontStyle,
    line_spacing: i32,
) {
    for i in 0..lines.len() {
        style.draw_paragraph_line(lines, i as i32, line_spacing, screen);
    }
}

/// Java `Font.getLines(para, w, h, lineSpacing)`.
pub fn get_lines(para: &str, w: i32, h: i32, line_spacing: i32) -> Vec<String> {
    get_lines_keep(para, w, h, line_spacing, false)
}

/// Java `Font.getLines(para, w, h, lineSpacing, keepEmptyRemainder)`.
pub fn get_lines_keep(
    para: &str,
    w: i32,
    h: i32,
    line_spacing: i32,
    keep_empty_remainder: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut para: String = para.to_string();

    let mut height = text_height();
    while !para.is_empty() {
        let split_index = get_line(&para, w);
        let chars: Vec<char> = para.chars().collect();
        lines.push(chars[..split_index].iter().collect::<String>());

        let mut skip = split_index;
        if split_index < chars.len() && (chars[split_index] == ' ' || chars[split_index] == '\n') {
            skip += 1; // skip the space/newline the line broke on
        }
        para = chars[skip..].iter().collect();

        height += line_spacing + text_height();
        if height > h {
            break;
        }
    }

    if !para.is_empty() || keep_empty_remainder {
        lines.push(para);
    }

    lines
}

/// Java `Font.renderFrame(screen, title, x0, y0, x1, y1)` with default colors.
pub fn render_frame(screen: &mut Screen, title: &str, x0: i32, y0: i32, x1: i32, y1: i32) {
    render_frame_colors(
        screen,
        title,
        x0,
        y0,
        x1,
        y1,
        default_background_color(),
        default_border_color(),
        default_title_color(),
    );
}

/// Java `Font.renderFrame(...)` with explicit colors.
#[allow(clippy::too_many_arguments)]
pub fn render_frame_colors(
    screen: &mut Screen,
    title: &str,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    col_background: i32,
    col_border: i32,
    col_title: i32,
) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            if x == x0 && y == y0 {
                screen.render(x * 8, y * 8, 13 * 32, col_border, 0);
            } else if x == x1 && y == y0 {
                screen.render(x * 8, y * 8, 13 * 32, col_border, 1);
            } else if x == x0 && y == y1 {
                screen.render(x * 8, y * 8, 13 * 32, col_border, 2);
            } else if x == x1 && y == y1 {
                screen.render(x * 8, y * 8, 13 * 32, col_border, 3);
            } else if y == y0 {
                screen.render(x * 8, y * 8, 1 + 13 * 32, col_border, 0);
            } else if y == y1 {
                screen.render(x * 8, y * 8, 1 + 13 * 32, col_border, 2);
            } else if x == x0 {
                screen.render(x * 8, y * 8, 2 + 13 * 32, col_border, 0);
            } else if x == x1 {
                screen.render(x * 8, y * 8, 2 + 13 * 32, col_border, 1);
            } else {
                screen.render(x * 8, y * 8, 2 + 13 * 32, col_background, 1);
            }
        }
    }

    draw(title, screen, x0 * 8 + 8, y0 * 8, col_title);
}

/// Java `Font.getLine(text, maxWidth)` — index (exclusive, in chars) where to split so the
/// first part is the longest line possible.
fn get_line(text: &str, max_width: i32) -> usize {
    if max_width <= 0 {
        return 0;
    }

    // Java: text.replaceAll(" ?\n ?", " \n ")
    let text = replace_newlines(text);

    let words: Vec<&str> = text.split(' ').collect();

    let mut cur_width = text_width(words[0]);

    if cur_width > max_width {
        // can't even fit the first word; fit what characters we can
        let chars: Vec<char> = words[0].chars().collect();
        let mut i = 1;
        while i < chars.len() {
            let prefix: String = chars[..i + 1].iter().collect();
            if text_width(&prefix) > max_width {
                break;
            }
            i += 1;
        }
        return i;
    }

    let mut i = 1;
    while i < words.len() {
        if words[i] == "\n" {
            break;
        }
        cur_width += text_width(&format!(" {}", words[i]));
        if cur_width > max_width {
            break;
        }
        i += 1;
    }
    // i now contains the number of words that fit on the line
    let line = words[..i].join(" ");
    line.chars().count()
}

/// Java regex `" ?\n ?" -> " \n "`.
fn replace_newlines(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // match: optional space, newline, optional space
        if chars[i] == '\n' || (chars[i] == ' ' && i + 1 < chars.len() && chars[i + 1] == '\n') {
            let mut j = i;
            if chars[j] == ' ' {
                j += 1;
            }
            debug_assert_eq!(chars[j], '\n');
            j += 1;
            if j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            out.push_str(" \n ");
            i = j;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_measures() {
        assert_eq!(text_width("HELLO"), 40);
        assert_eq!(text_height(), 8);
    }

    #[test]
    fn newline_normalization_matches_java_regex() {
        assert_eq!(replace_newlines("a\nb"), "a \n b");
        assert_eq!(replace_newlines("a \n b"), "a \n b");
        assert_eq!(replace_newlines("a \nb"), "a \n b");
        assert_eq!(replace_newlines("ab"), "ab");
    }

    #[test]
    fn get_lines_wraps_words() {
        let lines = get_lines("hello there good sir", 8 * 8, 100, 0);
        assert_eq!(lines, vec!["hello", "there", "good sir"]);
        let lines = get_lines("press escape to cancel", 288, 192, 0);
        assert_eq!(lines, vec!["press escape to cancel"]);
    }
}
