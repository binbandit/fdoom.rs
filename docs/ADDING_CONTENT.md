# Adding Content

Step-numbered recipes, verified against the code. Items and crafting recipes are now
one-line changes; where a recipe still has many touch points (tiles, mobs), they are
listed exhaustively — that is the honest current cost, and this document doubles as
the spec for the remaining ergonomics work (a new tile or mob should eventually be
1–2 touch points, not 6+).

General rules:

- **Names are identity.** Items and tiles are looked up by case-insensitive name at
  runtime, and saves store names (not ids). Adding is safe; *renaming* breaks old saves
  and any recipe/drop that references the old name — lookups don't fail loudly (unknown
  item → `UnknownItem`, unknown tile → a log line + tile 0).
- Sprite coordinates and `color::get4` palettes are explained in
  [Sprite-sheet geography](#sprite-sheet-geography) at the bottom.
- After any change: `just check` (fmt + clippy + tests).

## New stackable / food item

All in `src/item/registry.rs`, inside `build_registry` (list order = creative-inventory
order; keep the new item next to its family):

1. Add a push using the family helper — name, sheet cell `(x, y)`, `get4` palette:
   ```rust
   items.push(stackable("Ruby", (10, 4), get4(-1, 400, 500, 511)));
   // or
   items.push(food("Carrot", (9, 4), get4(-1, 210, 430, 540), 2));
   ```
   `food(name, cell, colors, heal)` restores `heal` hunger points when eaten;
   `medical`, `armor`, `clothing`, `tile_item`, and `book` helpers sit right next to
   them for those families.
2. Make it obtainable: a crafting recipe (next section), a mob drop
   (`mobai_drop_items` in the mob's `die`), a tile drop (the tile's `hurt_by`), or a
   chest loot table (`src/entity/furniture/dungeon_chest*.rs` / `structure.rs`).
3. Done — `registry::get(g, "Ruby")` and saves work by name automatically. Verify with
   `--debug` + `SHIFT-G` (give all items) that it appears and renders.

## New tool

1. If it's a new *kind* of tool (not just a material tier): add a variant to `ToolType`
   in `src/item/tool_type.rs` — name, sheet sprite row, durability, plus the `VALUES`
   array. All six material tiers (Crude/Wood/Rock/Iron/Gold/Gem, tool levels 0–5;
   `TOOL_LEVEL_NAMES`) are generated automatically by the `ToolType::VALUES` loop in
   `build_registry` (`src/item/registry.rs`). Names are derived as
   `"{tier} {type}"` — `"Crude Axe"`, `"Gem Pickaxe"` — so that's what recipes and
   saves reference.
2. Give it behavior: tool effectiveness against tiles is per-tile — see each tile's
   `hurt_by`/`interact` (`tool_type == ToolType::Pickaxe` checks and the like). Attack
   damage bonuses are in `attack`/`get_attack_damage` in
   `src/entity/mob/player_behavior.rs`.
3. Add recipes for each tier you want craftable (next section — the existing tool
   recipes in `workbench`/`anvil` are the pattern).

## New crafting recipe

In `Recipes::new()` in `src/item/recipe.rs`:

1. Add one line to the list for the station that should offer it — `craft` (personal
   crafting, `Z`), `workbench`, `loom`, `oven`, `furnace`, `anvil`, or `enchant`:
   ```rust
   r("Ruby Ring", "Ruby*2 + gold"),
   ```
   Costs are `+`-separated; `*N` sets an amount (default 1) on the product or a cost
   — `r("Stick*2", "Wood")` means one Wood yields two Sticks. Duplicate cost entries
   are summed. (`r` is `Recipe::parse`; the Java `"Name_amount"` wire format lives on
   underneath in `Recipe::new` and in save files.)
2. The product and every cost must be real item names from `build_registry`
   (case-insensitive). A typo won't fail the build — it crafts an `UnknownItem` /
   never becomes craftable — but `tests/crafting_chain.rs` sweeps every recipe's
   product and costs against the registry, so `cargo test` catches it.
3. Respect the progression ladder when picking a station. The early game is a
   deliberate 7-Days-style gathering chain (see the comment atop `Recipes::new()`):
   - **Personal crafting** (`craft`, `Z`) is for things makeable bare-handed in a
     field: Cord (3 Grass Fibers), Sharp Stone (knapped from 2 Stone), Sticks from
     Wood, the Crude Axe/Pickaxe (Stick + Cord + Sharp Stone), the Fishing Rod, the
     Workbench itself, torches. No recipe here should conjure a finished
     wood/metal-tier tool from raw materials.
   - **Workbench and up** is for assembled goods. Tool recipes stay verbose: a
     handle (Sticks), a lashing (Cord), and the head material (Wood/Stone/metal).
     Follow the `r("Wood Axe", "Wood*5 + Stick*2 + Cord")` pattern for new tiers.

## New tile

Touch points today: **2 new-code sites + 3–4 wiring sites across 3 files.** (This is
the count the planned refactor should collapse.) Example: a "Mud" tile.

1. `src/level/tile/mod.rs` — declare the module and the class variant:
   - add `pub mod mud;` to the module list,
   - add `Mud` to the `TileKind` enum (constructor config becomes fields, e.g.
     `Sapling { on_type, grows_to }` — plain `Mud` if none).
2. Create `src/level/tile/mud.rs` with a `make` constructor and only the behavior
   functions the tile overrides (copy the closest existing tile; `snow.rs` is a simple
   sprite tile, `fence.rs` a minimal solid one):
   ```rust
   pub fn make(name: &str) -> TileDef {
       let mut t = TileDef::new(name, TileKind::Mud);
       t.sprite = Some(Sprite::dual_dots(...));   // or a csprite for edge-connecting tiles
       t
   }
   pub fn stepped_on(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, e: &mut Entity) { ... }
   ```
3. `src/level/tile/dispatch.rs` — wire it up:
   - add a constructor wrapper in the constructors block:
     `pub fn make_mud_tile(name: &str) -> TileDef { mud::make(name) }`
   - add a `TileKind::Mud => mud::...` match arm in **each** dispatch function the tile
     overrides (`render`, `tick`, `may_pass`, `hurt_by`, `interact`, `stepped_on`,
     `bumped_into`, `connects_to`, `get_light_radius`, ...). Unlisted functions fall
     through to the `Tile.java` default — that is fine and intended.
4. `src/level/tile/mod.rs`, in `Tiles::new()` — register it with a free id:
   `set(50, dispatch::make_mud_tile("Mud")); // ids 46-49 are the depth tiles`
   Ids 46–127 are free; **don't use 128+** (reserved for auto-registered torch
   variants at `on_tile.id + 128`) and never renumber existing ids (levels in memory
   index by id). Saves store tile *names*, so new ids need no migration.
5. Get it into the world: world gen (`src/level/level_gen.rs`) and/or a placeable item —
   `tile_item("Mud", (2, 4), get4(...), "mud", &["hole", "water"])` in `build_registry`
   (`model` = tile name to place, `valid_tiles` = names it can be placed on).
6. Verify: `--debug`, `SHIFT-G`, place it; check walking on/into it and hitting it.

## New mob

Touch points today: **1 new module + wiring in 4 files** (5 if it spawns naturally).
The layer-accessor step is the easy one to forget and fails *silently*. Example: an
enemy "Ghoul".

1. Create `src/entity/mob/ghoul.rs` — copy `zombie.rs`, the smallest enemy:
   - `static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(x, y));`
     (sheet cell of the first walk frame),
   - `LVLCOLS` (palette per mob level), a `GhoulData { enemy: EnemyMobData }` struct,
   - `pub fn new(g: &Game, lvl: i32) -> Entity` via `EnemyMobData::simple(...)`,
   - `pub fn tick` (usually just `enemy_mob_tick_base(g, e)`) and `pub fn die` (drops).
   Passive mobs nest `PassiveMobData` instead and follow `cow.rs`.
2. `src/entity/mob/mod.rs` — add `pub mod ghoul;`.
3. `src/entity/mod.rs`:
   - add `Ghoul(mob::ghoul::GhoulData)` to `EntityKind`,
   - add a `Ghoul` arm to **every layer accessor its layers participate in**:
     `mob()`, `mob_mut()`, `mob_ai()`, `mob_ai_mut()`, `enemy_mob()`,
     `enemy_mob_mut()`. These matches end in `_ => return None`, so a missed arm
     compiles fine and the mob just takes no damage / has no AI. Grep for
     `EntityKind::Zombie` in this file and mirror every hit.
4. `src/entity/behavior.rs` — dispatch hubs; again mirror `EntityKind::Zombie`:
   - `entity_tick` → `ghoul::tick`,
   - `entity_render` → `enemy_mob_render` (or a custom one),
   - `die` → `ghoul::die`,
   - `touched_by` → add to the enemy-mob variant list (contact damage).
5. `src/saveload/` — three name mappings, or the mob vanishes on save/load:
   - `save.rs`: `entity_class_name` → `EntityKind::Ghoul(_) => "Ghoul"`,
   - `load.rs`: the `is_enemy_mob_class` name list (enemy mobs only — it makes the
     loader read the mob level field),
   - `load.rs`: the name → constructor match (`"Ghoul" => Some(mob::ghoul::new(g, moblvl))`).
6. Spawning — any or all of:
   - natural spawns: `try_spawn` in `src/level/mod.rs` (the `rnd <= 40` band selection),
   - a spawner: the `FurnitureItem` block of `build_registry` in `src/item/registry.rs`
     (also gets it into the creative inventory),
   - structures/world gen: `src/core/world.rs` (`generate_spawner_structures`).
7. Verify headlessly with the test harness (spawn one next to the player, tick, assert
   it moves/hurts — `TestWorld` in DEV_GUIDE "Headless testing") and visually via a
   demo run.

## New sound

1. Drop the file into `assets/` — **WAV only** (rodio is built with just the `wav`
   feature; see `Cargo.toml`).
2. `src/assets.rs` — embed it:
   `pub const SOUND_THUNDER: &[u8] = include_bytes!("../assets/thunder.wav");`
3. `src/core/io/sound.rs` — three edits in one file: add a `Thunder` variant to the
   `Sound` enum, append it to `Sound::ALL`, and map it in `wav_bytes`.
4. Play it from anywhere with `g.play_sound(Sound::Thunder);` (respects the sound
   setting; each sound has one channel — replaying restarts it). Looping:
   `g.play_sound_loop(Sound::Thunder, true)`.

## Sprite-sheet geography

One sheet: `assets/sprites.png`, generated by `cargo run --bin artgen`
(`src/bin/artgen.rs` is the source of truth — never edit the PNG), embedded at build
time (`src/assets.rs`), decoded in `src/gfx/sprite_sheet.rs`.

- The grid is **8x8-pixel cells**. A cell is addressed as `pos = x + y * 32` (32 cells
  per row); `Sprite::new1x1(x, y, colors)` takes the cell coordinates directly. Tiles
  are 2x2 cells (16x16 px), mobs 2x2 per animation frame
  (`compile_mob_sprite_animations(x, y)` in `src/gfx/sprite.rs` slices the standard
  down/up/left/right walk-frame block starting at cell `(x, y)`).
- The sheet is hybrid: gray pixels (`r==g==b`) are quantized to **4 shades** on load
  (`value / 64` → 0..3) and recolored per draw call; any non-gray pixel is true color,
  drawn literally. artgen's `G0..G3` inks are the four canonical grays.
- Color comes at render time from `color::get4(a, b, c, d)`: `a` recolors shade 0
  (darkest) ... `d` shade 3 (lightest); each is an RGB digit triple 0–5 per channel
  (`430` = r4 g3 b0), `-1` = that shade is transparent. One sprite + different `get4`
  constants = all the tool tiers, wool colors, mob levels, etc.
- Row landmarks (cell y): items mostly rows 4–5 and 12, menu-frame pieces row 13, mob
  walk frames rows 14–22, the font at row 30. Tiles are scattered above that — check the
  sprite constants of the tile/item you copy from rather than guessing.
- The sheet is 256x256 (32x32 cells); `pos = x + y*32` addresses all of it. Free-cell
  bookkeeping lives in `src/bin/artgen.rs`'s header comment and
  `tests/artgen_sheet.rs`'s `INVENTORY`. (The legacy Java sheets `icons.png` /
  `icons_ale.png` / `logo.png` were removed once everything worth keeping was traced
  into artgen — see git history.)

## Adding a biome (infinite worlds)

Biomes live entirely in `src/level/infinite_gen.rs`:

1. Add a variant to `enum Biome`.
2. Give it a region in `biome_at` — carve a slice of the temperature/moisture space
   (fields are continental-scale: period 384-512 tiles, so regions come out large).
3. Add its ground-cover arm in `surface_tile` — pick tiles per-position using `detail`
   scatter and mid-frequency `fractal` masks (see Marsh pools / Forest clearings).
4. If stairwell aprons should use a different ground tile, extend the gate-apron/pad tile selection in `generate_chunk`.
5. Run `cargo test level` (`biomes_are_large_and_all_present` will fail until the new
   biome actually appears) and eyeball it: `cargo test --test biome_frames` dumps
   rendered frames per biome into `target/verify/`.
