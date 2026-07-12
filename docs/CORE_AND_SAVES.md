# Core, Platform & Saves

Exhaustive reference for fdoom.rs's world-state root (`Game`), the per-tick order, the
platform shell (winit/softbuffer/rodio + the `FDOOM_DEMO` scripted driver), input handling,
settings, localization, and the complete save/load file formats. See also
[ARCHITECTURE.md](ARCHITECTURE.md) for the whole-codebase tour,
[TERRAIN.md](TERRAIN.md) for world generation and the chunk binary format (not repeated
here), and [RENDERING_AND_UI.md](RENDERING_AND_UI.md) for the renderer and display/menu
system that this document's tick order drives.

Every claim below is grounded in the source as of this writing; file:line references are
approximate anchors (line numbers drift), not guarantees — grep the quoted symbol if a
number is stale.

## 1. `Game` — the world-state root (`src/core/game.rs`)

Java scattered global state across `Game`/`Updater`/`World`/`Renderer`/`Settings`/`Sound`
statics; the port gathers all of it into one struct, threaded through nearly every function
as `g: &mut Game`. The field list below is grouped exactly as the source groups it (each
group is a former Java class's statics):

```rust
pub struct Game {
    // Java `Game` statics
    pub debug: bool,
    pub has_gui: bool,
    pub continous: bool,           // JAVA: "continous" (sic) — disables the focus nagger
    pub input: InputHandler,
    pub game_dir: PathBuf,
    pub notifications: Vec<String>,
    pub max_fps: i32,
    pub display: DisplayManager,
    pub game_over: bool,
    pub running: bool,

    // io
    pub settings: Settings,
    pub sound: SoundPlayer,
    pub sound_enabled: bool,        // cached "sound" setting, refreshed every tick
    pub localization: Localization,

    // Java `Updater` statics
    pub gamespeed: f32,
    pub paused: bool,
    pub tick_count: i32,
    time: i32,                      // private — backs get_time()/set_time()
    pub game_time: i32,
    pub past_day1: bool,
    pub note_tick: i32,
    pub as_tick: i32,
    pub saving: bool,
    pub save_cooldown: i32,
    pub tile_tick_count: i32,       // JAVA: Tile.tickCount (a static on Tile, but game state)

    // Java `World` statics + levels
    pub levels: Vec<Option<Level>>,
    pub tiles: Tiles,               // Java `Tiles` static registry
    pub player_dead_time: i32,
    pub pending_level_change: i32,
    pub world_size: i32,
    pub current_level: usize,

    // Java `Renderer`/`Initializer` statics
    pub ready_to_render_gameplay: bool,
    pub show_info: bool,
    pub has_focus: bool,            // window focus (Java polled canvas.hasFocus())
    pub fra: i32,                   // frames in the previous real second
    pub tik: i32,                   // ticks in the previous real second

    pub random: Rng,                 // shared incidental RNG (see PORTING.md "Randomness")
    pub items: Rc<Vec<Item>>,        // item prototype registry (Java `Items` static list)
    pub recipes: Rc<Recipes>,        // Java `Recipes` static lists
    pub entities: EntityArena,       // all live entities
    pub player_id: i32,              // eid of the main player (Java Game.player; main() sets 0)

    pub bed_state: BedState,         // Java `Bed`'s static sleep-tracking state
    pub air_wizard_beaten: bool,     // legacy save slot (Java `AirWizard.beaten`; mob removed)
    pub should_respawn: bool,        // Java `PlayerDeathDisplay.shouldRespawn` static
    pub loading_percentage: f32,     // Java `LoadingDisplay` percentage static
    pub loading_message: String,     // Java `LoadingDisplay` message static ("Level B3" etc.)
    pub world_name: String,          // Java `WorldSelectDisplay.worldName` static
    pub loaded_world: bool,          // Java `WorldSelectDisplay.loadedWorld` static
    pub world_seed: i64,             // Java `WorldGenDisplay.getSeed()` — seed for next gen
}
```

`Game::new(debug, has_gui, game_dir)` builds `localization` → `settings` (needs the
localization's language list) → `input` → the struct literal (`current_level` starts at
`3`, the surface slot; `levels` is sized to `IDX_TO_DEPTH.len()` all `None`;
`world_size` defaults to `128`; `random: Rng::from_time()`), then — **after** the struct
exists — sets `g.items = Rc::new(item::registry::build_registry(&g))`, because the item
registry reads settings (difficulty) during construction, mirroring Java's static-init
ordering (you cannot build the registry as part of the struct literal since it needs `&g`).

The `Renderer` (owning the `Screen` framebuffers) is deliberately **outside** `Game` — see
RENDERING_AND_UI.md §1 — so render code can take `(&mut Screen, &Game)` while tick code
takes `&mut Game`, without borrow conflicts.

### The take-out pattern (`Game::with_entity`)

```rust
pub fn with_entity<R>(&mut self, eid: i32, f: impl FnOnce(&mut Entity, &mut Game) -> R) -> Option<R>
```
(`src/entity/behavior.rs:22-35`). Removes the entity from `g.entities` (falling back to
pulling a player still sitting in a level's `entities_to_add` queue), calls `f(&mut entity,
self)` with both independently mutable, then reinserts (unless the entity's own tick
removed/replaced it). This is the mechanism every per-tick entity/player call in this
document goes through; see PORTING.md §2 and RENDERING_AND_UI.md §10.1 (the display stack
uses the identical idea via `taken_out`).

## 2. `Game::tick` — the exact per-tick order (`src/core/game.rs:258-472`)

```
Game::tick()
 1. refresh sound_enabled / max_fps from settings                      (259-260)
 2. apply_menu_transition()  — applies queued Set/Clear/Exit           (262)
 3. sleeping fast-forward: gamespeed=20 while asleep, day-pass +        (264-286)
    wake-up handling when tick_count crosses SLEEP_END_TIME
 4. autosave tick counter (as_tick); past ASTIME=7200 -> save_world_named   (288-305)
 5. if !paused: set_time(tick_count + 1)  — advances the day clock      (307-310)
 6. if window lost focus and has_gui: input.release_all()               (313-315)
 7. if has_focus || !has_gui:                                            (316)
    7a. game_time += 1  (if player alive & !game_over)                  (317-320)
    7b. input.tick()   — "the ONE place key states advance"             (322)
    7c. if display.menu_active():                                       (324)
         - player tick (with_entity) — BEFORE the display tick,          (328)
           "CRUCIAL" per the source comment
         - tick_current_display()  — take-out pattern on the top         (329)
           Display (e.g. LevelTransitionDisplay ticks here — see §2.2)
         - paused = true                                                (330)
        else (no menu open):
         - paused = false                                               (333)
         - death-display-open OR pending_level_change -> opens           (335-347)
           level_transition_display (queues a Set; see §2.2)
         - player tick (with_entity)                                    (351)
         - if current level exists: ensure_chunks(); tick_level();      (353-358)
           tile_tick_count += 1
         - F3 toggle + debug-only cheat-key handling (--debug builds)    (360-469)
```

### 2.1 Reading this correctly

This is **not** a flat sequence — steps 7c branch on whether a menu is active, and each
branch independently does "player tick, then something else":

- **Menu-active branch**: player tick happens first, then the top display's own `tick()`
  runs (this is where a `LevelTransitionDisplay` or any other menu's logic executes).
  Level tick and chunk streaming are **skipped entirely** while any menu — including the
  level-transition animation itself — is active.
- **No-menu branch**: a death/level-change check happens first (deciding whether to *open*
  a new display — which only takes effect next tick, see below), then the player ticks,
  then the level ticks and streams chunks, then (in debug builds) cheat keys are read.

Two things worth flagging as corrections to a "linear" mental model:

1. **Menu transitions apply *before* `input.tick()`**, not after — `apply_menu_transition()`
   is step 2, well before the input/menu/level block (step 7).
2. **Time-of-day (`set_time`) advances before input/menu/level ticking**, not last — it's
   step 5, tied to the sleep/autosave bookkeeping at the top of the function, not a trailing
   "misc updates" step.

### 2.2 Level transitions — the full multi-tick pipeline

`pending_level_change: i32` is set by `world::schedule_level_change(g, dir)` — just
`g.pending_level_change = dir` (`src/core/world.rs:46-48`; the actual dig/stair/chasm
trigger logic that calls this lives in `player_behavior.rs`, documented in TERRAIN.md §4).

It is consumed **only** in the no-menu branch (step 7c-else), and only opens a display — it
does not itself move the player:

```rust
} else if self.pending_level_change != 0 {
    let change = self.pending_level_change;
    self.pending_level_change = 0;
    crate::screen::level_transition_display::open(self, change);
}
```
`level_transition_display::open(g, dir)` is just `g.set_menu(LevelTransitionDisplay::new(dir))`
— i.e. it stages `PendingMenu::Set`, which is **not applied until the top of the next
tick's `apply_menu_transition()`** call (step 2). So there is a real, multi-tick pipeline:

```
tick N:   pending_level_change detected -> g.set_menu(...) staged (PendingMenu::Set)
tick N+1: apply_menu_transition() pushes LevelTransitionDisplay -> now menu_active()
          player still ticks; LevelTransitionDisplay::tick() runs (time = 1)
tick N+1..N+15: LevelTransitionDisplay::tick() increments `time`
tick N+15 (time == DURATION/2 == 15): world::change_level(g, dir) — the ACTUAL swap
tick N+15..N+30: animation continues
tick N+30 (time == DURATION == 30): g.clear_menu() staged (PendingMenu::Clear)
tick N+31: apply_menu_transition() pops + on_exit()s the display -> menu_active() false
          again; normal player+level tick resumes with the new current_level
```
`DURATION = 30` (ticks), defined in `src/screen/level_transition_display.rs`.

Note also the **mutual exclusivity**: death-display-opening and level-transition-opening
share one `if / else if` — if the player is dead/removed that tick, a pending level change
is not opened even if present (it simply stays pending, since only this branch clears it).

**`world::change_level(g, dir)`** (`src/core/world.rs:51-107`) — the actual swap, called
from inside `LevelTransitionDisplay::tick` at frame 15:

1. Removes the player from the current level.
2. `next_level = current_level + dir`, wrapping (underflow → last level, overflow → level 0).
3. `g.current_level = next_level`; snaps the player's position to tile-center of the
   pre-transition coords.
4. Takes the player entity out of the arena.
5. **Finite-level relocation**: if the destination level is not infinite
   (`!is_infinite()`), the code expects a `Stairs Down`/`Stairs Up` tile (matching travel
   direction) at the landing coordinates; if out of bounds or mismatched, it finds every
   matching-tile position via `level::get_matching_tiles` and relocates to the nearest one
   by squared distance (falling back to level center if none exist). This only triggers for
   finite destinations — infinite layers just stream chunks around wherever the player
   lands, no relocation needed. See TERRAIN.md §6.2 for the world-gen-side reason this
   mismatch can happen (infinite mine gate coordinates vs. the finite dungeon's fixed gate).
6. Re-adds the player to the new level at the final coordinates.
7. `ensure_chunks(g, lvl)` — streams chunks around the new position immediately, rather
   than waiting for the next regular `ensure_chunks` call in step 7c's no-menu branch.

### 2.3 `src/core/updater.rs` — constants only

No tick logic lives here (a doc comment states this explicitly — `Game::tick` owns it all
since the Java statics it would have wrapped are now `Game` fields). Contents:

```rust
pub const NORM_SPEED: i32 = 60;         // ticks/sec
pub const DAY_LENGTH: i32 = 64800;      // ticks/day
pub const SLEEP_END_TIME: i32 = DAY_LENGTH / 8;               // 8100
pub const SLEEP_START_TIME: i32 = DAY_LENGTH / 2 + DAY_LENGTH / 8;  // 40500

pub enum Time { Morning, Day, Evening, Night }
// tick_time(): Morning=0, Day=16200, Evening=32400, Night=48600
```

### 2.4 `src/screen/level_transition_display.rs`

The "sweeping black squares" stair-transition animation (Java `LevelTransitionDisplay`).
`open(g, dir)` = `g.set_menu(LevelTransitionDisplay::new(dir))` — the sole call site is
`game.rs`'s pending-level-change dispatch (§2.2). `DURATION = 30`; `tick()` increments
`time` each call, calls `world::change_level` at `time==15`, calls `g.clear_menu()` at
`time==30`. `render()` draws a diagonal wipe of 8x8 black squares over a fixed 200x150 grid
(a preserved Java quirk — it ignores the actual screen size), sliding up or down depending
on `dir`.

### 2.5 Weather (`src/core/weather.rs`)

Common-ambience rain/snow, layered *under* the rare-events scheduler (`core::events`) and
ticked right after it in `Game::tick` (so the day counter it reads is current). Nothing is
saved — like the event calendar, the whole schedule is a **pure function** of
`(world_seed, g.events.day_number, tick_count)`:

- **Schedule**: each day splits into `SLICES_PER_DAY = 6` slices (`SLICE_LEN =
  DAY_LENGTH/6` ticks). Each slice hashes (`WEATHER_SALT`, same SplitMix64 avalanche as
  terrain/events) into rain-or-dry (one slice in `RAIN_SLICE_ODDS = 5`) with a hash-picked
  plateau peak in 0.55..1.0. Intensity ramps by smoothstep over the last/first
  `SLICE_LEN/8` ticks of each slice, meeting at the midpoint of the two adjacent peaks —
  continuous everywhere, so rain always fades in/out. Day 0 is always dry (calm first
  session day, same convention as events). `schedule_intensity(seed, day, tick)` is the
  pure curve; `rain_intensity(g)` / `is_raining(g)` are the live queries.
- **Biome gating at the player** (presentation + effects): Desert only passes a rare
  per-slice roll (`desert_slice_wet`, ~15% — a gate, not a scaled drizzle); *cold
  country* presents the same intensity as snowfall (`Precip::Snow`) — the smooth
  climate field below `COLD_REACH = 0.36` (`snow_climate`), i.e. all of Tundra
  (`< 0.30`) plus the 0.30..0.36 **cold fringe** of its neighbors. Snow does **not**
  count as rain for the effects API; underground renders nothing (gate in
  `gfx::lighting::render_pass`, surface slot only) and cues stay silent.
- **Accumulation & thaw** (`level::tile::snowfall`, snow wave): where snow falls it
  also *settles* — `snowing_at(g, x, y)` (schedule x cold-reach) drives a random-tick
  interpose in `tile::dispatch::tick` (fire-style) that converts the natural families
  one tile at a time on loaded surface chunks: grass/tufts → Snow (1-in-700 per random
  tick), broadleaf Tree → Snow Tree (1-in-450) — roughly a quarter of a clearing per
  snowy slice. Clear weather thaws *visiting* snow back (1-in-1500 / 1-in-1100), but
  never where snow is native (`snow_native`: Tundra or Mountains by either `biome_at`
  or `biome_at_blended`, protecting the generated patchy boundary and summit caps).
  Nothing else ever converts — floors, farmland, sand, dirt and all player work are
  untouched, and the climate gate keeps dynamic snow 20+ tiles from any sand.
- **Effects API** (for the fire/mob/crop waves): `extinguishes_fire(g)` (intensity >
  0.5), `growth_boost(g)`, `fireflies_hidden(g)` — tile/entity hooks are one-liners on
  the consumer side.
- **Cues**: "Rain patters down..." / "The rain clears." — snow variants in Tundra
  ("Snow drifts down..." / "The snow eases.") and, outside Tundra where snow only
  visits, "The cold creeps in..." / "The snow begins to thaw." — fire on
  `CUE_THRESHOLD = 0.05` crossings, surface-only. **Stateless** edge detection:
  `weather::tick` re-derives the previous intensity from the pure schedule at
  `tick_count - 1` (day-normalized), guarded by `g.paused` and the day-cycle divisor so a
  frozen clock can never re-fire an edge. Events coexist with weather; an Ember Rain
  night merely suppresses the rain *visuals* (see `gfx::lighting`).
- **Rendering** (`gfx::lighting::render_pass`, surface only): a cool ambient dim
  multiplied into the grade LUTs, world-anchored diagonal rain streaks / slow snow flecks
  (Bayer-ordered activation + hash jitter, like the radiance dither), and — independent
  of weather — fish bubbles on open-water tiles where `fish_presence(seed, x, y)` (a
  pure value-noise field, `FISH_SALT`) exceeds `FISH_PRESENCE_THRESHOLD`; the upcoming
  fishing wave reads the same field for its hotspots.

Test coverage: `tests/weather.rs` (schedule fraction/determinism, ramp smoothness,
desert/tundra gating, cue edges, fish-presence field, render smoke + perf ceiling);
`tests/snow_accumulation.rs` (cold-reach presentation, gradual settle + tree flip,
thaw vs. tundra-permanence, worked-tile immunity, visiting-snow cues, staged
screenshot run into `target/verify/snow_spell_*.png`).

## 3. Platform layer (`src/platform/`)

### 3.1 `src/platform/mod.rs` — winit loop, tick/render split, blit

```rust
struct App {
    game: Game,
    renderer: Renderer,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    demo: Option<demo::Demo>,
    last_time: Instant, last_render: Instant, unprocessed: f64,
    frames: i32, ticks: i32, last_timer1: Instant,
}
```
Implements winit's `ApplicationHandler`. `resumed()` creates the window (initial scale
`3.0`, i.e. 864x576 for the 288x192 framebuffer), builds the softbuffer `Context`/`Surface`,
sets `ControlFlow::Poll` — guarded so it only runs once even if `resumed` fires again.
`window_event` handles `CloseRequested` (`game.quit(); event_loop.exit()`),
`Focused(bool)` (→ `game.has_focus`), `KeyboardInput` (translated via
`keys::java_key_name`, forwarded to `game.input.key_toggled`/`key_typed`), and
`RedrawRequested` (→ `self.redraw()`). `about_to_wait` calls `loop_iteration`.

**Tick accumulator — exact math** (`loop_iteration`):
```rust
let now = Instant::now();
let mut ns_per_tick = 1e9 / updater::NORM_SPEED as f64;
if !self.game.display.menu_active() {
    ns_per_tick /= self.game.gamespeed as f64;   // gameplay speed only applies outside menus
}
self.unprocessed += now.duration_since(self.last_time).as_nanos() as f64 / ns_per_tick;
self.last_time = now;
while self.unprocessed >= 1.0 {
    self.ticks += 1;
    if let Some(demo) = &mut self.demo { demo.on_tick(&mut self.game); }
    self.game.tick();
    self.unprocessed -= 1.0;
}
std::thread::sleep(Duration::from_millis(2));   // JAVA: Thread.sleep(2)
```
The accumulator field is literally named `unprocessed` (an `f64`), matching Java's
`Initializer.run()` variable. **There is no cap on the catch-up loop** — no max-iterations
constant, no clamping of `unprocessed`. If the app stalls (window drag, breakpoint, OS
scheduling hiccup), `unprocessed` accumulates and the very next `about_to_wait` call runs as
many `game.tick()` calls as needed to drain it, in one burst, with no upper bound — this
mirrors the Java original, which has the same unbounded behavior.

Render is fully decoupled from tick: a redraw is only *requested*
(`window.request_redraw()`) when `now - last_render > 1.0 / g.max_fps`; zero or many ticks
may have run since the last actual render. FPS/TPS bookkeeping snapshots `frames`/`ticks`
into `g.fra`/`g.tik` once per real second (`last_timer1 += Duration::from_secs(1)`,
additive to avoid drift), then resets both counters.

**`redraw()`**: calls `self.renderer.render(&mut self.game)`, then `demo.on_frame(&self.renderer)`
(demo hook fires right after render — see §3.2), then blits. Blit/scale math (see
RENDERING_AND_UI.md §9.1 for the full derivation): `scale = min(win_w/W, win_h/H)`,
centered/letterboxed, nearest-neighbor sampled, `(pixel as u32) & 0x00FF_FFFF` masked into
the softbuffer surface. `surface.resize(...)` runs every `redraw()` call against the
window's current physical size, so there is no dedicated resize-event handler — resize is
just implicitly re-derived every frame.

`run(game, renderer)` — the public entry point: builds the `EventLoop`, sets
`ControlFlow::Poll`, constructs `App`, blocks on `event_loop.run_app(&mut app)`.

Sound device init is **not** in this file despite the module doc comment mentioning "audio
device" — rodio setup lives in `src/core/io/sound.rs` (`SoundPlayer::new`, constructed
inside `Game::new`), not in the platform shell.

### 3.2 `FDOOM_DEMO` scripted driver (`src/platform/demo.rs`)

Example: `FDOOM_DEMO="wait:30;shot:/tmp/a.png;key:ENTER;quit"`.

**Step syntax** — the script string is split on `;` into non-empty tokens; each token is
split on the first `:` (`(name, "")` if no colon), matched against:

| Token | `Step` variant | Meaning |
|---|---|---|
| `wait:<n>` | `Wait(i32)` | do nothing for `n` ticks (`arg.parse().unwrap_or(1)`) |
| `shot:<path>` | `Shot(String)` | request a screenshot at `path` — see blocking behavior below |
| `key:<name>` | `Key(String)` | tap: press this tick, auto-release next tick; if `name` is a single char, also feeds it as a typed character |
| `down:<name>` | `Down(String)` | press and hold, no auto-release |
| `up:<name>` | `Up(String)` | release |
| `type:<ch>` | `Type(char)` | inject one typed character (first char of `arg`, or space if empty) |
| `quit` | `Quit` | stop the script / quit |
| anything else | — | `panic!("unknown FDOOM_DEMO step: {other}")` |

`key`/`down`/`up` names are the same Java-style key/action names `InputHandler` uses (e.g.
`W`, `ENTER`, `SHIFT-Q`), routed straight into `game.input.key_toggled`.

**Shot-blocking — the exact mechanism**: `Demo` holds `pending_shot: Option<String>`. At
the very top of `on_tick`, `if self.pending_shot.is_some() { return; }` — once a `shot:`
step fires, **every subsequent tick becomes a no-op** (script doesn't advance, no keys
toggle, no wait counts down) until the shot is fulfilled. The `Shot(path)` step handler
itself just sets `pending_shot = Some(path)`; it does not write the file. The actual write
happens in `on_frame` (called from `App::redraw()`, after `Renderer::render` runs):
`if let Some(path) = self.pending_shot.take() { dump_png(&path, &renderer.screen.pixels) }`.
So: ticks can keep running at full simulation rate, but the *script* is frozen until a real
frame is rendered and dumped — which is itself gated by the `max_fps` render-throttle in
`loop_iteration` (§3.1). `dump_png` writes an RGB8 PNG, extracting `(p>>16)&0xff`,
`(p>>8)&0xff`, `p&0xff` from each packed pixel (dropping the top byte).

Other mechanics: `on_tick` forces `game.has_focus = true` every call ("scripted runs must
not depend on the OS granting window focus"); `wait_left` counts down once per tick, only
falling through to advance the script once it hits 0.

### 3.3 `src/platform/keys.rs`

One function: `java_key_name(code: KeyCode) -> Option<&'static str>` — a pure `match` from
winit `KeyCode` to the Java `KeyEvent.VK_*`-style string names `InputHandler` expects (`W`
→ `"W"`, `ArrowUp` → `"UP"`, both Shift keys → `"SHIFT"`, `Enter`/`NumpadEnter` both →
`"ENTER"`, etc., covering letters, digits, arrows, modifiers, punctuation, F1-F12, numpad).
Unmatched codes return `None`; the platform layer logs and drops the event in that case.

## 4. Input system (`src/core/io/input_handler.rs`)

### 4.1 Core types

```rust
pub struct KeyState { pub down: bool, pub clicked: bool }   // public snapshot

struct Key {                    // private per-physical-key state machine
    presses: i32, absorbs: i32,
    down: bool, clicked: bool,
    sticky: bool,
    stay_down: bool,             // JAVA: set but never read — dead field, kept for fidelity
}
```
`Key::toggle(pressed)` sets `down`, incrementing `presses` on a press edge unless `sticky`.
`Key::tick()` is the press→click debounce/auto-repeat algorithm: while `absorbs < presses`
it advances `absorbs` (clamped so `presses - absorbs` never exceeds 3) and sets `clicked =
true`; once `presses > 3` it flips into `sticky` (auto-repeat while held), with `clicked`
mirroring `sticky` thereafter and both counters reset. `Key::release()` hard-resets
everything.

```rust
struct Mapping { action: String, keys: String, debug_only: bool }

pub struct InputHandler {
    keymap: Vec<Mapping>,             // Vec, not a HashMap — preserves display order for
                                        // the key-binding screen
    keyboard: HashMap<String, Key>,    // physical key name -> state, lazily populated
                                        // (SHIFT/CTRL/ALT pre-seeded as stay_down=true)
    last_key_typed: String,
    key_typed_buffer: String,
    pub key_to_change: Option<String>,
    key_changed: Option<String>,
    overwrite: bool,
    pub debug: bool,                   // mirrors Game.debug
}
```

### 4.2 Default action bindings (`init_key_map`)

```
UP="UP|W"            DOWN="DOWN|S"          LEFT="LEFT|A"       RIGHT="RIGHT|D"
SELECT="ENTER"        EXIT="ESCAPE"          ATTACK="SPACE|C"    MENU="X"
INVENTORY="E|I"       CRAFT="Z|SHIFT-E"      PICKUP="V"          DROP-ONE="Q"
DROP-STACK="SHIFT-Q"  SAVE="R"               PAUSE="ESCAPE"      MAP="M"
NIGHT="N"                    (debug_only)
SURVIVAL="SHIFT-S|SHIFT-1"  (debug_only)
CREATIVE="SHIFT-C|SHIFT-2"  (debug_only)
POTIONEFFECTS="P"     INFO="SHIFT-I"
```
A comment in the source explicitly calls these "modern defaults," diverging from the
v0.1.0 straight-port baseline (which had `ATTACK=C|SPACE|ENTER`, `MENU=X|E`,
`PICKUP=V|P` clashing with `POTIONEFFECTS`, and an always-on `NIGHT`) — this is the
"post-port era, fix inherited bugs" convention from PORTING.md/CLAUDE.md in action.

### 4.3 `debug_only` — exactly what it gates

`debug_only` is not just a UI-hiding hint — it makes the action's query return inert
outside `--debug`. In `get_key_impl`:
```rust
if let Some(m) = self.keymap_get(&keytext) {
    if m.debug_only && !debug {
        return KeyState::default();   // always down=false, clicked=false
    }
    keytext = m.keys.clone();
}
```
It also gates **rebinding** (`set_key` only applies to a `debug_only` mapping if
`!m.debug_only || debug`) and **persistence** (`get_key_prefs` filters out `debug_only`
mappings unless `debug` is true) — so cheat bindings are neither rebindable nor
save/load-able outside a `--debug` run.

### 4.4 Action resolution, compound keys, `get_physical_key`

`get_key(keytext)` uppercases `keytext`, looks it up as an *action* name, resolves
`|`-separated alternatives with OR semantics (any alternative's `down`/`clicked` makes the
whole thing true), and for `-`-joined compound bindings (e.g. `SHIFT-Q`) requires the named
modifiers to be *exactly* the ones currently held (a bare action with no `-` requires *no*
modifiers held at all — this is why `Q` alone doesn't fire while Shift is held; covered by
the `compound_keys_need_modifiers` unit test).

```rust
pub fn get_physical_key(&mut self, keytext: &str) -> KeyState {
    let debug = self.debug;
    self.get_key_impl(keytext, false, debug)   // get_from_map = false: bypass the action keymap
}
```
Treats `keytext` as a literal physical key name, still applying the `|`/`-`/modifier logic.
Used by text-entry menu rows (RENDERING_AND_UI.md §10.3's `captures_typing`) so typed
letters never double as navigation.

### 4.5 Key rebinding + persistence

Flow: `KeyInputEntry::tick` (RENDERING_AND_UI.md §10.3) calls
`g.input.change_key_binding(action)` (press `C`/`Enter`, overwrite mode) or
`add_key_binding(action)` (press `A`, additive mode) — both just stage `key_to_change` +
an `overwrite` flag. The next non-modifier keypress through `key_toggled` sees
`key_to_change.is_some()`, builds the new binding string (either the key alone, or
`"<old>|<mods><new>"` if additive), writes it via `keymap_put`, stores `key_changed`, and
consumes that keypress (it is *not* also registered as a normal toggle that tick).
`KeyInputDisplay` polls `get_changed_key()` to update its row label.

**Persistence is not through `Settings`** — it's a separate blob written straight into the
`Preferences` save file (§6.1): `get_key_prefs(debug)` serializes each visible `Mapping` as
`"ACTION;keys"`, joined with `:`, as the last line of `Preferences.miniplussave`. Loading
reverses this (`load.rs`) via `set_key(action, keys, debug)`, which also tolerates a legacy
on-disk `"ACTION=debug"` suffix by stripping it before lookup.

## 5. Settings (`src/core/io/settings.rs`)

```rust
pub enum Value { Str(String), Int(i32), Bool(bool) }   // src/screen/entry/array_entry.rs

pub struct Settings {
    values: HashMap<String, Value>,
    languages: Vec<String>,     // scanned from assets at startup
}
```

`KEYS: [(&str, &str); 12]` — the schema table, `(key, display label)`:

```
fps -> "Max FPS"              diff -> "Difficulty"          mode -> "Game Mode"
sound -> "Sound"              autosave -> "Autosave"        size -> "World Size"
theme -> "World Theme"        type -> "Terrain Type"        worldtype -> "World"
unlockedskin -> "Wear Suit"   skinon -> "Wear Suit"         language -> "Language"
```
(`unlockedskin`/`skinon` share a label but are distinct keys — owned-vs-currently-worn.)

`options_of(option)` is the single source of truth for legal values / menu choices:

| Key | Options |
|---|---|
| `fps` | `10..=300` (as `Int`) |
| `diff` | `Easy, Normal, Hard` |
| `mode` | `Survival, Creative` — *"Survival is the only real mode; Creative remains for the `--debug` cheat toggle"* |
| `sound`/`autosave`/`unlockedskin`/`skinon` | `true, false` |
| `size` | `128, 256, 512` (as `Int`) |
| `theme` | `Normal, Forest, Desert, Plain, Hell` |
| `type` | `Island, Box, Mountain, Irregular` |
| `worldtype` | `Infinite, Classic` |
| `language` | `self.languages` |
| anything else | empty |

`default_of`: `fps=60, diff=Normal, mode=Survival, sound/autosave/unlockedskin=true,
skinon=false, size=128, theme=Normal, type=Island, worldtype=Infinite, language=english`.
`get(option)` indexes directly (panics if unknown); `get_idx(option)` returns the position
within `options_of` via `Value::matches` (case-insensitive string equality); `set(option,
value)` only applies if `value` is one of `options_of(option)` — otherwise silently
ignored; `set_idx` is the index-based counterpart.

### 5.1 The "survival-only" gate — three separate mechanisms, not one flag

There is **no single runtime assertion** forcing survival mode; the restriction is the
combination of:

1. `options_of("mode")` still includes `Creative` (it is a legitimate, settable `Value`) —
   the comment only states intent, not a hard block.
2. `src/screen/world_gen_display.rs` — the new-world screen builds settings widgets for
   only `["worldtype", "size", "theme", "type"]`; **`mode` is deliberately excluded from the
   UI list** ("survival is the only game mode (user direction); no mode picker"), so the
   normal new-world flow can never select Creative.
3. `src/core/game.rs` (inside the debug cheat-key block, §2) — the only runtime code that
   flips `mode` to `"creative"` is gated behind the `debug_only` `CREATIVE`/`SURVIVAL`
   input actions (§4.3), so it's unreachable without `--debug`.

So Creative exists in the `Value`/`Settings` domain but is reachable only via a debug-only
keybind — a UI omission plus an input gate, not a `Settings`-level enforcement.

### 5.2 UI bridge

See RENDERING_AND_UI.md §10.4 for `settings_widgets::make_entry`/`sync` — `Settings` itself
knows nothing about menu widgets; the bridge module builds `ArrayEntry` rows from the
schema and syncs edits back every tick the Options/WorldGen screen is active.

## 6. Localization (`src/core/io/localization.rs`)

```rust
pub struct Localization {
    known_unlocalized_strings: RefCell<HashSet<String>>,   // debug-log dedup only
    localization: HashMap<String, String>,                  // source string -> localized string
    selected_language: String,
    loaded_languages: Vec<String>,
    pub debug: Cell<bool>,       // mirrors Game.debug; RefCell/Cell so get_localized can take &self
}
```

Language files are embedded at compile time as `crate::assets::LOCALIZATIONS: [(name,
text)]` (`.mcpl` format). `Localization::new()` seeds `loaded_languages` from that list,
defaults `selected_language = "english"`, and loads it immediately.

**File format** (`load_selected_language_file`) — alternating key/value lines: `#`-prefixed
and blank lines are skipped; otherwise the first non-skip line is the key, the next
non-skip line is its value, then the pattern repeats. **`JAVA: entries accumulate across
language switches — the map is never cleared`** — `change_language` re-runs the loader but
never clears `self.localization` first, so keys unique to a previously-loaded language can
stick around after switching (harmless in practice since keys are the shared untranslated
source strings, normally overwritten cleanly, but worth knowing if debugging a
stale-translation report).

**Lookup / fallback** (`get_localized(string)`): blank strings and strings that parse as
`f64` are returned as-is (never treated as localization keys — numbers aren't localized).
Otherwise looks up `string` in the map; **on a miss, the original string is echoed back
unchanged** (`unwrap_or_else`), with a one-time (deduped via `known_unlocalized_strings`)
debug-console warning if `debug` is set. So a missing translation never breaks display text
— worst case it shows the untranslated source string.

## 7. Save/load formats

### 7.1 Save directory layout (`src/core/file_handler.rs`)

Per-OS game dir (`determine_game_dir`):

| OS | Path |
|---|---|
| macOS | `$HOME/fdoom` |
| Windows | `%APPDATA%\fdoom` |
| Linux | `$HOME/.fdoom` |

On first run, if an old Linux-style `"{save_dir}/.fdoom"` folder exists and differs from
the computed `game_dir` (relevant on mac/Windows carrying over an old install), its contents
are migrated in (`copy_folder_contents`, renaming on collision by appending `"(Old)"`
repeatedly, extension normalized to `.fdoom` — distinct from the `.miniplussave` save-file
extension).

Per-world folder: `<game_dir>/saves/<world_name>/`. Files that can appear (current version
`3.0.0`):

| File | Written by | Condition |
|---|---|---|
| `Game.miniplussave` | `Save::write_game` | always |
| `Level{n}.miniplussave` / `Level{n}data.miniplussave` | `Save::write_world` | one pair per **non-infinite** level (`n` = 0..4) |
| `Player.miniplussave` | `Save::write_player` | skipped if `g.is_valid_server()` |
| `Inventory.miniplussave` | `Save::write_inventory` | skipped if `g.is_valid_server()` |
| `Entities.miniplussave` | `Save::write_entities` | always |
| `WorldMeta.miniplussave` | `save_world_named` (not the `Save` struct) | only if any level `is_infinite()` — TERRAIN.md §5.2, not repeated here |
| `chunks/<depth>/<cx>_<cy>.bin` | `save_chunk` | chunked/infinite layers only — TERRAIN.md §5.3, not repeated here |

Global (not per-world), directly under `game_dir`: `Preferences.miniplussave`,
`Unlocks.miniplussave` (both via `Save::write_prefs`); a lowercase legacy
`unlocks.miniplussave` is read-only (one-time migration, §7.7).

**Case-folding quirk worth knowing**: `save_world_named` always lowercases the world name
for the `WorldMeta` path and the chunk directory, but the main world folder used for
`Game`/`Level*`/`Player`/`Inventory`/`Entities` is built from the **as-given-case** world
name. A separate rename step in `Save::new_at` only lowercases the folder if its parent is
literally named `"saves"` (true only when `game_dir` was passed as a relative path — an
edge case). In practice, a mixed-case world name can therefore land its `WorldMeta`/`chunks/`
in a lowercase folder while its other five file kinds sit in the originally-cased folder,
unless that rename path fires first.

### 7.2 Serialization styles

- **Non-world files** (`Preferences`, `Unlocks`, `Game`, `Player`, `Inventory`, `Entities`):
  one logical value per **line** (`\n`-joined).
- **World/tile files** (`Level{n}`, `Level{n}data`): one comma-terminated blob — *every*
  value (including the last) gets a trailing `,`. `Level5` files get one *extra* trailing
  comma (`JAVA:` quirk, legacy/dead since the current level range is 0..4). Loading
  (`java_split(text, ',')`) mimics `String.split` — keeps interior empty fields, drops all
  trailing empty fields — so the extra comma(s) are harmless.
- **Entity lines**: a custom DSL, `"{ClassName}[{x}:{y}{:extra-fields}]"`, colon-delimited
  inside the brackets (chest contents use `;` as a sub-delimiter — see §7.6).
- **Inventory item tokens**: `"{name}_{count-or-durability}"` (see §7.5).

### 7.3 `Preferences.miniplussave` (global)

Written by `Save::write_prefs`, one value per line, in order:

| # | Field | Encoding |
|---|---|---|
| 1 | version | `"3.0.0"` (`Version::to_string()`) |
| 2 | sound | `"true"`/`"false"` |
| 3 | autosave | `"true"`/`"false"` |
| 4 | fps | integer |
| 5 | savedIP | `""` (multiplayer stub, always empty) |
| 6 | savedUUID | `""` (stub) |
| 7 | savedUsername | `""` (stub) |
| 8 | language | string |
| 9 | keymap blob | `"action;keys:action;keys:..."` (`get_key_prefs`) |

Load-side version gating is intricate — see §7.9 items 7-9.

### 7.4 `Unlocks.miniplussave` (global)

One line per unlock. Currently at most one possible line: `"AirSkin"`, present only if
`g.settings.get("unlockedskin").as_bool()`. (No unlock → an empty file.) Legacy tokens
`HOURMODE`/`MINUTEMODE`/`*_ScoreTime` are recognized on load (Score-mode achievement
thresholds from a removed game mode) and rewritten/discarded — dead-but-harmless migration
code, §7.9 item 21.

### 7.5 Per-world `Game.miniplussave`

Written by `Save::write_game`, one value per line, in order: version (`"3.0.0"`), mode
index (`get_idx("mode")`), `tick_count`, `game_time`, difficulty index (`get_idx("diff")`),
`air_wizard_beaten` (`"true"`/`"false"`). Load-side field presence/interpretation is
version-gated — see §7.9 items 4-6.

### 7.6 Per-world `Level{n}` / `Level{n}data`

Only for non-infinite levels. `Level{n}`: world-size **setting** value written **twice**
(not the level's actual dimensions — a JAVA-inherited quirk, explicitly commented in the
source; the real `w`/`h` used for the tile loop comes from the in-memory `Level`, not this
field), then `depth`, then `w*h` tile **name** strings in **x-outer, y-inner** order
(`for x { for y { tile_at(x,y).name } }` — "the list reads down, then right one, rather
than right, then down one," per the source comment). `Level{n}data`: `w*h` tile-data-byte
strings, same order, no header.

Load side cross-checks parent/child `Stairs Down`/`Stairs Up` consistency between adjacent
finite levels and force-corrects mismatches (logging `"INCONSISTENT STAIRS detected"`).
Numeric-tile-id vs. tile-name legacy handling and the Lapis→Gem-Ore migration are
version-gated — see §7.9 items 10-11 (TERRAIN.md covers the terrain-generation angle of
this file format; this section covers the format itself).

### 7.7 Per-world `Player.miniplussave`

Written by `write_player`, one value per line, in order:

| # | Field | Notes |
|---|---|---|
| 1 | x | pixel coords |
| 2 | y | |
| 3 | spawnx | |
| 4 | spawny | |
| 5 | health | |
| 6 | hunger | |
| 7 | armor | |
| 8 | armor_damage_buffer | **only if `cur_armor.is_some()`** |
| 9 | cur_armor name | **only if `cur_armor.is_some()`** |
| 10 | score | |
| 11 | current_level | |
| 12 | potion effects | `"PotionEffects[Type1;dur1:Type2;dur2:...]"`, `"PotionEffects[]"` if none |
| 13 | shirt_color | int |
| 14 | skinon | `"true"`/`"false"` |

Fields 8-9 being conditional is why old-version loading branches heavily on where in the
line sequence armor data lands (§7.9 item 13). **Not saved here** (see §7.8 "what's not
saved" table): `move_speed`, `multiplier`/`multiplier_time`, `active_item`/`attack_item`/
`prev_item`, `attack_time`, `attack_dir`, `on_stair_delay`, `stamina` and its recharge/delay
fields, `hunger_stam_cnt`/`stam_hunger_ticks`/`step_count`/`hunger_charge_delay`/
`hunger_starve_delay`, `showpotioneffects`, `cooldowninfo`, `regentick`.

### 7.8 Per-world `Inventory.miniplussave`

Written by `write_inventory`, one value per line: an **optional first line** =
`pd.active_item.get_data()` (only if an item is actively held/equipped — this is where
"currently held item" persists, *not* in the Player file), then one line per inventory slot
= `item.get_data()`.

**`Item::get_data()` token format**:

| Kind | Token |
|---|---|
| `Tool { dur, .. }` | `"{name}_{dur}"` — e.g. `"Iron Pickaxe_84"`. Tool **level/tier is not a separate field**: each tier is a distinct registered item name (`"Wood Pickaxe"` vs `"Iron Pickaxe"`), so tier is implicit in the name; only current durability is an explicit numeric suffix. |
| Stackable / Unknown / Food / Armor / Clothing / Potion / TileItem / Torch / Bucket | `"{name}_{count}"` — e.g. `"Wood_16"`, `"Cooked Pork_3"` |
| PowerGlove / Furniture / Book (non-stackable, non-Tool) | bare `"{name}"` |

Load (`registry::get_opt`): splits on the **first** `_` (or `;` as a fallback separator) —
if the item is stackable, the suffix is a count; if it's a `Tool`, the suffix is durability.
Additional legacy handling: `"Power Glove"` tokens are dropped (removed item); for
`world_ver <= "2.0.4"`, a token containing `;` is parsed via a separate legacy path
(`name;count`, manually dispatched to `set_count`+`add` or `add_num`) — the modern writer
never emits `;`-delimited inventory tokens (that separator is reserved for chest contents,
§7.9 item 3, and potion-effect entries in the Player file).

### 7.9 Per-world `Entities.miniplussave`

One line per entity: `"{ClassName}[{x}:{y}{extra-fields}]"`. `extra-fields` is a fixed
`:`-prefixed sequence, present conditionally based on entity kind:

1. `x`, `y` — always.
2. `health` — if the entity `is_mob()` (any mob, including Player).
3. `enemy level` — additionally if `is_enemy_mob()` (Zombie/Snake/Knight/MarshLurker/
   FeralHound/StoneGolem/NightWisp).
4. **Chest contents** — if it's a Chest/DeathChest/DungeonChest: one `:{item_name}` per
   slot, with `;{count}` appended to that same field for stackable items (e.g.
   `:Wood;12`). DeathChest additionally appends `:{time}` (decay countdown); DungeonChest
   appends `:{is_locked}` (`"true"`/`"false"`).
5. **Spawner**: `:{caged_mob_class_name}:{level_or_1}`.
6. **Lantern**: `:{lantern_type_ordinal}` (0=Norm, 1=Iron, 2=Gold).
7. Level-depth index — **always last**: `:{lvl_idx(depth)}`.

`ClassName` comes from `entity_class_name()` — e.g. `Player`, `Cow`, `Pig`, `Sheep`,
`GlowWorm`, `Zombie`, `Snake`, `Knight`, `MarshLurker`, `FeralHound`, `StoneGolem`,
`NightWisp`, `ItemEntity`, `Arrow`, `Zap`, `Particle`, `TextParticle`, `Furniture`, `Chest`,
`DeathChest`, `DungeonChest`, `Bed`, `Lantern`, `Spawner`, `Tnt` — **except** any
`EntityKind::Crafter(c)`, whose written name is `c.crafter_type.name()` (`Workbench`,
`Anvil`, `Enchanter`, `Loom`, `Furnace`, `Oven`) — never the literal `"Crafter"`.

**Never written for local saves** (returns `""`, dropped): `ItemEntity`, `Arrow`, `Zap`,
`Particle`, `TextParticle` — dropped items, in-flight projectiles, and all particle/zap
effects do not persist (see §8; the Zap inherits the Java Spark's deliberate exclusion:
not saving them prevents "an unfair cheat" of clearing them by reloading).

Example lines (constructed from the field rules; illustrative, not literal file excerpts):
```
Zombie[144:208:20:2:3]                          x,y,health,enemylevel,level-idx
Cow[80:64:10:3]                                  x,y,health,level-idx (no enemy-level slot)
Bed[112:96:2]                                    x,y,level-idx (no mob/chest/spawner fields)
Workbench[112:96:2]                              a Crafter — name is the crafter type, not "Crafter"
Chest[176:64:Wood;12:Iron Sword:2]               one stackable + one non-stackable item
DeathChest[176:64:Wood;12:4500:2]                4500 = decay time, before the level-idx
DungeonChest[176:64:Iron Sword:true:4]           true = locked
Spawner[176:64:Zombie:3:1]                        caged Zombie, level 3
Lantern[112:96:2:1]                               ordinal 2 = Gold, level-idx 1
```

**A `Player[...]` line can be written** (Player maps to class name `"Player"` and the
player is a normal arena entity), but **`Load::load_entities` explicitly skips any line
starting with `"Player"`** — the main player is restored solely from the `Player`/
`Inventory` files, never from an `Entities` line.

### 7.10 Version gating (`src/saveload/version.rs`, `src/saveload/load.rs`)

```rust
pub struct Version { make: i32, major: i32, minor: i32, dev: i32, valid: bool }
```
Parsed from a dotted string; the third segment may carry a `-dev`/`-pre` suffix (both
treated identically — just a trailing number after stripping the literal substring
`"dev"`/`"pre"`). Invalid input sets `valid=false` and prints `"INVALID version number:
..."` (no panic — comparison proceeds against a zeroed struct). Ordering compares
`make`→`major`→`minor`→`dev`, with one twist: **`dev == 0` sorts *after* any nonzero
`dev`** (a zero dev value means "final release," which is newer than its own dev
pre-releases) — e.g. `"2.0.4-dev3" < "2.0.4"`.

`version()` (`src/core/game.rs`) returns `Version::new("3.0")`, which **displays as
`"3.0.0"`** — every freshly-written version line in `Game`/`Preferences` is literally
`3.0.0`.

**Every version gate found in `load.rs`** (the headline 3.0-refusal is already in
TERRAIN.md §5.1 — this list is everything else):

1. `new_world` — provisional version peek from the `Game` file's raw first line
   (`if data[0].contains('.') { parse it } else { default to "1.8" }`) — later overwritten
   by the authoritative parse inside `load_game`.
2. `if !has_global_prefs { has_global_prefs = world_ver >= "1.9.2" }` — infers prefs support
   for worlds saved before global Preferences existed as a concept.
3. **The 3.0 refusal** (TERRAIN.md §5.1) — `if world_ver < "3.0" { refuse to load }`.
4. `load_game`: mode field only read `if world_ver >= "2.0.4-dev8"` (older `Game` files have
   no mode line).
5. `if world_ver >= "1.9.3-dev2" { past_day1 = game_time > 65000 } else { game_time = 65000 }`
   — anti-time-cheat clamp for saves that predate proper day-1 tracking.
6. `if world_ver < "1.9.3-dev3" { diff_idx -= 1 }` — difficulty enum shifted a slot.
7. `load_prefs`: `if !data[2].contains(';') { pref_ver = parse(data.remove(0)) } else { pref_ver = "2.0.2" }`
   — detects whether a leading version line exists at all in `Preferences`.
8. `if pref_ver >= "2.0.4-dev2" { read fps field }`.
9. `if pref_ver < "2.0.3-dev1" { rest = savedIP.. } else { discard savedIP; if pref_ver > "2.0.3-dev3" { discard savedUUID/savedUsername }; if pref_ver >= "2.0.4-dev3" { read language } }`.
10. `load_world`: `if world_ver < "1.9.4-dev6" { tile name is actually a numeric id — look up via the OLD_IDS table }`.
11. `if depth == MIN_LEVEL_DEPTH+1 && tilename == "LAPIS" && world_ver < "2.0.3-dev6" { 80% chance -> "Gem Ore" }` — randomized rebalance migration (uses `g.random`, a deliberate porting improvement over Java's true nondeterministic `Math.random()` for this specific spot).
12. `load_player`: hunger field only read `if world_ver >= "2.0.4-dev7"`.
13. `if armor > 0 { if world_ver < "2.0.4-dev7" { read armor name + damage buffer from the TAIL of the data line, in reverse } else { read damage buffer then armor name inline } }` — the two fields swapped position and anchor across that version.
14. `if world_ver < "2.0.4-dev7" { read a trailing arrow_count field; if world_ver < "2.0.1-dev1" { actually add that many arrows to inventory } }` — arrows used to be a Player-file field, not a plain inventory item.
15. `if world_ver < "2.0.4-dev8" { read + apply a redundant per-player mode field }`.
16. `if world_ver < "1.9.4-dev4" { shirt color is a bracketed RGB-triplet string, decode each channel /50 and concatenate } else { plain int }`.
17. `load_inventory`: `if world_ver < "1.9.4" { apply sub_old_name to every item token }`.
18. `if world_ver <= "2.0.4" && token.contains(';') { legacy semicolon count-parsing path }`.
19. `load_entity`: `if world_ver < "1.9.4-dev4" { apply sub_old_name to chest item tokens }`.
20. `if world_ver >= "1.9.4" && info.len() > 3 { rebuild Lantern from its saved type ordinal }` — pre-1.9.4 Lantern entities have no type field (defaults to `Norm`).
21. `legacy_update_unlocks`/`load_unlocks` — filename-case migration (`unlocks` →
    `Unlocks`) plus `HOURMODE`/`MINUTEMODE` token rewrite to a `_ScoreTime` convention,
    which is then discarded (Score mode is gone).

### 7.11 What's not saved

- **`ItemEntity`, `Arrow`, `Zap`, `Particle`, `TextParticle`** — never written for local
  saves (§7.9's entity table); ground items, in-flight arrows, and all visual-effect
  entities vanish on save/reload. Zaps are excluded *deliberately* (inherited from the
  Java Spark: reloading should not be a way to cheat them away).
- **Player transient combat/movement fields**: `move_speed`, `attack_item`, `prev_item`,
  `attack_time`, `attack_dir`, `on_stair_delay`.
- **Stamina/hunger internals**: `stamina`, `stamina_recharge[_delay]`, `hunger_stam_cnt`,
  `stam_hunger_ticks`, `step_count`, `hunger_charge_delay`, `hunger_starve_delay` — only the
  visible `hunger` meter itself persists.
- **Score-mode multiplier state**: `multiplier`, `multiplier_time` (dead weight — `score`
  itself still persists as a general stat).
- **Derived/UI potion state**: `showpotioneffects`, `cooldowninfo`, `regentick` — only the
  raw `potioneffects` timer map persists.
- **`active_item`** is not in the Player file at all — it's the optional first line of the
  *Inventory* file (§7.8).
- **A level's true width/height** — `Level{n}` writes the world-size *setting* twice, not
  the level's real dimensions (§7.6); real `w`/`h` is recomputed in-memory.
- **Untouched (non-dirty) chunks** — never written to disk; they regenerate byte-identical
  from `(seed, depth, x, y)` on demand (TERRAIN.md §2).
- **Multiplayer-only entity fields** (owner eid for Arrow/Zap, item-entity motion
  vectors, TextParticle color) — the code path exists but is gated behind
  `!is_local_save`, unreachable for any on-disk save in this single-player-stubbed build.

### 7.12 Pre-3.0-era item/armor/potion migration shims (enumerated)

(TERRAIN.md notes "many exist"; here is the full list, matching §7.10's version gates.)

1. **`sub_old_name(name, world_ver)`** — the item-rename table: `Hatchet→Axe`,
   `Pick→Pickaxe` (with a `Pickaxeaxe→Pickaxe` double-substitution cleanup),
   `Spade→Shovel`, `Pow glove→Power Glove`, strips old Roman-numeral `"II"` tier suffixes,
   `W.Bucket→Water Bucket`, `L.Bucket→Lava Bucket`, `G.Apple→Gold Apple`, `St.→Stone`,
   `Ob.→Obsidian`, `I.Lantern→Iron Lantern`, `G.Lantern→Gold Lantern`, `BrickWall→Wall`,
   spacing fixes around `Brick`/`Wall`, and `"Bucket"`→`"Empty Bucket"` exactly; a broader
   tier (`world_ver < "1.9.4"`) additionally renames `I.Armor→Iron Armor`,
   `S.Armor→Snake Armor`, `L.Armor→Leather Armor`, `G.Armor→Gold Armor`.
2. `"Power Glove"` tokens dropped wherever found (inventory and chest contents) — a removed
   item that must not resurrect from old saves.
3. Semicolon-delimited legacy inventory count format (`name;count` vs. modern `name_count`)
   for `world_ver <= "2.0.4"`.
4. Armor field position/order swap in the Player file (tail-reversed vs. inline-forward)
   across `"2.0.4-dev7"`.
5. Legacy trailing arrow-count field in the Player file, with a further sub-gate
   (`< "2.0.1-dev1"`) to actually re-add those arrows as inventory items.
6. Redundant per-player mode field, read only pre-`"2.0.4-dev8"`.
7. Bracketed-RGB-triplet shirt color decoding, pre-`"1.9.4-dev4"`.
8. Numeric tile ids (vs. names) in `Level{n}` files, pre-`"1.9.4-dev6"`, resolved via the
   `OLD_IDS` table (including a duplicate light/torch id range for pre-`"1.9.4-dev3"`
   compatibility).
9. Randomized Lapis→Gem-Ore migration on the first mine layer, pre-`"2.0.3-dev6"`.
10. Global-prefs/world-Game version-line absence handling (defaults `"2.0.2"`/`"1.8"`
    respectively) for saves that predate the version-string convention itself.
11. `unlocks`→`Unlocks` filename-case migration plus the `HOURMODE`/`MINUTEMODE` token
    rewrite (discarded after rewriting, since Score mode no longer exists).

## 8. HOW TO EXTEND

### 8.1 Add a setting

1. Add a `(key, label)` entry to `KEYS` in `src/core/io/settings.rs`.
2. Add its legal values to `options_of` and its default to `default_of`.
3. If it needs a menu row, add it to the relevant screen's settings-entry list (e.g.
   `OptionsDisplay` or `WorldGenDisplay` — grep `settings_widgets::make_entry` call sites)
   — RENDERING_AND_UI.md §10.4 covers the widget bridge.
4. If the setting must persist across restarts as part of `Preferences` and it isn't
   already covered by an existing line in `write_prefs`/`load_prefs`, add a line to both
   (remember `load_prefs`'s version-gating pattern if you ever need to add a field to an
   *existing* save format without breaking old saves — see §7.10 for the established idiom
   of "only read this field if `version >= X`").
5. If it's world-scoped rather than global (like `worldtype`/`size`/`theme`), make sure it's
   read at world-creation time (`WorldGenDisplay`/`init_world`) rather than expecting
   `Preferences` to carry it.

### 8.2 Add a saved field safely

This codebase's established pattern for adding a field to an existing save file without
breaking old saves is **version-gated reads, not format versioning per file**:

1. Bump `version()` in `src/core/game.rs` if the change is significant enough to warrant a
   new version string (most field additions do not need this — see how many of §7.10's
   gates use `-dev`/`-pre` sub-versions instead).
2. In `save.rs`, add the new field's write unconditionally (new saves always get the new
   field).
3. In `load.rs`, guard the *read* with `if world_ver >= Version::new("your-new-version") {
   read it } else { use a sensible default }` — following the exact idiom used throughout
   §7.10 (e.g. gate 12's hunger field, gate 8's fps field). Never assume an old save has
   your new field.
4. If the field changes the *position* of subsequent fields in a comma/line format, prefer
   appending new fields at the end rather than inserting in the middle — inserting requires
   every subsequent version gate to also know the insertion happened (see gate 13's armor
   field reordering for how painful a mid-sequence change gets to maintain).
5. Update this document's field table for the affected file.

### 8.3 Add a demo-script step

1. Add a new `Step` variant in `src/platform/demo.rs`.
2. Add its token-prefix parsing to the `match` in `Demo::from_env` (§3.2's syntax table).
3. Add its execution to `Demo::on_tick` (or `on_frame` if it needs to observe a rendered
   frame, like `Shot` does) — remember the `pending_shot`-style blocking pattern if your
   step needs to wait for something asynchronous relative to the tick loop.
4. If it interacts with input, route it through `game.input.key_toggled`/`key_typed` rather
   than reaching into `InputHandler`'s internals directly, so it behaves identically to a
   real keypress (including the click/sticky debounce in `Key::tick`).

### 8.4 Add a sound

1. See `src/core/io/sound.rs` for `SoundPlayer`/`Sound` (constructed in `Game::new`, rodio
   sink per restartable sound — not deeply covered in this document; it is out of the
   `platform/mod.rs` file despite that module's doc comment mentioning "audio device").
2. Add an asset via `src/assets.rs`'s embedding mechanism (mirrors how sprites/localizations
   are embedded — `include_bytes!`).
3. Add a `Sound` enum variant and play it via `g.play_sound(Sound::YourSound)` at the
   relevant call site (tile interact, entity hurt, UI confirm/select/back — grep
   `Sound::Select`/`Sound::Confirm`/`Sound::Back` for the existing UI-sound conventions
   documented in RENDERING_AND_UI.md §10).

## 9. Invariants & gotchas

- **`Game::tick`'s menu-active and no-menu branches are not symmetric.** Level tick + chunk
  streaming only happen in the no-menu branch — this includes the entire 30-tick duration
  of the level-transition animation itself. Code that assumes "the level always ticks every
  frame" will be surprised during any menu, not just obviously-paused ones.
- **Menu-stack and level-change mutations are never immediate.** `set_menu`/`clear_menu`/
  `exit_menu` (and, by extension, opening `LevelTransitionDisplay`) only stage `pending`
  state, applied at the top of the *next* tick. Code that pushes a display and expects to
  see it as the new top-of-stack in the same tick — or that sets `pending_level_change` and
  expects `current_level` to have already changed a few lines later — will observe stale
  state.
- **The tick-accumulator catch-up loop is unbounded.** A long stall bursts through as many
  `game.tick()` calls as needed to drain `unprocessed` in one `about_to_wait` call, with no
  cap. Anything that assumes "at most one tick per frame" (e.g. a per-frame side effect that
  isn't idempotent) will misbehave under a stall; prefer per-tick counters/timers over
  per-frame ones for anything gameplay-visible.
- **`debug_only` bindings are triply gated**: inert when queried outside `--debug`,
  unrebindable outside `--debug`, and excluded from `Preferences` serialization outside
  `--debug`. Don't expect a debug cheat keybind to survive a save/reload cycle done without
  `--debug`, even if it was rebound while `--debug` was active.
- **Key rebinding does not go through `Settings`.** It is a wholly separate `InputHandler`
  mechanism serialized directly into `Preferences`. Code that wants to know "what key does
  action X map to right now" should query `InputHandler`, not `Settings`.
- **World-folder casing can diverge from the `WorldMeta`/`chunks` casing** (§7.1) for a
  mixed-case world name created outside the one narrow rename-normalization path. Don't
  assume all of a world's files share one exact-case folder without checking.
- **A `Player[...]` line can exist in `Entities.miniplussave` but is always skipped on
  load** — the player is restored exclusively from `Player`/`Inventory`. Don't "fix" the
  entities-writer to skip writing the player line without checking nothing else relies on
  it being present (nothing currently does, per the research behind this document, but the
  skip-on-load behavior means a stray Player entity line is silently harmless either way).
- **Untouched infinite-layer chunks are never saved** — this is by design (TERRAIN.md §2),
  not a save-format gap; don't add code that tries to "make sure every chunk is written."
- **Version comparisons treat `dev == 0` as newer than any nonzero `dev`** for the same
  make/major/minor — a naive numeric comparison of the dev field alone would get this
  backwards. Use `Version`'s `Ord` impl, don't hand-roll a comparison.

## 10. Test coverage map

| Test | Locks in |
|---|---|
| `src/core/io/input_handler.rs` inline `#[cfg(test)]` | click-is-one-tick, sticky-after-4-presses, compound-key modifier gating (`compound_keys_need_modifiers`), multi-mapping OR-combine, typed-text accumulation/backspace. |
| `tests/keymap_check.rs` | Key-binding/mapping sanity across the full default keymap. |
| `tests/save_load_roundtrip.rs` | Full save→load round-trip for a world (fields survive, version gates don't misfire on a freshly-written save). |
| `tests/display_flow.rs` | Menu/display push/pop/exit sequencing, including the take-out (`taken_out`) interaction — see RENDERING_AND_UI.md §10. |
| `tests/gameplay_soak.rs` | Extended tick-loop soak (many `Game::tick()` calls) — a general regression guard on the tick order not panicking/diverging over time. |
| `tests/infinite_world.rs` (`infinite_world_boots_and_walks`) | Save+reload round-trips world type and seed for infinite worlds (TERRAIN.md §9 — cross-referenced here since it exercises this document's save-format code too). |

Run `cargo test` for everything; see [DEV_GUIDE.md](DEV_GUIDE.md) for `FDOOM_DEMO` usage
patterns and headless-testing conventions used across this repo's test suite.
