# Development Guide

The daily-driver reference. Architecture background is in
[ARCHITECTURE.md](ARCHITECTURE.md); content recipes in
[ADDING_CONTENT.md](ADDING_CONTENT.md).

## Commands

```sh
cargo run                                    # play
cargo run -- --debug                         # play with cheat keys (below)
cargo run -- --savedir /tmp/somewhere        # sandboxed saves (game dir becomes <dir>/fdoom)
cargo test                                   # unit + headless integration tests
cargo clippy --all-targets -- -D warnings    # must stay clean
cargo fmt                                    # default rustfmt style
```

Or via [`just`](https://github.com/casey/just):

```sh
just run / run-debug / test
just check          # fmt --check + clippy -D warnings + test (run before pushing)
just seed 42        # create + enter a fresh world with seed 42 (windowed, throwaway saves)
just worldview seed=123   # world-inspection map window (see below)
just biome-map 42   # headless biome overview PNG for a seed (target/verify/)
just demo-title     # scripted run: screenshot the title screen into target/verify/
just demo-world     # scripted run: generate a world named PIT, screenshot gameplay
just shots          # run all visual test harnesses, upscale everything in target/verify
just soak           # long randomized gameplay soak (release build)
just preview        # stitch assets/sprites/** into the atlas and open the preview
just upscale        # 3x-upscale target/verify PNGs for easier viewing
just clean-saves    # DELETE all saves + preferences (~/fdoom)
```

## Scripted runs: the FDOOM_DEMO driver

`src/platform/demo.rs` runs the *real* windowed game from a script in the `FDOOM_DEMO`
env var — key events + frame dumps, one step at a time. Perfect for "does this actually
render/behave right" verification without clicking through menus by hand.

Syntax: steps separated by `;`, each `cmd` or `cmd:arg`:

| Step | Meaning |
|---|---|
| `wait:N` | do nothing for N ticks (60 ticks ≈ 1 s) |
| `shot:PATH` | dump the framebuffer as PNG to PATH — **the script blocks until that frame has actually rendered**, so no wait-tuning is needed before a shot |
| `key:NAME` | tap a key: press this tick, release next tick (single-char names also feed the text-typing channel) |
| `down:NAME` / `up:NAME` | hold / release a key (for movement) |
| `type:c` | type one character into a text field (world name, seed) |
| `quit` | exit the game |

Key names are Java `KeyEvent`-style, matching what the game binds: `ENTER`, `SPACE`,
`ESCAPE`, `UP`/`DOWN`/`LEFT`/`RIGHT`, `SHIFT`, single letters/digits. Window focus is
forced during demo runs, so they work even while you keep typing elsewhere.

**Use absolute paths for `shot:`** — the PNG is written relative to the game's working
directory otherwise. Parent directories are created automatically.

### Recipes (verified)

Boot to the title screen and screenshot it (splash lasts 200 ticks):

```sh
FDOOM_DEMO="wait:220;shot:$PWD/target/verify/title.png;quit" cargo run
```

Generate a world named PIT and screenshot gameplay (fresh save dir, so "Play" goes
straight to World Gen Options; the name field has focus first; two `DOWN`s reach
"Create World"; the second `shot` waits out world generation):

```sh
rm -rf /tmp/fdoom-demo
FDOOM_DEMO="wait:220;key:ENTER;wait:5;type:P;type:I;type:T;wait:2;key:DOWN;wait:2;key:DOWN;wait:2;key:ENTER;wait:600;shot:$PWD/target/verify/world.png;quit" \
  cargo run -- --savedir /tmp/fdoom-demo
```

Drive the player around (continue the previous script instead of `quit`, or load a
world): hold movement keys with `down:`/`up:`.

```
...;down:D;wait:40;up:D;down:S;wait:40;up:S;shot:$PWD/target/verify/moved.png;quit
```

Note: with an existing save dir, `Play` opens a Load World / New World submenu first —
add one extra `key:ENTER` (Load World is preselected) or `key:DOWN;key:ENTER`.

## World inspection: `worldview`

`src/bin/worldview.rs` is a standalone map window for eyeballing what a seed generates —
biome layout, structure spawn rates, flora distribution — without playing. It calls the
pure generators (`infinite_gen`, `structures_gen`) directly, so the picture is
byte-for-byte what the game would generate.

```sh
just worldview seed=123          # or: cargo run --bin worldview -- 123
cargo run --bin worldview -- 123 --depth -3 --mode tile --zoom 1
```

No seed = a random one. Two render modes, toggled with Tab:

- **BIOME** — `biome_at` region colors, with trails overlaid as their tiles.
- **TILE** — actual `generate_chunk` tiles (structures, flora, ores... as stamped).

Both modes overlay structure markers (origin of each placement) and a legend panel:
orange = ruins, purple = cemetery, cyan = standing stones, yellow = camp, red = village,
white = dungeon gate (depth -3). Unmapped tile ids render loud magenta.

| Key | Action |
|---|---|
| arrows / W-A-D | pan one chunk (64 tiles) |
| `+` / `-` | zoom 1 / 2 / 4 px per tile (default 2) |
| Tab | toggle BIOME / TILE mode |
| `N` | new random seed (prints structure counts to stdout) |
| `S` | screenshot to `target/verify/worldview_<seed>.png` |
| Esc | quit |

Seed, depth, mode, zoom, and center coordinates are shown in the window title and the
legend header. Chunks are generated lazily and cached, so panning is instant.

Headless (CI / agent) hook — render one frame straight to a PNG, no window:

```sh
cargo run --bin worldview -- --dump 123 target/verify/wv_biome.png
cargo run --bin worldview -- --dump 123 target/verify/wv_tile.png --mode tile
cargo run --bin worldview -- --dump 123 out.png --center -2048 -1024   # away from 0,0
```

The dump also prints per-kind structure counts for the rendered rect to stdout.

## Pixel-art studio: `pixel_studio`

`src/bin/pixel_studio.rs` is the game's art tool: a standalone winit/softbuffer
window for making and editing sprite art in place. The PNG files it edits **are the
source of truth** — `assets/sprites/**` is the art (see docs/ART_GUIDE.md), and the
studio writes those files directly. This section is the full manual: modes, every
key, the new-sprite flow, and the edit-to-in-game loop.

```sh
just studio                                  # assets/sprites — the normal way in
just studio target=assets/golden_atlas.png   # inspect a monolithic sheet
cargo run --bin pixel_studio -- assets/sprites --canvas       # start in whole-sheet view
cargo run --bin pixel_studio -- assets/sprites --file items/pan.png
```

### The three views

- **Files** (default): the left pane is a browser over every `*.png` under the
  target folder (`*.bak.png` hidden). 8x8 items and 16x16 tiles are edited whole;
  bigger strips are edited one 8/16px window at a time (arrows and `I`/`K` step,
  Tab toggles 8/16). Press `/` and type to jump to a file by name.
- **Whole sheet** (`W`): every file stitched into one editable canvas laid out
  exactly like the real atlas (manifest pins on the 32x32 base grid, new art on the
  auto-allocated rows below). Paint anywhere — every edit routes to the file that
  owns those pixels, dirty files get a red outline in the left pane, and `S` saves
  **only the dirty files** (each backed up once per session). Eyedrop, copy/paste
  and shape tools work across file boundaries; `G` snaps the window to the file
  under the cursor (odd cell origins included); Shift+arrows wrap-nudges just the
  file under the window. `W` again returns to the file browser.
- **Sheet** (fallback, for `--sheet <png>` targets like the golden atlas): same as
  whole-sheet view but for a monolithic PNG, with a built-in sprite map naming the
  classic regions, and `G` snapping via that map.

The right side is always the editor canvas (zoomable, pixel grid with 8px cell
lines), the palette banks, and the preview strip.

### Every key

| Input | Action |
|---|---|
| left-click / drag | paint with the current color |
| right-click | eyedrop the pixel under the cursor |
| `F` | flood-fill at the cursor |
| `L` | line tool (drag start to end; toggle back to pencil) |
| `R` / Shift+`R` | rectangle / filled rectangle tool |
| `M` | mirror-draw across the window's vertical axis |
| `[` / `]` | shade-shift the hovered pixel (grays walk the 4-shade ladder, colors step ±16/channel) |
| `H` / `V` | flip the window horizontally / vertically |
| `E` (or the T swatch) | eraser — paint transparent |
| Ctrl+`C` / Ctrl+`V` | copy the window / arm paste (click to place, works across files in whole-sheet view) |
| Shift+arrows | wrap-nudge the image (whole-sheet view: just the file under the window) |
| `U` / Ctrl+`Z` | undo (64 levels) |
| `Y` / Ctrl+`Y` / Ctrl+Shift+`Z` | redo |
| arrows | browse files (file view) / move the window by one cell (sheet views) |
| `I` / `K` | step the window vertically inside tall strips |
| Tab | toggle 8/16px window |
| `G` | snap the window to the sprite/file under the sheet-pane cursor |
| `W` | toggle file browser ↔ whole-sheet canvas |
| `N` | create a new sprite (modal: name, size preset, create + open) |
| `/` | find a file by typed name (Up/Down next/prev match, Enter done) |
| wheel / middle-drag | zoom at the cursor / pan |
| `P` / Shift+`P` | cycle the preview palette (player, zombie tiers, tool tiers, terrain tiles) |
| `D` / Shift+`D` | cycle the in-context backdrop (grass / sand / snow / water / night) |
| `A` | animate sibling frames at the game's walk cadence |
| `B` / `O` | capture an onion-skin reference / toggle it |
| `C` | toggle the custom RGB swatch (arrows step channels, Shift = fine) |
| `S` / Ctrl+`S` | save (first save of a session writes `<name>.bak.png` first) |
| `X` | revert from disk |
| Esc | close modal/finder/paste, else quit (asks twice if dirty) |
| `?` | key list overlay |

### The preview strip

Under the palette banks: the window at raw 1x/2x/4x, then **in-game previews** —
the sprite composited over the real terrain textures (sampled from the loaded
sheet and recolored through the exact tile palettes from the game code) at 1x, 2x
and 4x. `D` cycles grass / sand / snow / water / night-graded grass, so
terrain-taste judgments ("calm base, sparse detail") happen here, not after a game
boot. `P` runs palette-mode art through real game palettes the same way. 16px
windows also get a 3x3 tiling preview for judging seamless edges.

### Making a new sprite, end to end

1. `just studio`, press `N`. Type the path-name (folders included, no extension):
   `items/moonfruit`, `tiles/bog_flower`, `mobs/mirelurk/walk`. Up/Down picks a
   size preset (item 8x8, tile 16x16, mob walk strip 64x16, texture row 32x8...),
   Shift+arrows dials a custom size in 8px steps. Enter creates the transparent
   PNG and opens it.
2. Draw it. True color for new art (never `r == g == b` — that becomes a palette
   pixel); crib outline ink and palette from neighboring sprites (`W` shows
   everything at once). Watch the warning slot: the studio flags pal/rgb mode
   violations per file.
3. `S` to save. Check the in-game preview slots (`D` for the right biome).
4. **No manifest edit** — unpinned files auto-allocate; code finds the sprite by
   name (`sheet.cell("items/moonfruit")`, or a registry entry — see
   docs/ADDING_CONTENT.md for items).
5. Add the file to `UNPINNED_RGB` (or `UNPINNED_PAL`) in `tests/sprite_atlas.rs` —
   the studio's create message reminds you which.
6. `just run` — dev builds stitch `assets/sprites/` fresh at every boot, so the
   art is simply there. `cargo test --test sprite_atlas` for the integrity checks.

### Your art workflow (the edit loop)

Dev builds read the sprite folder live at boot (`assets::sprite_sheet()`, which
prefers a checkout's `assets/sprites/` over the embedded copy and panics loudly if
the folder is broken — your edit can never be silently ignored). So the loop is:

1. keep the studio open, edit, `S`;
2. relaunch the game (`just run`, or `just demo-title` / `just shots` for headless
   screenshots) — no rebuild step, the PNGs are read as-is;
3. for release builds only, `cargo build` re-embeds the tree automatically.

There is no in-game reload key (yet): a relaunch is the refresh.

### Palette rules recap

Every opaque **gray** pixel (`r == g == b`) is a *palette pixel*: the renderer
quantizes `gray / 64` to shade 0-3 and recolors it through the draw call's packed
palette. The SHADES bank is exactly the four legal grays **0 / 85 / 170 / 255** —
off-ladder grays land on the wrong shade silently, so never invent others. Any
saturated color draws literally; alpha < 128 is transparent. Palette mode is only
for art that recolors at draw time (mob tints, player shirt, tool tiers, font);
all new art should be true color. The studio warns per file (against the manifest
mode) or per 8x8 cell (mixed grays + colors — almost always a mistake).

### Headless hooks (CI / scripts)

Same backup+save path as the interactive editor, no window:

```sh
cargo run --bin pixel_studio -- <png> --set 3 5 FF00AA --set 10 5 t  # image coords
cargo run --bin pixel_studio -- assets/sprites --file tiles/grass.png --set 0 0 336699
cargo run --bin pixel_studio -- assets/sprites --canvas --set 120 208 336699  # canvas coords
cargo run --bin pixel_studio -- assets/sprites --new items/moonfruit 8x8
cargo run --bin pixel_studio -- --sheet assets/golden_atlas.png --snap 16 11  # report G-snap
cargo run --bin pixel_studio -- assets/sprites --shot out.png [--backdrop 2] [--pal 1] [--demo-new]
```

`--set X Y COLOR` takes `RRGGBB`, `RRGGBBAA`, or `t`; `--blit SX SY W H DX DY`
copies rects; `--nudge DX DY` wrap-shifts (single files only). With `--canvas`,
coordinates are stitched-canvas coordinates and only the touched files are
rewritten. `tests/pixel_studio.rs` round-trips all of these through the game's own
sheet loader.

### Troubleshooting

- **"My edit isn't in the game."** You're running a release/installed binary (uses
  the embedded copy — rebuild), or you edited a `.bak.png`, or the game predates
  the save (relaunch). Dev builds cannot silently miss the folder: they panic if
  `assets/sprites/manifest.txt` is unreadable.
- **"My gray pixel changed color in game."** It was a palette pixel — use a
  true color with one channel nudged (`31,27,24`, not `28,28,28`), or press `P` in
  the studio to see what the palette does to it.
- **"Sprite looks right in the editor, wrong scale in game."** File dimensions
  must be multiples of 8, and multi-cell pieces follow the orders in
  docs/ART_GUIDE.md (frames left-to-right, quarters TL/TR/BL/BR).
- **"The golden test failed."** You changed *pinned* art. Deliberate? Regenerate
  the fixture (ART_GUIDE, "The golden atlas"). Accidental? `X` revert, or restore
  from the `.bak.png` the studio wrote next to the file.
- **"Canvas save says NOTHING TO SAVE."** No dirty files — paints on gap cells
  (checkerboard between placements) belong to no file and are dropped on save;
  the status bar warns when that happens.

## Headless testing

The game core never touches the platform layer, so tests can build a `Game`, generate a
world, and tick it — no window, no audio. **Start from `fdoom::testutil`** — it owns
the boot boilerplate every test used to copy-paste:

```rust
use fdoom::testutil::TestWorld;

let mut tw = TestWorld::infinite().seed(42).build(); // world made, first tick done
// .creative() / .debug() / .name("mytest") as needed

tw.place("tall grass", 1, 0);          // stage a tile next to the player
assert!(tw.hit(1, 0, 1));              // bare-handed attack path
tw.interact_with("Crude Axe", 1, 0);   // tool-interact path (stamina/durability)
tw.give("Wood", 10);                   // registry items into the inventory
tw.press("E");                         // tap a key like the platform layer would
tw.goto_biome(Biome::Marsh);           // teleport + stream chunks
tw.screenshot("mything.png");          // headless frame -> target/verify/
assert!(tw.display.menu_active());     // TestWorld derefs to Game for everything else
```

More harness pieces (see `src/testutil.rs` for the full API):

- `TestWorld::infinite().build().g` moves the plain `Game` out when a test wants to
  drive it directly.
- `bare_game("name")` — a `Game` with the player but **no world**: registry/recipe
  checks and save/load tests that fabricate their own levels.
- `tick_recover()` — tick + close menus + respawn: for soak loops that must keep
  the level ticking through deaths and transitions.
- `find_biome`, `find_recipe`, `renderer`, `save_png`, `verify_path` — the shared
  free helpers.

Templates: `tests/keymap_check.rs` (smallest), `tests/flora_gen.rs` (tile staging +
drops), `tests/gameplay_soak.rs` (long loops), `tests/lighting.rs` (custom rendering),
`tests/save_load_roundtrip.rs` / `tests/level_gen_determinism.rs` (save format and
world-gen regression patterns).

Details the harness already handles, for when you go under the hood:

- **Tick once after `init_world` before touching the player** (the builder does).
  Freshly spawned entities sit in the level's `entities_to_add` queue until the first
  `tick_level` drains them into the arena; before that, `g.entities.take(g.player_id)`
  returns `None` (though `g.player()` / `g.try_player()` do look through the queues).
- **Pin the clock before jumping it.** New worlds spawn at a seed-random time of day;
  the builder resets to morning-0 so `set_time`/`change_time_of_day` jumps never read
  as a midnight wrap to the event scheduler.
- `has_gui = false` gives you a silent `SoundPlayer` and skips focus handling
  (`TestWorld::render` flips it back on so frames draw); `debug = true` enables the
  debug-gated key bindings in `InputHandler`.

## Debug cheat keys

All of these need `cargo run -- --debug`. First, the three debug-gated *bindings*
(rebindable, defined in `init_key_map` with the `=debug` marker in
`src/core/io/input_handler.rs`):

| Key | Effect |
|---|---|
| `N` | skip to night |
| `SHIFT-S` or `SHIFT-1` | switch to survival mode |
| `SHIFT-C` or `SHIFT-2` | switch to creative mode (fills creative inventory) |

Then the hardcoded cheats in the debug block of `Game::tick`
(`src/core/game.rs`) and `player_behavior.rs`:

| Key | Effect |
|---|---|
| `1` / `2` / `3` / `4` | time of day: morning / day / evening / night |
| `SHIFT-0` / `SHIFT-=` / `SHIFT--` | game speed: reset / faster / slower |
| `SHIFT-G` | give one of every item |
| `CTRL-H` / `CTRL-B` | take 1 damage / lose 1 hunger |
| `0` / `=` / `-` | move speed: reset / +1 / -1 |
| `SHIFT-U` / `SHIFT-D` | place stairs up / down under the player |
| `SHIFT-R` | regenerate the world in place |
| `SHIFT-P` | clear all potion effects |
| `CTRL-P` | print all players + coordinates to stdout |

`F3` (debug info overlay: FPS, position, time, mob count) works **without** `--debug`.

## Dev console (`--debug` only)

`src/screen/dev_console.rs` — an info overlay plus a command line, both gated behind
`--debug` (wired in the debug block of `Game::tick`):

| Key | Effect |
|---|---|
| `F4` | toggle the info overlay: FPS, world seed, day + clock + time of day, level name/depth, biome (surface), player tile coords, tile name + data under the player |
| `/` | open the command line (a display on the menu stack, so game input is swallowed while typing; `ENTER` runs, `ESC` cancels) |

Commands (each confirms — or complains — via the notification system):

| Command | Effect |
|---|---|
| `give <item> [n]` | add `n` (default 1) of a registry item to the inventory; names are case-insensitive and may contain spaces: `give crude axe 2` |
| `tp <x> <y>` | teleport to tile coordinates on the current level |
| `time <morning\|noon\|dusk\|night>` | set the time of day (`day`/`evening` accepted as aliases) |
| `heal` | full health, hunger, and stamina |

The parser is a plain `&mut Game` function (`dev_console::run_command`), tested
headlessly in `tests/dev_console.rs`. Scripted-run example:
`...;key:F4;key:SLASH;type:h;type:e;type:a;type:l;key:ENTER;...`

## Save locations

From `src/core/file_handler.rs`:

| OS | Game dir |
|---|---|
| macOS | `~/fdoom` |
| Windows | `%APPDATA%\fdoom` |
| Linux | `~/.fdoom` |

Inside: `Preferences.miniplussave` (options + key bindings), `Unlocks.miniplussave`,
and `saves/<worldname>/` per world (`Game`, `Level0..5`, `Level0data..`, `Player`,
`Inventory`, `Entities`, all `.miniplussave`). `--savedir DIR` relocates the base dir
(the game dir becomes `DIR/fdoom`). An old `~/.fdoom` on mac/windows is migrated
automatically at startup.

## Troubleshooting

**Old keybindings after an update (E doesn't open inventory, etc.).**
`Preferences.miniplussave` stores the *full* keymap and overrides the new defaults from
`init_key_map` at startup. Fix: rebind in Options → Change Key Bindings, or delete the
preferences file (`rm ~/fdoom/Preferences.miniplussave` on macOS — this also resets
options), or nuke everything with `just clean-saves`.

**`TILES.GET: invalid tile requested: FARM` during world generation.** Expected — a
preserved Java quirk (`src/level/level_gen.rs` asks for "farm"; the tile is named
"Farmland", and the lookup intentionally falls back). Harmless.

**`Dropping DeviceSink, audio playing through this sink will stop...` on exit.** Rodio
being chatty at shutdown. Harmless.

**No audio device (CI, ssh).** `SoundPlayer` logs one line and runs silent; the game is
unaffected.

**The build fails on `winit`/`softbuffer` only.** Those live exclusively in
`src/platform/`; `cargo test` exercises the whole game core headlessly, so platform
breakage never blocks logic work.
