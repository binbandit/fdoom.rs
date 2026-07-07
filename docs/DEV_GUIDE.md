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
just sheet          # regenerate assets/sprites.png via artgen and open it
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
