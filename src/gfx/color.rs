//! Port of `fdoom.gfx.Color` — all functions are bit-for-bit equivalents of the Java
//! static methods, operating on Java-style signed 32-bit ints.
//!
//! Encodings (Java comment vocabulary, kept here):
//! - `rgbByte`: 0-216 value encoding r,g,b (0-5 each) in base 6; 255 means transparent.
//! - `rgbInt`: classic 24-bit 0xRRGGBB color.
//! - `rgb4Sprite`: four rgbBytes packed in one int (one per sprite gray shade).
//! - `rgbReadable`: decimal digits 0-5 in the 100s/10s/1s places (e.g. 530).

pub const TRANS: i32 = get(-1, -1);
pub const WHITE: i32 = get(-1, 555);
pub const GRAY: i32 = get(-1, 333);
pub const DARK_GRAY: i32 = get(-1, 222);
pub const BLACK: i32 = get(-1, 0);
pub const RED: i32 = get(-1, 500);
pub const GREEN: i32 = get(-1, 50);
pub const BLUE: i32 = get(-1, 5);
pub const YELLOW: i32 = get(-1, 550);
pub const MAGENTA: i32 = get(-1, 505);
pub const CYAN: i32 = get(-1, 55);

/// Java `Color.get(a, b, c, d)` — packs four readable colors into one rgb4Sprite int.
pub const fn get4(a: i32, b: i32, c: i32, d: i32) -> i32 {
    (get_byte(a) << 24)
        .wrapping_add(get_byte(b) << 16)
        .wrapping_add(get_byte(c) << 8)
        .wrapping_add(get_byte(d))
}

/// Java `Color.get(a, bcd)`.
pub const fn get(a: i32, bcd: i32) -> i32 {
    get4(a, bcd, bcd, bcd)
}

/// Java `Color.pixel(a)`.
pub const fn pixel(a: i32) -> i32 {
    (get_byte(a) << 24)
        .wrapping_add(get_byte(a) << 16)
        .wrapping_add(get_byte(a) << 8)
        .wrapping_add(get_byte(a))
}

/// Java `Color.get(d)` — readable (0-555, or negative for transparent) to rgbByte.
pub const fn get_byte(d: i32) -> i32 {
    if d < 0 {
        return 255;
    }
    let r = d / 100 % 10;
    let g = d / 10 % 10;
    let b = d % 10;
    r * 36 + g * 6 + b
}

const fn limit(num: i32, min: i32, max: i32) -> i32 {
    if num < min {
        min
    } else if num > max {
        max
    } else {
        num
    }
}

/// Java `Color.rgb(red, green, blue)` — 0-255 components to a readable color.
pub fn rgb(mut red: i32, mut green: i32, mut blue: i32) -> i32 {
    if red > 255 {
        red = 255;
    }
    if green > 255 {
        green = 255;
    }
    if blue > 255 {
        blue = 255;
    }
    if red < 50 && red != 0 {
        red = 50;
    }
    if green < 50 && green != 0 {
        green = 50;
    }
    if blue < 50 && blue != 0 {
        blue = 50;
    }
    red / 50 * 100 + green / 50 * 10 + blue / 50
}

/// Java `Color.hex("#rrggbb")` — returns a readable color.
pub fn hex(hex: &str) -> i32 {
    let hex = hex.replace('#', "");
    let r = i32::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = i32::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = i32::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    rgb(r, g, b)
}

/// Java `Color.tint(color, amount, isSpriteCol)`.
pub fn tint(color: i32, amount: i32, is_sprite_col: bool) -> i32 {
    if is_sprite_col {
        let rgb_bytes = separate_encoded_sprite(color);
        let t: Vec<i32> = rgb_bytes.iter().map(|&b| tint_byte(b, amount)).collect();
        (t[0] << 24) | (t[1] << 16) | (t[2] << 8) | t[3]
    } else {
        tint_byte(color, amount)
    }
}

fn tint_byte(rgb_byte: i32, amount: i32) -> i32 {
    if rgb_byte == 255 {
        return 255; // transparent stays transparent
    }
    let rgb = decode_rgb(rgb_byte);
    let r = limit(rgb[0] + amount, 0, 5);
    let g = limit(rgb[1] + amount, 0, 5);
    let b = limit(rgb[2] + amount, 0, 5);
    r * 36 + g * 6 + b
}

/// Java `Color.separateEncodedSprite(rgb4Sprite)` — reverse of `get4`.
pub fn separate_encoded_sprite(rgb4_sprite: i32) -> [i32; 4] {
    let a = (rgb4_sprite >> 24) & 0xFF;
    let b = (rgb4_sprite & 0x00FF_0000) >> 16;
    let c = (rgb4_sprite & 0x0000_FF00) >> 8;
    let d = rgb4_sprite & 0x0000_00FF;
    [a, b, c, d]
}

/// Java `Color.separateEncodedSprite(rgb4Sprite, true)`.
pub fn separate_encoded_sprite_readable(rgb4_sprite: i32) -> [i32; 4] {
    let [a, b, c, d] = separate_encoded_sprite(rgb4_sprite);
    [un_get(a), un_get(b), un_get(c), un_get(d)]
}

/// Java `Color.decodeRGB(rgbByte)` — rgbByte to 0-5 r/g/b.
pub fn decode_rgb(rgb_byte: i32) -> [i32; 3] {
    let r = (rgb_byte / 36) % 6;
    let g = (rgb_byte / 6) % 6;
    let b = rgb_byte % 6;
    [r, g, b]
}

/// Java `Color.unGet(rgbByte)` — rgbByte to rgbReadable.
pub fn un_get(rgb_byte: i32) -> i32 {
    let c = decode_rgb(rgb_byte);
    c[0] * 100 + c[1] * 10 + c[2]
}

/// Java `Color.mixRGB(rgbByte1, rgbByte2)`.
pub fn mix_rgb(rgb_byte1: i32, rgb_byte2: i32) -> i32 {
    if rgb_byte1 == 255 || rgb_byte2 == 255 {
        return -1;
    }
    (rgb_byte1 + rgb_byte2) / 2
}

/// Java `Color.upgrade(rgbByte)` — rgbByte to 24-bit rgbInt (the palette mapping used at
/// the framebuffer). 255 becomes 0xFFFFFFFF (-1), which reads as white.
pub fn upgrade(rgb_byte: i32) -> i32 {
    if rgb_byte == 255 {
        return 0xFFFF_FFFFu32 as i32;
    }
    let r = ((rgb_byte / 36) % 6) * 51;
    let g = ((rgb_byte / 6) % 6) * 51;
    let b = (rgb_byte % 6) * 51;

    let mid = (r * 30 + g * 59 + b * 11) / 100;

    let r1 = ((r + mid) / 2) * 230 / 255 + 10;
    let g1 = ((g + mid) / 2) * 230 / 255 + 10;
    let b1 = ((b + mid) / 2) * 230 / 255 + 10;

    (r1 << 16) | (g1 << 8) | b1
}

/// Java `Color.downgrade(rgbInt)` — 24-bit color to rgbByte.
pub fn downgrade(rgb_int: i32) -> i32 {
    let c = decode_rgb_color(rgb_int);
    get_byte(rgb(c[0], c[1], c[2]))
}

/// Java `Color.mixRGBColor(rgbInt1, rgbInt2)`.
pub fn mix_rgb_color(rgb_int1: i32, rgb_int2: i32) -> i32 {
    if rgb_int1 >> 24 > 0 || rgb_int2 >> 24 > 0 {
        return 0x01FF_FFFF;
    }
    let c1 = decode_rgb_color(rgb_int1);
    let c2 = decode_rgb_color(rgb_int2);
    rgb_color(
        (c1[0] + c2[0]) / 2,
        (c1[1] + c2[1]) / 2,
        (c1[2] + c2[2]) / 2,
    )
}

/// Java `Color.rgbColor(r, g, b)`.
pub fn rgb_color(r: i32, g: i32, b: i32) -> i32 {
    (r << 16) | (g << 8) | b
}

/// Java `Color.getColor(rgbReadable)` — readable to 24-bit rgbInt.
pub fn get_color(rgb_readable: i32) -> i32 {
    if rgb_readable < 0 {
        return 0x01FF_FFFF;
    }
    let r = rgb_readable / 100 % 10;
    let g = rgb_readable / 10 % 10;
    let b = rgb_readable % 10;

    let rr = r * 255 / 5;
    let gg = g * 255 / 5;
    let bb = b * 255 / 5;
    let mid = (rr * 30 + gg * 59 + bb * 11) / 100;

    let r1 = ((rr + mid) / 2) * 230 / 255 + 10;
    let g1 = ((gg + mid) / 2) * 230 / 255 + 10;
    let b1 = ((bb + mid) / 2) * 230 / 255 + 10;

    (r1 << 16) | (g1 << 8) | b1
}

/// Java `Color.tintColor(rgbInt, amount)`.
pub fn tint_color(rgb_int: i32, amount: i32) -> i32 {
    if rgb_int < 0 {
        return rgb_int; // "transparent"
    }
    let c = decode_rgb_color(rgb_int);
    let r = limit(c[0] + amount, 0, 255);
    let g = limit(c[1] + amount, 0, 255);
    let b = limit(c[2] + amount, 0, 255);
    (r << 16) | (g << 8) | b
}

/// Java `Color.decodeRGBColor(rgbInt)`.
pub fn decode_rgb_color(rgb_int: i32) -> [i32; 3] {
    let r = (rgb_int & 0xFF_0000) >> 16;
    let g = (rgb_int & 0x00_FF00) >> 8;
    let b = rgb_int & 0x00_00FF;
    [r, g, b]
}

#[cfg(test)]
mod tests {
    use super::*;

    // All expected values captured by running the actual fdoom.gfx.Color class on a JVM.
    #[test]
    fn get_matches_java() {
        assert_eq!(get4(-1, 100, 530, 211), -14367153);
        assert_eq!(get4(5, 5, 5, 550), 84215250);
        assert_eq!(get(-1, 500), -4934476);
        assert_eq!(get4(20, 20, 121, 121), 202125617);
        assert_eq!(get(0, 555), 14145495);
        assert_eq!(get_byte(-1), 255);
        assert_eq!(get_byte(0), 0);
        assert_eq!(get_byte(555), 215);
        assert_eq!(get_byte(345), 137);
        assert_eq!(get_byte(211), 79);
        assert_eq!(pixel(555), -673720361);
        assert_eq!(pixel(-1), -1);
        assert_eq!(pixel(320), 2021161080);
    }

    #[test]
    fn rgb_and_hex_match_java() {
        assert_eq!(rgb(60, 63, 65), 111);
        assert_eq!(rgb(255, 255, 255), 555);
        assert_eq!(rgb(10, 0, 300), 105);
        assert_eq!(rgb(51, 102, 204), 124);
        assert_eq!(hex("#2c2c2c"), 111);
        assert_eq!(hex("#ff0000"), 500);
        assert_eq!(hex("#123456"), 111);
    }

    #[test]
    fn upgrade_matches_java() {
        assert_eq!(upgrade(255), -1);
        assert_eq!(upgrade(0), 657930);
        assert_eq!(upgrade(100), 8762291);
        assert_eq!(upgrade(215), 15790320);
        assert_eq!(upgrade(37), 2691881);
    }

    #[test]
    fn tint_matches_java() {
        assert_eq!(tint(get4(-1, 100, 530, 211), 1, true), -11547270);
        assert_eq!(tint(get4(20, 20, 121, 121), -1, true), 101058054);
        assert_eq!(tint(100, 2, false), 179);
        assert_eq!(tint(255, 3, false), 255);
        assert_eq!(tint(215, 1, false), 215);
    }

    #[test]
    fn separate_and_decode_match_java() {
        assert_eq!(
            separate_encoded_sprite(get4(-1, 100, 530, 211)),
            [255, 36, 198, 79]
        );
        assert_eq!(
            separate_encoded_sprite_readable(get4(-1, 100, 530, 211)),
            [103, 100, 530, 211]
        );
        assert_eq!(decode_rgb(211), [5, 5, 1]);
        assert_eq!(un_get(135), 343);
        assert_eq!(mix_rgb(100, 200), 150);
        assert_eq!(mix_rgb(255, 10), -1);
    }

    #[test]
    fn rgb_int_functions_match_java() {
        assert_eq!(get_color(555), 15790320);
        assert_eq!(get_color(-1), 33554431);
        assert_eq!(get_color(320), 8283961);
        assert_eq!(tint_color(0xd6d6d6, 20), 15395562);
        assert_eq!(tint_color(-1, 20), -1);
        assert_eq!(tint_color(0x0a0a0a, -30), 0);
        assert_eq!(downgrade(0xffffff), 215);
        assert_eq!(downgrade(0x336699), 51);
        assert_eq!(mix_rgb_color(0x336699, 0x224466), 2774399);
        assert_eq!(mix_rgb_color(0x01ffffff, 0x224466), 33554431);
    }
}
