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
just demo-title     # scripted run: screenshot the title screen into target/verify/
just demo-world     # scripted run: generate a world named PIT, screenshot gameplay
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

## Headless testing

The game core never touches the platform layer, so tests can build a `Game`, generate a
world, and tick it — no window, no audio. Templates:

- `tests/keymap_check.rs` — smallest full-game test: build, init world, inject keys.
- `tests/display_flow.rs` — display-stack behavior (open/close menus).
- `tests/headless_render.rs` — render into a `Screen` and dump PNGs to
  `target/test-frames/`.
- `tests/save_load_roundtrip.rs`, `tests/level_gen_determinism.rs` — save format and
  world-gen regression patterns.

The boilerplate:

```rust
let tmp = std::env::temp_dir().join("fdoom_test_mything");
let _ = std::fs::remove_dir_all(&tmp);
let mut g = fdoom::core::game::Game::new(true, /* has_gui= */ false, tmp);
fdoom::core::world::reset_game(&mut g, true);
g.settings.set("size", 128);
g.world_name = "dbg".into();
fdoom::core::world::init_world(&mut g);

g.tick(); // IMPORTANT — see below

// drive input like the platform layer would:
g.input.key_toggled("E", true);
g.tick();
g.input.key_toggled("E", false);
```

**Tick once after `init_world` before touching the player.** Freshly spawned entities
(including the player) sit in the level's `entities_to_add` queue until the first
`tick_level` drains them into the arena; before that, `g.entities.take(g.player_id)`
returns `None` (though `g.player()` / `g.try_player()` do look through the queues).

`has_gui = false` gives you a silent `SoundPlayer` and skips the focus handling; `debug
= true` enables the debug-gated key bindings in `InputHandler`.

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
| `SHIFT-T` | score mode |
| `CTRL-T` | (score mode) set the score timer to 5 seconds |
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
