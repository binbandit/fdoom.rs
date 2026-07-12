# Terrain & World Generation

Exhaustive reference for fdoom.rs's world/terrain systems: chunked storage, the two
generators (infinite pure-function and classic finite noise), multi-level dig-based
descent, and save persistence. See also [ARCHITECTURE.md](ARCHITECTURE.md) for the
whole-codebase tour and [ADDING_CONTENT.md](ADDING_CONTENT.md) for the tile/item content
recipes this document extends with terrain-specific ones (§7).

Every claim below is grounded in the source as of this writing; file:line references are
approximate anchors (line numbers drift), not guarantees — grep the quoted symbol if a
number is stale.

## 1. Overview + mental model

Two generators coexist:

- **`src/level/infinite_gen.rs`** — the default ("Infinite" world type). Every tile is a
  **pure function of `(world_seed, depth, x, y)`**: `generate_chunk(seed, depth, cx, cy,
  tiles)` can be called for any chunk, in any order, on any machine, and always produces
  the same bytes. No RNG stream, no mutable generation state, no history of what chunks
  were generated before. This is what makes "unbounded world, streamed on demand,
  saved/unloaded independently" possible at all.
- **`src/level/level_gen.rs`** — the classic finite generator ("Classic" world type),
  a direct port of Java `LevelGen`/`HistoryGen`. It builds one full `w×h` tile array up
  front using a shared mutable `Rng` stream (order-dependent, not position-evaluable).
  Still used for Classic worlds and *always* used for the dungeon (depth -4), which has
  no infinite equivalent (§6).

```
 depth   IDX_TO_DEPTH   Java name   infinite?              generator
 ─────   ────────────   ─────────   ─────────              ─────────
   0          [3]        Surface    yes (Infinite worlds)   infinite_gen::surface_tile
  -1          [2]        Iron       yes                     infinite_gen::mine_tile (depth -1)
  -2          [1]        Gold       yes                     infinite_gen::mine_tile (depth -2)
  -3          [0]        Lava       yes                     infinite_gen::mine_tile (depth -3)
  -4          [4]        Dungeon    NEVER (finite only)      level_gen::create_dungeon
```

`Level.chunks: Option<ChunkMap>` is the single switch: `Some` = chunked/infinite storage,
`None` = classic finite `Vec<u8>` storage. Every tile accessor on `Level` (`tile_id`,
`get_data`, `set_data`, `set_tile_id`) branches on this, so tile/entity/render code never
needs to know which mode a level is in — see §2.

ASCII picture of one infinite layer's chunk grid around the player (`@`), with the two
concentric radii from `src/level/chunk.rs`:

```
   ┌───────────────────────────────┐  UNLOAD_RADIUS = 4 chunks
   │   ┌───────────────────┐       │  (chunks farther than this get
   │   │   ┌───────┐       │       │   saved to disk and dropped)
   │   │   │   @   │       │       │
   │   │   │ LOAD  │       │       │  LOAD_RADIUS = 2 chunks
   │   │   │ RADIUS│       │       │  (always generated/loaded)
   │   │   └───────┘       │       │
   │   └───────────────────┘       │  chunks strictly between the two
   └───────────────────────────────┘  radii just stay resident, untouched
```

Each chunk is `CHUNK_SIZE × CHUNK_SIZE` = 64×64 tiles (`CHUNK_SHIFT = 6`). Tile coords are
global `i32`; `chunk_coord(tile) = tile >> CHUNK_SHIFT`.

## 2. Chunked storage (`src/level/chunk.rs`)

```rust
pub const CHUNK_SHIFT: i32 = 6;
pub const CHUNK_SIZE: i32 = 1 << CHUNK_SHIFT;   // 64
pub const LOAD_RADIUS: i32 = 2;                 // chunks always generated/loaded
pub const UNLOAD_RADIUS: i32 = 4;               // chunks beyond this get saved + dropped
```

`Chunk` holds four parallel arrays, all `CHUNK_SIZE² = 4096` entries:

| Field | Type | Purpose |
|---|---|---|
| `tiles` | `Vec<u8>` | tile id per cell (same id space as classic `Level.tiles`) |
| `data` | `Vec<u8>` | per-tile data byte (dig stage, flower facing, ore variant, ...) |
| `visible` | `Vec<bool>` | map fog-of-war: has the player ever seen this tile |
| `dirty` | `bool` | needs saving before unload |

`ChunkMap` is a `HashMap<(i32, i32), Chunk>` keyed by chunk coordinates. `local(tile) =
tile & (CHUNK_SIZE - 1)` gets the in-chunk index; note this uses `&`, not `%`, which is
why it round-trips correctly for negative tile coordinates (Rust's `%` would return a
negative remainder; `&` on the two's-complement mask does not) — see the `coords_round_trip`
unit test in `chunk.rs`.

**Accessors and the "unloaded = rock" fallback.** `ChunkMap::tile`/`data`/`is_visible`
return `Option`/default-false when the chunk isn't resident. `Level::tile_id` passes that
`None` straight through; the one place that turns it into a concrete tile is
`Game::tile_at` (`src/level/mod.rs`):

```rust
pub fn tile_at(&self, lvl: usize, x: i32, y: i32) -> Rc<TileDef> {
    match self.levels[lvl].as_ref().and_then(|l| l.tile_id(x, y)) {
        Some(id) => self.tiles.get_id(id as i32),
        None => self.tiles.get("rock"),   // <-- the fallback
    }
}
```

So querying arbitrarily far outside the loaded ring (or off the edge of a finite level)
always reads as solid rock rather than panicking — this is deliberate and load-bearing:
`tests/infinite_world.rs` asserts `g.tile_at(3, 100_000, -100_000)` doesn't panic.

**Streaming (`ensure_chunks` / `ensure_chunks_at`, `src/level/mod.rs`).** Called every
tick (`ensure_chunks`, driven off the player's current level and tile position) and also
directly with an arbitrary tile position (`ensure_chunks_at`, used by chasm-digging to
force-load the destination chunk on the layer below, and by any title/flyover camera).
Each call:

1. Computes the player's chunk coords `(pcx, pcy)`.
2. For every chunk within `LOAD_RADIUS` not already loaded: try `saveload::save::load_chunk`
   from disk first, fall back to `infinite_gen::generate_chunk(seed, depth, cx, cy,
   &g.tiles)` if there's no save file. Insert into the map.
3. For every loaded chunk farther than `UNLOAD_RADIUS` (Chebyshev/max-coordinate distance,
   not Euclidean): remove it; if `chunk.dirty`, `save_chunk` it first, otherwise just drop
   it (it can be regenerated byte-identically later, so there is nothing to lose).

Chunks in the ring *between* `LOAD_RADIUS` and `UNLOAD_RADIUS` are simply left alone —
neither generated nor evicted — which is why the two radii are distinct constants rather
than one: it avoids thrashing load/unload at the boundary when the player oscillates near
an edge.

**Dirty tracking**: only `set_tile`/`set_data`/first-time `mark_visible` flip `dirty =
true`. A chunk that generates, gets looked at, and never changes is written to disk only
once fog-of-war first touches it — mark_visible only sets dirty on the *first* reveal of
each tile (`if !c.visible[i] { ...; c.dirty = true; }`), so re-walking already-seen ground
doesn't keep re-marking the chunk dirty.

**Memory lifecycle summary**: generate/load on approach → tick with the game → mark dirty
on any tile write or newly-seen tile → save + drop once far enough away → regenerate or
reload identically if the player comes back.

## 3. Generation pipeline (`src/level/infinite_gen.rs`)

### 3.1 Noise primitives

```rust
fn hash(seed: i64, salt: u64, x: i32, y: i32) -> u64   // SplitMix64-style avalanche
fn unit(h: u64) -> f64                                  // -> uniform [0, 1)
fn value_noise(seed, salt, x, y, period) -> f64         // bilinear + smoothstep lattice noise
fn fractal(seed, salt, x, y, base_period, octaves) -> f64  // octaved value_noise, normalized to [0,1)
```

`hash` folds `seed`, `salt`, and the packed `(x, y)` through fixed-point multiply/xor-shift
constants (`0x9E3779B97F4A7C15`, `0xBF58476D1CE4E5B9`, `0x94D049BB133111EB` — standard
SplitMix64/MurmurHash3-finalizer-style constants). `value_noise` hashes the four lattice
corners of a `period`-tile cell and bilinearly interpolates with a smoothstep fade (`3t² -
2t³`) for C¹-continuous terrain. `fractal` sums `octaves` layers of `value_noise` at
halving periods and halving amplitude (classic fBm), normalized by total amplitude so the
result stays in `[0, 1)` regardless of octave count.

**Every noise call takes a `salt`** — a distinct `u64` per logical field, so that e.g. the
temperature field and the moisture field are decorrelated even though they share the same
`hash` function and the same seed. Table of every salt currently in use:

| Salt | Field | Function | Period | Octaves | Notes |
|---|---|---|---|---|---|
| `1` | continent | `biome_at` | 384 | 3 | land/ocean base signal |
| `6` | climate / temperature | `biome_at` / `surface_tile` | 512 | 1 / 2 | one field, two reads: the 1-octave `climate_at` drives the Tundra/Desert/Savanna gates (smooth ⇒ wide temperate buffers, §3.2); the 2-octave `temperature` variant survives only in the Mountains snow-cap arm |
| `2` | moisture | `biome_at` | 448 | 2 | drives Marsh/Forest/Savanna/Plains |
| `5` | rough (ruggedness) | `biome_at` | 48 | 4 | perturbs coastline + gates Mountains |
| `9` | belt | `biome_at` | 320 | 2 | mountain range mask |
| `3` | detail | `surface_tile` | n/a (raw `hash`, not `fractal`) | n/a | per-tile scatter roll (trees/flowers/tufts/cactus/snow-rock) |
| `4` | tuft variant | `surface_tile`'s `tuft()` closure | n/a (raw `hash`) | n/a | picks among the 3 tall-grass tile ids |
| `7` | pool | `surface_tile` (Marsh) | 14 | 2 | blobby marsh water pools |
| `8` | clearing | `surface_tile` (Forest) | 24 | 2 | forest clearings (lower tree density) |
| `10` | parched | `surface_tile` (Savanna) | 18 | 2 | dry sand patches in Savanna |
| `10 + \|depth\|` | cave | `mine_tile` | 32 | 4 | carved-space vs solid-rock mask; salt is depth-dependent (11/12/13 for depth -1/-2/-3) |
| `salt + 90` | detail | `mine_tile` | n/a (raw `hash`) | n/a | ore-type / dirt-vs-lava-vs-water roll |
| `salt + 40` | vein | `mine_tile` | 12 | 2 | ore vein mask inside solid rock |
| `salt + 70` | pocket | `mine_tile` | 24 | 2 | lava pocket / underground water mask in open cave floor |
| `0x6A7E6A7E6A7E6A7E ^ depth.unsigned_abs()` | gate | `gate_in_cell` | n/a (raw `hash`, `GATE_GRID`-cell) | n/a | dungeon gate presence + jitter, depth -3 only |
| `0xF055_1C4E` | richness | `richness_at` | 96 | 2 | fossicking: shared mineral-richness field, every depth (§4.5) |
| `0x5EE9` | stain | `mineral_stain_at` | n/a (raw `hash`) | n/a | per-tile mineral-seep stain roll on rich surface rock |
| `0x5EA0` | skerry | `skerry_at` | n/a (raw `hash`, 2x2 cells) | n/a | ocean rock stacks (sea stacks in open water) |
| `0xC4AC_4ED0` | rock character | `fossick::rock_character` | n/a (raw `hash`) | n/a | runtime field (not gen): cracked/dense rock per position (§4.5) |
| `0x484D_4C45_5421_0006` | hamlet placement | `structures_gen::spec(Hamlet)` | n/a (raw `hash`, 320-tile cells) | n/a | towns wave: the little-town placement grid between the rare villages |
| `0xA6ED_70B1_0007` | town age | `structures_gen::town_age` | n/a (raw `hash`, per placement) | n/a | Overgrown / Weathered / Settled axis for hamlets + villages |
| `0x484D_0001`..`0x484D_0007` | hamlet detail | `hamlet_buildings` + Hamlet blueprint | n/a (raw `hash`) | n/a | house slots/sizes, lane wear, green tufts, hamlet paving |
| `0x6F76_0001`..`0x6F76_0003` | overgrowth / garden | `stamp_house` / town ground / `stamp_garden` | n/a (raw `hash`) | n/a | flora reclaiming Overgrown floors + Settled picket rot |
| `0x0A6E_D001` | surviving lantern | `lantern_positions` | n/a (raw `hash`, per placement) | n/a | whether one lamp still burns in an Overgrown town |
| `0x5CAF_0001`..`0x5CAF_0004` | scavenge containers | `container_positions` | n/a (raw `hash`, per placement/house) | n/a | cupboard/barrel/crate presence per house, camp, ruin |
| `0x5CAF_100D` | container loot | `spawn_chunk_entities` → `fill_scav_container` | n/a (raw `hash`, per tile) | n/a | seeded one-time rummage loot per container position |
| `0xF06A3` | mist day | `weather::mist_day` | n/a (raw `hash`, per day) | n/a | ambient fog: ~40% of days open misty; bits 32+ pick the day's peak strength |
| `0xF06B7` | haze day | `weather::haze_day` | n/a (raw `hash`, per day) | n/a | ambient fog: ~15% of days haze over before golden hour |
| `0xF06C1` | bank day | `weather::bank_day` | n/a (raw `hash`, per day) | n/a | ambient fog: ~35% of days grow the regional banks (humid-ground dawns, coastal evenings) |
| `0xF06D5` | fog humidity | `weather::fog_moisture` | 80 | 1 (lattice) | modulates the per-biome humidity base into regional wet/dry pockets |
| `0xF06E1` / `0xF06E2` | mist patches | `ambience::mist_patches` | 96 px / 40 px (pixel-space lattice) | 1 each | render-only drifting bank texture; never touches gen |
| `0xBEE5_0001` | beehive scatter | `surface_tile` (Forest) | n/a (raw `hash`) | n/a | content wave: ~2% of warm-fringe broadleaf trees carry a wild Beehive |
| `0xBAD_0001` | badlands mesa | `surface_tile` (Badlands) | 26 | 2 | mesa/hoodoo rock clusters in the Badlands |
| `0xBAD_0003` | ore freckle | `surface_tile` (Badlands) | n/a (raw `hash`) | n/a | exposed ore pips on rich clay (gated on the shared `richness_at` field) |
| `0xBAD_0004` / `0xBAD_0005` | freckle metal / clay strata | `tile/clay.rs` | n/a (raw `hash`) | n/a | render/drop-side: iron-vs-coal pick per freckle; strata band phase per 48-tile column |
| `0x4807_5350_0001`..`0003` | hot spring placement / pool / rim stones | `features_gen` | n/a (raw `hash`, 192-tile cells) | n/a | content wave: warm pools in Tundra/Mountains (see §3.3.3) |
| `0x4D1E_5AF7_0001`..`0003`, `_100D` | mine shaft placement / detail / crate / loot | `features_gen` | n/a (raw `hash`, 288-tile cells) | n/a | content wave: surface headframe + the depth -1 gallery + its supply crate (see §3.3.3) |

If you add a new noise field, **pick an unused salt** (anything not in the table) —
reusing a salt correlates two fields that should be independent and will look wrong (e.g.
deserts always coinciding with mountains).

### 3.2 Biome table (`biome_at`, `Biome` enum)

```rust
pub enum Biome { Ocean, DeepOcean, Beach, Mountains, Tundra, Desert, Marsh, Forest, Savanna, Plains }
```

`biome_at(seed, x, y)` is a Whittaker-style classifier evaluated in this order (first
match wins):

```
land = continent + (rough - 0.5) * 0.08
  land < 0.36                          -> DeepOcean   (too deep to swim; raft country)
  land < 0.42                          -> Ocean
  land < 0.445                         -> Beach
  belt > 0.70 && rough > 0.55          -> Mountains    (belt is its own fractal field, salt 9)
  climate < 0.30                       -> Tundra
  climate > 0.70 && moisture < 0.42    -> Badlands if moisture < 0.22, else Desert
  moisture > 0.74                      -> Marsh
  moisture > 0.48                      -> Forest
  moisture < 0.34 && climate > 0.42    -> Savanna    (warm-dry only)
  else                                 -> Plains
```

`climate` is `climate_at` — the **single-octave** (period-512) backbone of the salt-6
field. The extreme-climate gates deliberately read this smooth variant rather than a
multi-octave fractal: single-octave value noise has a hard gradient bound (~1.5/512
per tile per axis), so the `0.30..0.70` strip between the cold and hot gates is always
~100+ tiles wide, and the `0.30..0.42` strip below the Savanna gate ~30+ tiles — snow
can never sit next to sand, even after `biome_at_blended`'s ±4-tile jitter. The
`tundra_never_borders_desert_or_savanna` test guards this property across seeds.

The *dynamic* weather layer reads the same field: `core::weather` presents
precipitation as snow wherever `climate_at < COLD_REACH = 0.36` (all of Tundra plus
the 0.30..0.36 cold fringe), and `level::tile::snowfall` settles/thaws snow there one
random tick at a time (see CORE_AND_SAVES.md §2.5). The gradient bound makes that
safe too — 0.36 stays 20+ tiles from the Savanna gate, so even fully wintered fringe
country never touches sand.

**Ambient fog** (`core::weather`, fog section) is the other dynamic layer built on
these fields, pure `f(seed, day, tick, x, y)` like the rain schedule. Three moods
on day-fraction windows: **morning mist** (~40% of days, dawn → burns off by 0.17
of the day), **afternoon haze** (~15%, a warm wash through 0.42..0.605, riding into
golden hour), and **regional banks** (~35% of days: very humid ground mists at dawn
even without the world roll, shorelines grow an evening bank over 0.54..0.74).
Regional density comes from `fog_moisture` — a per-biome humidity base (Marsh 1.0 →
Desert 0.0) modulated by the salt-`0xF06D5` lattice and floored by shoreline
proximity on the public `land_at` field (`≈0.435` is the waterline) — so marsh
dawns are the densest in the world and desert interiors never fog. Rain suppresses
fog (`x (1 - schedule_intensity)`). Densities are hard-capped at
`AMBIENT_FOG_MAX = 0.55`, well under `WHISPER_FOG_FLOOR = 0.85`, which
`weather::fog_density(g, x, y)` — the one read future systems should consume —
reports in marsh country during a Whisper Fog night: the rare event owns the top of
the scale (its own night-fog *visual* is a queued follow-up; today it is cues +
spawn pressure). Rendering: `gfx::lighting::fog_grade` (cool desaturating wash for
mist, warm amber for haze) + `gfx::ambience::mist_patches` (Bayer-banded drifting
banks, thinned in two quantized rings around the player so your surroundings stay
readable).

Thresholds are chosen empirically to keep regions "expansive" (hundreds of tiles) per the
`biomes_are_large_and_all_present` test, which asserts fewer than 40 biome changes over a
2048-tile straight-line walk and that `{Ocean, Beach, Tundra, Desert, Forest, Plains}` all
appear within an 8192×8192 sample. `Mountains` and `Marsh`/`Savanna` aren't in that
assertion list (they're rarer / gated by extra conditions) but do get their own coverage
in `mines_have_ores` indirectly and in `biome_frames.rs`'s visual dump.

`DeepOcean` is distinct from `Ocean`: `Ocean` renders as (crossable) `water`; `DeepOcean`
renders as `Deep Water`, which needs a Raft (§4).

### 3.3 Ground-cover rules (`surface_tile`)

`surface_tile(seed, x, y, ids)` maps a biome to a concrete tile id using a per-tile
`detail = unit(hash(seed, 3, x, y))` roll (and biome-specific secondary fractals) as
piecewise thresholds:

| Biome | Rule |
|---|---|
| Ocean | tidal band first (§tides), then **skerries** — sparse permanent rock stacks, hashed per 2x2 cell (salt `0x5EA0`, ~1 cell in 400) so they surface as small sea stacks the tide never covers — then shallows (recomputed `land > 0.400`, same salt-1/5 fields as `biome_at`): `detail<0.10` seaweed · `<0.13` coral · else (and everywhere deeper) `water` |
| DeepOcean | always `deep_water` |
| Beach | `detail<0.02` palm · else sand |
| Mountains | `temperature(salt6)<0.42 && belt(salt9)>0.76` snow (snow-capped cold peaks) · else rock |
| Tundra | `detail<0.030` pine · `<0.055` snow tree · `<0.062` rock · else snow |
| Desert | `detail<0.004` fruiting cactus · `<0.014` cactus · `<0.018` dead tree · `<0.026` dry bush · `<0.032` rock · else sand |
| Badlands | `mesa(0xBAD_0001,period26)>0.73` rock (mesas/hoodoos) · `detail<0.005` rock · `<0.011` dead tree · `<0.034` dry bush · rich ground (`richness>0.55`, freckle roll `<0.04`) Ore Freckle · else Layered Clay. **No water arm** — the dryness is the biome |
| Marsh | `pool(salt7,period14)>0.66` water (blobby, so no lone 1-tile ponds) · `>0.60` mud · `>0.54` wet fringe (`detail<0.05` willow · `<0.50` reeds) · else `detail<0.16` tuft · `<0.175` flower · else grass |
| Forest | `clearing(salt8,period24)>0.62` lowers tree odds to 0.03 (else 0.16); below that threshold tree — pine instead when `climate(salt6, 1 octave)<0.42` (cold fringe, same field as the Tundra gate), and ~2% of the warm-fringe broadleafs carry a wild **Beehive** (salt `0xBEE5_0001`) — then +0.008 berry bush, +0.016 mushroom, +0.066 tuft, else grass |
| Savanna | `parched(salt10,period18)>0.74` sand · else `detail<0.008` flat-crown tree · `<0.016` dry bush · `<0.10` tuft · else grass |
| Plains | ponds (`pond(salt12,period40)`) + meadows (`meadow(salt11,period96)`) first · `detail<0.015` tree · `<0.020` berry bush · `<0.060` flower · `<0.105` tuft · else grass |

"tuft" = `ids.tall_grass[hash(seed, 4, x, y) % 3]`, i.e. one of Small/Medium/Tall Grass
chosen uniformly per tile. The flora arms deliberately add **no new noise salts**: the
Ocean shallows recompute the continent/rough fields (salts 1/5) and the Mountains
snow-cap + Forest cold fringe re-sample the salt-6 climate/temperature and salt-9 belt fields — the
same logical fields `biome_at` reads, so the reuse is correct (they *must* correlate
with the biome boundaries; only genuinely new fields need fresh salts).

#### 3.3.1 Flora (tile ids 51+, `src/level/tile/`)

The flora wave adds the following tiles; all are stamped by the `surface_tile` /
`mine_tile` rules above (plus two structure stamps, see §3.3.2). Ids, like all tile ids,
are in-memory only — saves store names.

| Id | Tile | Kind | Where | Behavior / drops |
|---|---|---|---|---|
| 51 | Pine Tree | `TreeSpecies{Pine}` | Tundra; Forest cold fringe | tree behavior on a **snow** base, health 20; fells into 1-2 Wood + 2-4 Sticks (extra sticks in lieu of a resin item) |
| 52 | Dead Tree | `TreeSpecies{Dead}` | Desert | brittle snag on sand, health 8; Sticks only (2-3), never Wood |
| 53 | Willow | `TreeSpecies{Willow}` | Marsh wet fringe (near pools) | grass base, health 20; 1-2 Wood + 1-2 Sticks |
| 54 | Palm Tree | `TreeSpecies{Palm}` | Beach | sand base, health 20; 1-2 Wood + **1-2 Coconuts** (plus a rare per-hit coconut shake) |
| 55 | Flat-Crown Tree | `TreeSpecies{FlatCrown}` | Savanna (lone trees) | grass base, health 16; 1-2 Wood + 1-2 Sticks |
| 56 | Berry Bush | `BerryBush` | Plains + Forest scatter | data 0 = ripe (fresh chunks ripe for free), 1 = regrowing. A hit on a ripe bush picks 1-2 **Berries** (bush survives); random ticks (`1/2000` per tick, tall-grass cadence ≈ a few in-game days) re-ripen it; hitting a bare bush tears it out (1 Stick) |
| 57 | Mushroom | `Mushroom` | Forest floor; mine cave floors (all depths) | walk-through; one hit drops a **Mushroom**; base follows the level (grass on the surface, dirt underground) |
| 58 | Fruiting Cactus | `FruitingCactus` | Desert (rarer than plain cactus) | a hit knocks off 1-2 **Cactus Fruit** and leaves a plain Cactus *with the same damage byte*; pricks on contact like any cactus |
| 59 | Seaweed | `Seaweed` | Ocean shallows near beaches | renders over water, passes like water (swimmers only); breaks into 1-2 Grass Fibers |
| 60 | Coral | `Coral` | Ocean shallows near beaches | as seaweed; breaks into 1-2 Stone (calcified skeleton) |
| 61 | Reeds | `TallGrass{kind:3}` | Marsh wet fringe | reuses the tall-grass mechanics: never grows, never blocks, shreds into 2 Grass Fibers (no pebbles) |
| 62 | Jack-O-Lantern | `Pumpkin{lit:true}` | cemeteries + razed villages (§3.3.2) | light radius **7** (plain pumpkin: 3); smashing drops a **Jack-O-Lantern** item (plain pumpkins now drop a **Pumpkin**) |
| 63 | Dry Bush | `DryBush` | Desert + Savanna | walk-through tumbleweed shrub; one bare-handed hit snaps it into 1-2 Sticks; ground restores to sand next to sand, grass otherwise |

Notes:

- **Food-item names are a contract with the item registry**: Berry, Mushroom, Apple,
  Cactus Fruit, Coconut, Pumpkin, Jack-O-Lantern. Drop code goes through
  `registry::get`, which falls back to an `UnknownItem` of the requested name, so tile
  code is safe even if an item lands later. The broadleaf Tree (and Snow Tree) also has
  a rare per-hit **Apple** drop (1 in 100 per hit, `tree.rs::hurt_dmg`).
- **Thicket paddocks**: fully-grown Tall Grass (kind 2) no longer blocks per-tile —
  it only blocks when 6+ of its 8 neighbors are also fully-grown thicket
  (`tall_grass::may_pass`), so meadow *cores* stay impenetrable while fringes and lone
  tufts are brushed through.
- **TODO(art): final cells** — until the art agent lands dedicated cells, every species
  reuses existing art: tree species use the broadleaf canopy cells + species palette
  (the true-color tree art ignores palettes, so species currently read via base ground
  + the Dead Tree darken), the berry bush / reeds reuse the tall-grass cell, mushrooms
  and coral reuse the ore-nub cell recolored, dry bush reuses the wheat-stalk cell, the
  fruiting cactus and ripe berry bush overlay red `Sprite::dots` specks, and the
  Jack-O-Lantern renders as a plain pumpkin (its light radius tells them apart).

#### 3.3.2 Jack-O-Lantern structure stamps (`structures_gen.rs`)

~30% of cemeteries stamp one lit Jack-O-Lantern just inside the fence corner
(`(ox-rx+1, oy+ry-1)` — off the even-offset grave lattice by construction), and ~20% of
razed villages keep one burning at the plaza edge (`(ox+3, oy+2)`, outside the 3×3 well
footprint, inside the plaza circle). Both are hashed per placement origin (salts
`0xCE4E_0004` / `0x56C4_000A`), so they are pure and chunk-border-safe like every other
structure write. The trail pass also treats all new scatter flora as soft
`trail_ground`, so old routes wear through pine stands and dry brush the same way they
do broadleaf forest.

### 3.3.3 Wild features (`features_gen.rs`): hot springs + abandoned mine shafts

Content-wave landforms of the inhospitable biomes, deliberately separate from
`structures_gen` (whose kinds are settlement-shaped and biome-gated away from
Mountains/Tundra). Placement is the `gate_in_cell` hash-grid pattern — one coarse
cell grid per feature, at most one per cell at a jittered, biome-gated point — and
blueprints are pure `f(seed, origin)`, stamped from `generate_chunk` after the
structure pass and before the gate set-pieces.

- **Hot springs** (grid 192, Tundra/Mountains): a 3-4 tile L-cored ragged pool of
  `Spring Water` (id 73) with an ochre mineral rim and the odd sitting stone. The
  tile swims like water, breathes steam wisps on its random tick, never freezes or
  snows over, and `core::temperature` clamps cold to comfort within
  `SPRING_BASK_RADIUS = 3` tiles (`Modifiers::near_spring`) — a found sanctuary in
  the coldest country.
- **Abandoned mine shafts** (grid 288, Mountains): a weathered surface headframe —
  spoil apron, plank shed-floor remnant, two standing Timber Props, rubble — around
  a CHASM mouth, and this is the one feature that writes TWO layers: the same
  origin on depth -1 generates the pre-carved gallery (ragged dirt pocket, the
  Ladder home at the exact origin, standing props that genuinely suppress cave-ins,
  weak `RUBBLE_FLAG` rocks, and an iron/lapis vein bias whose pip count scales with
  the shared `richness_at` field). `features_gen::spawn_chunk_entities` (called
  beside the structures one in `ensure_chunks_at`) stocks ~65% of galleries with a
  supply crate of mining gear — props, coal, a pan, rarely a Vice.

Tests: `tests/content_wave.rs`. The Badlands tiles (Layered Clay id 75, Ore
Freckle id 76 — pickaxe a freckle for 1-2 Iron Ore/Coal) and the forest Beehive
(id 74, data 0 = full / 1 = regrowing on the berry-bush timer family) land in the
same wave; ids 73-76 follow the flora-wave convention (names are the save contract,
ids are in-memory only).

### 3.4 Mine layers (`mine_tile`, depths -1..-3)

`salt = 10 + depth.unsigned_abs()` (11/12/13 for -1/-2/-3). Two-stage decision:

1. **Cave vs rock**: `cave = fractal(seed, salt, x, y, period 32, octaves 4)`. If `cave`
   is *outside* `0.32..0.62`, the tile is solid — either plain `rock`, or an ore vein.
   The vein noise is sampled **anisotropically** (one axis compressed 2x — `(x*2, y)`,
   or `(x, y*2)` at depth -2), so veins run as long thin seams rather than round blobs,
   and the threshold is richness-gated: `vein = fractal(seed, salt+40, vx, vy, 12, 2) >
   0.80 - 0.05 * richness_at(seed, x, y) && detail < 0.6` (`detail = unit(hash(seed,
   salt+90, x, y))`). The richness field is the same one the surface reads (§4.5), so
   ground under a mineral-seep stain genuinely carries more ore:
   - depth -1: `detail<0.08` → Lapis, else Iron
   - depth -2: `detail<0.08` → Lapis, else Gold
   - depth -3: always Gem
2. **Open cave floor** (`cave` inside `0.32..0.62`): `pocket = fractal(seed, salt+70, x, y,
   24, 2)`. Above a depth-dependent `lava_threshold` → lava; below `0.12` → water;
   then `detail < 0.012` → Mushroom (the same walk-through pickup tile as the forest
   floor, rendering a dirt base underground); otherwise → dirt. `lava_threshold` = 0.86
   at depth -3, 0.93 at -2, 0.985 at -1 (deeper layers grow noticeably more lava
   pockets).

`mines_have_ores` locks in that each depth's characteristic ore (iron/-1, gold/-2, gem/-3)
appears more than 40 times in a 256×256 sample at a fixed seed.

### 3.5 Dungeon gates (`gate_in_cell`, `gates_in_rect`)

Rare, deterministic per-cell portals from the deepest infinite mine layer (depth -3) down
to the *finite* dungeon (depth -4, §6). Grid-hashed rather than noise-thresholded, so
exactly zero or one gate exists per `GATE_GRID = 160`-tile cell:

```rust
pub fn gate_in_cell(seed, depth, cell_x, cell_y) -> Option<(i32, i32)> {
    if depth != -3 { return None; }                 // dungeon gates only on the deepest mine
    let h = hash(seed, GATE_SALT ^ depth.unsigned_abs(), cell_x, cell_y);
    if unit(h) > 0.5 { return None; }                // ~50% of cells have no gate
    // else: jitter the gate to a pseudo-random point inside the cell (8..GATE_GRID-8 margin)
}
```

`gates_in_rect` sweeps the grid cells overlapping a rectangle (with one cell of margin so
gates near a rect edge aren't missed) and returns every gate found — used by
`generate_chunk` to find gates whose 5×5 stamp might overlap the chunk being generated
(chunks stamp features from *neighboring* cells' gates too, via the `margin = 2` apron
expansion, so a gate straddling a chunk boundary still renders fully in both chunks).

### 3.6 Chunk assembly & gate/apron stamping (`generate_chunk`)

For depth 0 and -1..-3, each of the 4096 cells in a chunk is filled by `surface_tile` or
`mine_tile` respectively (any other depth passed in falls back to solid `rock` — infinite
gen only covers the surface and the three mines; the dungeon at depth -4 never goes
through this path). Then, **only for depth 0 and depth -3**, "gates" are stamped over the
freshly generated tiles:

- **Depth 0 (surface)**: a "sky-tower" landing pad — `Stairs Up` at the gate's exact
  point, surrounded by a 5×5 ring: `hard rock` for the ring cells except a doorway gap on
  the south edge (`dx==0 && dy==2`), and plain `rock` for the interior pad cells. This is
  where a player who ascended from the dungeon (or otherwise needs a fixed rendezvous)
  lands, biome-matched apron included (the `pad_id` for depth 0 is `ids.rock`, matching
  Mountains rather than clashing with grass/sand).
- **Depth -3 (deepest mine)**: `Stairs Down` ringed by `obsidian wall`, pad filled with
  `obsidian` — visually announcing "this leads to the dungeon" before the player even
  steps on it.

Both use the exact same stamping loop, parameterized by `ring_id`/`pad_id`/`stairs`:
`gates_in_rect` finds every gate whose stamp could overlap `[x0-2, y0-2, x1+2, y1+2]`
(chunk bounds plus the `margin=2` apron plus one more +2 for the 5×5 stamp radius itself),
then for each `(gx, gy)` writes a `5×5` block centered on it, clipped to the current
chunk's bounds (`tx/ty` range checks) since a gate near a chunk edge stamps into two (or
four) chunks independently but consistently (pure function of the same `(gx, gy)`
regardless of which chunk is generating).

**Important**: these are the *only* pre-placed stairs on infinite layers. Regular
mine-to-mine descent (depth 0 → -1 → -2 → -3) has **no** stairwells at all — it is 100%
player-dug (§4). `no_preplaced_stairs_on_infinite_layers` asserts generated chunks at
depths 0/-1/-2 never contain `stairs down`/`stairs up` tile ids.

### 3.7 Spawn point (`find_surface_spawn`)

Outward ring scan from the origin, step 4 (biome regions are hundreds of tiles wide, so a
coarse step is fine), up to radius `300 * 4 = 1200` tiles, looking for a tile that is both
in a "hospitable" biome (`Plains | Forest | Savanna | Marsh`) and renders as `grass`.
Falls back to `(0, 0)` if nothing qualifies in range (never observed in practice —
`spawn_lands_on_grass` checks several seeds).

## 4. Multi-level terrain (`src/level/tile/depth.rs`)

Sandbox-era addition with no Java counterpart: since infinite layers don't pre-place
stairwells between mine depths, descent is entirely dig-based. State machine:

```
   grass/dirt (surface tile)
         │  Shovel
         ▼
   Dug Pit, data = 0 ──Shovel──▶ data = 1 ──Shovel──▶ data = MAX_STAGE (=2)
         │                                                    │
         │ (any dig also drops one "dirt" item at the tile)   │ Pickaxe
         │                                                    ▼
         │                                              ┌───────────┐
         │                                              │   Chasm   │──▶ standing on it:
         │                                              └───────────┘    scheduleLevelChange(-1)
         │                                                    │
         │                                                    │ carves a matching pocket one
         │                                                    │ layer below + stamps a Ladder
         │                                                    ▼
         │                                    layer (lvl-1): 3×3 dirt patch with a
         │                                    Ladder at the same (x, y) ── standing
         │                                    on it: scheduleLevelChange(+1)
```

`MAX_STAGE = 2` (`src/level/tile/depth.rs`). Mechanics, in `dug_pit_interact`:

- Shovel while `stage < MAX_STAGE`: pays `4 - tool_level` stamina and one durability;
  `set_data(lvl, xt, yt, stage+1)`; drops a `dirt` item; if the new stage hits `MAX_STAGE`,
  pushes the notification "The pit hits solid rock." and plays `Sound::MonsterHurt`.
- Shovel at `stage >= MAX_STAGE`: refuses, notifies "Too rocky - a pickaxe could break
  through."
- Pickaxe at `stage >= MAX_STAGE`: pays stamina/durability, calls `open_chasm`, drops a
  `Stone` item, plays the same sound.

`open_chasm(g, lvl, xt, yt)`:

1. Sets this tile to `chasm` on the current layer.
2. If `lvl == 0` (the deepest mine slot, `IDX_TO_DEPTH[0] == -3`), stops — nothing below
   the deepest mine; the dungeon is gated, not dug into (§6).
3. Otherwise, `ensure_chunks_at(g, below, xt, yt)` to force-load the destination chunk
   (so the carve below always lands on generated ground, not an unloaded placeholder),
   overwrites a 3×3 dirt patch centered on `(xt, yt)` on the layer below, then stamps a
   `ladder` at the exact center — so the ladder is always guaranteed walkable ground on
   arrival, regardless of what was generated there.

Rendering: `dug_pit_render` draws the dirt tile then darkens progressively deeper with
stage (`48 + stage*48` alpha) and adds a rock speckle once bottomed out; `chasm_render`
draws dirt with two nested darken rects (a black hole read); `ladder_render` draws dirt
then overlays the "Stairs Up" sprite (reusing that glyph as an unmistakable "climb" cue
rather than defining new art).

**Travel mechanics** (`src/entity/mob/player_behavior.rs`, in the per-tick tile check):
the player's current tile is checked against `Stairs Down`, `Stairs Up`, `Quick Sand`,
`Chasm`, and `Ladder` ids together. If `on_stair_delay <= 0`:
`g.pending_level_change = if tile is Stairs Up or Ladder { 1 } else { -1 }`
(so Ladder behaves exactly like Stairs Up — ascend — and Chasm exactly like Stairs
Down — descend), `on_stair_delay` resets to 10, and the function returns early (skipping
the rest of that tick for the player). If still on such a tile with `on_stair_delay > 0`,
the delay is simply held at 10 (preventing a change until the player steps off for 10+
ticks in a row); off such a tile, the delay only ticks down.

`g.pending_level_change` is consumed by `World::change_level` (`src/core/world.rs`, driven
from the main tick loop) — see §5/§6 for what happens across finite/infinite boundaries.

**Deep water + raft** (`deep_water_may_pass`): only a `Player` carrying an item named
"Raft" (case-insensitively) or in creative mode may enter `Deep Water`; `ItemEntity`
always may (items drift over it); every other entity kind is blocked. Rendering
(`deep_water_render`) reuses the regular water sprite darkened by 96, so it reads as
"deeper water" on any art style without new sprites. The player-render raft visual
(`src/entity/mob/player_behavior.rs`, in `render`) checks `tile_at(lvl, x>>4, y>>4).id ==
Deep Water` and draws a small two-tile raft glyph under the player sprite when true —
this is purely cosmetic and independent of the `may_pass` gate (a creative player without
a literal Raft item still gets the visual, since they're allowed to stand there).

## 4.5 Fossicking — mining as reading the earth (`src/level/tile/fossick.rs`)

The mining overhaul (sandbox era, no Java counterpart). One deterministic field ties it
together: `infinite_gen::richness_at(seed, x, y)` — creek-scale (period 96, 2 octaves,
salt `0xF055_1C4E`), **shared identically by every depth**, so surface signs truthfully
advertise the mines below.

**Prospector's Pan** (item; hand recipe `Stick*3 + Cord + Stone`). Used on Mud (always),
an exposed Tidal Flat, or sand/dirt with a wet cardinal neighbor (Water / Deep Water /
Tidal Flat / Mud — `fossick::water_adjacent`). Each pan costs 3 stamina and rolls
`fossick::pan_outcome(richness, roll)` — a pure, test-pinned table where every find band
widens with richness: gem `0.001+0.004r` · gold nugget `+0.004+0.026r` · iron fleck
`+0.020+0.080r` · coal fleck `+0.040+0.100r` · stone `+0.300` · else nothing. At r=0 a
pan pays ~36% of the time (almost all stone); at r=1 ~58%, with metal ~27% — some creeks
are worth working, most aren't. Finds drop as the ordinary ore/stone items with a short
notification ("A gold nugget winks up at you!").

**Surface signs.** Rock outcrops on rich ground (`richness > 0.70`, per-tile hash salt
`0x5EE9` keeps it to ~35% of qualifying tiles) render a mineral-seep stain: a damp
streak with ochre flecks (`rock.rs::render`). Because the richness field is shared
across depths, digging straight down under a stained outcrop finds denser veins — the
mine-tile vein threshold is `0.80 - 0.05 * richness` (§3.4).

**Vein-chasing.** Mining out an ore tile calls `fossick::vein_ping`: every ore tile
still hiding within Chebyshev 2 gets a brief smash-particle sparkle, so the player
follows the seam instead of strip-mining. Veins themselves are sampled anisotropically
(§3.4) so there is a seam to follow.

**Rock character** (`fossick::rock_character`, raw hash salt `0xC4AC_4ED0`, evaluated at
runtime — no tile data, so it survives regeneration and save/load for free): ~20%
of rock is *cracked* (renders shaded darker; health 30 vs 50 — breaks ~40% faster), ~10%
*dense* (pale boss in the face; health 80, +1 stone on break).

**Cave-ins + Timber Props.** Rock tile data layout: bit 7 = rubble flag
(`fossick::RUBBLE_FLAG`), low 7 bits = accumulated damage (decays on tick, flag
preserved). When a non-rubble rock breaks at depth < 0 (`fossick::collapse_check`): if
the 5x5 open-floor count (open = not Rock/Ore/HardRock/Wall; unloaded chunks read as
rock, i.e. solid) is >= 13, no Timber Prop stands within Chebyshev 3, and a 1-in-4
`g.random` roll lands, the collapse *arms*: "The ceiling groans...", `Sound::Fuse`, and
the freshly broken dirt tile's data is set to 255 (`COLLAPSE_FUSE`). On that tile's next
random tick (`dirt::tick` → `fossick::fuse_tick`, ~1s at the 1-in-50 tile-tick cadence)
the roof falls: up to 4 open, mob-free tiles in the immediate neighborhood become rock
with the rubble flag (weak — health 12, 1-2 stone, never coal, never re-triggers a
collapse), with "The ceiling comes down!" + `Sound::Explode`. A prop raised during the
groan beat still saves the gallery. **Timber Prop** (tile id 65, `TileKind::TimberProp`;
tile item placeable on dirt; hand recipe `Wood*2 + Stick*2`) is walk-through, prevents
collapse within radius 3, and one hit knocks it down for 1-2 Wood + 1-2 Sticks.

**Highland (tier-2) rock.** On the infinite surface only, `infinite_gen::highland_at` —
the same belt/rough fields as the Mountains gate, thresholded higher (`belt > 0.75 &&
rough > 0.55`, i.e. the band just under the `> 0.80` snow line) — marks summit rock: it
renders visibly raised (bright rim chips north, shaded flanks, hard drop shadow south)
and takes **double damage to break** (health 100), paying +2 stone. Deliberately kept as
damage/render modulation on the one rock tile (v1); true climbable elevation tiers can
layer on later without a save migration since no tile data is used.

TODO(art): dedicated cells for the pan icon, timber-prop icon + tile, wet-sand pan
ground, and a real seep-stain overlay — all current visuals reuse existing cells
(bucket/wall/fence-post/speck) with palette tricks, marked with `TODO(art)` comments at
each site.

## 5. Persistence (`src/saveload/save.rs`, `src/saveload/load.rs`)

### 5.1 Version gate

`crate::core::game::version()` returns `"3.0"` — the "sandbox pivot" (six-level Java
layout with the sky/Air Wizard/Score mode collapsed to today's five-layer layout).
`Load::new_world` refuses anything with `world_ver < Version::new("3.0")`: it prints a
`LOAD ERROR` and leaves the world unloaded rather than attempting a shape it can't
represent. Every other version check in `load.rs` (there are many, all pre-3.0-era item/
armor/potion serialization quirks inherited from Java) only matters for saves *at or
after* 3.0, since anything older is already rejected up front.

### 5.2 What regenerates vs. what persists

| World type | Levels 0..-3 (surface + mines) | Level -4 (dungeon) |
|---|---|---|
| Infinite | chunked; **regenerate** from `(seed, depth, x, y)` on demand; only *changed* tiles persist as chunk files | finite; **fully written/read** like a classic level file |
| Classic | finite; **fully written/read** as a classic level file | finite; **fully written/read** as a classic level file |

`write_world` (`Save::write_world`) skips any level where `g.level(l).is_infinite()` —
those layers "persist via the chunks/ directory" instead (comment in the source). Classic
(non-infinite) levels get the full `Level{n}` + `Level{n}data` files as before.

An **`Infinite` world always writes a `WorldMeta.miniplussave`** file at
`saves/<world>/WorldMeta.miniplussave` with contents `"Infinite,{world_seed}"` — this is
the *only* thing that needs to survive for an infinite world's surface/mine layers to
reconstruct identically: reload re-reads the seed, marks those 4 levels as chunked empty
maps, and lets `ensure_chunks`/`ensure_chunks_at` regenerate (or reload individual saved
chunks) lazily as the player approaches.

### 5.3 Chunk file binary format (`chunk_dir`, `save_chunk`, `load_chunk`)

Path: `<game_dir>/saves/<world_name lowercased>/chunks/<depth>/<cx>_<cy>.bin`.

Byte layout (`CHUNK_SIZE² = 4096` cells; `AREA = 4096`):

```
offset             length            content
0                  4096              chunk.tiles[i]  (one byte per cell, row-major lx + ly*64)
4096               4096              chunk.data[i]   (one byte per cell, same order)
8192               ceil(4096/8)=512  chunk.visible[] packed LSB-first, 1 bit per cell
```

Total file size: exactly `4096 + 4096 + 512 = 8704` bytes per chunk, always — there is no
length-prefixing or versioning inside the chunk file itself (a truncated/corrupt file
just fails the `bytes.len() < area * 2` guard in `load_chunk` and is treated as "not
saved", so the chunk regenerates from the pure function instead of erroring).

`save_chunk` packs `visible` bit-by-bit (`bit`/`acc` accumulator, flushed every 8 bits,
plus a final partial byte if `4096 % 8 != 0` — it divides evenly, so this partial-flush
path is dead code for the current `CHUNK_SIZE`, but keeps the routine correct if
`CHUNK_SHIFT` ever changes to something whose square isn't a multiple of 8).
`load_chunk` reverses this exactly; chunks loaded from disk have `dirty` forced to
`false` (a freshly loaded chunk is, by definition, in sync with what's on disk).

`save_all_chunks` (called once at the top of `save_world_named`) walks every level, and
for each chunked level, every *currently loaded* chunk that is `dirty`, and saves it — it
does **not** save unloaded chunks (they were already saved/discarded when they were
evicted by `ensure_chunks_at`, or they were never touched and don't need a file at all).

### 5.4 Entity/player persistence

Unaffected by infinite vs. classic — entities are saved by `write_entities` regardless of
which level they're on (`Save.writeEntity`'s per-entity string format, e.g.
`"Zombie[x:y:health:enemylevel:leveldepthidx]"`), and `Load.load_entity` restores them the
same way. The one infinite-aware wrinkle: `crate::level::lvl_idx(depth)` is what gets
written as the trailing level-index field, so entities always reattach to the correct
slot in `g.levels` regardless of whether that slot happens to be chunked.

## 6. Classic worlds (finite path)

**World type "Classic"** (`g.settings.get("worldtype") == "Classic"`, the other option
being the default `"Infinite"`) uses `level_gen.rs` for *every* depth, including 0..-3 —
i.e. the whole port of Java `LevelGen`: `create_top_map` (surface, four `gen_type`
variants — Island/Box/Mountain/Irregular — crossed with five `theme`s —
Normal/Forest/Desert/Plain/Hell — plus `HistoryGen` "human influence" scenery),
`create_underground_map` (mines, restored from a disabled Java-fork stub — see the
"restored post-v0.1.0" comment in `level_gen.rs`), and `create_dungeon`.

**The dungeon (depth -4) uses this finite generator unconditionally, on *every* world
type**, because it has no infinite equivalent — `infinite_gen.rs`'s doc comment says so
explicitly ("Layer plan... Stairs are placed on a deterministic grid" applies only to
surface + mines). `core::world::init_world` special-cases this: for `i in
(-3..=0)` under an Infinite world type, it builds an empty chunked `Level`; for every
other `(i, world_type)` combination — which includes depth -4 always — it calls
`level_gen::create_and_validate_map` and stores the resulting full tile/data arrays.

Linking parent/child finite levels (`populate_from_parent`, `src/core/world.rs`): only
applies "between finite neighbors" — every `Stairs Down` tile on the parent level gets a
matching `Stairs Up` stamped at the same `(x, y)` on the child, plus a small apron (`hard
rock` around surface entrances, plain `dirt` around mine entrances, or a full obsidian
"dungeon gate" structure — §6.1 — when the child is the dungeon). When the parent is
chunked (infinite mines above a finite dungeon), this direct linkage doesn't apply — the
dungeon instead gets exactly one fixed landing gate stamped at world-size/2 in
`init_world`'s "else if" branch, and the *deep mine* gets its `gates_in_rect`-driven set
of hash-grid gates (§3.5) that go the other direction (Stairs Down into the dungeon).

### 6.1 The dungeon gate structure (`src/level/structure.rs`)

`Structure` is a small prefab system: a `Vec<TilePoint>` (relative offset + tile name) and
a `Vec<(Point, fn() -> Entity)>` (relative offset + furniture constructor), applied via
`Structure::draw(level, tiles, xt, yt, lvl_idx)`. `dungeon_gate()` builds the ~9×9
obsidian ring/wall pattern (with an Iron Lantern) drawn around every dungeon-side `Stairs
Up`/gate point — `draw_dungeon_gate` is the `Game`-level convenience wrapper used both by
`populate_from_parent` (classic parent→dungeon linkage) and by `init_world`'s
infinite-dungeon landing-gate special case.

### 6.2 Set-piece relocation on level change (`change_level`, `src/core/world.rs`)

Because an infinite mine's gate coordinates and the finite dungeon's fixed gate
coordinates don't line up numerically, `change_level` detects "landing on a finite level
whose current tile isn't the stairs we expect" (`in_bounds` check + tile-id check against
`Stairs Down`/`Stairs Up` depending on travel direction) and relocates the player to the
*nearest* matching-tile on that level via `get_matching_tiles` + a squared-distance
`min_by_key`. Classic finite↔finite transitions already line up exactly (parent/child
stairs are stamped in lockstep by `populate_from_parent`) and skip this relocation
entirely (`!g.level(lvl).is_infinite()` guards the whole block, and even then it only
triggers when the expected tile isn't already under the player).

## 7. HOW TO EXTEND

### 7.1 Add a biome

1. Add a variant to `enum Biome` in `infinite_gen.rs`.
2. Carve out its slice of `biome_at`'s temperature/moisture/land space — fields are
   continental-scale (period 384–512 tiles), so keep regions large. Insert your branch
   somewhere in the `if/else if` chain in `biome_at`; order matters (first match wins).
3. Add its ground-cover arm to `surface_tile`'s `match biome_at(...) { ... }`. Reuse the
   `detail` roll or add a new fractal field with a **fresh, unused salt** (see the salt
   table in §3.1 — do not reuse an existing one).
4. If stairwell/gate aprons should use a biome-specific ground tile instead of the
   hardcoded `ids.rock` (surface) / `obsidian` (deep mine), extend the `pad_id` selection
   in `generate_chunk`'s gate-stamping block (currently a flat `if depth == 0 { ids.rock }
   else { obsidian }` — not biome-aware today; see §8 discrepancy note).
5. Run `cargo test level` — `biomes_are_large_and_all_present` fails until the new biome
   actually appears in an 8k×8k sample. Then `cargo test --test biome_frames` to dump a
   rendered frame (needs a biome you can locate — add a case to that test's biome list, or
   confirm `find_biome`'s ring-scan reaches yours within its search radius).

### 7.2 Add an ore

Ores are wired into `mine_tile`'s per-depth vein branch (`match depth { -1 => ..., -2 =>
..., _ => ... }`). To add a fourth ore tier or change which ore appears at which depth:

1. Register the tile in `Tiles::new()` (`src/level/tile/mod.rs`) if it's new — reuse the
   existing `ore.rs` module (`dispatch::make_ore_tile(OreType::X)`) and add an `OreType`
   variant if needed.
2. Add its id to `struct Ids` / `Ids::get` in `infinite_gen.rs`.
3. Extend the `match depth { ... }` arm inside the `vein > 0.78 && detail < 0.6` branch of
   `mine_tile`.
4. Add an assertion to `mines_have_ores` (`infinite_gen.rs`'s test module) for the new
   ore/depth pair.
5. If it should also appear in Classic worlds, extend `create_underground_map` in
   `level_gen.rs` similarly (it currently drops ore as `(iron_ore.id + depth - 1)`, a
   contiguous-id trick — see the JAVA comment there; adding a truly new ore id may break
   that arithmetic and need its own explicit branch instead).

### 7.3 Change biome region size

Adjust the `period` arguments passed to `fractal(...)` in `biome_at` (continent=384,
temperature=512, moisture=448, rough=48, belt=320). Larger period = larger, smoother
regions; the `rough` field intentionally uses a much smaller period (48) since it's
meant to add local jaggedness (coastlines, mountain-edge irregularity) on top of the
continental fields, not define regions itself. Re-run `biomes_are_large_and_all_present`
after any change — its "<40 changes over 2048 tiles" assertion is a regression guard on
region size, not just presence.

### 7.4 Add a new depth-tile behavior (a 5th multi-level tile, e.g. a trap door)

1. Add a `TileKind` variant in `src/level/tile/mod.rs`.
2. Implement `make_*`, render, interact/tick functions in `src/level/tile/depth.rs` (or a
   new sibling module if it doesn't fit the "digging" theme) following the existing
   `make_deep_water`/`make_dug_pit`/`make_chasm`/`make_ladder` pattern.
3. Wire every relevant `dispatch.rs` match (`render`, `may_pass`, `interact`, `tick`,
   `get_light_radius`, ...) — missing one silently falls through to that dispatch
   function's default rather than erroring, so double check each one you need.
4. Register with `set(<next free id>, ...)` in `Tiles::new()` — ids 46–49 are already
   taken by Deep Water/Dug Pit/Chasm/Ladder; pick the next unused id (never renumber
   existing ids — see ADDING_CONTENT.md's tile-id rule, which applies here unchanged).
5. If the new tile should trigger a level transition like Chasm/Ladder, add its id(s) to
   the tile-id check in `player_behavior.rs`'s per-tick stair/chasm/ladder block (search
   for `stairs_down_id`/`chasm_id`/`ladder_id` in that file) and decide the direction
   (`pending_level_change = 1` for "up", `-1` for "down").

### 7.5 Add a structure/gate type

Follow `src/level/structure.rs`'s `Structure`/`dungeon_gate()` pattern for a fixed prefab
(tiles + furniture at relative offsets). For a *procedurally placed, sparse* gate like the
deep-mine dungeon gates, follow the `gate_in_cell`/`gates_in_rect` pattern in
`infinite_gen.rs`: pick a grid size (`GATE_GRID`), hash `(cell_x, cell_y)` with a
dedicated salt to decide presence + jitter the exact point within the cell, then stamp a
fixed-radius apron in `generate_chunk`, remembering to query `gates_in_rect` over the
*expanded* rect (chunk bounds + apron margin + stamp radius) so gates near a chunk edge
still render fully.

### 7.6 Tune dig depth stages

`MAX_STAGE` in `src/level/tile/depth.rs` controls how many shovel hits a Dug Pit takes
before it bottoms out and needs a pickaxe. Raising it doesn't require touching the render
function (`dug_pit_render`'s darkening scales as `48 + stage * 48`, so it auto-adjusts,
though very large `MAX_STAGE` values will overflow the `u8` alpha-ish parameter passed to
`darken_rect` — sanity-check the visual at the new max). `dug_pit_interact`'s stamina cost
(`4 - tool_level`) and the "hits solid rock" notification threshold both key off
`MAX_STAGE` already, so no other change is needed for a pure numeric tune.

### 7.7 Make a new layer

The layer count is currently fixed at five (`IDX_TO_DEPTH: [i32; 5] = [-3, -2, -1, 0,
-4]`, `MIN_LEVEL_DEPTH = -4`, `MAX_LEVEL_DEPTH = 0`) — Surface + 3 mines + Dungeon. Adding
a layer means:

1. Extend `IDX_TO_DEPTH` and adjust `MIN_LEVEL_DEPTH`/`lvl_idx`'s branching (it currently
   hardcodes the dungeon's special index `4` and does linear `depth + 3` for the rest —
   both need updating for a non-contiguous or longer depth range).
2. Update `LEVEL_NAMES` (`src/level/mod.rs`) to match the new count.
3. Decide whether the new layer is infinite (extend the `-3..=0` range check in
   `init_world`/`load_world` and add a `depth => mine_tile(...)`-style arm in
   `generate_chunk`) or finite (add a `level_gen::create_and_validate_*` case).
4. If infinite, decide its ore/cave rules in `mine_tile` (new `salt` offset, new
   `lava_threshold`/vein rules) — or, if it's a *surface-like* layer, give it its own
   `surface_tile`-style function.
5. Update every place that special-cases depth 0/-3/-4 by literal value: the gate-stamping
   `if depth == 0 || depth == -3` in `generate_chunk`, the `if lvl == 0` deepest-mine check
   in `open_chasm`, `try_spawn`'s `if depth < 0`/`if depth > 0` mob-level scaling, and
   `Level::empty`'s `if depth != -4 && depth != 0` monster-density branch.

## 8. Invariants & gotchas

- **Determinism is absolute for infinite layers.** Nothing in `infinite_gen.rs` may read
  wall-clock time, a mutable RNG stream, or any prior generation state — every value must
  be a pure function of `(seed, depth, x, y)` (plus fixed salts). This is what the
  `chunk_generation_is_deterministic` test locks in, and what makes independent-order
  chunk generation, save-by-omission (only dirty chunks persist), and multiplayer-style
  regeneration all safe. The classic finite generator (`level_gen.rs`) is explicitly
  *not* held to this for the surface's `HistoryGen` scenery pass in the original Java (it
  used a fresh time-seeded `Random` per call) — the port instead threads one shared `Rng`
  through, which is deterministic *given the same call sequence*, but the classic
  generator as a whole is still "build once, mutate a shared stream" rather than
  position-evaluable; don't port that shape into `infinite_gen.rs`.
- **Chunk-boundary stamping margins.** Any feature stamped in `generate_chunk` after the
  per-tile fill loop (currently only the gate rings) must independently re-derive from
  `(seed, depth, ...)` which features could overlap the *current* chunk from a
  neighboring cell, using an expanded search rect (chunk bounds + apron margin + stamp
  radius), then clip writes to the current chunk's actual bounds. Getting the margin
  wrong either drops half a stamped structure at a chunk seam or (worse) draws it
  inconsistently depending on which chunk generates first — there is no "first chunk
  wins" ordering guarantee since chunks generate in an arbitrary streaming order.
- **The take-out entity pattern interacts with chasm-digging.** `open_chasm` calls
  `ensure_chunks_at` and multiple `set_tile_default` calls while the *player* entity is
  still taken out of the arena mid-interact (`dug_pit_interact` receives `player: &mut
  Entity` by the caller's take-out convention — see PORTING.md §2). None of the terrain
  code queries the entity arena, so this is safe today, but any future depth-tile
  behavior that needs to look up *other* entities (e.g. "collapse damages nearby mobs")
  must account for the digging player being temporarily absent from arena queries.
- **Unloaded-chunk tile fallback is `rock`**, not "air" or "unknown" — see §2. Code that
  wants to distinguish "definitely rock" from "haven't generated this yet" cannot do so
  through `Game::tile_at`; it must go through `Level::tile_id` directly (or check
  `ChunkMap::is_loaded`) and handle `None` explicitly.
- **The uppercase tile-name convention.** `TileDef.name` is always stored uppercase
  (`TileDef::new` calls `.to_uppercase()`); `Tiles::get(name)` uppercases its argument
  before comparing, so lookups are case-insensitive by construction, but anything that
  compares `tile.name` directly against a literal (e.g. `set_area_tiles`'s `.to_lowercase()
  .contains("stairs")` check, or the Load-path `tilename.eq_ignore_ascii_case("LAPIS")`
  check) must remember the stored form is uppercase and normalize before comparing. Save
  files store tile *names* (not raw ids), so this convention is also why terrain content
  never needs a save-format migration when new tile ids are added — only if a tile is
  *renamed* does old save data break.
- **`GATE_GRID = 160` vs `CHUNK_SIZE = 64` are deliberately not aligned.** Gates are
  sparser than chunks and jitter within their cell — don't assume a 1:1 or clean-multiple
  relationship between the gate grid and the chunk grid when reasoning about how many
  chunks a single gate's apron can span (its 5×5-tile stamp radius means at most a 2-tile
  bleed past its own point in each direction, i.e. it can span into at most 2×2 chunks
  regardless of the grid mismatch).
- **`ChunkMap::local` uses bitwise `&`, not `%`,** specifically so negative tile
  coordinates resolve correctly — see §2. If `CHUNK_SIZE` is ever made non-power-of-two,
  this breaks silently (wrong tiles, not a panic); keep it a power of two, matching
  `CHUNK_SHIFT`'s whole reason for existing.
- **Infinite worlds still write a `WorldMeta` file even though most of the world data
  never touches classic per-level files** — don't assume "no WorldMeta file" means
  "nothing to load"; conversely a Classic-world save has *no* `WorldMeta` file at all
  (only `save_world_named` writes one, gated on `l.is_infinite()` being true for *any*
  level).

## 9. Test coverage map

| Test file | Locks in |
|---|---|
| `src/level/chunk.rs` (`coords_round_trip`, inline `#[cfg(test)]`) | Negative-coordinate chunk/local indexing round-trips; unloaded chunk reads as `None`. |
| `src/level/infinite_gen.rs` (`chunk_generation_is_deterministic`) | Same `(seed, depth, cx, cy)` → identical chunk bytes; different seed → different bytes. |
| `src/level/infinite_gen.rs` (`no_preplaced_stairs_on_infinite_layers`) | Generated chunks at depths 0/-1/-2 never contain `Stairs Down`/`Stairs Up` — descent is dig-only (§4), gates are the only exception (depth -3/0, and only near a gate point). |
| `src/level/infinite_gen.rs` (`biomes_are_large_and_all_present`) | Region-size regression guard (<40 biome changes over 2048 tiles) + coverage guard (6 named biomes appear in an 8k×8k sample). |
| `src/level/infinite_gen.rs` (`spawn_lands_on_grass`) | `find_surface_spawn` always returns a grass tile, for several seeds. |
| `src/level/infinite_gen.rs` (`mines_have_ores`) | Each mine depth's characteristic ore (iron/-1, gold/-2, gem/-3) appears >40 times in a 256×256 sample. |
| `tests/multi_level_terrain.rs` (`dig_down_through_the_world`) | Full dig state machine end-to-end: grass→dirt→Dug Pit stage 0..MAX_STAGE→Chasm, matching Ladder stamped one layer down, standing on Chasm/Ladder actually triggers `change_level` in the right direction. |
| `tests/multi_level_terrain.rs` (`deep_water_needs_a_raft`) | `deep_water_may_pass` blocks a raftless player and allows one carrying a "Raft" item. |
| `tests/infinite_world.rs` (`infinite_world_boots_and_walks`) | End-to-end: infinite world boots with the expected chunked/finite level split (surface+mines chunked, dungeon finite), chunks stream in around spawn (`>=25` loaded), long walk keeps streaming without panicking, out-of-range `tile_at` doesn't panic, save+reload round-trips the world type and seed. |
| `tests/biome_frames.rs` (`frames_across_biomes`) | Not an assertion-heavy test — a *visual* harness: teleports into each of six biomes and dumps a rendered PNG to `target/verify/biome_<name>.png` for eyeballing art/readability. Run with `cargo test --test biome_frames`; inspect the PNGs afterward (nothing fails automatically if a biome merely looks wrong). |
| `tests/level_gen_determinism.rs` | Classic finite generator: same seed -> same map; different seeds differ; all 4 `gen_type` × 5 `theme` combinations produce a map without error. |
| `tests/underground_gen.rs` (`underground_has_ores_and_caves`) | Classic finite mine generator: ore/rock/dirt tile-count thresholds per depth, stairs-down present (except depth -3, which has none in the classic generator either — the dungeon gate stamp at depth>2 replaces it), and zero surface tiles (`grass`/`tree`) leak underground. |
| `tests/underground_gen.rs` (`every_layer_has_stairs_down`) | Classic finite generator places at least one `Stairs Down` at every depth 0..-3 (progression isn't softlocked). |
| `src/level/infinite_gen.rs` (`ocean_has_skerries`) | Sparse rock stacks generate in open Ocean water, outside the tidal band (skerry cells actually come out as rock). |
| `tests/mining.rs` | Fossicking overhaul (§4.5): pan table pure + richness-scaled and only wet ground pans; cracked/dense hash distribution (~20%/~10%) and the 30/50/80 break thresholds; vein ping sparkles hidden ore within 2 tiles; collapse arms in a wide unpropped gallery (groan → fuse → rubble) and never with a Timber Prop in radius 3; rubble is weak and never cascades; prop place/break item round-trip; highland rock needs 100 damage and pays ≥3 stone. |
| `tests/flora_gen.rs` | Flora wave (§3.3.1): chunk determinism incl. flora; every species appears in its home biome over a wide ring sweep (incl. snow-capped cold Mountains and the Forest cold-fringe pine); cave mushrooms at every mine depth; jack-o-lanterns present-but-rare in cemetery/village blueprints; berry pick → regrow → re-pick → tear-out cycle; palm fells into Coconuts, dead tree into sticks-only; pumpkin/Jack-O-Lantern drops + light radii; thicket paddock-core-only blocking. |

Run everything terrain-related with `cargo test level` (matches test names/paths
containing "level") plus the explicit test-file names above for the ones that don't match
that filter (`tests/multi_level_terrain.rs`, `tests/infinite_world.rs`,
`tests/biome_frames.rs`, `tests/underground_gen.rs` don't have "level" in their path, so
run them by file: `cargo test --test multi_level_terrain --test infinite_world --test
underground_gen`, and `cargo test --test biome_frames` separately since it's a slow visual
dump best run on demand). See [DEV_GUIDE.md](DEV_GUIDE.md) for the `FDOOM_DEMO` scripted-run
driver if you want to *see* the game boot an infinite world interactively rather than read
a PNG dump — e.g. `just demo-world` generates a fresh world and screenshots gameplay.

## World structures (see src/level/structures_gen.rs)

Surface chunks get deterministic structures via the same hash-grid pattern as gates:
ruins (broken stone builds, 60% with a loot chest), cemeteries (grave plots that
decay over one to two in-game weeks and leak night zombies via the gravestone tile's
per-tile state), standing stones, abandoned camps (torch, loot chest), and rare
destroyed villages. Placement is pure `f(seed, cell)` per type with biome gating;
blueprints emit global tile writes clipped per chunk, so structures straddling chunk
borders are bit-identical from every side.

Each placement also rolls a **layout variant** from its hash (`variant_of`, equally
weighted, chunk-border safe like the blueprint itself):

| Kind            | Grid | Odds | Variants                                          |
|-----------------|-----:|-----:|---------------------------------------------------|
| Ruins           |  224 | 0.70 | square room / L-shaped two-room / round tower     |
| Cemetery        |  288 | 0.60 | fenced / unfenced overgrown (tufts) / stone-walled|
| Standing stones |  320 | 0.62 | ring / straight avenue (5-7) / dolmen cluster     |
| Camp            |  256 | 0.80 | plank lean-to / cold camp (fire ring + bedroll)   |
| Village         |  512 | 0.40 | round plaza / crossroads (roads meet at the well) |

The odds column reflects the density wave (+46% (measured) structures per unit area vs. the
original 0.45/0.40/0.35/0.50/0.40, biased toward camps/stones/ruins; villages
unchanged — they stay set pieces). Chest entities spawn only when a chunk generates
fresh (never when loaded from disk); the owning chunk is marked dirty so it persists
and never re-rolls. The module doc in `structures_gen.rs` is the authoritative
reference.
