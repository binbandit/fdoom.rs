# Fossickers Doom (fdoom.rs)

A Minicraft-Plus-style 2D top-down survival game, written in pure Rust: software
renderer, infinite procedurally generated worlds (surface, three mine layers, and a
dungeon set-piece), gather-chain crafting,
potions, and an Air Wizard to kill. No engine, no GPU — just `winit` + `softbuffer` +
`rodio` and a 288x192 pixel framebuffer.

The project began as a faithful 1:1 port of the Java game
[Fossicker](https://github.com/binbandit/Fossicker). **Tag `v0.1.0` is the pure port**
(byte-identical rendering and world gen versus the JVM). Everything after that tag
evolves the game on its own terms: modernized controls, a new RNG, inherited bugs fixed.
`PORTING.md` documents the port-era architecture decisions; they still describe the code.

## Quickstart

```sh
cargo run                # play the game
cargo run -- --debug     # play with cheat keys enabled (see docs/DEV_GUIDE.md)
cargo test               # unit + headless render tests
just --list              # all dev tasks (build checks, scripted demo runs, ...)
```

Requires stable Rust 1.85+. No system dependencies beyond a working audio output
(the game runs fine without one — it just logs and goes silent).

## Controls

Defaults live in `init_key_map` in `src/core/io/input_handler.rs`; rebind in-game under
Options → Change Key Bindings.

| Action | Key(s) |
|---|---|
| Move | `W A S D` or arrow keys |
| Attack / use item | `SPACE` or `C` |
| Inventory | `E` or `I` |
| Craft (personal crafting) | `Z` or `SHIFT-E` |
| Stash held item (back into inventory) | `X` |
| Pick up furniture (power glove) | `V` |
| Drop one / drop stack | `Q` / `SHIFT-Q` |
| Save world | `R` |
| Map | `M` |
| Potion-effects readout | `P` |
| Player info screen | `SHIFT-I` |
| Pause / close menu | `ESCAPE` |
| Menu select / accept | `ENTER` |
| FPS + debug overlay | `F3` |

Debug-gated bindings (only active with `--debug`): `N` skip to night,
`SHIFT-S`/`SHIFT-1` survival mode, `SHIFT-C`/`SHIFT-2` creative mode — plus a set of
unmapped cheat keys (time of day, game speed, give items, ...) listed in
[docs/DEV_GUIDE.md](docs/DEV_GUIDE.md#debug-cheat-keys).

> Upgrading from an older build? A stale `Preferences.miniplussave` keeps the *old*
> keybindings. See [docs/DEV_GUIDE.md](docs/DEV_GUIDE.md#troubleshooting).

## Features

- **Infinite worlds**: Minecraft-style chunked terrain streamed around the player,
  deterministic per seed, with large natural biomes, world structures (ruins, decaying
  cemeteries, destroyed villages, old trails), and dig-based descent between layers
  (no pre-placed stairs — you dig down and climb back on the ladder you leave).

- Procedural island/box/mountain/irregular worlds, 128–512 tiles square, seedable
- 5 vertical levels: surface, three mine layers, and a dungeon reached through deep-mine gates
- Day/night cycle with mob spawning, beds, farming, 46 tile types, ~150 items
- **Progression**: a 7-Days-style survival start — punch tall grass for fibers and
  loose stones, punch trees for sticks; twist fibers into cord, knap stone sharp, and
  lash together your first crude axe and pickaxe with bare hands. Real wood/stone
  tools then need a workbench (and cord), and the metal tiers an anvil beyond that.
- Crafting stations: workbench, oven, furnace, anvil, enchanter, loom
- Open-sandbox survival (Creative remains as a --debug tool), three difficulties,
  selectable day-cycle pacing up to realtime, rare deterministic world events
- Save/load in the original Java-compatible text format, autosave
- Localization (English, Italiano, Norsk)
- Fully headless-capable core: tests tick the world and render PNGs without a window

## Project layout

```
src/
  core/        Game struct (all game state), tick loop, renderer front-end, file paths
  core/io/     input handler, settings store, sound, localization
  entity/      EntityCommon + EntityKind; mob/, furniture/, particle/ modules;
               behavior.rs = dispatch hubs (tick/render/die/...)
  level/       Level (tile arrays), world gen, structures; tile/ = TileDef/TileKind,
               per-tile modules, dispatch.rs hubs
  item/        Item/ItemKind, prototype registry, recipes, inventories
  screen/      Display trait + ~25 screens, Menu widget, entry/ row types
  gfx/         software renderer: Screen, Sprite, SpriteSheet, color math, font
  saveload/    save/load of worlds, player, preferences
  platform/    the ONLY module touching winit/softbuffer/rodio; also the FDOOM_DEMO
               scripted-run driver
  rng.rs       deterministic xoshiro256++ RNG
assets/        sprite sheet, sounds, localization files (embedded via include_bytes!)
tests/         headless integration tests
```

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — guided tour of the codebase (15 min)
- [docs/DEV_GUIDE.md](docs/DEV_GUIDE.md) — daily commands, scripted demo runs, headless
  testing, cheat keys, troubleshooting
- [docs/ADDING_CONTENT.md](docs/ADDING_CONTENT.md) — step-by-step recipes for new items,
  recipes, tiles, mobs, sounds, sprites
- [PORTING.md](PORTING.md) — the Java→Rust port design (history + architecture rationale)
