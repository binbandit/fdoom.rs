# Rendering & UI

Exhaustive reference for fdoom.rs's software renderer, sprite/font systems, the art
generator that produces `assets/sprites.png`, the per-frame render pipeline, and the
display/menu system. See also [ARCHITECTURE.md](ARCHITECTURE.md) for the whole-codebase
tour, [TERRAIN.md](TERRAIN.md) for world generation, and
[CORE_AND_SAVES.md](CORE_AND_SAVES.md) for the `Game` struct and tick order this pipeline
runs inside.

Every claim below is grounded in the source as of this writing; file:line references are
approximate anchors (line numbers drift), not guarantees — grep the quoted symbol if a
number is stale.

## 1. Overview + mental model

Everything is software-rendered into `Screen.pixels: Vec<i32>` — a flat 288x192 buffer of
Java-style signed `i32` pixels (`src/gfx/screen.rs`). There is no GPU path. The pipeline,
top to bottom:

```
platform::App (winit loop, src/platform/mod.rs)
  │  drives Game::tick() at a fixed rate, independent of rendering
  │
  └─ RedrawRequested → App::redraw()
        │
        └─ Renderer::render(&mut self, g: &mut Game)      [src/core/renderer.rs]
             ├─ render_level(g)      OR      render_flyover(g)   (mutually exclusive)
             ├─ render_gui(g)                (only alongside render_level)
             ├─ top Display::render(&mut screen, g)        (menus drawn over gameplay)
             └─ render_focus_nagger(g)       (drawn absolute last, if unfocused)
        │
        └─ nearest-neighbor scale `renderer.screen.pixels` into the softbuffer window
             buffer (src/platform/mod.rs)
```

Two `Screen`s exist inside `Renderer`: `screen` (the main framebuffer, holds "upgraded"
24-bit RGB) and `light_screen` (meant to hold raw 0-255 brightness for a day/night and
cave-darkness overlay). The overlay pass (`Screen::overlay`, §2) is fully implemented but
**never called** from `renderer.rs` — this is the Java fork's cave-darkness/light overlay,
preserved commented-out per PORTING.md's "preserved quirks" (do not "fix" — it is
deliberately disabled, code present and correct, just unreachable).

Colors are never literal RGB in game code — they are packed 4-shade palettes (`i32`,
`color::get4`) resolved against a **hybrid** sprite sheet where each pixel is either an
indexed grayscale shade (recolored at draw time) or literal true-color RGB (drawn as-is).
This hybrid is what makes item/mob recoloring "free" (same art, different palette word)
while still allowing painterly true-color scenery (trees, fire, logos) that would look
wrong quantized to 4 shades.

## 2. `Screen` — the pixel buffer (`src/gfx/screen.rs`)

```rust
pub const W: i32 = 288;   // Java Renderer.WIDTH / Screen.w
pub const H: i32 = 192;   // Java Renderer.HEIGHT / Screen.h
pub const CENTER: Point = Point { x: W / 2, y: H / 2 };   // (144, 96)

const MAXDARK: i32 = 128;
const BIT_MIRROR_X: i32 = 0x01;
const BIT_MIRROR_Y: i32 = 0x02;
const DITHER: [i32; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
```

```rust
pub struct Screen {
    x_offset: i32,
    y_offset: i32,
    pub pixels: Vec<i32>,      // len = W*H = 55296, public — the platform blit reads this
    sheet: Arc<SpriteSheet>,
}
```

`x_offset`/`y_offset` (set via `set_offset`) are the level→screen coordinate conversion —
every `render`/`darken_rect` call subtracts them so callers can pass *level*-space pixel
coordinates and the screen scrolls for free.

### Core primitives

| Method | Purpose |
|---|---|
| `clear(color)` | `pixels.fill(color)` |
| `render_slice(pixel_colors)` | bulk copy (map view) |
| `render(xp, yp, tile, colors, bits)` | the core 8x8 sprite-cell blit — see below |
| `darken_rect(xp, yp, w, h, amount)` | darken a **level-space** rect (subtracts offset, delegates to `darken_rect_screen`) |
| `darken_rect_screen(xp, yp, w, h, amount)` | darken a **screen-space** rect — see below |
| `render_pixel_array(xp, yp, w, h, img_pixels)` | raw pixel-array blit, per-axis bounds-checked |
| `set_offset(x, y)` | level→screen conversion factor |
| `overlay(screen2, current_level, xa, ya, tick_count, past_day1, time)` | day/night + cave-darkness merge — implemented, unreachable from `renderer.rs` (see §1) |
| `copy_rect(self, screen2, x2, y2, w2, h2)` | copies **from `self` into `screen2`** — note the reversed direction vs. what the name suggests |
| `render_light(x, y, r)` | writes a radial brightness gradient (raises pixel value only, never lowers) — feeds the light screen |

**`render(xp, yp, tile, colors, bits)`** (screen.rs:60-97) — the one function that turns a
sheet cell into pixels:

```rust
let x_tile = tile % 32;
let y_tile = tile / 32;
let toffs = x_tile * 8 + y_tile * 8 * sheet.width;   // sheet is 256px wide -> 32 cells/row
```
For each of the 64 pixels in the 8x8 cell (mirrored per-pixel if `bits` has
`BIT_MIRROR_X`/`BIT_MIRROR_Y` set), it matches the sheet pixel (see `SheetPixel`, §3):

- `Palette(shade)`: `col = (colors >> ((3 - shade) * 8)) & 0xFF`; if `col < 255`, writes
  `color::upgrade(col)`. Byte value `255` in the packed palette means "this shade is
  transparent for this sprite" — the same sentinel `color::get_byte(-1)` produces.
- `Rgb(rgb)`: writes `rgb` directly, no palette involved.
- `Transparent`: no-op.

Bounds are checked per-pixel (`x+xp`/`y+yp` against `0..W`/`0..H`), so a cell straddling
the screen edge just clips silently.

**`darken_rect_screen(xp, yp, w, h, amount)`** (screen.rs:106-118) — the single shared
darkening primitive used by *both* world-space fog/dimming and UI panels (see §7's
"smoked-glass panel"):

```rust
let keep = (255 - amount.clamp(0, 255)) as u32;
// per pixel already on screen:
let r = ((p >> 16 & 0xFF) * keep) >> 8;
let g = ((p >> 8 & 0xFF) * keep) >> 8;
let b = ((p & 0xFF) * keep) >> 8;
pixels[i] = (r << 16) | (g << 8) | b;
```
Multiplicative: `amount=0` leaves the pixel untouched, `amount=255` (`keep=0`) makes it
black. Uses `>> 8` (divide by 256, not 255) as a fast approximation — matches the Java
original. This is **distinct** from `color::tint_color` (§4), which is additive/clamped
and used by the day/night `overlay` pass instead.

## 3. `SpriteSheet` — the hybrid sheet (`src/gfx/sprite_sheet.rs`)

```rust
pub const BOX_WIDTH: i32 = 8;    // one sprite cell, 8x8px
pub const TILE_SIZE: i32 = 16;   // one world tile = 2x2 cells

pub enum SheetPixel {
    Transparent,
    Palette(u8),   // shade 0..3 — recolored at draw time via a caller-supplied palette
    Rgb(i32),       // 0xRRGGBB — drawn literally, ignores the caller's palette entirely
}

pub struct SpriteSheet {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<SheetPixel>,
}
```

`SpriteSheet::from_png(png_bytes)` decodes via the `png` crate, expanding whatever the PNG's
`ColorType` is (Grayscale, GrayscaleAlpha, Rgb, Rgba — indexed PNGs are `unreachable!()`
since the `png` crate auto-expands them) into `(r, g, b, a)` per source pixel, then
classifies:

```rust
if a < 128        { SheetPixel::Transparent }
else if r == g && g == b { SheetPixel::Palette(r / 64) }   // buckets 0-63/64-127/128-191/192-255 -> shade 0/1/2/3
else               { SheetPixel::Rgb((r << 16) | (g << 8) | b) }
```

So the sheet PNG itself is a normal RGBA image; the *classification rule* is what makes it
hybrid — any pixel with `r==g==b` (true gray) becomes a recolorable `Palette` shade, any
other color is baked in as literal `Rgb`, and low alpha is always `Transparent` regardless
of color. **`artgen.rs` bakes exact gray levels `0/85/170/255`** for the four shades (see
§6) — the `/64` bucketing maps these cleanly to shades `0/1/2/3`; do not author "true-color"
art using an accidental `r==g==b` value or it will silently become a recolorable palette
cell instead of literal color (`artgen.rs`'s `rgb()` helper asserts `!(r==g && g==b)` to
catch this at generation time).

## 4. `color` — packed palettes and the 0-5 cube (`src/gfx/color.rs`)

Vocabulary (kept from the Java `Color` class):

- **`rgbByte`**: 0-215, base-6 encoding of `(r,g,b)` each 0-5, or `255` = transparent.
- **`rgbInt`** / **`rgb_int`**: 24-bit `0xRRGGBB`.
- **`rgb4Sprite`**: four `rgbByte`s packed into one `i32`, one per sprite shade — this is
  what `colors: i32` means everywhere in the render call chain.
- **`rgbReadable`**: decimal-digit shorthand, e.g. `530` = `r=5,g=3,b=0`. This is the form
  used at nearly every call site (`color::get4(-1, 200, 500, 533)`).

```rust
pub const fn get4(a: i32, b: i32, c: i32, d: i32) -> i32 {
    (get_byte(a) << 24)
        .wrapping_add(get_byte(b) << 16)
        .wrapping_add(get_byte(c) << 8)
        .wrapping_add(get_byte(d))
}
// get(a, bcd)  = get4(a, bcd, bcd, bcd)   — one byte + a readable color repeated for shades 1-3
// pixel(a)     = get4(a, a, a, a)          — same color for all four shades
```
`wrapping_add` (not `|`) is deliberate — Java int overflow semantics matter here because
`get_byte(-1) == 255` shifted into the top byte can overflow into the sign bit.

```rust
pub const fn get_byte(d: i32) -> i32 {
    if d < 0 { return 255; }                  // transparent sentinel
    let r = d / 100 % 10; let g = d / 10 % 10; let b = d % 10;
    r * 36 + g * 6 + b
}
```

**`upgrade(rgb_byte) -> i32`** (0-5 cube → final RGB, called by `Screen::render` for every
`Palette` pixel):

```rust
if rgb_byte == 255 { return -1; }   // 0xFFFFFFFF, opaque-white/"transparent" sentinel
let r = ((rgb_byte / 36) % 6) * 51;   // 0..5 -> 0..255 in steps of 51
let g = ((rgb_byte / 6) % 6) * 51;
let b = (rgb_byte % 6) * 51;
let mid = (r*30 + g*59 + b*11) / 100;                 // luma-weighted mid tone
let r1 = ((r + mid) / 2) * 230 / 255 + 10;             // blend toward luma, compress to ~[10,240]
let g1 = ((g + mid) / 2) * 230 / 255 + 10;
let b1 = ((b + mid) / 2) * 230 / 255 + 10;
(r1 << 16) | (g1 << 8) | b1
```
This is a deliberate desaturate-toward-luma + dynamic-range-compress transform, not a
straight `*51` mapping — it is the entire reason the palette produces the game's muted
retro look rather than harsh RGB primaries. Verified against JVM-captured values in the
file's `#[cfg(test)]` block (e.g. `upgrade(0) == 657930`).

**`tint_color(rgb_int, amount)`** — the *other* darken/brighten function, additive and
clamped per channel (`limit(c+amount, 0, 255)`), passes values `< 0` (transparent sentinel)
through unchanged. Used by `Screen::overlay`'s day/night math; **not** the same primitive
as `darken_rect_screen`'s multiplicative scaling (§2) — UI panels use the multiplicative
one, the (currently unreachable) light overlay uses this additive one.

Named consts: `TRANS`, `WHITE = get(-1,555)`, `GRAY = get(-1,333)`, `DARK_GRAY = get(-1,222)`,
`BLACK = get(-1,0)`, `RED`/`GREEN`/`BLUE`/`YELLOW`/`MAGENTA`/`CYAN`. Other helpers:
`rgb(r,g,b)` (0-255 → readable, clamps/snaps to fifths), `hex("#rrggbb")`,
`tint`/`tint_byte`, `separate_encoded_sprite[_readable]` (inverse of `get4`),
`decode_rgb`/`un_get`/`mix_rgb`/`downgrade`/`get_color`.

## 5. Sprites (`src/gfx/sprite.rs`)

**Addressing** — `Px { sheet_pos: i32, mirror: i32 }`, one 8x8 cell reference:
```rust
pub fn new(sheet_x, sheet_y, mirroring) -> Px {
    Px { sheet_pos: sheet_x + 32 * sheet_y, mirror: mirroring }   // pos = x + y*32 (32 cells/row)
}
```

**`Sprite`**:
```rust
pub struct Sprite {
    pub sprite_pixels: Vec<Vec<Px>>,       // [row][col]
    pub color: i32,                         // packed rgb4Sprite palette (or literal for true-color art)
    pub sheet_loc: (i32, i32, i32, i32),    // (x, y, w, h) on the sheet, in cells
}
```
Constructors: `from_pos(pos, color)`, `new1x1`, `new(sx,sy,sw,sh,color,mirror)`,
`new_onepixel(...)` (every cell reuses one sheet cell — solid fills like `blank`/`repeat`),
`with_mirrors(...)` (per-cell mirror bitmask array), `from_pixels(...)` (raw),
`missing_texture(w,h)` = `Sprite::new(30,30,w,h,color::get(505,505),0)` (matches the
`artgen_sheet.rs` cell `(30,30)`), `blank(w,h,col)` = cell `(7,2)`, `repeat(sx,sy,w,h,col)`
(tiles one cell across an arbitrarily large block), `dots(col)` (the 4 "dots" cells at row
0), `random_dots(seed,col)` (deterministic via `Rng::new(seed)`).

`mob(sx, sy, w, h, mirror)` — **whole-sprite flip**, distinct from per-cell `Px::mirror`:
```rust
let flip_x = mirror & 0x01; let flip_y = mirror & 0x02;
// for each (r, c): source = (sx + if flip_x {w-1-c} else {c}, sy + if flip_y {h-1-r} else {r})
```
i.e. it re-indexes into the *same* source block mirrored as a whole (a genuine left-right
swap of which cell goes where), whereas `Px::mirror` flips each individual 8x8 cell's own
pixels at render time. Mob walk animations use the whole-block flip to derive left-facing
frames from right-facing art (see `MobSprite` below) — a per-cell flip alone would produce
a mirrored-but-still-wrong-order block.

### `MobSprite` — walk-cycle animation compiler

```rust
pub type MobAnims = Vec<Vec<Sprite>>;   // [direction][frame]

pub fn compile_sprite_list(sheet_x, sheet_y, width, height, mirror, number) -> Vec<Sprite> {
    // `number` consecutive Sprite::mob blocks side by side: sheet_x + width*i
}

pub fn compile_mob_sprite_animations(sheet_x, sheet_y) -> MobAnims {
    let set1 = compile_sprite_list(sheet_x, sheet_y, 2, 2, 0, 4); // down1,up1,right1,right2
    let set2 = compile_sprite_list(sheet_x, sheet_y, 2, 2, 1, 4); // down2,up2,left1,left2 (mirror=1)
    // -> [[down1,down2], [up1,up2], [left1,left2], [right1,right2]]
}
```
Down/up's second walk frame and both left-facing frames are the **same source art** as the
right-facing/first-frame cells, reused via `mirror=1` (whole-sprite flip). This is why
`artgen.rs`'s mob recipes only ever paint a right-facing walk cycle — everything else is
derived by this mirror, not separately drawn.

### `ConnectorSprite` — terrain-edge pieces

The *data* struct lives in `src/level/tile/mod.rs:81-108` (not `sprite.rs`), but is
documented here since it's the sprite system's connector-piece mechanism:

```rust
pub struct ConnectorSprite {
    pub sparse: Sprite,        // 3x3-cell sub-sprite grid — edge/corner pieces
    pub sides: Sprite,         // 2x2 straight-edge block (inner-corner case)
    pub full: Sprite,          // 2x2 solid-fill block (fully surrounded)
    pub check_corners: bool,   // whether diagonal neighbors matter
}
// ::new(sparse, sides, full)  -> check_corners = true
// ::simple(sparse, full)      -> sides = sparse.clone(), check_corners = false
```

The actual per-neighbor rendering is `csprite_render` in `src/level/tile/dispatch.rs:433-550`.
For a tile at `(x,y)` it queries the 4 orthogonal + 4 diagonal neighbors via
`connects_to(def, neighbor, is_side)` (a `TileDef`-level predicate — e.g. grass's is "the
neighbor's `connects_to_grass`"), producing booleans `u,d,l,r` (orthogonal) and (only
checked when both adjacent orthogonals connect, and only if `check_corners`) `ul,ur,dl,dr`
(diagonal). It then renders the tile's **four 8x8 quadrants independently** — e.g. the
top-left quadrant:

```
if u && l:
    if ul || !check_corners  -> full sub-cell (1,1)     // interior look
    else                      -> sides sub-cell (0,0)    // inner-corner: side connects, diagonal doesn't
else                          -> sparse sub-cell (l?1:2, u?1:2)   // eroded edge/corner
```
(top-right/bottom-left/bottom-right follow the mirrored pattern with different `full`/
`sides` sub-cell coordinates and `sparse` axis assignment — see the file for exact
per-quadrant coordinates if extending this). Each quadrant also blends `sparse`'s palette
toward the neighboring tile's own sparse color via `get_sparse_color`, so an edge tile's
color gradually shifts toward its neighbor (grass fading into sand/dirt at a border) rather
than having a hard color seam.

Concrete example — grass (`src/level/tile/grass.rs:14-25`):
```rust
def.csprite = Some(ConnectorSprite::simple(
    Sprite::new(11, 0, 3, 3, color::get4(141, 141, 252, 321), 3),   // sparse: sheet (11,0), 3x3
    Sprite::dots(color::get4(141, 141, 252, 321)),                  // full: the "dots" texture
));
```
`connects_to` for grass: diagonal neighbors always connect; orthogonal neighbors connect
only if `other.connects_to_grass`.

## 6. Font system (`src/gfx/font.rs`, `src/gfx/font_style.rs`)

**Glyph layout**: `CHARS` (font.rs:9-10) is the literal ordering of glyph cells on the
sheet, starting at sheet position `30*32` (row 30):
```
"ABCDEFGHIJKLMNOPQRSTUVWXYZ      0123456789.,!?'\"-+=/\\%()<>:;^@bcdefghijklmnopqrstuvwxyz"
```
6 spaces after `Z` are reserved blank cells. **Lowercase glyphs at the tail are currently
unreachable**: `Font::draw` uppercases text before lookup, and `artgen.rs`'s `glyph(ch)`
only has ASCII-art defined for `A-Z`, `0-9`, and the listed punctuation — there is no
lowercase branch, so those sheet cells exist as reserved space but are never populated with
actual strokes (confirmed by `tests/artgen_sheet.rs`'s `FONT_CHARS`, which stops at `@`).

Default style helpers: `default_background_color()`, `default_border_color()`,
`default_text_color()`, `default_title_color()` — used by `render_frame` (below).

### Draw functions

| Function | Behavior |
|---|---|
| `draw(msg, screen, x, y, col)` | uppercases, looks up each char's index in `CHARS`, `screen.render(x+i*8, y, ix+30*32, col, 0)` |
| `draw_centered(msg, screen, y, color)` | `FontStyle::new(color).set_y_pos(y).draw(msg, screen)` |
| `draw_paragraph_str(para, screen, style, line_spacing)` | word-wraps via `get_lines`, then `draw_paragraph` |
| `draw_paragraph(lines, screen, style, line_spacing)` | loops `style.draw_paragraph_line(lines, i, line_spacing, screen)` |
| `render_frame(screen, title, x0, y0, x1, y1)` / `render_frame_colors(...)` | draws a bordered UI panel in **tile-cell** coordinates (each `*8` for pixel pos) |
| `text_width(text)` | `chars().count() * 8` |
| `text_height()` | `sprite_sheet::BOX_WIDTH` (8) |
| `get_lines`/`get_lines_keep(para, w, h, line_spacing, keep_empty_remainder)` | greedy word-wrap, stops once accumulated height would overflow `h` |
| `get_line(text, max_width)` (private) | word-wrap core; falls back to character truncation if a single word alone exceeds `max_width`; a literal `"\n"` token forces a break |

`render_frame` corner/edge sheet cells: corners at `13*32` (mirrored 0/1/2/3 per corner),
horizontal edges at `1+13*32` (mirrored 0/2), vertical edges at `2+13*32` (mirrored 0/1),
interior fill also `2+13*32` mirror 1 with `col_background`. Title text overlaps the top
border via `draw(title, screen, x0*8+8, y0*8, col_title)`.

### `FontStyle` — positioning/shadow builder

```rust
pub struct FontStyle {
    main_color: i32, shadow_color: i32,
    shadow_type: String,        // 8-char "10101010"-style bitstring, one char per compass dir
    anchor: Point,
    rel_text_pos: RelPos,       // whole-text-block alignment to anchor
    rel_line_pos: RelPos,       // per-line alignment within paragraph bounds
    configured_para: Option<Vec<String>>,
    para_bounds: Rectangle,
    pad_x: i32, pad_y: i32,
}
```
`SHADOW_POS_MAP: [i32; 16]` gives 8 compass-direction offsets (first 8 = x, next 8 = y);
`draw()` draws up to 8 shadow copies (one per `'1'` char in `shadow_type`) then the main
text on top. `set_shadow_type(color, full)`: `full` ⇒ `"10101010"` (full outline), else
`"00010000"` (single default shadow direction). Chainable builder methods (`set_color`,
`set_x_pos[_align]`, `set_y_pos[_align]`, `set_anchor`, `set_rel_text_pos[_both]`,
`set_rel_line_pos`, `set_shadow_type[_custom]`) all consume/return `self`.
`configure_for_paragraph`/`setup_paragraph_line`/`draw_paragraph_line` carve one row's rect
out of the whole paragraph's bounding box per call, so each line can independently align
(e.g. right-pad) within the paragraph.

## 7. The art generator (`src/bin/artgen.rs`, ~3500 lines)

**Contract** (top doc comment): this file is *the* source of truth for the sprite sheet —
never hand-edit the PNG. Run with `cargo run --bin artgen`. Pure `std` + the `png` crate, no
`rand`-crate RNG — a deterministic hash function (`speck`) stands in for "noise" so output
is bit-for-bit reproducible run to run.

**Output**: `fn main()` (end of file) calls every recipe function in a fixed order (terrain
→ items/UI → furniture → mobs → text), then:
```rust
let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sprites.png");
s.save(&path);
```
**`assets/sprites.png`** (repo-root-relative via `CARGO_MANIFEST_DIR`) is the confirmed
output path. It is loaded back at runtime via `src/assets.rs`:
```rust
pub const SPRITES_PNG: &[u8] = include_bytes!("../assets/sprites.png");
```
and decoded into the live `Arc<SpriteSheet>` in `src/lib.rs` via `SpriteSheet::from_png`.
**There is no build-time regeneration** — if you edit `artgen.rs`, you must re-run the
binary and commit the resulting PNG; the game embeds whatever bytes are on disk at compile
time.

### Sheet contract (from the file's own doc comment — worth mining verbatim)

256x256px, 32x32 grid of 8x8 cells, `pos = cx + cy*32` (same addressing as `Px::new`).
Palette-mode cells are grayscale quantized to 4 shades (`G0..G3 = [0, 85, 170, 255]`,
alpha 255) recolored via `color::get4` at draw time; true-color cells are drawn literally
(alpha 0 = transparent). **Never author an `r==g==b` color for true-color art** — it will
silently reclassify as a palette shade (`SheetPixel::Palette`) instead of `Rgb` (see §3);
`artgen.rs`'s `rgb()` helper asserts against this at generation time.

### Drawing kit (artgen.rs:80-280)

- `Ink = [u8; 4]` — one RGBA pixel. `TR` = transparent; `G0..G3` = the four palette grays.
- `const fn rgb(r,g,b) -> Ink` — panics if `r==g==b` (the true-color guard above).
- `Sheet { px: Vec<Ink> }` — `new()` (all transparent), `set`/`get`, `save(path)` (writes
  RGBA PNG via `png::Encoder`).
- `C<'a> { s: &mut Sheet, ox: i32, oy: i32 }` — a cursor canvas anchored at one cell's pixel
  origin (`cell(s, cx, cy) -> C`). Methods: `set`, `rect`, `hline`/`vline`,
  `disc(cx,cy,r,ink)` (rasterized circle), `dither(x,y,w,h,phase,ink)` (checkerboard),
  `pat(x,y,rows: &[&str], map: &[(char,Ink)])` — **the dominant authoring style**: paint a
  cell from an ASCII-art block, one string per row, unmapped chars (`.`) leave the pixel
  untouched — `outline(x,y,w,h,ink)` (auto 1px silhouette outline for true-color sprites).
- `speck(x,y,seed,one_in) -> bool` — deterministic hash noise (fixed multiply-xor-shift
  constants), returns true roughly 1-in-`one_in`.
- `rounded_inside(x,y,w,h,inset,r) -> bool` — point-in-rounded-rect test for blobs/discs.

A shared ~24-color true-color palette (`OUT`, `LEAF_DK/MD/LT/HI`, `BARK[_DK]`,
`WOOD_LT/MD/DK`, `SAND_LT/DK`, `STONE_LT/MD/DK`, `IRON_LT/DK`, `FLAME_YL/OR/RD`, `CREAM`,
`RED_CL`, `MAGIC[_LT]`, `PUMPK[_DK]`, `GOLDEN`, `MOSS`) keeps all scenery art-directed
consistently.

### Per-sprite recipe organization

One function per logical sheet region — generic reusable painters (`blob24` for
connector-sparse blobs parameterized by radius/inks/seed, used by rock/grass/water
connectors; `sides16` for rock/cloud/wall "sides" blocks; `icon8`/`item_icon`/`center8` for
auto-centered 8x8 inventory icons; `spr16`/`tc16` generic 16x16 palette/true-color
painters; `frame16`, the 16x16 ASCII-art painter used for every mob frame — the player/suit/carry
sets are pixel-for-pixel transcriptions of the original Java sheet (`icons.png`,
removed after tracing — see git history),
and `marsh_lurker`, `pig`, `knight`, `feral_hound`, `cow`, `stone_golem`, `night_wisp`,
`sheep`, `snake`, `glow_worm` each have their own recipe; the old `humanoid` head+body
composer was retired with the traced player art), plus per-tile-family functions (`wool_cell`, `cloud_full_cells`, `farm_cell`,
`ore_cells`, `quicksand_cells`, `stairs_cells`, `cactus_cells`, `wheat_cells`,
`sapling_cell`, `torch_cell`, `floor_cells`, `tree_cells`, `door_cells`, `wall_cells`,
`gravestone_cells`, `pumpkin_cells`, `tall_grass_cells`, `furniture_sprites`,
`furniture_icons`, `items_row4`/`items_row5`, `ui_row12`/`ui_row13`, `splash_cells`), and
text (`glyph(ch)` — 7-row ASCII-art glyph defs; `font(s)` — paints `G0` background +
`G3`-only strokes per char; `logo(s)` — the true-color "FDOOM" title art, 2 rows tall at
cells `(0..14, 6..8)`).

`missing_texture(s)` — cell `(30,30)`: flat `G1`/`G2` dither checkerboard, recolored magenta
at runtime by `Sprite::missing_texture`'s `color::get(505,505)`.

### Palette-mode vs. true-color rules (enforced by tests, §8)

The doc comment inventories which cell ranges are palette-mode vs. true-color and per-cell
shade-role conventions (e.g. shade1=interior, shade2=edge band, shade3="outside"/recolored
for connector blobs). As a rule: font glyphs, item/tool icons, mob walk-cycle rows, HUD
icons, terrain connector blobs, and UI frame pieces are **palette-mode** (so they can be
recolored per-instance); tree canopies, cactus, torch flame, pumpkin, the title logo, TNT,
grave stones, and quicksand are **true-color** (painterly detail that a 4-shade quantization
would flatten).

## 8. `tests/artgen_sheet.rs` — what it locks in

**Important**: this is *not* a byte-identical/hash/pixel-diff test against a separately
checked-in golden image. It loads the **same** `assets/sprites.png` bytes shipped in the
repo (via `fdoom::assets::SPRITES_PNG`, decoded through the real `SpriteSheet::from_png`)
and asserts **structural/semantic** properties of that decoded sheet:

1. `sheet_is_256x256` — dimensions.
2. `every_referenced_cell_has_art` — a hand-maintained `INVENTORY: &[(cx, cy, w, h, &str)]`
   table (built by auditing every `Sprite::*` constructor, raw `screen.render(.., pos, ..)`
   call, the font, and the title logo) asserts every listed cell range has at least one
   non-`Transparent` pixel.
3. `font_glyphs_are_palette_grayscale` — every char up to `@` in `Font::CHARS` (the
   actually-renderable prefix) is `pure_grayscale()` (all `Palette`), and non-space glyphs
   have at least one non-zero-shade pixel (actual strokes, not just background).
4. `palette_cells_stay_grayscale` — a second hand-maintained range list (dots, terrain
   blobs, wool, ore, stairs, floor, wheat, item icons, tools, furniture/HUD icons, frame,
   effect sprites, mob rows, doors, walls) must **never** contain `SheetPixel::Rgb`.
5. `scenery_cells_are_true_color` — spot-checks 8 cells (tree canopy, cactus, torch,
   pumpkin, title logo, tnt, grave stone, quicksand) each contain at least one `Rgb` pixel.

So the test would **not** catch a color value changing within a cell as long as the pixel
*mode* (palette vs. rgb vs. transparent) is unchanged — only `artgen.rs`'s own determinism
(no RNG, a stable hash) guarantees byte-identical regeneration across runs. If you add a new
sprite/cell, add it to `INVENTORY` (and to `palette_cells_stay_grayscale`/
`scenery_cells_are_true_color` if it needs a mode guarantee) or the test won't catch a
missing/blank cell.

## 9. The frame pipeline

### 9.1 Platform loop → `Renderer::render` (`src/platform/mod.rs`, `src/core/renderer.rs`)

`platform::App` (winit `ApplicationHandler`) drives two independent cadences:

- **Tick** (`about_to_wait` → `loop_iteration`): a fixed-timestep accumulator
  (`unprocessed: f64`) against `ns_per_tick = 1e9 / updater::NORM_SPEED`, divided by
  `g.gamespeed` only when no menu is active. `while unprocessed >= 1.0 { game.tick(); ... }`
  — unbounded, no catch-up cap (see CORE_AND_SAVES.md for what a freeze does to this).
- **Render**: gated by wall-clock elapsed-since-last-render vs. `1.0 / g.max_fps`; when due,
  calls `window.request_redraw()`, which winit later delivers as `RedrawRequested` →
  `App::redraw()` → `self.renderer.render(&mut self.game)`. Rendering is fully decoupled
  from ticking — zero or many ticks may have run since the last render.

`App::redraw()` then reads `renderer.screen.pixels` and nearest-neighbor scales it into the
softbuffer window surface: `scale = min(win_w/W, win_h/H)` (aspect-preserving, letterboxed
with black bars, centered), sampling `src = dest / scale` (floored, clamped) per destination
pixel, masking `(pixel as u32) & 0x00FF_FFFF` to strip any sign-extension garbage from the
`i32→u32` cast. Window resize has no dedicated handler — every `redraw()` re-reads
`window.inner_size()` and recomputes scale/offsets from scratch.

### 9.2 `Renderer::render` entry point (`renderer.rs:49-74`)

```rust
pub fn render(&mut self, g: &mut Game) {
    if !g.has_gui { return; }

    if g.ready_to_render_gameplay {
        self.flyover = None;
        self.render_level(g);
        self.render_gui(g);
    } else if g.display.menu_active() {
        self.render_flyover(g);
    }

    if let Some(mut top) = g.display.stack.pop() {          // take-out pattern
        g.display.taken_out = true;
        top.render(&mut self.screen, g);
        g.display.taken_out = false;
        g.display.stack.push(top);
    }

    if !g.has_focus && !g.is_online() && !g.continous {
        self.render_focus_nagger(g);
    }
}
```
Draw order bottom-to-top: gameplay **or** flyover backdrop (mutually exclusive — gameplay
tears down the flyover state the instant it starts rendering) → the top `Display` on the
stack, if any (menus always drawn over gameplay) → the focus-nagger box, drawn absolute
last. The display-stack pop/render/push is the same take-out idea PORTING.md describes for
entities, reused here so `top.render` can take `&mut Game` without an aliasing conflict with
`g.display.stack` (see CORE_AND_SAVES.md §"display stack" for the full mechanism).

### 9.3 Flyover backdrop (`render_flyover`, renderer.rs:78-136)

Purpose: the title-screen drone-flyover — a throwaway infinite chunked level slowly panned
under the main menu. `struct Flyover { seed: i64, cam_x: f64, cam_y: f64, heading: f64 }`.

**First-call setup**: builds a scratch `Level::empty(128, 128, 0, 1)` at level slot **3**
(the surface slot), force-converts it to infinite (`level.chunks = Some(ChunkMap::default())`),
seeds `g.world_seed`, finds a land spawn via `infinite_gen::find_surface_spawn`, converts to
pixel coords (`cam_x = sx*16`). If the slot is later taken over by a real world (loading
screen etc.), the flyover detects it's no longer infinite and tears itself down.

**Camera drift** (exact, every frame):
```rust
fly.heading += 1.0;                              // repurposed as a frame counter
if fly.heading as u64 % 2 == 0 { fly.cam_x += 1.0; }   // exactly 1px every OTHER frame
fly.cam_y += (fly.heading * 0.004).sin() * 0.12;       // slow north/south sine wander
```
Design rationale (source comment): a regular 1px-every-other-frame cadence reads smoother
than a fractional per-frame speed, which steps at irregular intervals. Chunks stream in via
`ensure_chunks_at`; only `render_background` is called (no entities/mobs — flyover shows
terrain only).

**Gradient dim** (exact, after `render_background` fills the screen):
```rust
for y in 0..H {
    let k = 128 - ((y - 40).clamp(0, 100) * 72) / 100;   // 128 (~50%) at top -> 56 (~22%) at bottom
    // per pixel in this row: multiply each channel by k, then >> 8
}
```
A straight per-channel multiply-and-shift directly on the packed pixel buffer, row by row —
darkest near the bottom (menu text area) for contrast, brightest near the top (showcase the
generated world).

### 9.4 `render_level` (`renderer.rs:139-194`)

Bails if the current level or player doesn't exist. Camera-follow math:
```rust
let mut x_scroll = player_x - W / 2;
let mut y_scroll = player_y - (H - 8) / 2;
```
**Finite vs. infinite clamp** (the key distinction):
```rust
if !g.level(lvl).is_infinite() {          // finite levels only
    x_scroll = x_scroll.clamp(0, lw*16 - W);
    y_scroll = y_scroll.clamp(0, lh*16 - H);
}
// infinite (chunked) levels: no clamp at all — camera follows the player unbounded
```
`Level::is_infinite()` is simply `self.chunks.is_some()` (see TERRAIN.md §1-2). Finite
levels stop scrolling at their borders; infinite layers never do, since there is no edge —
`ensure_chunks`/streaming (driven from `Game::tick`, see CORE_AND_SAVES.md) keeps terrain
generated around wherever the camera lands.

**Sky/dungeon parallax backdrop** (only for level index `> 3`, i.e. non-surface layers):
a 48x28 grid of one repeated 8px sprite tile, scrolled at 1/4 the camera's rate with an 8px
wraparound (`& 7`) — gives dungeon/underground levels a tiled sky-or-void backdrop behind
the tiles.

**Tile drawing**: `level::render_background(g, screen, lvl, x_scroll, y_scroll)` — iterates
the visible tile window (`W>>4 = 18` wide, `H>>4 = 12` tall, plus the scrolled tile origin),
`tile::dispatch::render` per tile, wrapping `screen.set_offset`/reset around the loop.

**Entity drawing — confirmed y-sort** (painter's algorithm): `level::render_sprites` collects
every entity in the visible tile window, **sorts by `e.c.y` ascending** ("Java spriteSorter"),
then renders each via `g.with_entity(eid, entity_render)` (the take-out pattern). Entities not
on this level or `removed` are pruned from the level rather than drawn.

**Lighting/atmosphere post-pass**: `render_level` ends with
`gfx::lighting::render_pass(&mut screen, &mut light_screen, g, lvl, x_scroll, y_scroll)` —
biome ground tint, time-of-day grading, emitter radiance, and event skies, applied before
the HUD so UI text stays crisp (the module's doc comment is the full reference). Emitter
stamping is **occlusion-aware**: tiles with `TileDef.blocks_light` (walls, rock, hard rock;
closed doors via `dispatch::blocks_light`) cast per-emitter tile-grid shadows, so torchlight
fills a room and beams through doorways/Windows instead of glowing through walls. Emitters
with no blocker in reach skip the mask — open terrain stamps at the pre-occlusion cost.

### 9.5 `render_gui` — full HUD anatomy (`renderer.rs:197-393`)

Draw order:

1. **Frame box** for the arrow/durability readout — `font::render_frame(screen, "", 26, 0, 35, 2)`.
2. **Debug overlay** (`render_debug_info`, see below) — drawn early so later HUD elements
   sit on top of it where they overlap.
3. **Arrow counter** — `∞` glyph if creative mode or count ≥ 10000, else the numeric count,
   at `(W-70, 8)`; arrow icon sprite at `(W-72, 7)`.
4. **Permanent status lines** (saving / sleeping / "N player(s) still awake") — only block
   shown, replaces notifications entirely while active; centered via `FontStyle`.
5. **Notifications** — only if no permanent status is showing:
   - capped to the **3 most recent** (older ones silently dropped when a 4th arrives),
   - each shown for **120 ticks** (`g.note_tick`), popped from the front (hard cut, no
     fade) once the timer expires,
   - drawn centered-ish via `FontStyle` at `y = H*2/5`.
6. **Tool durability %** — if the active item is a `Tool`, `dura = dur*100/(ttype.durability()*(level+1))` at `(W-38, 8)`.
7. **Potion effects overlay** — only if `showpotioneffects` and the effect map is non-empty;
   one line per active effect (`"{ptype} ({min}:{sec})"`) plus a hint line to hide it.
8. **Status icons — hearts/stamina/hunger/armor** — only drawn outside creative mode.
   Two frame boxes (`render_frame(screen,"",0,0,10,4)` and `(11,0,25,2)`), then a loop
   `for i in 0..MAX_STAT` drawing 4 stacked rows:
   | Row | y | Sprite tile | Filled condition |
   |---|---|---|---|
   | Armor | `H-24` | `3+12*32` | `i <= armor*MAX_STAT/MAX_ARMOR`, colored by `cur_armor.sprite.color` |
   | Hearts | `4` | `12*32` | `i < health`, red vs. dark-empty |
   | Stamina | `13` | `1+12*32` | blinks white/gray while `stamina_recharge_delay > 0` (every 4 ticks), else `i < stamina` |
   | Hunger | `21` | `2+12*32` | `i < hunger` |
9. **Active/current item** — bottom-toolbar icon+name via `Item::render_inventory(&mut screen, g, 94, 8, false)`.

**Debug overlay** (`render_debug_info`, renderer.rs:396-458) — gated on `g.show_info`
(toggled by the `INFO` input action, see CORE_AND_SAVES.md's input handler section). Lines,
in order: version string; fps; `"day tiks {tick_count} ({time})"`; ticks/sec
(`NORM_SPEED * gamespeed`); walk speed; tile X/Y (tile coord + subtile remainder); current
tile name; score (if score mode); mob count/cap; dungeon chest count (level 5 only);
hunger-stamina debug string; armor + damage-buffer (if any armor equipped).

**Focus nagger** (`render_focus_nagger`, renderer.rs:461-494) — "Come Back!" box, sets
`g.paused = true` while shown; text flashes dim/bright every 20 ticks. Drawn only when the
window is unfocused, not online, and `!g.continous`.

## 10. The display/menu system

### 10.1 `Display` trait + `DisplayManager` (`src/screen/display.rs`)

```rust
pub struct DisplayBase {
    pub menus: Vec<Menu>,
    pub selection: i32,
    pub can_exit: bool,
    pub clear_screen: bool,
}

pub trait Display {
    fn base(&self) -> &DisplayBase;
    fn base_mut(&mut self) -> &mut DisplayBase;
    fn init(&mut self, g: &mut Game) { }
    fn on_exit(&mut self, g: &mut Game) { }
    fn tick(&mut self, g: &mut Game) { display_tick_default(self.base_mut(), g); }
    fn render(&mut self, screen: &mut Screen, g: &mut Game) { display_render_default(self.base_mut(), screen, g); }
}
```
Every method has a working default; the ~25 screens override only what differs.
`display_tick_default`: if `can_exit` and the `"exit"` action was clicked, exits the menu
immediately; otherwise, with more than one sub-menu and the current one selectable, handles
left/right (or shift-left/shift-right if the focused entry `is_array_entry()`) to switch
between the display's own sub-menus, wrapping and skipping non-selectable ones; otherwise
ticks the currently-selected `Menu`. `display_render_default`: clears the screen if
`clear_screen`, then renders menus starting *after* the selected index and wrapping around,
so the selected menu is drawn **last** (on top).

```rust
pub enum PendingMenu { NoChange, Set(Box<dyn Display>), Clear, Exit }

pub struct DisplayManager {
    pub stack: Vec<Box<dyn Display>>,
    pub pending: PendingMenu,
    pub taken_out: bool,
}
```
`menu_active()` = `!stack.is_empty() || taken_out` ("is a menu open right now"). `menu_open()`
mirrors Java's `getMenu() != null` semantics against the *pending* state (`live = stack.len()
+ taken_out as usize`; `NoChange => live>0`, `Set(_) => true`, `Clear => false`, `Exit =>
live>1`).

`taken_out` exists for exactly the reason PORTING.md's entity take-out pattern does: a
`Display::tick`/`render` needs `&mut Game`, but `Game` owns `display.stack` which holds the
`Box<dyn Display>` being ticked — so the top display is physically popped out of the `Vec`,
ticked/rendered with a free `&mut Game`, then reinserted. Because a display's own body may
call `g.exit_menu()`/`g.menu_open()` *while it is taken out*, `menu_active()`/`menu_open()`
must count the taken-out display as still "there," or a display checking "is a menu still
open" mid-tick would incorrectly see `false`.

`g.set_menu(display)` / `g.clear_menu()` / `g.exit_menu()` (`src/core/game.rs`) only ever
stage `self.display.pending` — nothing structural happens until `apply_menu_transition()`
runs at the top of the *next* `Game::tick()` (see CORE_AND_SAVES.md for the exact tick
order and the level-transition interaction). `exit_menu` additionally no-ops if nothing is
active and plays `Sound::Back`.

### 10.2 `Menu` / `MenuBuilder` (`src/screen/menu.rs`)

```rust
pub struct Menu {
    entries: Vec<EntryHandle>,           // EntryHandle = Rc<RefCell<dyn ListEntry>>
    spacing: i32, bounds: Rectangle, entry_bounds: Rectangle, entry_pos: RelPos,
    title: String, title_color: i32, title_loc: Point, draw_vertically: bool,
    has_frame: bool, frame_fill_color: i32, frame_edge_color: i32,
    selectable: bool, pub should_render: bool,
    display_length: i32, padding: i32, wrap: bool,
    selection: i32, disp_selection: i32, offset: i32,
}
```

**Selection/navigation** (`Menu::tick`): no-ops if unselectable or empty. If the focused
entry `captures_typing()`, UP/DOWN are read via the **physical** key (`get_physical_key`)
rather than the remappable action, so a focused text field's letter keys type instead of
navigating (only the literal arrow keys still move the cursor). If the cursor didn't move
this frame, the selected entry is ticked instead (`entry.tick(g)`) — entries only tick on
frames where selection is stable. Otherwise, selection moves by `delta` mod `len`
(wrapping), skipping non-selectable entries, then `do_scroll()` keeps the display window
within `padding` of the selection.

**The "smoked-glass panel"** (`render_frame`, menu.rs:386-431, verbatim comment: *"smoked-
glass panel: darken what's behind instead of a flat opaque fill"*):
```rust
screen.darken_rect_screen(bounds.left(), bounds.top(), bounds.width(), bounds.height(), 185);
```
This is the exact same `darken_rect_screen` primitive documented in §2 — a per-pixel
multiplicative darken of whatever was already rendered underneath (`amount=185` ⇒
`keep=70/255`, ~27% brightness retained), **not** a flat-color alpha blend. After darkening,
a 9-slice sprite border is tiled around the rect using `frame_edge_color`.

`MenuBuilder` — fluent builder (`set_entries`, `set_positioning(anchor, menu_pos)`,
`set_size`/`set_menu_size`, `set_bounds`, `set_display_length`, `set_title[_pos][_color]`,
`set_frame[_colors]`, `set_scroll_policies(padding, wrap)`, `set_should_render`,
`set_selectable`, `set_selection[_disp]`). `create_menu(self, g)` consumes the builder,
computing borders/insets, measuring entry sizes, positioning via `RelPos::position_rect`,
and calling `Menu::init()` (clamps selection into a selectable entry, scrolls into view).

### 10.3 `ListEntry` hierarchy (`src/screen/entry/*.rs`)

```rust
pub trait ListEntry {
    fn flags(&self) -> EntryFlags;
    fn flags_mut(&mut self) -> &mut EntryFlags;
    fn tick(&mut self, g: &mut Game);
    fn to_display_string(&self, g: &Game) -> String;
    fn render(&mut self, screen: &mut Screen, g: &mut Game, x: i32, y: i32, is_selected: bool) { .. }
    fn get_color(&self, is_selected: bool) -> i32 { .. }
    fn get_width(&self, g: &Game) -> i32 { .. }
    fn is_selectable(&self) -> bool { flags.selectable && flags.visible }
    fn is_array_entry(&self) -> bool { false }
    fn is_blank_entry(&self) -> bool { false }
    fn captures_typing(&self) -> bool { false }
    fn set_selectable(&mut self, selectable: bool) { .. }
    fn set_visible(&mut self, visible: bool) { .. }
}
```
`EntryHandle = Rc<RefCell<dyn ListEntry>>` — entries are shared/mutable trait objects
(mirrors Java menus and `Settings` pointing at the *same* mutable entry objects).
`entry_height() = font::text_height()` (uniform row height).

| Type (file) | Role | Notes |
|---|---|---|
| `BlankEntry` | spacer row | unselectable, `is_blank_entry() -> true` |
| `ArrayEntry` | left/right cycling value (`Value::Str/Int/Bool`) | `boolean(...)` = On/Off; `range(...)` = non-wrapping int range; `is_array_entry() -> true` (see below) |
| `InputEntry` | free text input (world name/seed) | **only override of `captures_typing() -> true`**; `Validation::{Pattern, Always, UniqueName}` |
| `ItemEntry` | item icon + name | Java quirk preserved: always draws in "selected" color regardless of actual selection |
| `ItemListing` | unselectable "Have:"/"Cost:" info row | caller-supplied text instead of an item name |
| `KeyInputEntry` | one Controls-screen row | parses `"ACTION;mapping"`; `c`/`Enter` rebinds (overwrite), `a` adds a binding |
| `RecipeEntry` | one crafting-list row | shares `Rc<RefCell<Recipe>>` with `CraftingDisplay` so color reflects live "can craft" state |
| `SelectEntry` | button row running a boxed `FnMut(&mut Game)` | `take()`s the closure to call it (avoids re-entrant borrow), puts it back |
| `StringEntry` | static unselectable text line | optional custom color, multi-line batch constructors |

`captures_typing` is the mechanism §10.2 uses to route UP/DOWN through the physical key
while a text field is focused. `is_array_entry()` is what `display_tick_default` (§10.1)
checks to decide whether cross-menu left/right navigation needs the shift-prefixed variant
— so a focused `ArrayEntry`'s own left/right doesn't fight with the display switching
sub-menus.

### 10.4 Settings ↔ menu bridge (`src/screen/settings_widgets.rs`)

```rust
pub type SettingEntry = (String, Rc<RefCell<ArrayEntry>>);

pub fn make_entry(g: &Game, key: &str) -> SettingEntry {
    // Bool-typed options -> ArrayEntry::boolean; else ArrayEntry::with_flags pre-seeded
    // to the current value. "fps" doesn't wrap; "language" doesn't localize its options.
}

pub fn sync(g: &mut Game, entries: &[SettingEntry]) {
    // one-directional widget -> store: reads each entry's current Value, writes via
    // Settings::set (which silently ignores a value not in options_of).
}
```
`OptionsDisplay` is the primary consumer: `sync` runs **every tick** while the screen is
active (right after the default menu tick, so an edit is picked up the same frame) **and
again on `on_exit`** before persisting prefs / applying a language change / applying the new
fps cap. `world_gen_display.rs` uses the same pair for its own settings subset (worldtype/
size/theme/type). `Settings` itself (`src/core/io/settings.rs`) is a plain
`HashMap<String, Value>` with a `KEYS` schema table and `options_of`/`label_of`/`default_of`
— it knows nothing about widgets; see CORE_AND_SAVES.md for the full settings reference.

### 10.5 Other `Display` implementers (map of the whole stack)

| Display | Role | Typically pushed from |
|---|---|---|
| `SplashMenu` | intro logo splash | **true root** — `src/lib.rs` sets it once at startup |
| `TitleDisplay` | main title menu | pushed back to from Pause/Death/WorldSelect/Splash/Multiplayer |
| `PauseDisplay` | in-game pause menu | player behavior on the pause action |
| `OptionsDisplay` | settings screen | Title, Pause, and from Controls' "back" |
| `PlayerInvDisplay` | player inventory | player behavior on the inventory action |
| `ContainerDisplay` | chest + player inventory | chest interact |
| `CraftingDisplay` | recipe list (Have:/Cost:) | crafter furniture interact, personal crafting |
| `WorldGenDisplay` | new-world options (seed/name/diff/size/theme) | Title's "Play"/"New World" |
| `WorldSelectDisplay` / `WorldEditDisplay` | saved-world list / rename-copy-delete | Title; edit pushed from select |
| `KeyInputDisplay` | Controls key-rebinding screen | Options |
| `LevelTransitionDisplay` | sweeping-squares stair transition | opened by `Game::tick` on a pending level change — see CORE_AND_SAVES.md |
| `LoadingDisplay` | world load/generate progress | WorldGenDisplay, WorldSelectDisplay |
| `InfoDisplay` | player stats panel | player behavior |
| `BookDisplay` | paged text reader | WorldGenDisplay help, Title help, item interactions |
| `MapMenu` | world map (M key) | player behavior |
| `MultiplayerDisplay` | "not available" stub notice | **no call sites found** — dormant, network layer is stubbed (PORTING.md) |
| `PlayerDeathDisplay` | death screen | opened on player death — see CORE_AND_SAVES.md |
| `TempDisplay` | generic auto-exit-after-delay wrapper | **no call sites found** — currently unused scaffolding |
| `item_list_menu.rs`, `recipe_menu.rs`, `inventory_menu.rs` | not `Display`s themselves — shared `Menu`-building helpers | consumed by the displays above |

## 11. HOW TO EXTEND

### 11.1 Add a screen (`Display`)

1. Create `src/screen/your_display.rs`, define a struct holding a `DisplayBase` plus
   whatever state you need, `impl Display for YourDisplay` (override `init`/`tick`/`render`/
   `on_exit` as needed — all have working defaults, so a pure-menu screen may only need
   `base`/`base_mut`).
2. Build its `Menu`(s) via `MenuBuilder` (§10.2) in `init` or a constructor.
3. Push it with `g.set_menu(YourDisplay::new(...))` from whatever triggers it (an input
   action check, an item interact, another display's `SelectEntry` closure).
4. If it needs a "go back" row, either rely on `DisplayBase.can_exit` + the `"exit"` action
   (default handling) or add a `SelectEntry` that calls `g.exit_menu()`.
5. Register the module in `src/screen/mod.rs`.

### 11.2 Add a HUD element

1. Add your draw call inside `Renderer::render_gui` (`src/core/renderer.rs`) at the point in
   the draw order that matches its priority (earlier = drawn under later elements).
2. Reuse `font::render_frame` for a bordered box, `FontStyle` for text positioning, or
   `screen.render(x, y, tile, colors, 0)` directly for a raw sprite.
3. If it's conditional (creative mode, a specific level, a debug flag), gate it the same way
   neighboring blocks do (`if !g.is_mode("creative")`, `if g.show_info`, etc.) rather than
   inventing a new flag if an existing one fits.
4. If it needs new sprite cells, see §11.3 first.

### 11.3 Add a sprite + regenerate the sheet

1. Pick free sheet cells (grep `INVENTORY`/the range tables in `tests/artgen_sheet.rs` and
   the doc-comment cell inventory in `artgen.rs` to avoid colliding with an existing range).
2. Add a recipe function in `src/bin/artgen.rs` following the nearest existing pattern
   (`pat(...)` ASCII-art painting is the default idiom; reuse a generic helper like `icon8`/
   `spr16`/`blob24` if your sprite fits one of those shapes). Decide palette-mode vs.
   true-color (§7) — remember the `r==g==b` trap.
3. Call your function from `fn main()` in the same "terrain → items/UI → furniture → mobs →
   text" order as neighboring calls (order doesn't affect output, but keep it legible).
4. Run `cargo run --bin artgen` — this **overwrites `assets/sprites.png`**; check the diff
   and commit it (binary asset, not auto-generated at build time).
5. Wire the new cells into a `Sprite`/`ConnectorSprite`/`MobSprite` constructor at the game
   code call site (`src/level/tile/*.rs`, `src/entity/mob/*.rs`, `src/item/registry.rs`,
   etc.), using `color::get4`/`get` for the palette word if palette-mode.
6. Add your cell range to `tests/artgen_sheet.rs`'s `INVENTORY` (non-emptiness) and, if
   applicable, `palette_cells_stay_grayscale`/`scenery_cells_are_true_color` — otherwise a
   blank or wrongly-classified cell won't be caught.
7. Run `cargo test --test artgen_sheet` (and `cargo test` broadly, since headless render
   tests will show a missing-texture magenta checkerboard if a code path references a cell
   your art doesn't cover).

### 11.4 Add a menu entry type

1. Create `src/screen/entry/your_entry.rs`, define your struct with an `EntryFlags`, `impl
   ListEntry` (only `tick`/`to_display_string` are mandatory; override `render`/`get_color`/
   `get_width`/`is_selectable`/`captures_typing` as needed — see §10.3 for what each
   existing type overrides and why).
2. If it needs exclusive keyboard focus (a new kind of text/data entry), override
   `captures_typing() -> true` (currently only `InputEntry` does this) so `Menu::tick`
   routes UP/DOWN through the physical key while it's focused.
3. If it's a left/right cycling value competing with cross-menu navigation, override
   `is_array_entry() -> true` (only `ArrayEntry` does this today) so `display_tick_default`
   uses the shift-prefixed keys for switching sub-menus while your entry is focused.
4. Add a batch constructor (`use_*`) if callers will typically build many at once (see
   `ItemEntry::use_items`, `RecipeEntry::use_recipes`, `StringEntry::use_lines`) returning
   `Vec<EntryHandle>` for `MenuBuilder::set_entries`.
5. Register the module in `src/screen/entry/mod.rs`.

## 12. Invariants & gotchas

- **The light-screen day/night overlay is fully implemented but never called.** Don't
  "restore" `Screen::overlay` without checking with the maintainers first — it's a
  deliberately preserved Java-fork quirk (PORTING.md), not dead code by accident.
- **`darken_rect_screen` is multiplicative, `color::tint_color` is additive.** They are not
  interchangeable — UI panels (smoked-glass) use the former; the (unreachable) day/night
  overlay uses the latter. Pick the one that matches your effect's intent.
- **Palette bytes of `255` mean transparent-for-this-shade**, both in the packed
  `rgb4Sprite` word (`get_byte(-1) == 255`) and in `Screen::render`'s `if col < 255` guard —
  don't reuse `255` as a legitimate encoded color.
- **`r == g == b` in artgen-authored true-color art silently becomes a palette cell**, not
  an error at runtime (only `artgen.rs`'s own `rgb()` helper catches it, at generation time,
  not at load time). A true-color sprite that starts flickering between two random-looking
  colors after a recolor is a symptom of this — check the source art wasn't accidentally
  gray.
- **Lowercase font glyphs are unreachable.** `Font::draw` uppercases before lookup and
  `artgen.rs`'s `glyph()` has no lowercase branch — don't spend time trying to render
  lowercase text without first deciding whether to add real lowercase glyphs.
- **`MobSprite`'s left-facing/second-frame art is derived, not drawn.** Editing a mob's walk
  cycle only requires touching the right-facing recipe in `artgen.rs`; the mirror handles
  the rest. If a mob's left-right silhouette isn't actually symmetric under a flip (rare,
  but possible for asymmetric gear), that mob needs its own non-mirrored recipe instead of
  `compile_mob_sprite_animations`.
- **`Menu`/`Display` mutations via `set_menu`/`clear_menu`/`exit_menu` are never immediate**
  — they stage `PendingMenu` and apply on the *next* `Game::tick`'s
  `apply_menu_transition()`. Code that pushes a display and then expects to immediately
  query it as the new top-of-stack in the same tick will not see it yet.
- **`artgen_sheet.rs` is a structural contract test, not a snapshot test.** It will not catch
  a subtly-wrong color within a cell, only a wrong pixel *mode* or an empty cell that should
  have art. Don't rely on it to catch "the sprite looks different than before" — that needs
  a visual check (`tests/biome_frames.rs`-style PNG dump, or `cargo run`).
- **The flyover always uses level slot 3 and reuses whatever's there.** If you ever change
  which slot index is "the surface," update `LVL` in `render_flyover` too, or the title
  screen will start flying over the wrong level (or silently tear itself down if slot 3
  isn't infinite).

## 13. Test coverage map

| Test | Locks in |
|---|---|
| `src/gfx/color.rs` inline `#[cfg(test)]` | `get4`/`get_byte`/`upgrade`/etc. numeric values, captured from the real Java `Color` class on a JVM. |
| `src/gfx/font.rs` inline tests | `text_width`, `get_lines` word-wrap output for known inputs. |
| `tests/artgen_sheet.rs` | Sheet dimensions, every referenced cell has art, font glyphs are palette-grayscale, palette-mode cells never leak true-color, spot-checked scenery cells are true-color. See §8 for exactly what this does/doesn't catch. |
| `tests/headless_render.rs` | Renders frames to PNG with no window — the general smoke test that the render pipeline doesn't panic and produces plausible output; also where a missing sprite would show up as a magenta `missing_texture` checkerboard. |
| `tests/display_flow.rs` | Menu/display stack transitions (push/pop/exit sequencing) tick correctly end-to-end. |
| `tests/keymap_check.rs` | Key-binding related coverage — see CORE_AND_SAVES.md for the input-handler test details. |

Run `cargo test` for everything; `cargo run --bin artgen` to regenerate `assets/sprites.png`
after any `artgen.rs` change (then re-run `cargo test --test artgen_sheet` and
`cargo test --test headless_render` to confirm nothing regressed visually/structurally).
