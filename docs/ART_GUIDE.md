# Art Guide — the split sprite tree

The game's art is plain PNG files under `assets/sprites/**`, one file per sprite
(or per frame strip / connector set). Edit them with any pixel editor or the built-in
studio (`just studio`, see DEV_GUIDE.md). There is no generator: **the PNGs are the
source of truth.** The old `artgen` program and its monolithic `assets/sprites.png`
are gone; `assets/golden_atlas.png` remains only as a test fixture (see
[Golden atlas](#the-golden-atlas) below).

## Why an atlas at all (stitching rationale)

The renderer samples sprites from **one flat pixel array** — a single
`Vec<SheetPixel>` addressed as `pos = cx + cy * 32` cells of 8x8 px. That layout is
the hot loop's whole performance story: no per-sprite indirection, no texture
switches, one cache-friendly ~0.5 MB buffer for the entire game's art. So the split
files are *stitched back into that atlas at load time* (`SpriteSheet::from_parts`,
`src/gfx/sprite_sheet.rs`): you get per-file editing and reviewable diffs, the
renderer keeps its flat array.

Loading order (`assets::sprite_sheet()` in `src/assets.rs`):

1. **Dev / in-repo:** if `assets/sprites/manifest.txt` is found (via the crate path
   or the cwd), the folder is read at runtime — edit a PNG, relaunch, see it.
   No rebuild.
2. **Everywhere else** (release builds, installed binaries): a build-time embedded
   copy of the whole tree (`build.rs` -> `EMBEDDED_SPRITE_PARTS`) is stitched
   instead. `cargo build` refreshes it automatically when the folder changes.

## Folder layout

```
assets/sprites/
  manifest.txt        # pin table (below)
  tiles/              # terrain, connector sets, flora, furniture, structures
  mobs/<mob>/         # one folder per mob: walk.png / frames.png / poses
  items/              # 8x8 inventory icons
  ui/                 # HUD icons, menu frame, splash cells
  font/               # one 8x8 file per glyph (a.png .. z.png, 0.png, period.png ...)
  logo/               # title lockup strips (doom.png, fossickers.png)
  fx/                 # particles, slashes, smoke, tile-fire overlay
```

A sprite's **name** is its path minus `.png`: `items/berry`, `mobs/player/walk`,
`tiles/grass_texture`. That name is how code finds unpinned sprites at runtime:
`sheet.cell("items/berry")` returns its `CellRect` (and `.pos()` gives the classic
`cx + cy * 32` index render calls take).

## manifest.txt — the pin table

```
<path> <cell_x> <cell_y> <w_cells> <h_cells> <pal|rgb>
```

Every *legacy* sprite is pinned to the cell rectangle it historically occupied on
the 256x256 base atlas, because existing draw calls still address those cells by
number. The last column declares the file's pixel-mode rule (checked by
`tests/sprite_atlas.rs`).

**You never add manifest lines for new art.** Any `*.png` in the tree that is *not*
pinned is auto-allocated onto appended rows (row 32 and below — the atlas grows in
height, never width) and addressed by name lookup instead. The manifest only shrinks
over time, as call sites migrate from hard-coded cells to names.

Multi-cell files and their piece orders:

- **16x16 tiles / 2x2 blocks** (furniture, graves, flora, mud, quicksand...): one
  file, quarters in reading order TL, TR / BL, BR.
- **Mob frame strips** (`mobs/*/walk.png`, `frames.png`: 64x16 px = 8x2 cells): four
  16x16 frames left to right; each frame is a 2x2 cell block. Only right-facing
  frames exist — the renderer mirrors for left.
- **Connector sparse sets** (`*_sparse.png`, 24x24 px = 3x3 cells): the rounded
  "island" blob; the center cell doubles as the full-tile interior. Side sets
  (`*_sides.png`, 2x2 cells): straight-edge pieces.
- **Texture rows** (`tiles/dots.png`, `*_texture.png`: 32x8 px = 4x1 cells): four
  8x8 variant cells left to right, picked pseudo-randomly per tile position.
- **Tree pieces**: `tiles/tree_pieces.png` (2x2: TL outer, TR outer / BL outer,
  canopy fill) + `tiles/tree_fill.png` (1x2: fill-with-bark-knot / BR outer) — an
  L-shape split around the cactus block. Species trees (`tiles/tree_*.png`, 2x3):
  TL, TR / BL, BR standalone quarters, then fill / fill-with-knot.
- **`ui/frame.png`** (3x1): corner, top edge, left edge/fill — mirrored at draw time.
- **`items/crafting_icons.png`** (4x1): reserved fiber / stick / cord / sharp-stone
  icons for the crafting overhaul (not referenced by code yet).
- **Logo strips**: whole words, one file each.
- **Font**: one file per glyph, all 63 renderable chars of `Font::CHARS`
  (uppercase-only). `font/space_*.png` are solid shade-0 backing boxes (some callers
  color shade 0 as a text backing); the sixth space slot holds `tiles/missing.png`.

## Palette vs true-color (the two pixel modes)

The decoder (`src/gfx/sprite_sheet.rs`) classifies every pixel independently:

- **alpha < 128** -> transparent.
- **gray (`r == g == b`)** -> *palette pixel*: quantized `/64` to shade 0..3 and
  recolored at draw time through the call site's packed palette
  (`color::get4(a,b,c,d)`; a byte of `-1` makes that shade transparent).
- **anything else** -> *true color*, drawn literally; the palette is ignored.

Rules, enforced by `tests/sprite_atlas.rs`:

- `pal` files may contain **only the gray ladder `0 / 85 / 170 / 255`** (plus
  transparent). Those four values quantize exactly to shades 0/1/2/3; off-ladder
  grays land on the wrong shade silently. Use palette mode only for art that
  genuinely needs dynamic recoloring: mob level tints, the player shirt, wool
  colors, tool tiers, item icons drawn in list colors, the font.
- `rgb` files are free-color, but **never use `r == g == b` in true-color art** —
  it would silently become a palette pixel and recolor. Nudge one channel by 1
  (e.g. `31,27,24` instead of pure dark gray). All *new* art should be true color;
  pixel_studio warns when a cell mixes grays with saturated colors.

Shade-role conventions for the existing palette art (shade 0 -> 3): items use
0 = background (transparent via `get4(-1,..)`), 1 = outline, 2 = mid, 3 = light;
mobs use 1 = outline, 2 = dynamic mid (shirt/tint), 3 = skin/highlight; the font
draws strokes in shade 3 on a shade-0 backing. When editing a `pal` file, keep the
roles — the palettes live at the call sites and expect them.

## Pixel budget conventions

- The base grid is **8x8 cells**; a world tile is 16x16 (2x2 cells). Keep file
  dimensions multiples of 8 — the stitcher rejects anything else.
- Item icons are 8x8 with a 1px breathing margin, roughly centered in the cell
  (the artgen era auto-centered them; keep new icons visually centered too).
- Mob frames: 16x16 per frame, feet on the bottom row, right-facing.
- The 256px atlas *width* is fixed. Height is unlimited (auto-allocated rows), so
  never crowd art into leftover base-grid cells — just add a new file.

## Adding an item, end to end

1. Draw `assets/sprites/items/moonfruit.png` (8x8) — `just studio`, pick any items/
   file, or create the file with any editor. True color, remember: no pure grays.
2. **No manifest edit.** The stitcher auto-allocates it and
   `sheet.cell("items/moonfruit")` resolves it (`.pos()` for the render call).
3. Register the item in `src/item/registry.rs` (one line, see
   docs/ADDING_CONTENT.md) pointing at that sprite.
4. `cargo test --test sprite_atlas` — integrity checks pick the new file up
   automatically; `just preview` to eyeball the stitched atlas.

## The golden atlas

`assets/golden_atlas.png` is the frozen pre-decomposition sheet (the last
`sprites.png`, with one stray artgen overflow pixel at cell (21,0) scrubbed). It is
**not shipped or loaded by the game** — `tests/sprite_atlas.rs` stitches the tree
and asserts byte-identity against it, proving the decomposition (and any later
refactor of the stitcher) never changed what the renderer sees.

When you *deliberately* change pinned art, the golden test will fail; regenerate the
fixture from your approved tree: run `just preview` and copy the output over the
fixture (`cp target/verify/atlas.png assets/golden_atlas.png` — only valid while all
art is pinned; review the visual diff first).

## Acceptance checklist

Before calling an art change done:

- [ ] `cargo test --test sprite_atlas` green (golden updated deliberately, if pinned
      art changed; new files pass integrity automatically).
- [ ] `pal` files: only 0/85/170/255 grays; `rgb` files: no `r==g==b` pixels.
- [ ] File size is a multiple of 8 in both dimensions; frames/quarters follow the
      piece orders above.
- [ ] `just preview` — the atlas reads coherently (shared outline ink, ~24-color
      true-color palette; crib from neighbors).
- [ ] In-game check: `just run` (folder is loaded live) or the relevant headless
      shot (`just shots`, `just demo-title` for logo/UI changes).
- [ ] New sprite names are wired via `sheet.cell("...")` or a registry entry — no
      new hard-coded cell numbers.
