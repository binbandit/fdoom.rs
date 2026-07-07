# Architecture

A guided tour. Read top to bottom (~15 minutes) and the codebase should feel legible.
`PORTING.md` explains *why* things are shaped this way (Java heritage); this file
explains *what is where* today.

## The `Game` struct is the world

All game state lives in one struct: `Game` in `src/core/game.rs`. It is threaded through
nearly every function as `g: &mut Game`. There are no globals, no singletons, no
`lazy_static` game state. If you want to know "what state exists", read the `Game` field
list — settings, input, levels, the entity arena, the display stack, tick counters, the
item/tile/recipe registries, the shared RNG.

The one thing deliberately *outside* `Game` is the `Renderer` (`src/core/renderer.rs`),
which owns the `Screen` framebuffers. That split lets render code take
`(&mut Screen, &Game)` while tick code takes `&mut Game`, without borrow fights.

## The loop

`src/main.rs` → `fdoom::run` (`src/lib.rs`) parses `--debug` / `--savedir`, builds
`Game`, and hands it to `platform::run`. `src/platform/mod.rs` is the only module that
touches winit/softbuffer/rodio; everything below it is windowless-capable.

```
platform::run (winit event loop, fixed 60 ticks/sec, fps-capped render)
 │
 ├─ demo.on_tick(g)              only when FDOOM_DEMO is set (scripted runs)
 ├─ Game::tick (src/core/game.rs — "makes everything keep happening")
 │   ├─ apply_menu_transition    the pending Set/Clear/Exit display change
 │   ├─ input.tick()             the ONE place key states advance
 │   ├─ with_entity(player_id)   player ticks every tick, menu open or not
 │   ├─ if menu open: tick_current_display()   (take-out: display popped, ticked, pushed)
 │   └─ else: level::tick_level(g, current_level)
 │        ├─ drain level.entities_to_add into g.entities (the arena)
 │        ├─ random tile ticks (w*h/50 tiles per tick → tile::dispatch::tick)
 │        ├─ for each entity on the level: g.with_entity(eid, entity_tick)
 │        │                                 └── the TAKE-OUT PATTERN, see below
 │        └─ try_spawn (mob spawning vs. the level's mob cap)
 │
 └─ Renderer::render (src/core/renderer.rs)
     ├─ render_level  tiles then entities, sorted by y (painter's algorithm)
     ├─ render_gui    active item + durability/arrows, health/stamina/hunger bars,
     │                notifications, save/sleep/score text
     └─ top display's render(), if any → blit, nearest-neighbor scaled to the window
```

### The take-out pattern (read this before touching entity code)

An entity's tick mutates both itself *and* the world. To make that borrow-safe, the
entity is **removed from the arena, ticked, then reinserted**:

```rust
g.with_entity(eid, |e, g| { /* e: &mut Entity, g: &mut Game, independently */ })
```

It nests: if A touches B, B is taken out too. Consequences you must respect:

- While taken out, the entity is invisible to arena queries — lookups return `None` and
  callers must no-op (this mirrors Java's `if (e == this) continue;`).
- Never call `g.player()` / `g.player_mut()` from inside the *player's own* tick; use the
  `&mut Entity` you already have. (`with_entity` also finds a player still sitting in a
  level's `entities_to_add` queue, so `player_id` stays usable during world init.)
- The display stack uses the same idea (`DisplayManager.taken_out` flag) so
  "is a menu open" checks stay correct while the top display is being ticked.

## Entities

`src/entity/mod.rs`: an entity is `EntityCommon` (x, y, collision radii, level index,
eid, removed flag) + `kind: EntityKind` — one enum variant per concrete Java class
(`Player`, `Zombie`, `Chest`, `ItemEntity`, ...). All live entities sit in one
`EntityArena` (`g.entities`), keyed by stable eid; "which level" is just a field.

Java's inheritance chain became **nested data + shared layer functions**:

```
ZombieData { enemy: EnemyMobData { ai: MobAiData { mob: MobData } } }
```

- Layer accessors on `Entity`: `e.mob()`, `e.mob_ai()`, `e.enemy_mob()`, `e.player()`,
  `e.furniture()` (+ `_mut` variants). `instanceof` checks are predicates:
  `e.is_enemy_mob()`, `e.is_furniture()`, ...
- Layer behavior is shared functions in `src/entity/behavior.rs`: `mob_tick_base`,
  `mobai_tick_base`, `enemy_mob_tick_base`, `do_hurt`, `entity_move`, ... A concrete
  mob's tick calls its parent layer's function where Java called `super.tick()`.
- **Dispatch hubs** (Java virtual methods) also live in `behavior.rs`: `entity_tick`,
  `entity_render`, `die`, `touched_by`, `entity_interact` — each a `match e.kind`
  fanning out to the per-kind module (`src/entity/mob/zombie.rs` etc.).

## Tiles

`src/level/tile/mod.rs`: tiles are **stateless**. The world is two byte arrays per level
(`Level { tiles: Vec<u8>, data: Vec<u8> }` — tile id and per-tile data). A `TileDef`
holds the per-class config (name, sprite, connects-to flags, `may_spawn`, ...) plus
`kind: TileKind` (one variant per Java tile class, with constructor args as fields, e.g.
`Stairs { leads_up }`, `Ore { ore_type }`).

- Registry: `Tiles::new()` in `src/level/tile/mod.rs` builds the id → `TileDef` table
  (ids 0–45 currently; torch variants register lazily at `on_tile.id + 128`). Stored in
  `g.tiles`; lookup by name (`g.tiles.get("Grass")`) or id (`get_id`).
- Behavior: `src/level/tile/dispatch.rs` — one function per Java virtual method
  (`render`, `tick`, `may_pass`, `hurt_by`, `interact`, `stepped_on`, `bumped_into`,
  `connects_to`, ...), each matching `TileKind` and calling the per-tile module
  (`src/level/tile/grass.rs`, `water.rs`, ...) or falling through to the default.
- `dispatch.rs` also owns `csprite_render`, the neighbor-aware "connector sprite"
  renderer used by grass/water/sand/etc. edges.

Saves store tiles **by name**, not id, so ids are an in-memory concern only.

## Items

`src/item/mod.rs`: `Item` is a plain cloneable value — name + sprite +
`kind: ItemKind` (`Tool { ttype, level, dur }`, `Stackable { count }`, `Food`,
`TileItem` (placeable), `Furniture` (boxes a whole `Entity`), ...). Inventories are
`Vec<Item>` (`src/item/inventory.rs`).

- Prototype registry: `build_registry` in `src/item/registry.rs` builds the full list
  once per game (order matters for the creative inventory). `registry::get(g, "name")`
  clones a prototype; unknown names return an `UnknownItem` rather than panicking.
  Names in save files/recipes are matched case-insensitively; `"name_3"` means count 3.
- Recipes: `src/item/recipe.rs` — `Recipe::new("Product_amount", &["Cost_n", ...])`
  string DSL, grouped into per-station lists in `Recipes::new()`.
- Use/interaction logic: `src/item/interact.rs`.

## Settings

`src/core/io/settings.rs` is a plain typed key/value store with a declared schema:
`KEYS` (key + label), `options_of` (legal values), `default_of` — one place to touch per
setting. Read with `g.settings.get("diff").as_str()` / `.as_int()` / `.as_bool()`;
`get_idx` gives the option index (used for difficulty scaling).

The UI side is `src/screen/settings_widgets.rs`: option screens build `ArrayEntry` menu
rows from the schema (`make_entry`) and `sync` edited values back into the store every
tick. The store knows nothing about widgets.

## Screens (displays)

`src/screen/display.rs`: `Display` is a trait (`init/tick/render/on_exit` + a
`DisplayBase` of menus). The ~25 screens (title, inventory, crafting, world gen, ...)
live in `src/screen/*.rs`. `DisplayManager` (on `g.display`) is an explicit stack:

- `g.set_menu(d)` pushes, `g.exit_menu()` pops to parent, `g.clear_menu()` clears —
  all *pending* until the top of the next `Game::tick` (Java's `newMenu` double-buffer,
  preserved because gameplay code checks "will a menu be open" mid-tick via
  `g.menu_open()`).
- Only the top display ticks/renders. It is ticked with the take-out pattern; the
  `taken_out` flag keeps `menu_active()` true meanwhile.
- Menu internals: `Menu`/`MenuBuilder` in `src/screen/menu.rs`; row types (`SelectEntry`,
  `ArrayEntry`, `InputEntry`, ...) in `src/screen/entry/`.

## Saves

`src/saveload/save.rs` + `load.rs`. Text format, comma-separated, inherited from the
Java game (extension `.miniplussave`). A world is a folder
`<gamedir>/saves/<worldname>/` containing:

- `Game` — version, tick counts, air-wizard-beaten, settings snapshot
- `Level0..Level5` — tile **names**, row-major; `Level0data..` — the data bytes
- `Player`, `Inventory` — position/health/potions; item names with counts
- `Entities` — one line per entity, `Name[x:...,y:...,...]`

Global (per-install, not per-world): `Preferences.miniplussave` (options + key
bindings — loaded at startup by `load_prefs`, saved when leaving the options screen) and
`Unlocks.miniplussave`. Save-dir per OS is in `src/core/file_handler.rs` (see
DEV_GUIDE).

## Rendering and the palette

Everything is software-rendered into `Screen.pixels: Vec<i32>` (288x192, `src/gfx/
screen.rs`). Pixels stay Java-signed `i32` throughout; conversion to `u32` XRGB happens
only at the platform blit. The window scales the framebuffer nearest-neighbor.

**Sprites are colored at draw time, not on the sheet.** `assets/icons.png` is a
grayscale sheet of 8x8 cells; each pixel is quantized to one of 4 shades (0=darkest,
3=lightest, `src/gfx/sprite_sheet.rs`). Every render call takes a `colors: i32` produced
by `color::get4(a, b, c, d)` (`src/gfx/color.rs`):

- Each of `a,b,c,d` is a "readable" color: decimal digits RGB 0–5 each, e.g. `520` =
  full red, ~half green, no blue. `-1` = transparent (that shade isn't drawn).
- `a` colors the darkest shade of the cell, `d` the lightest. So
  `get4(-1, 100, 320, 430)` means: darkest pixels transparent, then dark-red,
  orange-brown, light-brown.
- The 0–5 cube is mapped to actual RGB by `color::upgrade` at blit time.

So a "sprite" in code is a sheet position + a 4-byte palette; recoloring an item or mob
is just a different `get4` constant. **A true-RGB rendering overhaul is planned** — the
4-shade palette encoding above is the main thing it will replace, which is why new art
should not get too invested in palette tricks.

`Sprite`/`Px` (`src/gfx/sprite.rs`) address the sheet as `pos = x + y*32` (32 cells per
row). Text is `src/gfx/font.rs`, drawn from the same sheet.

## RNG

`src/rng.rs`: xoshiro256++ with a `java.util.Random`-shaped API (`next_int_bound`,
`next_gaussian`, ...). **Deterministic per seed** — world gen for a given seed is
reproducible (`tests/level_gen_determinism.rs`). World gen seeds its own instances from
`g.world_seed`; incidental randomness uses the shared `g.random`. The `rand` crate is
deliberately not a dependency.

## Where do I look to change X?

| X | Look in |
|---|---|
| Player movement / stamina / attack | `src/entity/mob/player_behavior.rs` |
| Mob AI / aggro / speed | `src/entity/behavior.rs` (`mobai_tick_base`, `enemy_mob_tick_base`) + the mob's module in `src/entity/mob/` |
| Damage/health/knockback | `src/entity/behavior.rs` (`do_hurt`, `mob_hurt_by_mob`, `heal`) |
| Mob spawning rates/rules | `src/level/mod.rs` (`try_spawn`, `MOB_SPAWN_FACTOR`, `update_mob_cap`) |
| World generation (terrain, ores, structures) | `src/level/level_gen.rs`, `src/level/structure.rs`, `src/core/world.rs` (level population) |
| HUD (hotbar, hearts, notifications) | `src/core/renderer.rs` (`render_gui`) |
| Menus / a specific screen | `src/screen/<screen>.rs`; shared widget logic in `src/screen/menu.rs` + `src/screen/entry/` |
| Title screen entries | `src/screen/title_display.rs` |
| Key bindings (defaults) | `src/core/io/input_handler.rs` (`init_key_map`) |
| Settings (new option, defaults) | `src/core/io/settings.rs` (`KEYS`, `options_of`, `default_of`) |
| Saves (format, what's persisted) | `src/saveload/save.rs`, `src/saveload/load.rs` |
| Save directory / file paths | `src/core/file_handler.rs` |
| Sounds | `src/core/io/sound.rs` + `src/assets.rs` |
| Sprites / colors / font | `src/gfx/` (`sprite.rs`, `color.rs`, `font.rs`), sheet at `assets/icons.png` |
| Day/night lengths, tick speed | `src/core/updater.rs` |
| Crafting recipes | `src/item/recipe.rs` |
| Item definitions | `src/item/registry.rs` |
| Tile behavior | `src/level/tile/dispatch.rs` → per-tile module |
