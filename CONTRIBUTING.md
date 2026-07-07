# Contributing

The short version of how this codebase wants to be worked on. The deep docs are in
`docs/` — [ARCHITECTURE.md](docs/ARCHITECTURE.md) (what is where),
[DEV_GUIDE.md](docs/DEV_GUIDE.md) (commands, scripted runs, cheat keys),
[ADDING_CONTENT.md](docs/ADDING_CONTENT.md) (item/tile/mob recipes) — and
`PORTING.md` explains the Java heritage. This file is the house style.

## Before you push

```sh
just check   # cargo fmt --check + clippy --all-targets -D warnings + cargo test
```

All three must be clean. No new dependencies without updating PORTING.md.

## The shapes to follow

- **All game state lives in `Game`**, threaded as `g: &mut Game`. No globals, no
  singletons. Render fns take `(&mut Screen, &Game)`; tick fns take `&mut Game`.
- **Dispatch hubs, not traits-per-thing.** Entity and tile "virtual methods" are one
  `match` per method in `src/entity/behavior.rs` / `src/level/tile/dispatch.rs`,
  fanning out to per-kind modules. Add an arm; don't invent a new dispatch mechanism.
  Shared "super class" behavior is a plain function on the parent layer
  (`mob_tick_base`, ...), called explicitly.
- **World gen is pure.** Anything that decides what exists at a coordinate must be a
  pure function of `(seed, depth, x, y)` + fixed salts — no `g.random`, no order
  dependence (chunks generate in any order, on any session). See docs/TERRAIN.md.
- **Randomness** is only `crate::rng::Rng`. World gen seeds its own instances from the
  world seed; incidental gameplay randomness uses `g.random`. Never the `rand` crate.
- **Sprite-sheet cells are palette or true-color, never mixed carelessly.** Palette
  (grayscale) cells are recolored at draw time via `color::get4` — items, mobs, UI,
  anything with variants. True-color cells draw as-is — painterly scenery, logos.
  `tests/artgen_sheet.rs` enforces which is which; the sheet itself is generated
  (`just sheet`), never hand-edited. See docs/RENDERING_AND_UI.md.
- **Names are identity.** Items and tiles are looked up by case-insensitive name, and
  saves store names. Adding is safe; renaming breaks saves and recipes silently.

## Tests

- Integration tests live in `tests/`, headless — the game core never touches the
  platform layer. Boot through `fdoom::testutil::TestWorld` (see DEV_GUIDE
  "Headless testing"); don't re-write the world-boot boilerplate.
- Pure-generation tests (`structures_gen`, `level_gen_determinism`, ...) call the gen
  functions directly with a `Tiles::new()` — no `Game` needed.
- Visual output goes to `target/verify/` (`testutil::verify_path` / `screenshot`);
  `just shots` regenerates and upscales everything there for eyeballing.
- New behavior gets a test that would fail without it; registry/recipe changes are
  guarded by the `crafting_chain` sweep.

## Commits

Conventional-commit style, imperative, scoped where it helps:

```
feat(worldgen): old trails, destroyed villages, boulders
fix(art): snow footprints print cool blue-gray, not tan
docs: creative roadmap — events, mystical/bountiful layers
```

One logical change per commit. If behavior intentionally diverges from the Java
original, say so in the body (the `// JAVA:` comment convention covers the code side).
