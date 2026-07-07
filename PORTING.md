# Fossickers Doom — Java → Rust Port

This repository began as a faithful 1:1 port of
[Fossicker](https://github.com/binbandit/Fossicker) (a Minicraft-Plus-derived 2D top-down
game, Java package `fdoom`, version "2.6") to Rust. **The tag `v0.1.0` marks the pure
conversion**; development after that tag deliberately evolves beyond the Java original
(cleanups, modernized controls, infinite worlds), so "faithful" below describes the ported
baseline, not a constraint on new work. For a present-tense tour of the codebase (rather
than the port rationale), see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

"Faithful" means: identical rendering (same software renderer, same palette math, same
sprite sheet), identical game logic (same tick rates, same formulas, same RNG behavior for
world generation), identical save format, and identical controls. Where the Java code has
quirks (commented-out light overlay, odd loop bounds like `for (s = x2; s < w - s; s++)`),
we preserve the quirk and mark it with a `// JAVA:` comment rather than "fixing" it.

## Stack

| Concern            | Java                              | Rust                                   |
|--------------------|-----------------------------------|----------------------------------------|
| Window/events      | AWT `Canvas` + `JFrame`           | `winit` 0.30                            |
| Framebuffer blit   | `BufferedImage`+`BufferStrategy`  | `softbuffer` (CPU nearest-neighbor scale)|
| Audio              | `javax.sound.sampled.Clip`        | `rodio` 0.22 (one restartable sink per sound)|
| Sprite sheet decode| `ImageIO.read`                    | `png` crate, assets embedded via `include_bytes!`|
| RNG                | `java.util.Random`                | `Rng` (xoshiro256++, `src/rng.rs`) — post-v0.1.0; the tagged v0.1.0 release used an exact `java.util.Random` port |

Everything else — the renderer, font, tiles, entities, items, menus, world gen, save/load —
is a direct port with no engine dependency. The game core never touches winit/softbuffer/rodio
directly; `src/platform/` is the only module that does. This means the whole game can run
headless (tests can tick the world and render frames to PNG without a window).

## Module map (Java package → Rust module)

| Java                       | Rust                    |
|----------------------------|-------------------------|
| `fdoom.core.Game/Initializer/Updater/Renderer/World` | `src/core/` (one `Game` struct replaces the statics; see below) |
| `fdoom.core.io.*`          | `src/core/io/` (input, settings, sound, localization) plus `src/core/file_handler.rs` |
| `fdoom.gfx.*`              | `src/gfx/`              |
| `fdoom.level.*`            | `src/level/` (`tile/` submodule) |
| `fdoom.entity.*`           | `src/entity/` (`mob/`, `furniture/`, `particle/` submodules) |
| `fdoom.item.*`             | `src/item/`             |
| `fdoom.screen.*`           | `src/screen/` (`entry/` submodule) |
| `fdoom.saveload.*`         | `src/saveload/`         |
| `fdoom.network.*`          | `src/network/` — stubbed: singleplayer checks (`is_valid_server()` etc.) exist and return `false`; see "Multiplayer" below |

## Core design decisions

### 1. One `Game` struct instead of Java statics

Java scatters global state across `Game`/`Updater`/`Renderer`/`World`/`Settings` statics.
Rust gathers all of it into a single `Game` struct that is threaded through the code as
`&mut Game` (conventionally named `g`). Java `Game.foo` → Rust `g.foo`; `Updater.tickCount`
→ `g.tick_count`; `Settings.get("diff")` → `g.settings.get(...)`.

The `Screen`s live in the `Renderer` struct *outside* `Game`, so render methods can take
`(&self/&mut data, &mut Screen, &Game)` while tick methods take `&mut Game`.

### 2. Entities: one arena, take-out-to-tick

All entities live in one arena: `g.entities: EntityArena` (slab of `Option<Entity>` keyed by
stable `Eid`). An entity's Java `level` field becomes `level: Option<usize>` (index into
`g.levels`). Level entity queries filter the arena by level index — Java did linear scans of
a per-level set anyway.

`Entity` is a struct of common data (`EntityCommon`: x, y, xr, yr, removed, level, eid …)
plus `kind: EntityKind`, an enum with one variant per Java class
(`Player`, `Zombie`, `ItemEntity`, `Chest`, …). Java inheritance chains become nested data
structs (`Zombie` has `EnemyMobData` has `MobAiData` has `MobData`), and `super.tick()`
becomes a call to the shared function for that layer (`mob_tick(...)`). Java `instanceof X`
becomes `matches!`/helper predicates (`is_enemy_mob()` etc.).

Because an entity's `tick` mutates both itself *and* the world (moving triggers
`touchedBy` on others, tiles get stepped on, items get dropped…), we tick with the
**take-out pattern**: the entity is removed from the arena, ticked as
`tick(&mut self, g: &mut Game, self_id: Eid)`, then reinserted (unless removed). The helper
is `g.with_entity(eid, |e, g| …)`; it nests, so A-touches-B interactions take B out too.
While taken out, the entity is absent from arena queries — which matches the Java
`if (e == this) continue;` checks. Code that looks up an entity that is currently taken out
gets `None` and must no-op (Java had equivalent reentrancy hazards).

**The player** is an ordinary arena entity; `g.player_id` names it. `g.player()` /
`g.player_mut()` fetch it (panicking accessors mirror Java NPE semantics; `try_` variants
exist where Java null-checked). Inside `Player::tick`, "Game.player" is just `self`.

### 3. Tiles: stateless registry, ids are data

`Tiles` is a `Vec<Option<TileDef>>` registry indexed by tile id, built once (`tiles::init`),
immutable afterwards, stored in `g.tiles` behind an `Arc` (cloned handles are cheap). A
`TileDef` holds the per-instance config Java passed to constructors (name, connects-to
flags, material, the wrapped sprite/csprite, …) plus `kind: TileKind` for behavior dispatch.
Tile state lives where Java kept it: in `Level { tiles: Vec<u8>, data: Vec<u8> }`.
Tile methods take `(&self, g: &mut Game, lvl: usize, x: i32, y: i32, …)`.

### 4. Items: value objects

`Item` is a cloneable struct: common data (name, sprite) + `kind: ItemKind` enum
(`Tool { tool_type, level, dur }`, `Stackable { count }`, `Furniture(Box<Entity>)`, …).
The `Items::get(name)` registry-by-prototype becomes `items::get(name) -> Item` building
from a prototype table. Inventories are `Vec<Item>` exactly like Java.

### 5. Displays/menus: trait objects on an explicit stack

`Display` is a trait (`init/tick/render/on_exit/parent`), implementations are the ~25
screens. The Java `menu`/`newMenu` double-buffered current-display mechanic is preserved
verbatim in `g.display` (a small state machine applied at the top of `Updater::tick`).
Displays are stored outside the borrow of `Game` while ticked (same take-out idea).
The `Menu` widget class and `ListEntry` hierarchy port as a struct + `enum`/trait like
entities.

### 6. Rendering is `i32` all the way

Java pixel ints are signed; the color math (`Color.java`) relies on it (e.g. `-1` means
transparent, `0xFFFFFFFF` is `-1`). `Screen.pixels` is `Vec<i32>`; conversion to `u32`
XRGB happens only in the platform blit. Color functions are ported bit-for-bit and covered
by unit tests with values captured from the Java implementation.

### 7. Randomness

Until v0.1.0, `java.util.Random` was re-implemented bit-for-bit so worlds were
byte-identical to the JVM for a given seed (verified against 12 JVM dumps). Java-save/seed
compatibility was then dropped by request; `src/rng.rs` (xoshiro256++) replaces it with the
same ergonomic API. Generation is still fully deterministic per seed
(`tests/level_gen_determinism.rs`). Incidental randomness uses the shared `g.random`.

### 8. Threads → incremental state machines

Java spawns threads for world gen/loading so the loading bar animates. The Rust port keeps
one thread and instead runs generation incrementally from `LoadingDisplay::tick` (one level
per tick step), updating the same percentage counter. Identical visible behavior, no shared
mutable state across threads.

## Multiplayer

The Java tree contains a socket-based client/server (`fdoom.network`, ~1900 lines) carried
over from Minicraft Plus. The port keeps every `ISONLINE / isValidServer() /
isValidClient() / isConnectedClient()` call site so game logic reads the same as Java, but
`src/network/` is a stub in which those all return `false` — i.e. the game is always in
singleplayer mode, and `MultiplayerDisplay` reports that multiplayer is not available in
this build. All other 175+ Java classes are fully ported. The call-site preservation means
a real network layer can be added behind the same functions without touching game logic.

## Preserved quirks (deliberate, do not "fix")

- The cave-darkness/light overlay in `Renderer.renderLevel` is **commented out in the Java
  fork**; the port keeps it disabled (code present, `if false`-gated) to match.
- `Tile.tickCount` (global), odd loop bounds in `Level.checkChestCount` /
  `generateSpawnerStructures`, the unused `visible`/`updateVisible` map logic, misc unused
  fields — preserved.
- Typos in user-visible strings are preserved (savefile/localization compatibility).

## Verification

- `cargo test` — unit tests for color math, JavaRandom vs JVM-captured vectors, font
  wrapping, world-gen smoke tests, save/load round-trip.
- Headless render tests dump frames to PNG (no window needed) for visual comparison.
- `cargo run` — the real game.
