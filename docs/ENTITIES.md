# Entities

Exhaustive reference for fdoom.rs's entity system: the arena, the `EntityCommon` +
`EntityKind` model, the Java-inheritance-as-nested-data layer stack, the take-out tick
pattern, the dispatch hubs in `src/entity/behavior.rs`, movement/collision, the player,
the full mob and furniture roster as it exists today, item entities/projectiles/particles,
and mob spawning. See also [ARCHITECTURE.md](ARCHITECTURE.md) for the whole-codebase tour
(it introduces `EntityArena`, the take-out pattern, and the nested-data-layer idea in
brief) and [TERRAIN.md](TERRAIN.md) for the terrain/dig/level-change machinery this
document's player and movement sections lean on rather than re-derive.

Every claim below is grounded in the source as of this writing; file references are
approximate anchors (line numbers drift) — grep the quoted symbol if a number is stale.

## 1. Overview + mental model

An entity is `EntityCommon` (position, collision radii, level index, eid, `removed`) plus
`kind: EntityKind`, an enum with one variant per concrete Java class. Where Java used class
inheritance (`Zombie extends EnemyMob extends MobAi extends Mob extends Entity`), the port
nests plain data structs and shares behavior through ordinary functions in
`src/entity/behavior.rs` (the Java `super.tick()` call becomes "call the parent layer's
function"). There are four such inheritance spines today — `Mob`, the furniture branch, and
two free-floating branches (projectiles/particles, item entities) that don't inherit from
anything below `Entity` itself:

```
EntityCommon (x, y, xr, yr, level, eid, removed, col)
        │
        ├── kind: EntityKind::Player(Box<PlayerData>)          PlayerData { mob: MobData, ... }
        │
        ├── kind: EntityKind::{Cow,Pig,Sheep,GlowWorm}          *Data { passive: PassiveMobData }
        │                                                         PassiveMobData { ai: MobAiData, color }
        │                                                           MobAiData { mob: MobData, xa/ya, ... }
        │                                                             MobData { health, dir, sprites, ... }
        │
        ├── kind: EntityKind::{Zombie,Snake,Knight,MarshLurker,  *Data { enemy: EnemyMobData, <leaf fields> }
        │         FeralHound,StoneGolem,NightWisp,Ghost}           EnemyMobData { ai: MobAiData, lvl, lvlcols, detect_dist }
        │                                                            MobAiData { mob: MobData, ... }
        │                                                              MobData { health, dir, sprites, ... }
        │
        ├── kind: EntityKind::{Furniture,Chest,Bed,Crafter,      *Data { furniture: FurnitureData, <leaf fields> }
        │         Lantern,Spawner,Tnt}                             FurnitureData { push_time, push_dir, sprite, name }
        │         (Chest is itself a mid-layer: DeathChest/
        │          DungeonChest both nest `chest: ChestData`)
        │
        ├── kind: EntityKind::ItemEntity(ItemEntityData)         flat — a dropped-item's own physics state
        ├── kind: EntityKind::{Arrow,Zap}                        flat — projectile state (owner eid, dir/velocity)
        └── kind: EntityKind::{Particle,TextParticle}            flat — purely visual, no gameplay
```

This mirrors ARCHITECTURE.md's `ZombieData { enemy: EnemyMobData { ai: MobAiData { mob:
MobData } } }` example exactly; §3 below walks that chain field-by-field for Zombie, then
more briefly for the `PassiveMob` (Cow) and `Furniture` (generic) branches. Every concrete
mob/furniture type is a small leaf struct wrapping the shared layer plus whatever fields
that concrete type added (e.g. `MarshLurkerData` adds `ambush_armed`/`ambush_recharge` on
top of `EnemyMobData`).

All live entities — player, mobs, furniture, item entities, projectiles, particles — sit in
one arena, `g.entities: EntityArena` (§2). "Which level" is just a field
(`EntityCommon.level: Option<usize>`), not a separate per-level collection; Java did linear
scans of a per-level `Set` anyway, so the arena instead filters by that field
(`EntityArena::entities_on_level`).

## 2. The arena (`src/entity/mod.rs`)

```rust
pub struct EntityArena {
    map: std::collections::HashMap<i32, Entity>,
}
```

**This is a `HashMap<i32, Entity>`, not a slab/`Vec<Option<Entity>>`** — worth flagging
explicitly because both ARCHITECTURE.md and PORTING.md describe it as "a slab of
`Option<Entity>` keyed by stable eid." That prose predates (or was simplified from) the
actual implementation; the real backing store is a hash map. The externally-visible
contract (`get`/`get_mut`/`take`/`put_back`/`insert`/`delete`, keyed by a stable integer id)
is exactly what the docs describe — only the concrete container differs. See the
discrepancy note at the end of this document.

**Eid.** There is no `Eid` newtype despite both docs using the name — an entity id is a
plain `i32` (`EntityCommon.eid: i32`, `EntityArena::map: HashMap<i32, Entity>`). A
newly-constructed, not-yet-placed entity has `eid: -1` (see `EntityCommon::new`, which sets
`removed: true` and `eid: -1`); `EntityArena::insert` assigns a real id the first time the
entity is actually inserted:

```rust
pub fn insert(&mut self, mut e: Entity, random: &mut Rng) -> i32 {
    if e.c.eid < 0 {
        e.c.eid = self.generate_unique_entity_id(random);
    }
    ...
}
```

`generate_unique_entity_id` draws from `g.random` (or whatever `Rng` the caller passes) and
retries until it finds a positive, not-already-present id — `0` is reserved for the main
player (`g.player_id`, set explicitly by world/save-load code rather than through this
path; see the `// JAVA: ids must be positive; 0 is reserved for the main player` comment).
Stability: once assigned, an id never changes for the life of that logical entity —
`take`/`put_back` round-trip the same id, and `Level::add_at` explicitly preserves an
existing non-negative eid rather than reassigning one. There is **no reuse guarantee** in
the other direction: a deleted entity's id is simply gone; a new random draw might
eventually collide-and-retry against it, but nothing recycles ids on a schedule.

**Level attachment.** `EntityCommon.level: Option<usize>` is an index into `g.levels`,
confirming ARCHITECTURE.md's description. It is `None` for a brand-new, not-yet-placed
entity and for the sleeping-player edge case (§13). Entities are *filtered* by this field,
not stored in a per-level collection:

```rust
pub fn ids_on_level(&self, level: usize) -> Vec<i32> { ... filter(|e| e.c.level == Some(level)) ... }
pub fn entities_on_level(&self, level: usize) -> impl Iterator<Item = &Entity> { ... }
```

**Getting into the arena: the `entities_to_add` queue.** `Level` (`src/level/mod.rs`) owns
two queues, `entities_to_add: Vec<Entity>` and `entities_to_remove: Vec<i32>` (Java's
`entitiesToAdd`/`entitiesToRemove`). Nothing goes directly into `g.entities` except through
this queue (and the one bootstrap exception below):

- `Level::add(entity, lvl_idx)` / `Level::add_at(entity, x, y, tile_coords, lvl_idx)` set
  `entity.c.level = Some(lvl_idx)`, clear `removed`, position it, and push onto
  `entities_to_add` (deduplicating any older pending add/remove for the same eid first).
  `add_at` is what actually moves an entity between levels — see §13 for who calls it on a
  stairs/chasm/ladder transition.
- `Level::remove(eid)` pulls the id back out of `entities_to_add` if it's still pending, or
  queues it onto `entities_to_remove`.
- `level::tick_level` (see the loop below) drains `entities_to_add` into `g.entities` via
  `EntityArena::insert` **every tick**, before the random-tile-tick and entity-tick passes,
  so an entity queued this tick is live and ticked in the same tick it was added. It then
  (only on a "full tick") ticks entities already on the level, and finally drains
  `entities_to_remove` into `EntityArena::delete`.
- **Bootstrap exception**: the main player is special-cased. `Game::with_entity`,
  `Game::player_mut`, and `Game::try_player` all additionally search every level's
  `entities_to_add` queue for `eid == g.player_id` if the arena lookup misses — so
  `g.player_id` stays usable even before the player has been drained into the arena for the
  first time (world init adds the player via `Level::add`, but the very first `Game::tick`
  hasn't run the drain yet).

`g.player_id: i32` (default `0`) names which arena entry is "the" player;
`Game::player()`/`player_mut()` panic if it's genuinely missing (mirroring Java's implicit
non-null uses), `Game::try_player()` returns `Option`.

## 3. Entity = EntityCommon + EntityKind

### 3.1 `EntityCommon` fields (`src/entity/mod.rs`)

| Field | Type | Purpose |
|---|---|---|
| `x`, `y` | `i32` | Position in entity-pixel coordinates; each tile is 16×16 (`x >> 4` = tile coord). |
| `xr`, `yr` | `i32` | Half-width/half-height of the collision box (Java `Entity.xr/yr`); `bounds()` builds a `Rectangle` centered on `(x, y)` sized `2*xr × 2*yr`. |
| `removed` | `bool` | Set by `remove_entity`/`die`; the entity is logically gone even before the arena/level queues catch up. |
| `level` | `Option<usize>` | Index into `g.levels`, or `None` if not attached to any level (fresh, or mid-sleep — §13). |
| `col` | `i32` | Current render color (a `color::get4`-style packed 4-shade value); mutated per-tick by render code (hurt flash, level-color mobs, etc.). |
| `eid` | `i32` | Stable id, `-1` until first inserted (§2). |

`EntityCommon::new(xr, yr)` is the one constructor everyone calls; it defaults
`x=y=0, removed=true, level=None, col=0, eid=-1` — the concrete `new()` for each kind then
overwrites `x`/`y`/`col` as needed. `bounds()` and `is_touching(area)` are the ported
`getBounds()`/`isTouching(Rectangle)`.

### 3.2 `EntityKind` variants (`src/entity/mod.rs`)

```rust
pub enum EntityKind {
    Player(Box<mob::player::PlayerData>),
    // passive mobs
    Cow(mob::cow::CowData), Pig(mob::pig::PigData), Sheep(mob::sheep::SheepData),
    GlowWorm(mob::glow_worm::GlowWormData),
    // enemy mobs
    Zombie(mob::zombie::ZombieData), Snake(mob::snake::SnakeData),
    Knight(mob::knight::KnightData),
    MarshLurker(mob::marsh_lurker::MarshLurkerData),
    FeralHound(mob::feral_hound::FeralHoundData),
    StoneGolem(mob::stone_golem::StoneGolemData),
    NightWisp(mob::night_wisp::NightWispData),
    Ghost(mob::ghost::GhostData),
    // free-floating things
    ItemEntity(ItemEntityData), Arrow(ArrowData), Zap(ZapData),
    Fireflies(fireflies::FirefliesData),   // ambient swarm; not a mob
    // particles
    Particle(ParticleData), TextParticle(TextParticleData),
    // furniture
    Furniture(FurnitureData), Chest(furniture::chest::ChestData),
    DeathChest(furniture::death_chest::DeathChestData),
    DungeonChest(furniture::dungeon_chest::DungeonChestData),
    Bed(furniture::bed::BedData), Crafter(furniture::crafter::CrafterData),
    Lantern(furniture::lantern::LanternData), Spawner(furniture::spawner::SpawnerData),
    Tnt(furniture::tnt::TntData),
}
```

`Entity { c: EntityCommon, kind: EntityKind }` is the whole entity. Layer accessors on
`Entity` are the "Java upcast" — `mob()`/`mob_mut()`, `mob_ai()`/`mob_ai_mut()`,
`enemy_mob()`/`enemy_mob_mut()`, `passive_mob()`, `furniture()`/`furniture_mut()`,
`chest()`/`chest_mut()`, and the panicking `player()`/`player_mut()` — each a `match
&self.kind { ... => Some(&nested.layer), _ => None }`. `instanceof` becomes predicates:
`is_player()`, `is_mob()` (`= mob().is_some()`), `is_mob_ai()`, `is_enemy_mob()`,
`is_furniture()`, `is_particle()`, `is_chest()`.

### 3.3 The Zombie chain, field by field

| Layer (Java class) | Rust struct | Fields it adds | Shared functions operating at this layer (`behavior.rs`) |
|---|---|---|---|
| `Mob` | `MobData` | `sprites: &'static MobAnims`, `walk_dist`, `dir: Direction`, `hurt_time`, `x_knockback`, `y_knockback`, `health`, `max_health`, `walk_time`, `speed`, `tick_time` | `mob_tick_base`, `mob_move`, `is_wooling`, `mob_is_light`, `is_swimming`, `mob_hurt_tile`, `mob_hurt_by_mob`, `mob_hurt_by_eid`, `mob_do_hurt_base`, `heal` |
| `MobAi` | `MobAiData` (has a `mob: MobData`) | `random_walk_time`, `random_walk_chance`, `random_walk_duration`, `xa`, `ya`, `lifetime`, `age`, `slowtick` | `mobai_tick_base`, `mobai_move`, `mobai_render`, `randomize_walk_dir`, `mobai_drop_items`, `mobai_check_start_pos`, `mobai_die`, `mobai_do_hurt` |
| `EnemyMob` | `EnemyMobData` (has `ai: MobAiData`) | `lvl`, `lvlcols: Vec<i32>`, `detect_dist` | `enemy_mob_tick_base`, `enemy_mob_render`, `enemy_mob_die`, `enemy_check_start_pos`, `enemy_touched_by` (private; called from the `touched_by` dispatch) |
| `Zombie` (leaf) | `ZombieData` (has `enemy: EnemyMobData`) | *(no extra fields — Zombie is the "vanilla" enemy mob)* | `zombie::new/tick/die` (leaf; `tick` is a one-line call to `enemy_mob_tick_base`) |

So the full path from an `Entity` whose `kind` is `Zombie` down to raw health is
`e.kind → ZombieData → enemy: EnemyMobData → ai: MobAiData → mob: MobData → health`, and
`e.mob()`/`e.enemy_mob()`/`e.mob_ai()` are exactly the accessors that walk that path (see
the `mob()` match arm: `EntityKind::Zombie(m) => &m.enemy.ai.mob`).

`EnemyMobData::new`/`with_default_lifetime`/`simple` are three constructor shapes (fewer
args = more defaults) mirroring Java's overloaded `EnemyMob` constructors; the health
formula (`is_factor` case) is:

```
max_health = (lvl == 0 ? 1 : lvl * lvl) * health * 2^diff_idx
```

— i.e. mob level squares the base health, and difficulty doubles it per step
(`diff_idx` 0/1/2 for Easy/Normal/Hard). `col` (`EntityCommon.col`) is set from
`lvlcols[lvl - 1]` at construction; `enemy_mob_render` re-applies this every frame from the
*current* `lvl`/`lvlcols`.

### 3.4 The PassiveMob chain (Cow, more briefly)

| Layer | Struct | Adds | Shared functions |
|---|---|---|---|
| `Mob` | `MobData` | (same as above) | (same as above) |
| `MobAi` | `MobAiData` | (same as above) | (same as above) |
| `PassiveMob` | `PassiveMobData` (has `ai: MobAiData`) | `color: i32` | `passive_mob_render`, `passive_mob_die`, `passive_check_start_pos`; `randomize_walk_dir` has a **PassiveMob-specific branch** inlined in the shared function (see §5) |
| `Cow` (leaf) | `CowData` (has `passive: PassiveMobData`) | *(none)* | `cow::new/tick/die`; `tick` = `mobai_tick_base` directly (no `PassiveMob`-level tick override exists in Java either) |

`PassiveMobData::new(sprites, color, health_factor, diff_idx)` computes
`max_health = 5 + health_factor * diff_idx` and a fixed `lifetime = 5*60*NORM_SPEED`,
`random_walk_duration = 45`, `random_walk_chance = 40` — passive mobs don't scale health by
mob level (they have no `lvl` field at all; only enemy mobs do). Pig/Sheep/GlowWorm are
structurally identical to Cow at the data-layer level; only the sprite sheet coordinates,
`color`, `health_factor`, and `die()` drop table differ (§8).

### 3.5 The Furniture chain (generic)

| Layer | Struct | Adds | Shared functions (`furniture/behavior.rs`) |
|---|---|---|---|
| `Furniture` | `FurnitureData` | `push_time`, `multi_push_time`, `push_dir: Direction`, `sprite: Sprite`, `name: String` | `tick` (apply pending push), `render`, `try_push`, `take` (power-glove pickup), `use_furniture` (dispatch to per-kind `use`) |
| concrete furniture (leaf) | e.g. `ChestData { furniture: FurnitureData, inventory: Inventory }`, `TntData { furniture, ftik, fuse_lit, explode_ticks_left }`, `SpawnerData { furniture, mob: Box<Entity>, health, lvl, max_mob_level, spawn_tick }`, ... | whatever that one Java subclass added | per-kind `*_behavior.rs` module |

Some furniture kinds nest *two* nominal layers: `DeathChestData { chest: ChestData, time,
redtick, reverse }` and `DungeonChestData { chest: ChestData, is_locked }` both wrap a full
`ChestData` (which itself wraps `FurnitureData` + `inventory`) — mirroring Java's
`DeathChest extends Chest extends Furniture`. `Entity::chest()`/`chest_mut()` walk through
either directly (`Chest(c) => c`) or through the nested field
(`DeathChest(c) => &c.chest`), exactly like the `mob()` accessor does for the enemy chain.

## 4. Take-out tick pattern & reentrancy rules

```rust
// src/entity/behavior.rs
pub fn with_entity<R>(&mut self, eid: i32, f: impl FnOnce(&mut Entity, &mut Game) -> R) -> Option<R> {
    let mut e = match self.entities.take(eid) {
        Some(e) => e,
        None if eid == self.player_id => self.take_player_from_queues()?,
        None => return None,
    };
    let r = f(&mut e, self);
    self.entities.put_back(e);
    Some(r)
}
```

`EntityArena::take(eid)` is a plain `HashMap::remove` — the entity is **fully absent** from
the arena for the duration of the closure, not just borrow-checker-invisible. Consequences,
precisely:

```
tick loop calls with_entity(A) ──► A removed from arena, ticked as (&mut A, &mut Game)
                                     │
                                     │  A's tick calls entity_move → touches B
                                     │  (movement/interact code calls with_entity(B) itself)
                                     ▼
                                    with_entity(B) ──► B removed from arena too, ticked
                                     │                  (arena now missing BOTH A and B)
                                     │
                                     │  if B's code calls g.entities.get(A) here: None
                                     │  (A is still taken out one level up the call stack)
                                     ▼
                                    B's closure returns ──► B reinserted (put_back)
                                     │
                                     ▼
                                    A's closure finishes ──► A reinserted, UNLESS A.c.removed
                                                              was set (die()/remove_entity()) —
                                                              then A is simply never put back:
                                                              it stays gone from the arena.
```

- **Nesting**: `with_entity` calls nest freely — A touching B takes B out too, while A is
  still out. `entity_move2`'s entity-collision loop is exactly this: it calls
  `g.with_entity(other_id, |other, g| touched_by(g, e, other))` for every entity newly
  overlapped, while the mover (`e`) is the caller's already-taken-out `&mut Entity`.
- **Removal inside the closure**: nothing in `with_entity` special-cases `e.c.removed`;
  `remove_entity`/`die` just flip the flag (and queue the level's `entities_to_remove` — see
  §2), and the entity is still `put_back` into the arena at the end of the closure like
  normal. It is the *next* `tick_level` pass — draining `entities_to_remove` — that actually
  calls `EntityArena::delete` and drops it for good. So "removed" entities can transiently
  sit in the arena for the remainder of the tick they died in, still queryable by
  `g.entities.get`, just filtered out of iteration wherever code checks `.c.removed` (which
  every consumer of `ids_on_level` etc. does).
- **Queries during take-out return `None`/are skipped**: `g.entities.get(eid)` returns
  `None` for a currently-taken-out entity; anything that predates a `with_entity` call and
  then loops over ids (`tick_level`'s entity loop, `try_spawn`'s "closest player" lookups,
  ...) simply treats a `None` as "not there right now" and moves on — this mirrors Java's
  `if (e == this) continue;` idiom exactly, per PORTING.md.
- **The player is additionally reachable from `entities_to_add`** while it hasn't been
  drained into the arena yet (§2) — `with_entity`'s `None if eid == self.player_id` branch,
  and the parallel logic in `player()`/`player_mut()`/`try_player()`.

**The concrete rule**: never assume a `g.entities.get`/`get_mut` lookup during a tick will
succeed just because you know the entity logically exists — it may be the very entity
currently being ticked one frame up the call stack (including *itself*, if some code path
tried to look up its own eid instead of using the `&mut Entity` already in hand — see
ARCHITECTURE.md's explicit warning about not calling `g.player()` from inside the player's
own tick). Always handle `None`.

This is exactly the hazard TERRAIN.md's §8 documents for chasm digging: `dug_pit_interact`
receives the player as an already-taken-out `&mut Entity`, so any future depth-tile behavior
that wants to look up *other* nearby entities mid-dig (e.g. "collapse damages nearby mobs")
must go through the normal "may return `None`" discipline, not assume the arena is complete.

## 5. Dispatch hubs in `src/entity/behavior.rs`

Five dispatch hubs exist (Java virtual methods), each a `match &e.kind` (or `&this_e.kind`)
fanning out to a per-kind module function:

| Hub | Signature (abridged) | Matches on |
|---|---|---|
| `entity_tick` | `fn(g: &mut Game, e: &mut Entity)` | `e.kind` |
| `entity_render` | `fn(g: &mut Game, screen: &mut Screen, e: &mut Entity)` | `e.kind` |
| `touched_by` | `fn(g: &mut Game, this_e: &mut Entity, by: &mut Entity)` | `this_e.kind` |
| `die` | `fn(g: &mut Game, e: &mut Entity)` | `e.kind` |
| `entity_interact` | `fn(g: &mut Game, this_e: &mut Entity, player: &mut Entity, item: &mut Option<Item>, attack_dir: Direction) -> bool` | `this_e.kind` (only `Spawner`/`Tnt` override; everything else forwards to `item.interact(...)`) |

Representative arms (not exhaustive — see the source for the full match):

- `entity_tick`: `Player` → `player_behavior::tick`; `Cow`/`Pig`/`Sheep` → their own leaf
  `tick`, which is a one-line call to `mobai_tick_base`; `Zombie`/`Snake`/`Knight`/
  `FeralHound` similarly one-line-call `enemy_mob_tick_base`; `MarshLurker`/`StoneGolem`/
  `NightWisp` have real leaf bodies (water-speed + ambush re-arm / heavy-tread stall /
  dawn-despawn + zap cooldown — §8); furniture kinds mostly share
  `furniture::behavior::tick`, except `DeathChest`, `Spawner`, and `Tnt`, which have their
  own `tick` (expiry countdown, spawn-interval countdown, fuse/explosion respectively).
- `entity_render`: passive mobs share `passive_mob_render`; `Zombie`/`Snake`/`Knight`/
  `MarshLurker`/`FeralHound`/`StoneGolem` share `enemy_mob_render`; `GlowWorm` (single
  static sprite) and `NightWisp` (tick-timed two-frame pulse) have bespoke `render`s.
- `touched_by`: `ItemEntity` → pickup logic; `Zombie`/`Knight`/`FeralHound`/`NightWisp`
  route to the shared `enemy_touched_by` (standard `lvl * (hard?2:1)` damage);
  `Snake` (`lvl + diff_idx` — its once-dead custom override is now wired),
  `MarshLurker` (standard formula +2 on an armed ambush strike), and `StoneGolem`
  (`2*lvl + diff_idx`) have their own arms; any furniture falls to "player pushes it"
  unless it's a `DeathChest`/`DungeonChest`, which override for retrieve-on-touch /
  locked-can't-be-pushed respectively.
- `die`: every mob has a real per-kind `die` (drop tables, score); `Chest`/`DeathChest`/
  `DungeonChest` all share `chest_behavior::die` (spill inventory then remove); everything
  else defaults to plain `remove_entity`.
- `entity_interact`: only `Spawner` (pickaxe damage / power-glove pickup / creative
  level-cycling) and `Tnt` (light the fuse) override; every other kind falls through to
  "forward to the player's held item's own interact logic" (`item_interact::
  item_interact_entity`) — e.g. a Chest's actual "open the container" behavior is reached
  through `Furniture.use_furniture` (menu key), not `interact` (attack/pickup key), matching
  Java's separate `use()`/`interact()` methods.

### 5.1 `Mob` layer — `mob_tick_base`

The one thing every mob (including the player, via its own `tick` calling this first) gets:
increments `tick_time`; bails if already `removed`; applies lava self-damage if standing on
a `LAVA` tile (`mob_hurt_tile`, 4 damage, respecting the Lava-potion immunity check only for
players); dies if `health <= 0`; counts down `hurt_time`; applies knockback decay
(`x_knockback`/`y_knockback` ease toward zero, each step moving the mob via `mob_move`).
`is_swimming`/`is_wooling`/`mob_is_light` are simple tile-name lookups at the mob's current
tile (`WATER`/`LAVA`, `WOOL`, and the level's light map respectively). There is no "walking
sound" call anywhere in this layer — sound is per-kind (e.g. `mobai_do_hurt` plays
`Sound::MonsterHurt` when a player is near).

### 5.2 `MobAi` layer — `mobai_tick_base`

Adds: age/lifetime expiry (`remove_entity` once `age > lifetime`, for kinds with a finite
`lifetime`); the "Time potion nearby → slowtick" check (any player within 8 tiles holding
the Time potion effect sets `ai.slowtick`, which then makes `skip_tick` return true on 3 out
of every 4 ticks — a global mob-slowdown effect); if not skipped, moves by `ai.xa/ya *
speed` via `mobai_move` (= `mob_move` with `change_dir = true`); with `1/random_walk_chance`
probability, calls `randomize_walk_dir` to pick a new wander direction; counts down
`random_walk_time`. `randomize_walk_dir` has an explicit **PassiveMob branch inlined in the
shared function** (checked via `e.passive_mob().is_some()`) rather than a separate
override function — passive mobs get a "50% chance of just standing still" multiplier
(`(next_int_bound(3)-1) * next_int_bound(2)` for each axis) that enemy mobs don't.

**Movement personalities** (mob-life wave): `MobAiData.movement_style: MovementStyle`
(`Classic` default — the untouched original walk) is consumed by `style_step` inside
`mobai_tick_base`: `Slither`/`Curve` add a perpendicular S-curve side-offset (applied
via a separate `entity_move` so the facing direction never flips), `FreezeBurst` holds
still ~2 s then bursts at double step, `SineFloat` layers a vertical bob, and `Circle`
leaves the step alone but reshapes the *chase targeting* in `enemy_mob_tick_base`
(`circle_chase`: orbit the target at ~4 tiles tangentially, straight lunge one beat in
three). Zombies, knights, and all passive mobs stay `Classic`.

**Tall-grass stealth** (`mobai_render`): a mob standing on a `TallGrass` tile sinks in
(bottom sprite row clipped, like the swimming clip); a *hostile* mob there at night is
drawn as nothing but two warm eye-glint pixels (true-color cell (11,20) — palette-mode
yellow would be laundered gray by `color::upgrade` + the night grade). A hurt flash
always reveals the whole body.

### 5.3 `EnemyMob` layer — `enemy_mob_tick_base`

Adds the actual chase AI: find the closest player (`get_closest_player`); if nobody is
asleep (`g.bed_state.players_awake != 0`) and the mob isn't mid-random-walk, and the player
is within `detect_dist` tiles (squared-distance check), set `ai.xa`/`ai.ya` to walk directly
toward the player (one step per axis, not normalized — diagonal chase is just as fast as
axis-aligned); otherwise fall back to `randomize_walk_dir`. `enemy_mob_render` refreshes
`e.c.col` from `lvlcols[lvl - 1]` every frame before delegating to `mobai_render`.
`enemy_mob_die` awards `50 * lvl` score and a `+1` multiplier via `mobai_die`.
`enemy_check_start_pos` is the enemy-specific spawn-position gate (§11).

### 5.4 `PassiveMob` layer — `passive_mob_render` / `passive_mob_die` / `passive_check_start_pos`

No dedicated `passivemob_tick_base` exists — passive mobs tick with plain `mobai_tick_base`
(there is no Java `PassiveMob.tick()` override either; the class only overrides `render`,
`die`, and `checkStartPos`). `passive_mob_render` sets `e.c.col` from the passive mob's
fixed `color` field (not level-dependent, since passive mobs have no `lvl`) then calls
`mobai_render`. `passive_mob_die` awards a flat 15 score, no multiplier. There is **no
flee-from-player behavior and no breeding mechanic** in the current source — passive mobs
wander exactly like the base `MobAi` random-walk, with no special reaction to player
proximity or to each other. If you were expecting Minecraft-style breeding/fleeing here:
it doesn't exist; don't assume it does when reading spawn or AI code.

## 6. Movement/collision (`src/entity/behavior.rs`)

`entity_move(g, e, xa, ya)` is the two-axis wrapper (Java `Entity.move`): it calls
`entity_move2(g, e, xa, 0)` then `entity_move2(g, e, 0, ya)` — **axis-split exactly like
Java**, so diagonal movement is resolved as two independent single-axis attempts (you can
slide along a wall on one axis even if the diagonal combination would be blocked). If
either succeeded, it re-fires `tiles::stepped_on` for the tile now under the entity. A
no-op `(0, 0)` move, or any move while `g.saving`, is treated as "succeeded" without doing
anything (`return true` — "pretend it kept moving").

`entity_move2(g, e, xa, ya)` (one axis) does, in order:

1. Bail `false` if the entity has no `level`.
2. Compute the tile-rectangle the entity's collision box would occupy *before* and *after*
   the move.
3. For every tile in the post-move rectangle that wasn't already occupied pre-move: call
   `tiles::bumped_into` (used by tiles like cactus to deal contact damage), then check
   `tiles::may_pass`. **Any** blocking tile aborts the whole move (`return false`) — this is
   the "solid tile" case.
4. Otherwise, snapshot which entities intersect the current box (`was_inside`) and which
   would intersect the post-move box (`is_inside`), via `level::get_entities_in_rect`.
5. For every entity newly entered (`is_inside` minus `was_inside`, excluding self):
   call `touched_by`. There's a Java-preserved asymmetry here: **if the *other* entity is
   the player and the mover is not**, the mover gets `touched_by(g, mover, player)` called
   *on the mover* (so e.g. a Zombie moving into the player triggers the player-damage path
   keyed off the Zombie's own `touched_by`); otherwise it's the normal
   `touched_by(g, other, mover)` (mover touches other).
6. For every entity newly entered that `blocks(other, mover)` (both solid, or `other` is
   furniture): abort the move (`return false`) — this is the "collides with another entity"
   case, checked *after* touch callbacks have already fired (so e.g. Furniture's `try_push`
   or a Marsh Lurker's ambush strike happens even on a blocked bump).
7. If nothing aborted: actually apply `e.c.x/y += xa/ya`, return `true`.

`may_pass` interplay with tiles: `tiles::may_pass(g, tile, lvl, x, y, e)` is the tile
dispatch hub in `src/level/tile/dispatch.rs` — most tiles fall through to "solid iff
`TileDef`'s material says so"; a handful override per-entity, most notably
`deep_water_may_pass` (only a `Player` carrying a "Raft" item, or in creative mode, or an
`ItemEntity` — see TERRAIN.md §4). `is_solid(e)` (in `behavior.rs`) is the entity-side half
of "does this entity block others at all" — everything is solid except `ItemEntity`,
`Arrow`, `Zap`, `Particle`, `TextParticle`; `blocks(this, other)` special-cases furniture
to always block regardless of `is_solid` (furniture blocks everything, matching Java's
`Furniture.blocks()` override).

## 7. Player specifics (`src/entity/mob/player.rs` + `player_behavior.rs`)

### 7.1 Tick order

`player_behavior::tick(g, e)`, top to bottom:

1. Bail if no level or already removed. Refresh `inventory.creative` from `g.is_mode`.
2. Bail (do nothing) if a menu is open (`g.menu_open()`) — the player entity still exists
   and is still ticked every game tick regardless (per ARCHITECTURE.md), it just skips its
   own body while a menu has focus.
3. `mob_tick_base` (lava damage, health<=0 death check, knockback, hurt-time countdown).
4. Tick the score multiplier countdown (`tick_multiplier`).
5. Tick down every active potion effect's remaining time by 1 (unless asleep in a bed);
   at `time <= 1`, auto-remove via `apply_potion(..., false)`.
6. Tick the potion-effects-HUD toggle cooldown; toggle on the `potionEffects` key.
7. **Stairs/chasm/ladder/quicksand tile check** — cross-reference
   [TERRAIN.md §4](TERRAIN.md#4-multi-level-terrain-srclleveltiledepthrs) for the full
   dig/chasm/ladder state machine this feeds; the player-side logic here just checks the
   *current* tile id against `Stairs Down`/`Stairs Up`/`Quick Sand`/`Chasm`/`Ladder` and, if
   `on_stair_delay <= 0`, sets `g.pending_level_change` (`1` = up, for Stairs Up or Ladder;
   `-1` = down, for everything else) and returns early, skipping the rest of that tick.
8. Creative mode: force `stamina`/`hunger` to max (skip decay).
9. Stamina recharge-delay bookkeeping (40-tick penalty once stamina hits 0; recharge one
   bolt per `MAX_STAMINA_RECHARGE` accumulated "charge ticks", paused while swimming unless
   the Swim potion is active).
10. **Hunger system** (detailed in §7.3 below), including the health-regen-from-hunger and
    starve-damage sub-systems — skipped entirely while asleep in a bed.
11. Regen potion effect: +1 health every 60 ticks while active, capped at 10.
12. Save-cooldown countdown.
13. If not menu-open and not asleep: **movement input**, drop-item keys, **attack/pickup
    keys** (§7.2), menu-open keys (map/inventory/craft/info/pause), save key, debug keys,
    and finally the `attack_time` countdown (nulls `attack_item` at 0).

### 7.2 Attack flow

Triggered by the `attack` or `pickup` key being `.clicked` (edge-triggered, not held) and
`stamina != 0`. Stamina is spent immediately (1 point, waived by the Energy potion) before
anything else happens. `pickup` additionally swaps the active item for a fresh Power Glove
first (saving the real item to `prev_item`), calls `attack`, then `resolve_held_item` puts
the real item back — this is how "pick up furniture" and "use the power glove to grab
things" share the same attack call.

`attack(g, e)` (`player_behavior.rs`):

- Bumps `walk_dist` by 8 (arm-swing animation), even for reflexive (non-attack) items.
- If the active item "doesn't interact with the world" (a narrow reflexive-item category),
  it calls `item_interact_on_tile` against a dummy rock tile at `(0,0)` and returns — this
  branch never reaches combat at all.
- Otherwise: records `attack_dir`/`attack_item` from the current facing/held item.
- **Bow special case**: if holding a `Tool { ttype: Bow }` with durability left, stamina
  available, and at least one Arrow in inventory: consumes an arrow (unless creative),
  spawns an `Arrow` entity via `projectile::new_arrow`, sets `attack_time = 10`, pays 1
  tool durability, and returns — no melee/tile interaction happens on a bow swing.
- If holding *any* item: sets `attack_time = 10`, tries `interact_area` (the interaction box,
  `INTERACT_DIST = 12` deep, on entities) first, then falls back to the target tile
  (`get_interaction_tile`, same `INTERACT_DIST`) — item-on-tile interact, then tile-on-item
  interact (e.g. a shovel digging).
- If nothing above returned early: **melee hit**. `attack_time = 5`; `hurt_area` sweeps the
  `ATTACK_DIST = 20`-deep interaction box for mobs (`get_attack_damage` — base
  `1 + rand(0..2)` plus a tool-specific bonus, §7.2.1) and furniture (a `null`-item
  `entity_interact` call, which — per the dispatch table — only actually does anything for
  `Spawner`/`Tnt`); the target tile is also hit for `1 + rand(0..3)` damage
  (`tiles::hurt_by`). Tool durability is paid only if the sweep actually hit something.

**7.2.1 Damage formula** (`get_attack_damage_bonus`, only reached for `Tool` items, target
is always a mob; tiers run Crude=0 .. Gem=5 — the post-port Crude tier shifted the Java
Wood..Gem levels up by one):

| Tool type | Damage bonus formula | Crude tier (level 0) | Gem tier (level 5) |
|---|---|---|---|
| Axe | `(level+1)*2 + rand(0..4)` | 2–5 | 12–15 |
| Sword | `(level+1)*3 + rand(0..2+level²)` | 3–4 | 18–44 |
| Claymore | `(level+1)*3 + rand(0..4+3*level²)` | 3–6 | 18–96 |
| anything else | `1` | — | — |

(Ranges above are the bonus only, per the source's own doc-comments; add the base
`1 + rand(0..2)` from `get_attack_damage` for the total. `pay_durability` is called first
and must succeed — a depleted tool contributes zero bonus.) `ToolType::Pickaxe`/`Shovel` do
not appear in this table because they're not attack tools; their stamina/durability cost is
paid through `dug_pit_interact` (TERRAIN.md §4), a completely separate code path.

### 7.3 Stamina system

- **Costs**: 1 per attack/pickup click (§7.2, waived by the Energy potion); tool durability
  is a *separate* resource, not stamina; `dug_pit_interact`'s `4 - tool_level` stamina cost
  for shovel/pickaxe digging (TERRAIN.md §4); drowning (swimming without the Swim potion,
  every 60 ticks) costs 1 stamina or, once at 0, 1 health instead; eating food costs
  `stamina_cost` (a per-`Food`-item field, §7.4); TNT/explosion damage costs `dmg * 2`
  stamina (`hurt_by_tnt`).
- **Regen**: `stamina_recharge` accumulates 1 per tick (while `stamina_recharge_delay == 0`
  and not swimming without the Swim potion); every `MAX_STAMINA_RECHARGE = 10`
  accumulated, +1 stamina (capped at `MAX_STAMINA = 10`). Hitting 0 stamina imposes a
  40-tick `stamina_recharge_delay` before regen resumes. `pay_stamina` is a no-op success
  while the Energy potion is active (infinite stamina).
- Movement itself has **no direct stamina cost** — running doesn't drain stamina; only
  being *out* of stamina halves effective move speed (`stamina_recharge_delay % 2 == 0`
  gates whether the movement branch executes at all that tick).

### 7.4 Hunger system

`stam_hunger_ticks` ticks down from `MAX_HUNGER_TICKS = 400` by several sources each
player tick: 1 per `HUNGER_TICK_COUNT[diff]` ticks elapsed (time), 1 per
`HUNGER_STEP_COUNT[diff]` steps taken (exercise), extra penalties while `hunger_charge_delay`
is actively healing you, and while stamina is below max (doubled if stamina is exactly 0).
When it bottoms out, it wraps back to `MAX_HUNGER_TICKS` and `hunger_stam_cnt` (init
`MAX_HUNGER_STAMS[diff]`) drops by 1; when *that* bottoms out, actual `hunger` (0..10) drops
by 1 and `hunger_stam_cnt` resets. All four arrays (`MAX_HUNGER_STAMS`, `HUNGER_TICK_COUNT`,
`HUNGER_STEP_COUNT`, `MIN_STARVE_HEALTH`) are indexed `[Easy, Normal, Hard]` and get *harsher*
with difficulty (fewer stam-ticks needed to lose a hunger point, more frequent time/step
penalties, lower health floor before starving hurts).

Restoration: `ItemKind::Food { count, heal, stamina_cost }` — eating (only possible below
`MAX_HUNGER`, and only if `pay_stamina(stamina_cost)` succeeds) adds `heal` to `hunger`,
capped at `MAX_HUNGER`. Regeneration-from-hunger: while `health < MAX_HEALTH` and
`hunger > MAX_HUNGER/2`, `hunger_charge_delay` accumulates and, once it exceeds
`20 * (MAX_HUNGER - hunger + 2)²` (a quadratic — heals faster the fuller you are), grants
+1 health and resets. Starvation: at `hunger == 0` and `health > MIN_STARVE_HEALTH[diff]`, a
120-tick `hunger_starve_delay` counts down and then deals 1 damage via `do_hurt`, repeating
every 120 ticks while still starving.

### 7.5 Potions

`PlayerData.potioneffects: HashMap<PotionType, i32>` (remaining ticks). `PotionType` has 11
variants: `None, Speed, Light, Swim, Energy, Regen, Health, Time, Lava, Shield, Haste`.
`apply_potion(g, player, ptype, add_effect)` toggles a type's on/off *side effects*
(`toggle_effect` — e.g. Speed nudges `move_speed` by ±1.0) independently of the timer, then
sets or clears the `potioneffects` entry; `apply_potion_time` is the "drink a potion" entry
point that also seeds the duration. Every player tick (§7.1 step 5) walks a snapshot of the
current keys and either decrements the remaining time or, at `<= 1`, calls
`apply_potion(..., false)` to run the "turn it off" side effect and drop the map entry —
so expiry is driven entirely from the tick loop, not a separate timer system. Time,
Swim, and Energy potions are read directly by other systems (`mobai_tick_base`'s slowtick
check, the swim-stamina-drain check, `pay_stamina`) rather than through `toggle_effect`.

### 7.6 Death

`player_behavior::die(g, e)`: applies the score penalty (subtract 1/3 of current score),
resets the multiplier, then builds a **Death Chest** (`furniture::death_chest::new(g)`) at
the player's exact position, filling it with the player's entire inventory plus the active
item and worn armor if any (`death_chest.rs`/`death_chest_behavior.rs` — see §9 for the
chest's own decay-timer mechanic), plays `Sound::PlayerDeath`, adds the chest to the current
level, and finally calls `remove_entity(g, e)` on the player (Java's `super.die()` →
`Entity.die()` → `remove()`). Respawn is a separate call
(`player_behavior::respawn`/`find_start_pos`, invoked by `world::reset_game` when
`g.should_respawn`) — it does **not** happen automatically inside `die`; something external
(the death display's "respawn" menu entry, or a test's `tick_and_recover` helper) must
trigger it.

## 8. Mobs roster as it exists today (`src/entity/mob/`)

> **Roster overhaul (done):** the Minecraft-derived mobs — **Creeper, Slime, Skeleton** —
> and the dormant **AirWizard** boss (with its `Spark` projectile) have been removed.
> Four original mobs replaced them: **Marsh Lurker, Feral Hound, Stone Golem, Night
> Wisp**. The `Zap` projectile is the old Spark code adapted as the Night Wisp's ranged
> attack. Old saves containing removed mobs load fine — unknown entity names are skipped
> with a `LOAD WARNING` log (§12.1); a Spawner whose template mob is gone falls back to
> its default Zombie template.
>
> **Mob-life wave (done):** the single Snake became a four-variant family (Grass
> Snake / Adder / Rattler / Cave Serpent — the last keeps the `"Snake"` save name);
> the **Ghost** rises from broken graves at night; **Fireflies** drift out at dusk as
> a non-mob ambient swarm; and the shared MobAi layer gained movement personalities
> (`MovementStyle` — §5.2) plus tall-grass stealth rendering.

| Mob | Data layer | Notable behavior |
|---|---|---|
| Cow | `PassiveMobData` | Wanders; drops leather + raw beef on death (amount scales inversely with difficulty). |
| Pig | `PassiveMobData` | Wanders; drops raw pork on death. |
| Sheep | `PassiveMobData` | Wanders; drops wool on death — **no shearing mechanic** (explicit `// JAVA:` note that this fork never added wool-cutting). |
| GlowWorm | `PassiveMobData` | Single static 1×1 sprite; ambient light source (radius 2); self-removes outside night/evening; spawns as a side-effect of any surface passive-mob roll, placed beside the mob it escorts (the Java raw-`(0,0)` quirk was fixed post-port — §11). |
| Zombie | `EnemyMobData`, 4 mob levels | Chases and touches for damage; no leaf-level tick/render override (pure `enemy_mob_tick_base`); drops cloth (scales with difficulty), rare iron (1/60), rare colored-clothes (1/40). Cemetery staple. |
| Snake family | `SnakeData { enemy, variant, coiled, rattled, strike_primed }`, **5 mob levels**, one `EntityKind::Snake` with a `SnakeVariant` tag | **Mob-life wave**: the classic Snake became four zone-scaled variants, all `Slither` movers with the classic `lvl + diff_idx` base bite (dead-code dispatch bug long fixed). **Grass Snake** (`"GrassSnake"`): plains/forest ambience, health factor 2, harmless — inverts the chase and flees; rare single scale. **Adder** (`"Adder"`): marsh/savanna, health 6; bite adds a 2-stamina drain (same hurt-cooldown gate as the damage). **Rattler** (`"Rattler"`): desert, health 7; spawns **coiled** (own 16x16 pose, cell (4,20)) and sits still; a one-time dry-rattle warning (notification + sound cue) when the player is within 4 tiles; at ~1.5 tiles it uncoils with a primed strike worth **2x** damage, then slithers normally. **Cave Serpent** (saves as `"Snake"` — save-name compat; mines/dungeon, where the classic spawned): health 10/12, bite `lvl + diff_idx + 2`, dark palette. All variants drop scale + rare key except the Grass Snake. |
| Knight | `EnemyMobData`, 5 mob levels | Hostile dungeon keeper — standard `lvl*(hard?2:1)` touch damage via the shared `enemy_touched_by` path; drops shard, rare key drop; spawns naturally in the dungeon (§11). |
| Marsh Lurker | `MarshLurkerData { enemy, ambush_armed, ambush_recharge }` | **Original mob.** Lurks in marsh water: `can_swim` = true, leaf `tick` sets `speed = 2` while swimming / `1` on land (net: full walk speed in water, half on land). Ambush: an *armed* first touch deals the standard formula **+2**, then disarms; re-arms only after 300 ticks spent back in water. Short `detect_dist` 80, base health 6. Drops raw fish 0–2, rare Slime item (1/12). |
| Feral Hound | `FeralHoundData { enemy }` | **Original mob.** Plains/savanna pack hunter: spawns in packs of 2–3 (§11), `walk_time = 1` (full player speed — twice a normal mob), long `detect_dist` 120, fragile (base health 3). Standard touch damage. Drops leather 0–1, rare raw beef (1/20). |
| Stone Golem | `StoneGolemData { enemy }` | **Original mob.** Mines only: very slow (leaf `tick` stalls the chase acceleration outside a 1-in-4 tick window, on top of the shared `walk_time = 2` gate ⇒ half a normal mob's pace), very tanky (base health 12), heavy melee `2*lvl + diff_idx` via its own `touched_by`. Short `detect_dist` 60 — a lair guard. Drops stone 1–3 + coal 0–2, rare iron (1/8), rare gold (1/20). |
| Ghost | `GhostData { enemy }` | **Mob-life wave.** Rises from broken-grave tiles at night (`ghost::try_rise`, rolled in `try_spawn`; **mass-rises** during `events::hollow_night_active`). Phases through terrain *and* blocking entities — the phase check lives in `entity_move2` (entity layer), not the tile `may_pass` hub; `is_solid` = false. `SineFloat` bob; floats over lava/water like the wisp. Pulses on a 20-tick cycle (`is_solid_pulse`): **only damageable during the solid half** (gated in `behavior::do_hurt`); render flickers + dims the palette in the phase half (two pulse frames at (6,20)/(8,20), shade-1 eye holes). Touch: 1 damage + 3 stamina drain. Faint light radius 1 (a cold gleam). Despawns at dawn. Base health 4, detect 100. Drops: rare gem (1/30). |
| Fireflies | `FirefliesData { count, seed, time, state, home, dx, dy }` — **not a mob** (no `MobData`, `is_solid` = false, never counted against the mob cap, never saved) | **Mob-life wave.** One entity renders 4-8 glow specks as pure functions of `(time, seed, i)` — cheap ambience. Spawns at dusk near trees/marsh water (`firefly_check_start_pos`; `fireflies::weather_allows` is a stub `true` awaiting the weather wave). Wanders in curvy loops, then **roosts** on a nearby tree tile (specks glow over the canopy; the roost snapshot of the tile's data means an axe hit — or felling — spooks it). **Spook** (player within ~3 tiles or tree hit): burst scatter, regroup after ~10 s. Dawn despawn. Tiny light emitter (radius 2, like the glow worm). Speck cell (10,20) is true-color so the glow stays warm through the night grade. |
| Night Wisp | `NightWispData { enemy, zap_cooldown }` | **Original mob.** Night-time floating light: light radius 4, palette shades 0–1 transparent (glowing sprite), tick-timed two-frame pulse render. Floats over **all** terrain (`tiles::may_pass`/`bumped_into` early-return for it — the removed AirWizard's flight, generalized), immune to lava-underfoot and never "swims". Despawns at dawn on the surface like the GlowWorm. Ranged attack: fires a `Zap` (flat 1 damage, ~2–3 s lifetime, spent on impact) at a player within 8 tiles on a 90+rand(60)-tick cooldown. Base health 2. Rare gem drop (1/25). |

### 8.1 Detail notes not already covered above

- **Enemy-mob level clamping**: every enemy constructor clamps the incoming `lvl` to
  `1..=lvlcols.len()` with an explicit `// FIX:` comment — Java indexed `lvlcols[lvl-1]`
  unchecked and could panic on a hand-edited save or an out-of-range `Spawner`/`Load`
  level argument; this is a deliberate post-port bug fix, not a straight port.
- **No mob currently needs the inline-the-base-tick pattern** the removed Creeper/Slime
  used for their Java `move()`/`randomizeWalkDir()` overrides. All four new mobs layer
  their leaf behavior *after* `enemy_mob_tick_base` returns (speed tweaks, acceleration
  stalls, cooldowns), which composes with the shared functions. If a future mob needs a
  true mid-tick override, the whole base body must be inlined (see §12.1 step 4).

## 9. Furniture (`src/entity/furniture/`)

Shared `Furniture` layer (`furniture/behavior.rs`, all of which every furniture kind gets
unless it overrides): `tick` applies one tick of pending push movement then decays
`push_time`; `render` blits the sprite centered on the entity; `try_push` (called from the
`touched_by` dispatch when a player walks into non-furniture-specific furniture) starts a
10-tick shove in the player's facing direction; `take` is the power-glove pickup path — it
removes the entity from the world and turns it into an `ItemKind::Furniture` held item
(restoring whatever was previously held into the inventory); `use_furniture` is the
"MENU key while facing it" dispatch (`DungeonChest`/`Chest`+`DeathChest`/`Crafter`/`Bed`/
`Spawner` each override; everything else returns `false`, i.e. does nothing on `use`).
Placement is item-side: `ItemKind::Furniture { furniture: Box<Entity>, placed: bool }`
(`src/item/mod.rs`) is what the power glove/crafted-furniture item actually is;
`item_interact::item_interact_on_tile`'s `Furniture` arm checks `may_pass` at the target
tile and, if clear, adds a clone of the boxed entity to the level (§12 covers this
end-to-end for adding a new placeable furniture type).

| Furniture | Extra data | Quirks |
|---|---|---|
| Chest | `inventory: Inventory` | Generic, player-craftable/placeable container; `use` opens `ContainerDisplay`; `die` (destroyed) spills its inventory as dropped items. |
| DeathChest | `chest: ChestData`, `time`, `redtick`, `reverse` | Auto-spawned on player death (§7.6) holding the dropped inventory + active item + armor. `use` is overridden to `false` — **cannot be opened as a menu**, only retrieved by walking into it (`touched_by` adds its contents straight to the toucher's inventory and removes it). Decays: `time` counts down from a difficulty-dependent start (Easy 300s, Normal 120s, Hard 30s at `NORM_SPEED`); once under 30s it oscillates a red tint (`redtick`); at 0 it spills its contents like any other destroyed chest. |
| DungeonChest | `chest: ChestData`, `is_locked: bool` | Pre-populated with a random dungeon loot table at construction (`populate_inv` — weighted `try_add`/`try_add_num` rolls across food, materials, armor, potions, and weapons, with a guaranteed fallback if nothing hit); starts locked (requires a "Key" item, consumed from active item or inventory, to open); unlocking the *last* remaining dungeon chest on a level (`g.level(lvl).chest_count` hits 0) drops 5 Gold Apples and a "dungeon plundered" notification. (Java spawned the second-form AirWizard boss on the surface here; that mob was removed in the roster overhaul, and `g.air_wizard_beaten` survives only as a legacy save-format slot.) |
| Spawner | `mob: Box<Entity>` (template), `health`, `lvl`, `max_mob_level`, `spawn_tick` | An independent, level-agnostic mob source — **not** routed through `level::try_spawn`; it runs its own `spawn_tick` countdown (`200..500` ticks), then, gated by a quadratic chance based on the *level's* current `mob_count`/`max_mob_count` (same shape as `try_spawn`'s own throttle, but a separately-rolled instance), attempts to spawn a fresh instance of its template mob within `ACTIVE_RADIUS = 128` px of the closest player, at a valid nearby tile. Damageable by tools (Pickaxe deals extra); destroying it (`health <= 0`) awards 500 score. In creative mode, `use` cycles the template's mob level (wrapping at `max_mob_level`, itself derived per-kind — Knight hardcodes 5, the other enemy kinds use `lvlcols.len()`). |
| Bed | *(none — just `FurnitureData`)* | `use` puts the player to sleep if `check_can_sleep` (late enough in the day, or it's night and past day 1): saves the player's spawn point, records them in `g.bed_state.sleeping_players` (a session-only map, **not** persisted — §13), and removes the player entity from the arena/level entirely until they wake. Sleep-tracking (`players_awake`, `sleeping_players`) is genuinely global game state (`Game.bed_state: BedState`), not per-bed. |
| Tnt | `ftik`, `fuse_lit`, `explode_ticks_left: Option<i32>` | `interact` (attack key) lights the fuse; after `FUSE_TIME=90` ticks it detonates — damages every entity within `BLAST_RADIUS=32` (falloff formula, players also lose `2×damage` stamina), carves a "hole" tile at ground zero, and lights any other TNT it catches in the blast. Unlike Java (which removed the entity immediately and restored tiles via an out-of-band 300ms Swing timer), the port keeps the (already-exploded, invisible-on-render) entity alive for an `explode_ticks_left` countdown to restore the tile before finally removing itself — a deliberate `// JAVA:` documented restructuring, not a straight port. |
| Crafter | `crafter_type: CrafterType` (`Workbench`, `Oven`, `Furnace`, `Anvil`, `Enchanter`, `Loom`) | The generic "crafting station" entity — `use` opens `CraftingDisplay` with the recipe list for its `crafter_type` (`g.recipes.workbench`/`.oven`/`.furnace`/`.anvil`/`.enchant`/`.loom` — see [ADDING_CONTENT.md](ADDING_CONTENT.md)/`src/item/recipe.rs` for the recipe side, which this document doesn't duplicate). Saved/loaded by the crafter's *type name* directly as the entity name (`"Workbench"`, `"Anvil"`, ... rather than `"Crafter"` — see §12). |
| Lantern | `lantern_type: LanternType` (`Norm`/`Iron`/`Gold`) | Placeable light source only; light radius 9/12/15 by type. No `tick`/`touched_by`/`interact` override — pure `Furniture` base behavior. |
| Campfire | `fuel: i32` (remaining burn ticks; 0 = cold ember) | **Fire wave.** Hand-crafted (`Stone*5 + Stick*3 + Wood*2`), places lit with the 2 crafting Wood as fuel (`START_FUEL` ≈ 8 in-game minutes; 1 Wood = 4). Lit: light radius 7 through the normal furniture-emitter path (occlusion applies), two-frame flame render, a smoke particle every 20 ticks (thin wisps under 1 Wood of fuel), players within 2 tiles regen stamina at **2x** (`near_lit_campfire`, read from the player's recharge step), and rare stray sparks (`1/500` per tick) that `fire::ignite` one random neighboring tile. `interact` (attack key): Wood in hand adds fuel (cap 5 Wood — refused, not wasted, when full; relights an ember), a Mushroom over a lit fire roasts into a Cooked Mushroom, empty-handed reads the fuel state; anything else falls through so the power glove still picks it up. Out of fuel → ember sprite, no light/smoke/bonus. `fuel` persists via its own `write_entity`/`load_entity` extradata block; `FurnitureData.icon` carries its explicit item icon (its sprite lives outside the rows-8-9 derivation scheme). Cold-camp structures spawn a `new_ember()` variant (see `structures_gen::campfire_positions`). |

## 10. Item entities, projectiles, and particles

### 10.1 Item entities (`src/entity/item_entity.rs` + `item_entity_behavior.rs`)

A dropped item is real double-precision physics, not a snapped grid position: `xx/yy/zz`
(position) and `xa/ya/za` (acceleration) in `f64`, with the entity's actual integer `x/y`
updated only after `entity_move` resolves collision (and the sub-integer remainder folded
back into `xx/yy` to avoid drift — see the `expected_x`/`gotx` accounting in `tick`).
`za` decays by `0.15`/tick (gravity); hitting `zz < 0.0` "bounces" (`za *= -0.5`, `xa`/`ya`
damped by `0.6`) rather than stopping dead. **Lifetime**: `life_time = 600 + rand(0..70)`
ticks (~10–11.2 seconds) set at construction — this *is* a despawn timer (removed via
`remove_entity` once `time >= life_time`); it blinks (skips rendering every other 6 ticks)
for the last 120 ticks as a visual warning. **Pickup**: `touched_by` only fires for a
player toucher, requires `time > 30` (a short grace period preventing instant
re-pickup/dupe-race), and forwards to `player_behavior::pickup_item`, which stacks onto
the currently-held item if compatible (`stacks_with`) or adds to the inventory otherwise,
scores +1 point, and is a no-op on inventory (beyond the score) in creative mode. **Deep
water**: per TERRAIN.md §4, `deep_water_may_pass` always lets an `ItemEntity` through
regardless of Raft/creative — dropped items drift over deep water like everything floats.

### 10.2 Projectiles (`src/entity/projectile.rs` + `projectile_behavior.rs`)

Two kinds share this file, both free-floating (no `Mob` layer at all):

- **Arrow** (`ArrowData { dir, damage, owner: i32, speed }`) — fired by the player's Bow
  attack (§7.2). Speed is damage-dependent (8/7/6 for
  damage >3 / >=0 / negative). Moves in a straight line at `speed` px/tick along `dir`;
  each tick, checks for mob overlap at its new position (extra +3 damage against
  non-player targets, +1 more unless a ~82%-chance "critical hit" roll succeeds) via
  `mob_hurt_by_eid` (owner referenced by eid, not a live `&mut Entity` — arrows can outlive
  their shooter's take-out window, or the shooter itself); removes itself on hitting an
  impassable, non-water-connecting tile that isn't id 16 (the boat/pier-adjacent id kept as
  a literal — the source doesn't name it further). Finite (non-infinite) levels also bound
  it by `w`/`h` (an off-by-one Java quirk — `>` not `>=` — preserved verbatim).
- **Zap** (`ZapData { life_time, xa/ya/xx/yy, time, owner }`) — the old AirWizard
  `Spark`, adapted as the Night Wisp's ranged bolt (§8). Free-floating double-precision
  movement (no tile collision at all, unlike Arrow — zaps pass through walls); hits any
  mob **except another NightWisp** for a flat 1 damage and is spent on impact (unlike
  the Spark swarm, which persisted); lifetime `120 + rand(0..60)` ticks with the same
  end-of-life blink as item entities and arrows share stylistically.

### 10.3 Particles (`src/entity/particle.rs` + `particle_behavior.rs`)

Confirmed purely cosmetic — no gameplay effect of any kind. `Particle` (fire/smash/smoke
are just different constructor args over one struct: sprite + lifetime + a 1-D "radius"
`xr`, plus the fire wave's `rise`/`sway`/`phase` drift fields — a smoke puff
(`new_smoke_particle`) climbs and sways as a pure function of `time` at render, its
actual x/y never moving) counts up `time` and self-removes past `lifetime`; `TextParticle` (the floating damage/heal
numbers) additionally has the same double-precision drift-and-bounce physics as item
entities (`xa/ya/za`, gravity, ground-bounce) purely for visual arc. Neither kind is ever
touched by `touched_by`, never blocks movement (`is_solid` explicitly excludes both), and
`Particle`s are explicitly never written to local saves (`write_entity`'s early-return list
for `is_local_save`).

## 11. Spawning rules (`src/level/mod.rs`)

`tick_level(g, lvl, full_tick)` runs `try_spawn(g, lvl)` once per **full tick** (a
level-tick flag distinguishing the currently-active level from background levels — see
ARCHITECTURE.md's tick loop) *only if* the level's live mob count is currently under its
cap. Immediately before that, the same function enforces the cap from the other direction:
while the just-counted mob total exceeds `max_mob_count`, it repeatedly force-removes a
random `MobAi` entity on the level (`e.c.removed = true` + `Level::remove`) — so the cap is
a hard ceiling maintained every tick, not just a spawn gate.

**Mob cap** (`Level::update_mob_cap`, `MOB_SPAWN_FACTOR = 100`):

```
max_mob_count = 150 + 150 * diff_idx        // Easy 150, Normal 300, Hard 450
if depth == 0 || depth == -4:                // surface or dungeon
    max_mob_count = max_mob_count * 2 / 3    // lower cap on surface/dungeon than mines
```

`monster_density` (affects the spawn *position* validity radius, §11.1) is `16` by default,
lowered to `8` for any level where `depth != -4 && depth != 0` — i.e. the three mine
layers get denser mob packing allowed than the surface/dungeon. This matches TERRAIN.md's
own citation of `Level::empty`'s `if depth != -4 && depth != 0` branch exactly — same
source, same fact, described here from the entity/spawn side instead of the terrain side.

`try_spawn` itself, once past the cap gate: computes a skip chance,
`spawn_skip_chance = 100 * (mob_count/max_mob_count)²`, and randomly bails most of the time
as the level fills up (quadratic backoff — spawning gets *much* rarer as the level
approaches its cap, not just linearly rarer). If it doesn't bail, it loops up to 30
attempts (stopping at the first successful spawn):

- **Mob level scaling by depth** (TERRAIN.md's cited `if depth < 0`/`if depth > 0` branches,
  confirmed verbatim):
  ```
  min_level = 1, max_level = 1                          // default (surface, depth 0)
  if depth < 0: max_level = |depth| + (25% chance of +1) // mines: deeper = higher mob level
  if depth > 0: min_level = max_level = 4                // (no such level exists today — dead branch)
  ```
- **Position**: on infinite levels, a random point within a `CHUNK_SIZE * LOAD_RADIUS * 2`
  px square centered on the *player's* current tile (so spawns only ever happen in the
  currently-loaded chunk ring, never in unloaded-and-therefore-nonexistent territory); on
  finite levels, uniformly random over the whole level.
- **Enemy spawn — the natural-spawn table** (all rolls share one `rnd = 0..99` draw per
  attempt; a roll that lands outside every listed range still marks the attempt "spawned"
  and places nothing, which is what keeps overall spawn *rates* close to the old
  Zombie-40/Snake-35 split):

  | Where | Mob | Roll / gate |
  |---|---|---|
  | Surface, any hour | **Marsh Lurker** | `rnd <= 25` **and** `lurker_check_start_pos` (clearance + tile is `WATER`/`MUD` + unlit) — in practice marsh pools/pond rims |
  | Surface, day or night | **Grass Snake** | `grass_snake_biome` (Plains/Forest; finite: `GRASS`) **and** `rnd ∈ 13..=18` + `enemy_check_start_pos` |
  | Surface, day or night | **Adder** | `adder_biome` (Marsh/Savanna; finite: `MUD`) **and** `rnd ∈ 19..=25` + `enemy_check_start_pos` |
  | Surface, day or night | **Rattler** (spawns coiled) | `rattler_biome` (Desert; finite: `SAND`) **and** `rnd ∈ 19..=25` + `enemy_check_start_pos` |
  | Surface, day or night | **Feral Hound** (pack of `2 + rand(0..2)`, spread over adjacent passable tiles) | `hound_biome` (infinite: `biome_at` ∈ Plains/Savanna; finite: `GRASS` tile) **and** (`rnd <= 12` by day, `41..=60` by night) + `enemy_check_start_pos` |
  | Surface, night (past day 1) | **Zombie** | `rnd <= 40` + `enemy_check_start_pos` |
  | Surface, night (past day 1) | **Night Wisp** | `rnd ∈ 61..=75` + `wisp_check_start_pos` (clearance + unlit; no `may_spawn` tile gate — it floats) |
  | Surface, night (past day 1) | **Ghost** (rises from a broken grave within 4 tiles of the roll) | `rnd ∈ 86..=90` normally, `rnd <= 45` on a Hollow Night (the mass-rise) + `ghost_check_start_pos` (`ghost::try_rise` scans for the grave) |
  | Surface, dusk (`Time::Evening`) | **Fireflies** (ambient swarm; not a mob) | `rnd >= 91` + `fireflies::weather_allows` (stub `true`) + `firefly_check_start_pos` (tree within 2 tiles, or marsh water) |
  | Mines (depth < 0, not −4), any hour | **Zombie / Cave Serpent / Stone Golem** | `rnd <= 40` / `41..=70` / `71..=85` + `enemy_check_start_pos` (the mine/dungeon snake is the Cave Serpent — `snake::new`) |
  | Dungeon (depth −4), any hour | **Zombie / Cave Serpent / Knight** | `rnd <= 40` / `41..=55` / `56..=75` + `enemy_check_start_pos` (which requires `OBSIDIAN` there) |

  `enemy_check_start_pos` is unchanged (distance-from-player ≥ 60px, density-scaled
  "no other mob nearby" radius via `mobai_check_start_pos`, tile-type + unlit checks).
  `lurker_check_start_pos`/`wisp_check_start_pos` reuse its clearance half
  (`check_start_pos_clearance` in `behavior.rs`) with their own tile gates.
- **Passive spawn**: surface only (`depth == 0`), gated by `passive_check_start_pos`
  (similar distance/density check, plus must land on `GRASS`/`FLOWER`). Picks Cow
  (`rnd <= 22` at night, `<= 33` by day), Pig (`rnd >= 68`), or Sheep (everything else), and
  — regardless of which — **always additionally spawns a GlowWorm** beside the spawned
  mob (`add_at` at the same coordinates; the Java quirk of adding it at its raw default
  `(0, 0)` via `Level::add` was fixed post-port).

### 11.1 Spawn-position validation (`mobai_check_start_pos`, shared by both branches)

1. Reject if within `player_dist` px of the closest player (`60` for enemies, `80` for
   passives).
2. Reject if any entity at all already occupies a square of half-size
   `monster_density * solo_radius` centered on the candidate point (`solo_radius` is `13`
   normally, `15`/`22` on the dungeon depending on score mode, for enemies; `15` by day /
   `20` by night, `+7` more in score mode, for passives) — this is what keeps spawns
   spread out rather than clumping.
3. Finally, `tiles::may_spawn(tile)` — a per-`TileDef` flag (not every walkable tile allows
   spawning; see `src/level/tile/mod.rs`/`dispatch.rs`).

## 12. HOW TO EXTEND

### 12.1 Add a new mob

1. Add a variant to `EntityKind` (`src/entity/mod.rs`) and a `<Mob>Data` struct in a new
   `src/entity/mob/<mob>.rs` wrapping whichever layer fits (`PassiveMobData` for a friendly
   wanderer, `EnemyMobData` for a chaser) plus any leaf fields (fuse timers, cooldowns, ...).
2. Add accessor-match arms in `Entity::mob()`/`mob_mut()`, and `mob_ai()`/`enemy_mob()`/
   `passive_mob()` as appropriate (`src/entity/mod.rs`) — missing one means `e.mob()` (etc.)
   silently returns `None` for your new kind instead of erroring, which is easy to miss.
3. Wire every dispatch hub in `src/entity/behavior.rs` you need: `entity_tick`,
   `entity_render`, `die` always; `touched_by` if it should damage the player or react to
   being walked into (don't just add it to the shared `enemy_touched_by` arm unless the
   default `lvl*(hard?2:1)` formula is actually correct for it — Snake, MarshLurker, and
   StoneGolem each have their own arm for exactly this reason); `entity_interact` only if
   it needs a custom attack-key reaction beyond "forward to the held item."
4. If it needs a `super.tick()`-style call, use the matching base function
   (`mobai_tick_base` for a passive wanderer, `enemy_mob_tick_base` for a chaser). Leaf
   behavior that runs *after* the base tick (speed tweaks, cooldowns, stalling the chase
   acceleration — see the four new mobs) composes fine; a true mid-tick
   `move()`/`randomizeWalkDir()` override the way the removed Creeper/Slime worked
   requires inlining the whole base-tick body yourself (see §8.1) since there is no
   virtual dispatch to hook into; there is no shortcut for this today.
5. Register a sprite via `compile_mob_sprite_animations`/`compile_sprite_list`
   (`src/gfx/sprite.rs`) at whatever sheet coordinates are free. Pick a
   `MovementStyle` in the constructor if the mob shouldn't walk `Classic`
   (see §5.2 — `enemy.ai.movement_style = MovementStyle::...`).
6. If it should spawn naturally, add a roll to `level::try_spawn` (`src/level/mod.rs`) —
   note this is currently a hardcoded if/else-if chain (see the §11 spawn table), not a
   data table; also decide its `max_mob_level` in `furniture::spawner::max_mob_level`,
   add it to `spawner_behavior::new_mob_instance` + `spawner::mob_class_name`, and (for a
   creative-inventory spawner item) to `registry::build_registry`'s spawner-item list.
7. **Save/load naming**: add it to `entity_class_name` (`src/saveload/save.rs`) and to
   `get_entity`'s string match (`src/saveload/load.rs`) using the exact same name string —
   these two must agree verbatim or round-tripping breaks. If it's an `EnemyMob`-shaped kind
   (has a `lvl`), also add its name to the `is_enemy_mob_class` match in `load_entity` so the
   trailing `:lvl` field in the save string gets parsed. Unknown names (e.g. mobs removed
   from the roster) are tolerated on load: `get_entity` logs a `LOAD WARNING` and
   `load_entity` skips the entity instead of panicking; a `Spawner` whose saved template
   name is unknown falls back to its default Zombie template.

### 12.2 Add a new furniture

1. Add an `EntityKind` variant + a `<Furniture>Data` struct with a `furniture:
   FurnitureData` field (`src/entity/furniture/<name>.rs`), following the `TntData`/
   `LanternData` pattern for "flat" furniture or the `ChestData`-nesting pattern if it needs
   its own sub-inventory-like state.
2. Add match arms to `Entity::furniture()`/`furniture_mut()` (and `chest()`/`chest_mut()` if
   it's chest-shaped) in `src/entity/mod.rs`.
3. Wire `entity_tick`/`entity_render`/`die` in `behavior.rs`, and
   `furniture::behavior::use_furniture`'s dispatch if pressing the menu key near it should
   do something (open a display, start an action, ...); wire `entity_interact` only if the
   attack key should do something beyond the default "forward to the held item."
4. **Placement item**: to make it player-placeable, build an `ItemKind::Furniture { furniture:
   Box::new(your_entity), placed: false }` via `registry::new_furniture_item` (see
   `spawner_behavior::take`/the power-glove path for the pattern) and add it to whatever
   recipe should craft it (`src/item/recipe.rs`) — the entity-side placement mechanics
   (`item_interact::item_interact_on_tile`'s `Furniture` arm) are generic and need no
   changes for a new kind.
5. Save/load: same two-file naming requirement as §12.1 (`entity_class_name` +
   `get_entity`), unless it's crafter-shaped, in which case the *type name* (e.g.
   `"Anvil"`) is what gets saved/loaded, not `"Crafter"` — see `Crafter`'s special-cased
   `name = c.crafter_type.name()` in `write_entity` and the `is_crafter_name` check in
   `load_entity`.

### 12.3 Change spawn rules

- Mob cap: `Level::update_mob_cap` and `MOB_SPAWN_FACTOR`/the `150 + 150*diff_idx` formula,
  both in `src/level/mod.rs`.
- Depth-based mob-level scaling (which levels enemies can be) and the natural-spawn
  Zombie/Snake percentages: the `if depth < 0`/`if depth > 0` block and the `rnd <= 40`/
  `<= 75` checks inside `try_spawn`, same file.
- Position validation radii (how close to the player / other mobs a spawn may land):
  `mobai_check_start_pos`'s `player_dist`/`solo_radius` parameters, tuned per-call-site in
  `enemy_check_start_pos`/`passive_check_start_pos` (`src/entity/behavior.rs`).
- Per-level density (`monster_density`, mines vs. surface/dungeon): `Level::empty`
  (`src/level/mod.rs`) — see also TERRAIN.md §7.7 for the sibling terrain-side depth
  special-casing that has to move in lockstep if a new depth is ever added.

## 13. Invariants & gotchas

- **`EntityArena` is a `HashMap<i32, Entity>`, not the "slab" both ARCHITECTURE.md and
  PORTING.md describe it as.** The contract (stable integer key, `take`/`put_back` for the
  tick pattern) matches the docs; only the concrete container differs. Don't go looking for
  slab-style index reuse/generation counters — there aren't any.
- **There is no `Eid` newtype** despite both docs naming one — it's a plain `i32`
  everywhere (`EntityCommon.eid`, `g.player_id`, every function signature that takes an
  entity id). `0` is reserved for the main player by convention (enforced in
  `generate_unique_entity_id`, which never hands out `0`), not by the type system.
- **Snake's custom `touched_by` is now wired** (the historical dead-code bug — the
  dispatch routed Snake through the generic `enemy_touched_by` — is fixed): Snake deals
  its intended `lvl + diff_idx` touch damage. The AirWizard, whose override had the same
  bug, was removed outright in the roster overhaul. When adding a mob with a custom
  touch formula, remember the dispatch arm (§12.1 step 3) — the leaf function alone does
  nothing.
- **The take-out pattern means "the arena is complete" is never a safe assumption inside a
  tick** — see §4. Any code that walks `g.entities` (or looks up a specific eid) while
  inside `entity_tick`/`touched_by`/etc. must treat a missing entity as normal, not
  exceptional.
- **`entity.c.level` is updated by `Level::add`/`add_at`, not by the entity itself** —
  stairs/chasm/ladder travel (`World::change_level`, TERRAIN.md §4/§6) works by taking the
  player out of the arena, computing a new position, and calling `Level::add_at` on the
  destination level, which is what actually flips `c.level`. A hand-rolled "teleport" that
  only changes `c.x`/`c.y` without also going through `Level::add`/`add_at` (or the
  `entities_to_remove` + re-`add` dance `tick_level` does for ordinary removal) will leave
  the entity attached to the wrong level's iteration/save output.
- **Bed occupancy state (`g.bed_state`) is never persisted — an accepted gap.**
  `sleeping_players` and `players_awake` have no representation in
  `src/saveload/save.rs`/`load.rs` at all — only the player entity itself (via a dedicated
  `write_player` call, independent of the generic arena-entity dump) and the `Bed`
  furniture entity (a plain, dataless `FurnitureData`) get written. A save taken while a
  player is asleep reloads with the player simply standing wherever they were when they
  clicked "sleep" and `players_awake` reset to its `Default` (`1`). This was reviewed
  during the roster overhaul and deliberately left as-is: persisting it would need a new
  save-format field plus re-attachment of a level-less sleeping player on load, for a
  state that lasts seconds and self-corrects on wake. Revisit only if exact
  save-during-sleep fidelity ever matters.
- **`GlowWorm` now spawns beside the passive mob it escorts** — `try_spawn`'s passive
  branch calls `add_at` with the chosen spawn coordinates. (The Java original used
  `Level::add`, which left the worm at its raw default `(0, 0)`; fixed post-port under
  the post-`v0.1.0` "fix inherited bugs" rule.)
- **Furniture entities are never written with extra chest-shaped fields unless they
  actually are chest-shaped** — `write_entity`'s `if let Some(chest) = e.chest()` block is
  the only place inventory contents get serialized; a new furniture kind that carries its
  own ad-hoc state (like `SpawnerData`'s boxed mob template, or `LanternData`'s type) needs
  its own explicit `if let EntityKind::YourKind(...)` block added to both `write_entity` and
  `load_entity` — there is no generic "serialize whatever extra fields exist" mechanism.
- **The Ghost's phasing is an entity-layer check, not a tile one** — `entity_move2`
  skips both the tile `may_pass` loop and the entity-`blocks` loop for
  `EntityKind::Ghost` (the Night Wisp's carve-out lives tile-side in
  `dispatch::may_pass`; the Ghost's deliberately does not, so `src/level/tile/` stays
  untouched). Its damage gate is in `behavior::do_hurt` (`ghost::is_solid_pulse`).
- **Fireflies are never persisted** — `write_entity` skips them like particles, and
  they despawn at dawn anyway. They are also invisible to the mob cap (`is_mob()` is
  false — no `MobData` layer).
- **Snake variants share one `EntityKind::Snake`** — `SnakeVariant` picks save name,
  palette, health, and bite; only the Cave Serpent writes the legacy `"Snake"` name.
  When matching on snakes, remember the variant field (e.g. the Grass Snake must not
  fall into a "snakes bite" assumption).
- **Chest/DeathChest/DungeonChest all funnel through one `die()`** (`chest_behavior::die`)
  that spills the inventory as dropped items — a plain `Furniture` (no inventory) has no
  such override and just vanishes on death via the default `remove_entity` fallback in the
  `die` dispatch.

## 14. Test coverage map

There is **no dedicated `tests/entities.rs` or per-mob unit test file** — entity behavior is
exercised indirectly through headless gameplay/save-load integration tests and a couple of
narrowly-scoped display tests. No `#[cfg(test)]` modules exist inside `src/entity/` itself.

| Test file / test | Locks in |
|---|---|
| `tests/gameplay_soak.rs` (`soak_random_play_two_seeds`) | ~5000 ticks of pseudo-random movement/attack input across two seeds, with periodic day/night flips (so night-only enemy spawning and GlowWorm despawning both get exercised) — asserts the player entity never vanishes for good and the arena doesn't grow unbounded (`MAX_ENTITIES = 5000`), i.e. no entity-removal leak and no runaway spawn loop. |
| `tests/gameplay_soak.rs` (`soak_walk_all_levels`) | Walks all five level slots via five `-1` level changes (wrapping back to start), 200 ticks each — exercises `World::change_level`'s player re-attachment (`c.level` bookkeeping, §13) and per-level spawning across every depth without panicking. |
| `tests/gameplay_soak.rs` (`soak_tnt_explosion_near_player`) | Places a pre-fused `Tnt` entity next to the player and ticks through fuse + blast + the post-explosion tile-restore countdown; asserts the player survives the world-sanity check and the `Tnt` entity is fully gone (`EntityKind::Tnt` count == 0) once resolved — locks in the whole `tnt_behavior::tick` state machine end-to-end. |
| `tests/save_load_roundtrip.rs` (`world_roundtrip`) | Builds a world containing a leveled `Zombie` (with a modified `health`), a `Chest`, an `Iron`-type `Lantern`, a `StoneGolem`-templated `Spawner`, and a `DungeonChest`, saves, reloads, and checks the entities come back — the closest thing to an entity save-format regression test today; exercises `entity_class_name`/`get_entity` naming agreement (§12) and the chest/spawner/lantern extra-data round trip. |
| `tests/crafting_chain.rs` (`early_loop_crafts_a_crude_axe`, `crude_axe_outchops_fists_and_grass_yields_fibers`) | Drives a real player entity (`with_entity(0, ...)`) through crafting and tool use — indirectly exercises `PlayerData.inventory`/`active_item` plumbing and tool-attack damage, though it is a crafting-focused test, not an entity-focused one. |
| `tests/mob_life.rs` (11 tests) | The mob-life wave: snake-family params + save names (`"Snake"` compat), rattler warn→strike sequence, adder stamina drain / grass-snake harmlessness, ghost grave-rise + dawn despawn + phase-through-rock + solid-pulse damage gate, firefly dusk spawn + spook scatter + dawn despawn, per-biome spawn tables (desert rattler, marsh adder, plains grass snake), `MovementStyle` wiring + `style_step` semantics, and a tall-grass stealth render smoke (eye glints present, body clipped) plus a night gallery screenshot (`target/verify/mob_life_gallery.png`). |
| `tests/display_flow.rs` (`inventory_esc_closes`) | Takes the player out of the arena, builds `PlayerInvDisplay` from it, puts it back — exercises the take-out pattern from a UI-construction angle (a display needs a `&Entity` while the entity is deliberately out of the arena for the duration of the borrow). |

Run everything above with `cargo test --test gameplay_soak --test save_load_roundtrip
--test crafting_chain --test display_flow` (none of these paths match a narrower `cargo
test <keyword>` filter the way TERRAIN.md's `cargo test level` does, since "entity" isn't in
any of these file names). `cargo test` alone also runs them, plus everything else in the
suite. There is currently no test that isolates a single mob's tick/die/touched_by logic in
the way `tests/multi_level_terrain.rs` isolates the dig state machine — the roster is only
validated in aggregate, through soak/save-load coverage. A targeted `touched_by`-per-kind
test would still be valuable (the now-fixed Snake dispatch bug is exactly the class of
regression it would catch).
