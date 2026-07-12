# fdoom.rs — Fossickers Doom

An original open-sandbox survival game in pure Rust (no engine): infinite
deterministic worlds, dig-based descent, fossicking/mining, weather, tides, rare
world events, day/night lighting with occlusion, and a fully original mob roster.
Began as a 1:1 port of the Java game "Fossicker" — **tag `v0.1.0` is the pure port**;
everything after evolves the game on its own terms.

## Read first
- `docs/HANDOFF_TODO.md` — ACTIVE work program: in-flight lanes, queued waves,
  acceptance criteria. If you're picking this project up, start there.
- `docs/ARCHITECTURE.md` — 15-minute codebase tour; `docs/TERRAIN.md`,
  `docs/ENTITIES.md`, `docs/ITEMS_AND_CRAFTING.md`, `docs/RENDERING_AND_UI.md`,
  `docs/CORE_AND_SAVES.md` — exhaustive per-system references.
- `docs/ART_GUIDE.md` (once the sprite decomposition lands) + `CONTRIBUTING.md` —
  house style. `docs/REQUESTS_AUDIT.md` / `docs/AUDIT_RESULTS.md` — request tracking.
- `docs/DEV_GUIDE.md` — daily commands, FDOOM_DEMO scripted runs, headless testing.

## Commands
- `cargo run` — play (add `-- --debug` for cheat keys).
- `just check` — fmt --check + clippy `-D warnings` + full test suite. MUST be green
  before every commit.
- `just --list` — all dev verbs (worldview, studio, seed, biome-map, soak, shots...).
- `cargo run --bin worldview -- <seed>` — world inspection window.
- `cargo run --bin pixel_studio` — the pixel art editor (sole art tool; artgen is
  deprecated/removed).

## Hard conventions
1. **No `// JAVA:` comments.** Comment only what a maintainer needs: non-obvious
   behavior, constraints, formula rationale. (The historical sweep is in progress —
   see HANDOFF_TODO 1a.)
2. **`g: &mut Game`** is the world-state root (no statics). Render fns take
   `(&mut Screen, &Game-ish)`, tick fns `&mut Game`. Screens/Renderer live outside.
3. **Entities**: `EntityCommon` + `EntityKind` enum, take-out tick pattern
   (`g.with_entity`); an entity is absent from the arena while it ticks.
4. **World generation is pure**: everything derives from `(seed, depth, x, y)` (+ day
   clock for time-varying systems like tides/weather/events). No `Date::now`/ambient
   randomness in gen. Chunk-border exactness is tested — preserve it.
5. **Randomness**: only `crate::rng::Rng` (+ the SplitMix hash helpers in
   `infinite_gen`). No `rand` crate.
6. **Art**: sprites live as individual PNGs under `assets/sprites/**` (stitched into
   a runtime atlas; see ART_GUIDE). Palette-mode sprites use ONLY grays 0/85/170/255
   (+ transparent); true-color pixels must never be pure gray. Player and classic-mob
   cells are pixel-traced from the original — NEVER redesign their anatomy. Terrain
   texture taste: calm base, sparse clustered detail — no uniform dither.
7. **Testing**: use `fdoom::testutil::TestWorld` for game-booting tests; visual
   changes are verified by screenshots you actually look at (`target/verify/`).
8. **No new dependencies** without updating PORTING.md. Platform code stays in
   `src/platform/`; tools are self-contained bins in `src/bin/`.
9. **Multi-agent discipline**: one lane per commit with an explicit file list (never
   `git add -A` with concurrent work); never two agents writing one file
   (`item/registry.rs` is the classic collision).
10. Saves: version-gated (3.0+); world shape is 5 layers (surface, 3 mines, dungeon).
    Old-save tolerance: unknown entity names skip with a warning, never panic.

## Product taste (user-established, enforce in reviews)
- **North star: DayZ / 7-Days-to-Die survival, not Minecraft.** The Minecraft and
  minicraft markets are flooded — this game carves its own slice: scavenge-and-
  survive loops, gather-chain crafting, cooking at fires, weather that matters,
  places with mood (cemeteries, ruins, hollow nights). When a design choice could
  go "blocky sandbox" or "grounded survival", pick grounded survival.
- Original flavor over clones: fossicking identity (pans, veins, cave-ins), invisible
  fish, dig-descent instead of stairs, stamina-draining ghosts. Inspired-by is fine;
  1:1 copies of other games' mechanics are not.
- World coherence is a feature: no snow beside sand, flora on its true ground,
  ragged organic shapes over squares, seams that blend. Oddities are bugs.
- Show, don't tell: every user-facing change ships with a screenshot.
- Sandbox: no win condition, survival-only, worlds are infinite and seed-described.
