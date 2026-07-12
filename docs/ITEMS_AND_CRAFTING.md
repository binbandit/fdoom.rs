# Items & Crafting

Exhaustive reference for fdoom.rs's item system: the `Item` value-object model, the
prototype registry, tools and their tier ladder, inventories, recipes, the bare-handed
gather chain, item-use dispatch, and the Raft/deep-water rule. See also
[ARCHITECTURE.md](ARCHITECTURE.md) for the whole-codebase tour and
[ADDING_CONTENT.md](ADDING_CONTENT.md) for the item/recipe/tool content recipes this
document extends with deeper reference material (§9 here mirrors that doc's "New tool" /
"New crafting recipe" sections but goes further into the *why* and the exact tables).
[TERRAIN.md](TERRAIN.md) covers world generation and the dig-based multi-level descent;
§8 here cross-references its Raft/Deep-Water note from the item side.
[ENTITIES.md](ENTITIES.md) covers the entity/arena system, including the `ItemEntity`
physics for a dropped item and the furniture entities (chests, crafting stations) that
inventories and recipes attach to.

Every claim below is grounded in the source as of this writing; file:line references are
approximate anchors (line numbers drift), not guarantees — grep the quoted symbol if a
number is stale.

## 1. Overview + mental model

Items are **value objects**, not entities — there is no `Eid`, no arena slot, no tick
function. `src/item/mod.rs`:

```rust
pub struct Item {
    name: String,
    pub sprite: Sprite,
    pub kind: ItemKind,
}
```

`kind: ItemKind` is where all the behavioral variance lives. The real, current variant
list (13 variants — PORTING.md's "`Tool{tool_type,level,dur}`, `Stackable{count}`,
`Furniture(Box<Entity>)`, etc" is a rough sketch of a few representative variants, not
the full list; note also that `Furniture` is a **struct variant with two fields**, not a
bare tuple wrapping `Box<Entity>` as the shorthand implies):

| Variant | Fields | Java origin | Notes |
|---|---|---|---|
| `PowerGlove` | *(none)* | `PowerGloveItem` | picks up furniture on `interact`; never enters an inventory (see §10) |
| `Stackable` | `count: i32` | `StackableItem` | plain raw materials (Wood, Stone, Coal, ores, ...) |
| `Unknown` | `count: i32` | `UnknownItem` (a `StackableItem` subclass) | returned instead of `null`/panicking for unresolved names |
| `Food` | `count, heal, stamina_cost` | `FoodItem` | `stamina_cost` is always `5` (set by the `food()` builder in registry.rs) |
| `Armor` | `count, armor: f32, level: i32, stamina_cost` | `ArmorItem` | `stamina_cost` always `9`; `armor`/`level` are **not** the tool tier ladder — see §2.3 |
| `Clothing` | `count, player_col: i32` | `ClothingItem` | cosmetic only; recolors the player sprite |
| `Potion` | `count, ptype: PotionType` | `PotionItem` | see §7 and `potion_type.rs` |
| `TileItem` | `count, model: String, valid_tiles: Vec<String>` | `TileItem` | `model`/`valid_tiles` are tile *names*, stored uppercase (see §10) |
| `Torch` | `count, valid_tiles: Vec<String>` | `TorchItem` (a `TileItem` subclass with an empty model) | model is implicit: `Tiles::get_torch_tile` resolves the lit variant of whatever it's placed on |
| `Bucket` | `count, filling: Fill` | `BucketItem` | `Fill::{Empty, Water, Lava}` |
| `Medical` | `count, heal` | *(post-port, no Java origin)* | first-aid items (Bandage): restore `heal` **health** directly (contrast `Food`, which restores hunger); reflexive (`interacts_with_world()` false), costs 5 stamina, only usable while `health < MAX_HEALTH` |
| `Tool` | `ttype: ToolType, level: i32, dur: i32` | `ToolItem` | the tier ladder, §2 |
| `Furniture` | `furniture: Box<Entity>, placed: bool` | `FurnitureItem` | boxes a whole `Entity`; `placed` flips to `true` once placed (non-creative), which `is_depleted()` reads to clear the active-item slot |
| `Book` | `book: Option<&'static str>, has_title_page: bool` | `BookItem` | `book = None` is the blank player-writable book |

Common `Item` methods worth knowing before touching anything else in this doc:

- `is_stackable()` / `count()` / `count_mut()` — backed by a single `count_ref()` match
  arm that covers `Stackable | Unknown | Food | Medical | Armor | Clothing | Potion |
  TileItem | Torch | Bucket`. **Note this list**: `Armor` and `Clothing` carry a `count` field (so
  `is_stackable()` is true for them) even though in practice a player only ever wears one
  at a time — "stackable" here means "has a count field for save/inventory bookkeeping",
  not "the game lets you carry a meaningful stack of 40 iron armors". `Tool`, `Furniture`,
  `Book`, and `PowerGlove` are **not** stackable; `count()` on them always returns `1`.
- `stacks_with(other)` — `other.is_stackable() && other.name == self.name` (JAVA: the
  class is deliberately not compared — a `Stackable` and an `Unknown` with the same name
  would stack, though this never happens in practice since names are unique per
  registration).
- `item_equals(other)` — the general-purpose equality used for non-stackable counting and
  recipe-ingredient consumption. It special-cases `Tool` (type + level only, **durability
  is ignored**), `Torch` (any torch equals any torch), `TileItem` (name + model), `Potion`
  (name + type), `Bucket` (name + filling); everything else falls to "same enum
  discriminant + same name".
- `is_depleted()` — `Tool`: `dur <= 0 && ttype.durability() > 0` (so a `FishingRod`-like
  tool with a base durability of 0 could never deplete, though none currently has 0);
  `Furniture`: `placed`; everything else: `count() <= 0`.
- `get_data()` — the save/network string: `"{name}_{dur}"` for tools, `"{name}_{count}"`
  for anything with a count, or just `"{name}"` otherwise. This is the exact inverse of
  `registry::get_opt`'s `Name_amount` parsing (§2.1).

**Items vs. `ItemEntity`**: an `Item` is inert data — it only exists inside an
`Inventory`, an `active_item` slot, or a `Recipe`. The moment an item is dropped into the
world (tile-punch drops, death drops, thrown potions landing, ...) it gets wrapped as
`EntityKind::ItemEntity(ItemEntityData)` (`src/entity/item_entity.rs`), which *is* a live
arena entity with physics (bobbing, magnetism toward the player, despawn timers). This
document does not re-describe that physics — see
[ENTITIES.md](ENTITIES.md#10-item-entities-and-projectiles) for the full pickup-delay/
despawn/stacking treatment. The one place `ItemEntity` matters here is
`crate::level::drop_item` / `drop_items_counted` (`src/level/mod.rs`), the helper every
tile-drop call in §6 goes through — it randomizes a landing offset within the same tile
and spawns the `ItemEntity`.

## 2. The prototype registry

`src/item/registry.rs`. The registration mechanism is a **flat `Vec<Item>` built once**,
not a `HashMap` and not a big `match` keyed by name:

```rust
pub fn build_registry(g: &Game) -> Vec<Item> { ... }
```

`Game::new` calls this once and stores the result as `g.items: Rc<Vec<Item>>` (an `Rc`
handle, cheap to clone, mirroring `g.tiles: Arc<Tiles>` — see TERRAIN.md's tile registry
for the analogous pattern). Order matters: it is **the creative-inventory display order**
(ADDING_CONTENT.md already says this; verified here against the actual function body,
which pushes in exactly this sequence): PowerGlove → furniture items (spawners, chest,
crafters, lanterns, TNT, bed) → torch → buckets → books → tile items (flower, saplings,
planks/bricks/walls/doors per material, wool colors, sand, seeds, ...) → tool items (all
6 tiers × 7 non-fishing-rod `ToolType`s, plus a single `FishingRod`) → food → stackables
(raw materials) → clothing → armor → potions.

Lookup (`registry::get` / `registry::get_opt`) is a **linear scan**, not a hash lookup:

```rust
let found = g.items.iter().find(|i| i.get_name().eq_ignore_ascii_case(&name));
```

This is the actual "prototype-by-name" mechanism: `get(g, name)` clones the matching
prototype `Item`, then (if the prototype is stackable) sets its count from the parsed
amount, or (if it's a `Tool`) sets its `dur` from the parsed amount. `get` never returns
nothing — an unresolved name becomes `new_unknown_item(&name)` (with a `println!` warning
to stderr/stdout), matching Java's `Items.get` "never null" contract.

### 2.1 Name parsing (`registry::get_opt`)

```rust
let mut name = name.to_uppercase();
let mut amount = 1;
if let Some(idx) = name.find('_') {
    amount = name[idx + 1..].parse().unwrap_or(1);
    name.truncate(idx);
} else if let Some(idx) = name.find(';') {
    amount = name[idx + 1..].parse().unwrap_or(1);
    name.truncate(idx);
}
```

So `"Name_amount"` (or `"Name;amount"`, used by network/some save paths) is split on the
**first** `_`/`;`, the amount is parsed with a silent `unwrap_or(1)` fallback (a malformed
number never panics here — contrast with `Recipe::new`, §5.1, where it does), and the
name half is uppercased before the registry scan (`eq_ignore_ascii_case` on the stored
name, which is **not** itself stored uppercase — see the case-sensitivity note in §10).
`"NULL"` and `"UNKNOWN"` are special literal names handled before the registry scan.

### 2.2 The tool tier ladder

`TOOL_LEVEL_NAMES` (registry.rs):

```rust
pub const TOOL_LEVEL_NAMES: [&str; 6] = ["Crude", "Wood", "Rock", "Iron", "Gold", "Gem"];
```

**Six tiers, confirmed** — this matches the task brief's assumption exactly (no
discrepancy on count or names). The naming detail worth internalizing: **"Crude" was
added post-port** as tool level 0; Java's original five (`Wood..Gem`) all shifted up one
level to 1..5. Tool *names* and *recipes* are unaffected (a `"Wood Axe"` is still called
that), only the numeric `level` field shifted — which is exactly why some in-code damage
comments are now stale by one tier (see the discrepancy note at the end of this
section).

Construction (`new_tool_item`, `build_registry`'s tool loop):

```rust
for ttype in ToolType::VALUES {
    if ttype == ToolType::FishingRod { continue; }
    for lvl in 0..=5 {
        items.push(new_tool_item(ttype, lvl));
    }
}
```

i.e. every `ToolType` except `FishingRod` gets all 6 tiers auto-generated; `FishingRod`
gets exactly one prototype at "level 0" (it doesn't scale — see the durability formula
below, which still runs through the same code path but multiplies by 1). Name is
`"{TOOL_LEVEL_NAMES[level]} {ttype.name()}"` (`"Crude Axe"`, `"Gem Pickaxe"`, ...) except
`FishingRod`, whose name is the literal `"Fishing Rod"` (no tier prefix).

**Durability formula** (`new_tool_item`, quoted verbatim):

```rust
dur: ttype.durability() * (level + 1),
```

Linear multiply, not a lookup table: base durability (fixed per `ToolType`, see §3) times
`(level + 1)`, so Crude (level 0) is 1× base and Gem (level 5) is 6× base.

| Tier | `level` | Durability multiplier | Example: Axe (base 24) | Example: Sword (base 42) |
|---|---|---|---|---|
| Crude | 0 | ×1 | 24 | 42 |
| Wood | 1 | ×2 | 48 | 84 |
| Rock | 2 | ×3 | 72 | 126 |
| Iron | 3 | ×4 | 96 | 168 |
| Gold | 4 | ×5 | 120 | 210 |
| Gem | 5 | ×6 | 144 | 252 |

**Stamina cost per use** is *not* a tier lookup table either — it's `pay_stamina(player,
BASE - tool_level)` called from each tile's own `interact` fn, where `BASE` is a
per-*action* constant chosen by the tile file, and `tool_level` is literally the `Tool`
item's `level` field (0..5) destructured at the call site — **`tool_level` and the tier
`level` are the exact same number**, just named for readability at each call site; there
is no separate "tool_level" concept anywhere in the codebase. `pay_stamina` clamps the
cost to `>= 0` (`cost.max(0)`), so a high-tier tool against a low `BASE` action never
*restores* stamina, it just gets free (cost 0):

| `BASE` | Where used | Cost at Crude (lvl 0) | Cost at Gem (lvl 5) |
|---|---|---|---|
| `2` | `flower.rs` interact (Shovel picks a flower) | 2 | 0 (clamped) |
| `4` | `grass.rs`, `dirt.rs`, `sand.rs`, `farm.rs`, `depth.rs` (dig/till/chasm), `tree.rs` (chop), `rock.rs` (mine), `lava_brick.rs` (mine) | 4 | 0 (clamped) |
| `6` | `ore.rs` interact (Pickaxe on an ore vein) | 6 | 1 |

TERRAIN.md's dig-state-machine formula `4 - tool_level` (Dug Pit shovel/pickaxe hits,
`src/level/tile/depth.rs`'s `dug_pit_interact`) is this exact same mechanism with
`BASE = 4` — **not** a different concept from the tier ladder; this document and
TERRAIN.md are consistent in using "tool_level"/"level" interchangeably for the 0..5 tier
index.

**Attack damage bonus** (mob combat only; `get_attack_damage_bonus` in
`src/entity/mob/player_behavior.rs`) pays one durability point per swing and only three
`ToolType`s get a scaling bonus — everything else (`Shovel`, `Hoe`, `Pickaxe`, `Bow`,
`FishingRod`) is a flat `+1` on top of the base `1..=2` bare-fist roll:

```rust
ToolType::Axe      => (level + 1) * 2 + g.random.next_int_bound(4),
ToolType::Sword     => (level + 1) * 3 + g.random.next_int_bound(2 + level * level),
ToolType::Claymore  => (level + 1) * 3 + g.random.next_int_bound(4 + level * level * 3),
_                   => 1,
```

Computed ranges at the *current* (post-Crude-tier) level indices:

| Tier | `level` | Axe damage | Sword damage | Claymore damage |
|---|---|---|---|---|
| Crude | 0 | 2–5 | 3–4 | 3–6 |
| Wood | 1 | 4–7 | 6–8 | 6–12 |
| Rock | 2 | 6–9 | 9–14 | 9–24 |
| Iron | 3 | 8–11 | 12–22 | 12–42 |
| Gold | 4 | 10–13 | 15–32 | 15–66 |
| Gem | 5 | 12–15 | 18–44 | 18–96 |

**Discrepancy found and worth flagging**: the inline comments directly above this match
(`"wood axe damage: 2-5; gem axe damage: 10-13"`, `"wood: 3-5 damage; gem: 15-32
damage"`) describe the **pre-Crude-tier** numbering (Wood=level 0 .. Gem=level 4, five
tiers) — they match the formula exactly one tier index lower than where `TOOL_LEVEL_NAMES`
now places Wood/Gem. The comments were not updated when the Crude tier was inserted and
shifted every other tier's `level` up by one; the table above uses the *actual* current
values, not the stale comment text. Fix would be a one-line comment edit in
`player_behavior.rs`, not a behavior change.

### 2.3 Armor: a separate, non-generated tier list

Unlike tools, **armor does not run through `ToolType::VALUES`/`TOOL_LEVEL_NAMES` at all**.
`build_registry`'s `ArmorItem.getAllInstances()` block hand-authors exactly five items via
the `armor(name, sprite, armor_fraction, level)` helper:

| Item | `armor` (fraction of `MAX_ARMOR`) | `level` |
|---|---|---|
| Leather Armor | 0.3 | 1 |
| Snake Armor | 0.4 | 2 |
| Iron Armor | 0.5 | 3 |
| Gold Armor | 0.7 | 4 |
| Gem Armor | 1.0 | 5 |

There is no "Crude"/level-0 armor and no "Wood" armor tier — this is a genuinely
*different* 5-item ladder that happens to share level *numbers* 1–5 with four of the tool
tier names (Iron/Gold/Gem/and nominally "Wood"↔"Leather"/"Snake" not lining up at all) by
convention, not by shared code. Mechanically (`src/entity/mob/player_behavior.rs`, the
player-hurt path around `cur_armor`): `armor * MAX_ARMOR` becomes the armor's remaining
"durability" pool (`pd.armor`, decremented per hit taken), while `level` is used purely as
a damage-reduction divisor — incoming damage accumulates in `armor_damage_buffer` and only
overflows into real health damage once the buffer reaches `level + 1`
(`// JAVA: >= curArmor.level+1 — preserved verbatim`), so higher `level` armor blocks more
raw damage per health point lost, independent of the `armor` fraction. If you're adding a
new armor material, follow this section's pattern (`armor()` helper + anvil/loom recipe),
**not** the `ToolType::VALUES` loop in §2.2.

## 3. Tool types (`src/item/tool_type.rs`)

Eleven `ToolType` variants — the eight Java originals plus the post-port survival
weapons `Spear`/`Crossbow`/`Slingshot` (§3.1):

| `ToolType` | Sprite row | Base durability | Primary use |
|---|---|---|---|
| `Shovel` | 0 | 24 | dig grass/dirt/sand/farmland into a Dug Pit or a hole (§4/§6, TERRAIN.md §4) |
| `Hoe` | 1 | 20 | till grass/dirt into farmland; occasionally harvests loose seeds from grass |
| `Sword` | 2 | 42 | no tile interaction; attack-damage bonus only (§2.2) |
| `Pickaxe` | 3 | 28 | mine rock/ore/lava-brick tiles; break through a bottomed-out Dug Pit into a Chasm |
| `Axe` | 4 | 24 | chop trees |
| `Bow` | 5 | 20 | ranged attack (fires an `arrow` entity — see `player_behavior.rs`'s attack path); uses `TOOL_BOW_COLORS` instead of `TOOL_LEVEL_COLORS` for its sprite tint |
| `FishingRod` | 6 | 16 | `interactOn` fishable water (`water`, `Deep Water`, submerged Tidal Flat) starts fishing (`go_fishing`, §7.1); does **not** get the 6-tier treatment — a single fixed-name "Fishing Rod" prototype |
| `Claymore` | 7 | 34 | no tile interaction; heaviest attack-damage bonus (§2.2), craftable only by upgrading a same-tier Sword (§6) |
| `Spear` | 2 *(placeholder — sword cell; TODO(art) wants 8)* | 30 | tiered reach weapon: melee sweep uses `ATTACK_DIST + 8` px; damage bonus `(level+1)*2 + rand(0..2+level)` (between Axe and Sword); **SHIFT-attack throws it** as a projectile that lands as a pickup, durability preserved (§3.1) |
| `Crossbow` | 5 *(placeholder — bow cell; TODO(art) wants 9)* | 40 | single-tier (like FishingRod); fires an arrow at flat damage 7 (vs the Bow's `tool_level` 0..=5); `attack_time = 30` doubles as a re-cock delay — a click while it's still counting down is a dry trigger pull (§3.1) |
| `Slingshot` | 5 *(placeholder — bow cell; TODO(art) wants 10)* | 18 | single-tier early ranged weapon; consumes one `Stone` item per shot, fires a short-range pellet at damage 0 (the arrow-vs-mob +3/+1 bonus is all it has) (§3.1) |

`ToolType::name()` returns the exact string embedded in every tiered item's name
(`"{tier} {name()}"`), so it is also what recipes/saves reference for that family (e.g.
every `"Iron Pickaxe"` recipe cost/product string is built from `ToolType::Pickaxe.name()
== "Pickaxe"` prefixed by a `TOOL_LEVEL_NAMES` entry). Single-prototype tools are the
`ToolType::flat_name()` set — `Fishing Rod`, `Crossbow`, `Slingshot` — which
`build_registry` pushes once at level 0 instead of running through the 6-tier loop.

### 3.1 Survival weapons (post-port)

All ranged/thrown weapons ride on the `Arrow` entity (`src/entity/projectile.rs`),
extended with a `ProjectileStyle` (`Arrow`/`Spear`/`Knife`/`Pellet`), an optional
`range_ticks` flight limit (`< 0` = unlimited, the plain-arrow behavior), and an optional
`payload` — an `Item::get_data()` string dropped as an `ItemEntity` where the projectile
lands (on hitting a mob, a blocking tile, or running out of range). Payloads are **not**
persisted: a save taken mid-flight reloads the projectile as a plain arrow (flight lasts
well under a second; accepted loss). Tuning constants live at the top of
`src/entity/mob/player_behavior.rs` (`CROSSBOW_DAMAGE`, `SPEAR_RANGE_TICKS`, ...).

| Weapon | Fire path | Ammo | Damage passed to the projectile | Recoverable? |
|---|---|---|---|---|
| Bow (Java) | attack key | 1 `arrow` | `tool_level` (0..=5) | no |
| Crossbow | attack key, gated on `attack_time == 0` (30-tick re-cock) | 1 `arrow` | 7 | no |
| Slingshot | attack key | 1 `Stone` | 0, range 12 ticks | no (stone spent) |
| Throwing Knife | attack key while a `"Throwing Knife"` stack is held (it's a plain `Stackable`, matched by name in `attack()`) | itself (count −1) | 2, range 15 ticks | yes — lands as a `Throwing Knife_1` pickup |
| Spear (thrown) | **SHIFT-attack** while a Spear is held (the whole active item is thrown) | itself | `2 + 2*tool_level`, range 16 ticks | yes — lands as a pickup with durability preserved via the payload data string |

The SHIFT-attack chord needs special input handling: the plain `"attack"` binding never
fires while SHIFT is held (modifier matching zeroes it), so the player tick checks the
physical chords `"shift-space|shift-c"` explicitly — and only routes them to `attack()`
while a Spear is the active item, leaving every other SHIFT combo untouched. All
projectile hits reuse the arrow's `+3` vs-non-player / `+1` non-crit bonus (§ENTITIES
10.2), so even a damage-0 pellet lands 3–4 on a mob.

**Which tile interactions key off which `ToolType`** (grep `ToolType::` in
`src/level/tile/*.rs` — this is the authoritative cross-reference for TERRAIN.md's dig
state machine, which only documents Shovel/Pickaxe on Dug Pit/Chasm):

| Tile file | `ToolType` checked | Effect |
|---|---|---|
| `grass.rs` | `Shovel` | → `dirt`, fiber/seed drop chance (§6) |
| `grass.rs` | `Hoe` | → `farmland` (or a seeds drop instead, 1-in-5) |
| `dirt.rs` | `Shovel` | → `Dug Pit` (TERRAIN.md §4), drops a `dirt` item |
| `dirt.rs` | `Hoe` | → `farmland` |
| `farm.rs` | `Shovel` | farmland → `dirt` |
| `flower.rs` | `Shovel` | picks the flower (drops Flower + maybe Rose), tile → `grass` |
| `sand.rs` | `Shovel` | → `dirt`, drops a `sand` item |
| `tree.rs` | `Axe` | damages the tree (§6); non-lethal hits leave `data` = accumulated damage |
| `rock.rs` | `Pickaxe` | damages the rock, **eligible for coal** (§6) |
| `ore.rs` | `Pickaxe` | damages the ore vein (any pickaxe tier works — see §2.2's stamina table; there is **no** tier gate on which ore a given pickaxe tier can mine) |
| `lava_brick.rs` | `Pickaxe` | → `lava` |
| `depth.rs` (`DugPit`) | `Shovel` then `Pickaxe` | dig stages then open a Chasm (TERRAIN.md §4) |

Tools that don't appear in this table (`Sword`, `Bow`, `Claymore`, `Spear`, `Crossbow`,
`Slingshot`, `FishingRod` outside its own water check) have no `interact` dispatch arm on
any tile — they only ever act on mobs (via `attack`/`get_attack_damage_bonus`, §2.2, or
the projectile paths in §3.1) or, for `FishingRod`, on the one fishable-water special
case in `src/item/interact.rs` (§7.1).

## 4. Inventories (`src/item/inventory.rs`)

```rust
pub struct Inventory {
    items: Vec<Item>,
    player_inv: bool,
    pub creative: bool,
}
```

Confirms PORTING.md's "Inventories are `Vec<Item>` exactly like Java" — with two extra
flags that recreate what was, in Java, an anonymous subclass on `Player`:
`Inventory::new()` (used for chests and any other furniture inventory, see
`ChestData::with_name` in `src/entity/furniture/chest.rs`, which calls plain
`Inventory::new()`) vs. `Inventory::new_player()` (`player_inv: true`), with `creative`
toggled live by the player tick to mirror `Game.isMode("creative")`. **This is the only
inventory-level distinction between a player's inventory and a chest's** — same struct,
same stacking rules, just these two booleans, both `false`/default for a chest.

### 4.1 Stacking rules (`add`/`add_at`/`add_base`)

`add(item)` appends via `add_at(items.len(), item)` → `add_at` → `add_base` (unless
`player_inv && creative`, see §4.2). `add_base` is the real merge logic:

```rust
if item.is_stackable() {
    let to_take = item;
    for existing in self.items.iter_mut() {
        if to_take.stacks_with(existing) {
            let add = to_take.count();
            existing.set_count(existing.count() + add);
            return;
        }
    }
    self.items.insert(slot as usize, to_take);
} else {
    self.items.insert(slot as usize, item);
}
```

So: a stackable item merges into the **first** existing slot whose name matches (linear
scan, first-match, not last-match or best-fit); if no existing stack matches, it becomes
a **new slot** at the given index (not necessarily appended to the end — `add_at` lets
callers insert at an arbitrary slot, though ordinary `add()` always targets the end).
Non-stackable items (tools, furniture, books, ...) **always** get their own new slot —
two `"Iron Pickaxe"` items with different remaining durability never merge, by design
(`item_equals` for tools compares type+level, but stacking uses `stacks_with`, which only
ever returns true for stackable-kind items).

There is **no capacity limit** anywhere in `Inventory` — `items: Vec<Item>` grows
unbounded; the inventory *screen* (`src/screen/player_inv_display.rs`) may paginate or
scroll for display purposes, but nothing in `Inventory` itself caps slot count.

### 4.2 The creative-mode special case

Both `add_at` and `remove` branch on `player_inv && creative` — this exists purely for the
creative-mode "unlimited supply" inventory UX (Java's anonymous subclass override):

- **Adding** (`add_at`): if the player already has *any* count of that item
  (`self.count(&item) > 0`), the add is a no-op (you don't get duplicate slots for
  something you already conceptually "have" in creative). Otherwise the item is force-set
  to `count = 1` (even if the caller passed a bigger stack) before inserting — creative
  slots exist to represent "you have access to this", not a real quantity.
- **Removing** (`remove`): if the item is stackable, its count is force-reset to `1`
  first; if after that its `count(&item) == 1` (i.e. it was the last conceptual unit), the
  slot is removed **and immediately re-added at index 0** (`add_base(0, cur.clone())`) —
  so a creative player can never actually run out of an item slot by using/dropping it;
  it just moves to the front of the inventory list. Non-creative removal is the plain
  `Vec::remove`.

### 4.3 Removal, counting, and non-stackable item consumption

- `remove_items(given, count)` dispatches to `remove_from_stack` for stackable `given`
  (decrements/removes matching stacks until `count` is satisfied, logging a warning if it
  runs out of matching stock) or a linear `item_equals` scan for non-stackables (removes
  whole matching item instances one at a time — e.g. consuming one `"Iron Sword_1"` as a
  Claymore-upgrade recipe cost removes **any** Iron Sword regardless of remaining
  durability, since `item_equals` for tools ignores `dur`).
- `count(given)` sums stackable-matching counts (`stacks_with`) plus one per
  non-stackable `item_equals` match — this is what `Recipe::can_craft_with` and the
  crafting-menu "Have: N" panel both call.

## 5. Recipes (`src/item/recipe.rs`)

### 5.1 The `"Name_amount"` format — literally parsed, not a simplification

```rust
pub fn new(created_item: &str, req_items: &[&str]) -> Recipe {
    let sep: Vec<&str> = created_item.split('_').collect();
    let product = sep[0].to_uppercase();
    let amount: i32 = sep[1].parse().unwrap();
    ...
    for req in req_items {
        let cur_sep: Vec<&str> = req.split('_').collect();
        let cur_item = cur_sep[0].to_uppercase();
        let amt: i32 = cur_sep[1].parse().unwrap();
        ...
    }
}
```

This confirms the brief's assumption precisely: it is a literal `split('_').collect()`,
and `sep[1].parse().unwrap()` — **this panics at startup** (during `Recipes::new()`, i.e.
at `Game::new` time, not lazily) if a product/cost string is missing its `_amount` suffix
or the suffix isn't a valid `i32`. `split('_')` splits on **every** underscore in the
string, not just the first — so a product/cost name containing an underscore would
produce more than 2 pieces and `sep[1]` would then be a schema fragment, not the amount,
likely failing to parse and panicking. No current item name contains an underscore, but
this is a real, unenforced invariant (see §10). Duplicate cost entries for the same item
across separate `req_items` strings are **summed** into one `(name, amount)` pair rather
than kept as separate entries (`if let Some(existing) = costs.iter_mut().find(...) {
existing.1 += amt } else { costs.push(...) }`).

### 5.2 Stations — and THE BENCH

`Recipes` (a plain struct built once in `Recipes::new()`, stored as `g.recipes`) holds
`craft` (personal, no furniture, bound to the `Z`/`Shift-E` key — see `init_key_map`
in `src/core/io/input_handler.rs`), one list per legacy `CrafterType` variant, and
`bench_modules` (the module recipes shown at THE BENCH):

```rust
pub enum CrafterType { Workbench, Oven, Furnace, Anvil, Enchanter, Loom, Bench }
```

**THE BENCH** (UI_REDESIGN §4) is the live crafting identity: personally craftable
(`plank*8 + Cord*2`), it opens with the saw built in (the workbench list) plus the
three module recipes, and grows by bolting on **modules** — `Module::{Vice, Spindle,
AssayKit}` absorb the anvil/loom/enchanter families respectively. Fit one by USING
the bench with the module held (consumed; persisted as trailing ordinals in the
entity save record, old-save tolerant). Modules are found in ruins/village loot or
crafted at the bench itself — the loot is a shortcut, never a gate. The standalone
Anvil/Loom/Enchanter/Workbench **recipes are retired**; legacy placed stations from
old saves still open their lists, and their furniture items break down into their
module via ENTER in the pack (`survival_display::hold_selected`). Heat stays in the
world: Oven and Furnace remain separate, buildable stations.

`CrafterData { furniture, crafter_type, modules }` — `modules` is only meaningful
for the bench; the recipe lists live on `Game`, assembled per-open by
`crafter_behavior::bench_recipes`. Station → sprite/interaction radius mapping
(`CrafterType::sprite()`, `CrafterType::radius()`) also lives in `crafter.rs`.

### 5.3 Craft flow (end to end)

```
 player walks up to a Crafter furniture (or presses Z/Shift-E anywhere for personal craft)
        │
        ▼
 player_behavior.rs: "craft" key clicked && !player_use(g,e)
        │  (player_use(g,e) returning true means a furniture WAS in front of the
        │   player and its own `use_furniture` ran instead — see below)
        ▼
 CraftingDisplay::with_personal(g, g.recipes.craft.clone(), "Crafting", e, true)
        │
        │  (furniture path instead: crafter_behavior::use_furniture picks the list
        │   by crafter_type — g.recipes.workbench / .oven / .furnace / .anvil /
        │   .enchant / .loom — and calls CraftingDisplay::new(g, recipes, name, player))
        ▼
 CraftingDisplay::new/with_personal:
   - wraps every Recipe in Rc<RefCell<Recipe>>
   - recipe.check_can_craft(g, inventory) for every recipe (caches the bool on
     the Recipe itself — this is why Recipe::craft/can_craft_with don't need
     mutation to be called from the tick loop's `select`/`attack` handler)
   - builds 3 menus: the recipe list, a "Have: N" panel for the selected
     recipe's product, and a "Cost: n/have" panel per ingredient
        │
        ▼  player moves selection (arrow keys) → refresh_data() re-reads inventory
        │  counts and re-renders Have:/Cost: panels for the newly selected recipe
        ▼
 player presses select/attack on a recipe whose cached can_craft == true
        │
        ▼
 Recipe::craft(g, inventory):
   - re-validates can_craft_with(g, inventory) (creative mode always true)
   - if not creative: inventory.remove_items(registry::get(g, cost), amt) for
     every (cost, amt) pair
   - inventory.add(registry::get(g, product)) called `amount` times
        │
        ▼
 refresh_data() again + check_can_craft() re-run for every recipe in the list
 (crafting one item may make/break affordability of others sharing ingredients)
```

`player_use` (the `!player_use(g, e)` guard before opening personal crafting or the
player inventory) is the same "is there furniture directly in front of the player" check
used by the `E`/`I` inventory key — if a `Crafter` is in front of you, `Z` opens *its*
recipe list via `use_furniture`, never the personal list; personal crafting only opens in
open air.

## 6. The gather chain

Verified tile-by-tile against `src/level/tile/{grass,tall_grass,tree,rock}.rs`. Every
percentage below is the literal `g.random.next_int_bound(N) == k` roll in the source, not
an approximation.

| Tile | Action | Tool requirement | Drops | Notes |
|---|---|---|---|---|
| Grass | Shovel dig (`interact`) | Shovel (any tier) | → `dirt` tile always; `Grass Fibers` 1-in-4; `Seeds ×2` 1-in-5 | tile becomes `dirt`, not consumed instantly — see TERRAIN.md's Dug Pit chain for what happens if you then shovel the resulting `dirt` |
| Grass | Hoe till (`interact`) | Hoe (any tier) | 1-in-5: `Seeds` (no farmland made that hit); else → `farmland` | |
| Tall Grass (kind 2, "Tall") | punch/`hurt_by` (**no tool needed** — any damage source) | none | `Grass Fibers ×2` guaranteed; `Stone` 1-in-4 | tile → `grass`; the reliable fiber source of the bare-handed loop |
| Tall Grass (kind 0/1, "Small"/"Medium") | punch/`hurt_by` | none | `Grass Fibers` 1-in-3; `Stone` 1-in-8 | tile → `grass`; also self-grows kind 0→1→(stays 2, never regresses), each tick at 1-in-2000 (slowed post-port so a growth stage takes a few in-game days, not seconds) |
| Tree | Axe chop (`interact`) | Axe (any tier; damage scales with tier, §2.2) | per hit: `Apple` 1-in-100, `Stick` 1-in-6; on reaching `tree_health = 20` cumulative damage: `Wood ×1-2`, `Acorn ×1-2`, `Stick ×1-2` | tile → `grass` once felled; `hurt_by` (mob/explosion damage) reaches the same drop table via the shared `hurt_dmg` |
| Rock | Pickaxe mine (`interact`) | Pickaxe (any tier; damage scales with tier) | on reaching `rock_health = 50` cumulative damage: `Stone ×1-4` (no coal) if mined by a non-pickaxe source (`hurt_by`, mob smash); `Stone ×1-2` + `Coal` (`0-1` normal / `1-2` on non-Hard difficulty) if mined by a Pickaxe `interact` | the Java "coallvl" global-state bug (coal availability leaking across worlds) is fixed post-port: a `drops_coal: bool` flag threaded per-break instead of a singleton-tile field — see the `// JAVA:`/`// FIX:` comment atop `rock.rs` |

**The resulting bare-handed → crude-tools → workbench ladder**, cross-referenced against
the actual `Recipe::new` calls in `Recipes::new()` (`src/item/recipe.rs`):

```
  Tall Grass (punch)         Tree (Axe or bare fists)        Rock/Tall Grass (pickaxe or
   → Grass Fibers                → Stick (1-in-6/hit)            pebble luck)
   → Stone (rare)                                                → Stone
        │                              │                              │
        ▼                              ▼                              ▼
   craft: "Cord_1"             craft: "Stick_2"              craft: "Sharp Stone_1"
    ← Grass Fibers_3            ← Wood_1                      ← Stone_2
        │                              │                              │
        └──────────────┬───────────────┴──────────────┬───────────────┘
                        ▼                              ▼
           craft: "Crude Axe_1"              craft: "Crude Pickaxe_1"
            ← Stick_1, Cord_1, Sharp Stone_1  ← Stick_1, Cord_1, Sharp Stone_1
                        │                              │
                        └──────────────┬───────────────┘
                                       ▼
                     craft: "Workbench_1"  ← Wood_10, Stone_2
                                       │
                                       ▼
     workbench recipes unlock: Wood/Rock-tier tools (e.g. "Wood Axe_1" ←
     Wood_5, Stick_2, Cord_1 — the "handle + lashing + head material"
     pattern ADDING_CONTENT.md calls out), Raft_1 ← Wood_10, Cord_2,
     Oven_1 ← Stone_15, Furnace_1 ← Stone_20, Anvil_1 ← iron_5, Loom_1 ←
     Wood_10, Wool_5, Chest_1 ← Wood_20, Enchanter_1 ← Wood_5, String_2,
     Lapis_10
                                       │
                    Anvil unlocks Iron/Gold/Gem-tier tools (e.g.
                    "Iron Pickaxe_1" ← Wood_5, iron_5 — no Stick/Cord
                    needed once you have a metal-working station) plus
                    Claymore upgrades ("Iron Claymore_1" ← Iron Sword_1,
                    shard_15 — consumes a finished Iron Sword, not raw
                    materials)
```

**Survival-weapons/food additions (post-port)** — the same station progression extended:

```
  hand (personal craft):
    Crude Spear_1      <- Stick_2, Cord_1, Sharp Stone_1
    Throwing Knife_1   <- Sharp Stone_1, Stick_1, Cord_1
    Slingshot_1        <- Stick_2, Cord_2            (fires Stone items)
    Bandage_1          <- Cord_2, Grass Fibers_2     (Medical: +3 health)
    Jack-O-Lantern_1   <- Pumpkin_1, Torch_1         (placeable light, LanternType::Jacko)
    Fruit Medley_1     <- Berry_2, Apple_1           (no-cook food, heal 3)
    Campfire_1         <- Stone_5, Stick_3, Wood_2   (fire wave: places lit, the 2 Wood
                                                      already burning as fuel — see
                                                      ENTITIES.md's furniture table)
  workbench:
    Wood Spear_1  <- Wood_5, Stick_2, Cord_1     Rock Spear_1 <- Stone_5, Stick_2, Cord_1
    Crossbow_1    <- Wood_5, Stick_2, Cord_2, Crossbow Mechanism_1
  anvil:
    Crossbow Mechanism_1 <- iron_3               (the crossbow's forged half)
    Iron/Gold/Gem Spear_1 <- Wood_5 + iron_5 / gold_5 / gem_50
  oven AND furnace:
    Cooked Mushroom_1 <- Mushroom_1, coal_1
```

Field cooking (fire wave, generalized by the farming wave): interacting with a
**lit Campfire** while holding anything in `item/cooking.rs`'s `cooked_result` table
roasts it 1:1 with no coal (raw meats/fish, Mushroom, Potato, Corn, Pumpkin, and the
Mushroom Skewer), at a small fuel cost plus a smoke puff and the Craft sound — an
entity-interact path (`campfire_behavior::interact`), not a recipe list, so it does
not appear in any crafting menu.

Registered forage foods (these exact names are the contract): `Berry` (heal 1),
`Mushroom` (1), `Apple` (1, pre-existing), `Cactus Fruit` (1), `Coconut` (2),
`Cooked Mushroom` (3), `Pumpkin` (2, dropped by smashing a pumpkin tile — along
with 0-2 `Pumpkin Seeds`), `Fruit Medley` (3).

**Farming & cooking wave** — the crop loop and the pot dishes. Every seed comes out
of the world, none from a menu:

| Crop | Seed item | Seed source | Raw (heal) | Cooked (heal) |
|---|---|---|---|---|
| Carrot Crop | `Carrot Seeds` | Wild Carrot plants (plains/forest bands), village fields/chests, harvest return | `Carrot` (1) | — (stew ingredient) |
| Potato Crop | `Seed Potato` | **panning** river banks (`PanFind::Tuber`, surface only), harvest return | `Potato` (1, Queasy risk raw) | `Baked Potato` (3) |
| Corn Crop | `Corn Kernels` | village fields (structures_gen plots) + village chests, harvest return | `Corn` (1) | `Roast Corn` (3) |
| Pumpkin Vine | `Pumpkin Seeds` | smashed pumpkins (wild plains patches; legacy blob spawns) | `Pumpkin` (2) | `Roast Pumpkin` (4) |

Crop tiles (`level/tile/crop.rs`, `TileKind::Crop`) ride the wheat clock: data byte
= age 0..50, three drawn stages, partial harvest from 40, full at 50 (2-3 produce +
1-2 seeds back, tile → dirt). Growth leans into the weather sim: +1 step next to
water, +1 while it rains (`weather::growth_boost` — wheat got the same rain step).
A ripe Pumpkin Vine *becomes* a `pumpkin` tile instead of being harvested in place.

The eating tiers, top to bottom: composed hot dish (`Hearty Stew` 8, `Fish Chowder`
7 — oven-only pot cookery; eating one also refills stamina and grants a 600-tick
Regen, `cooking::is_hearty`) > stick food (`Roasted Skewer` 6) > cooked singles
(3-4) > raw (1-2). Raw flesh and raw potato gamble a 1-in-3 **Queasy** spell on
eating (`PotionType::Queasy`, 3600 ticks, halves stamina-recharge progress; rides
the potion-effect timer/HUD/save machinery but has no brewable potion item).
New recipes: `Mushroom Skewer` ← Stick + Mushroom*2 (personal); `Baked Potato` /
`Roast Corn` / `Roast Pumpkin` / `Roasted Skewer` ← raw + coal (oven AND furnace);
`Hearty Stew` ← Raw Beef + Potato + Carrot + coal and `Fish Chowder` ← Raw Fish +
Potato + Corn + coal (oven only). Tests: `tests/farming_cooking.rs`.

Every recipe quoted above is copied verbatim from `Recipes::new()` — no quantities were
invented. Note the "no station should conjure a finished wood/metal tool from raw
materials" rule ADDING_CONTENT.md states is enforced exactly as described:
`tests/crafting_chain.rs`'s `personal_crafting_offers_the_survival_chain` asserts the six
`Wood *` tools are **absent** from `g.recipes.craft` and that their `workbench` recipes
all require `Cord` (see §11).

## 7. Item-use dispatch (`src/item/interact.rs`)

Two entry points, both dispatched by a `match &item.kind`:

- `item_interact_entity(g, item, player, entity, attack_dir)` — only `PowerGlove` does
  anything here (`furniture_take`, which picks up a furniture entity into the inventory;
  `DeathChest`/`DungeonChest` override the take behavior, everything else uses the
  generic `furniture::behavior::take`). Every other `ItemKind` returns `false` (Java
  `Item.interact` default).
- `item_interact_on_tile(g, item, lvl, xt, yt, player, attack_dir)` — the big one, one
  arm per `ItemKind`:

| `ItemKind` arm | Behavior |
|---|---|
| `Tool { ttype: FishingRod, .. }` on fishable water (`water`, `Deep Water`, or a Tidal Flat while `tidal::is_submerged`) | pays durability, calls `player_behavior::go_fishing` (§7.1) |
| `Tool` (any other type/tile) | falls through to `false` here — actual tool-vs-tile effects (dig/chop/mine) are dispatched separately through `level::tile::dispatch::interact`, not through this function (see §3's table) |
| `TileItem { model, valid_tiles, .. }` | if the tile at `(xt,yt)` matches any `valid_tiles` name (`tiles::matches`), places `model` there (`set_tile_named`) and consumes one unit (`stackable_interact_on`); otherwise pushes a "Can only be placed on ..." / "Dig a hole first!" notification based on whether `model` contains `"WALL"`/`"DOOR"` vs `"BRICK"`/`"PLANK"` |
| `Torch { valid_tiles, .. }` | if the tile name is in `valid_tiles`, swaps in the lit-torch variant via `Tiles::get_torch_tile` |
| `Bucket { filling, .. }` | fills from an empty bucket onto matching liquid, or empties a full bucket back onto a `hole`; swaps the held bucket's `Fill` via `edit_bucket` (splits one off a stack if `count > 1`, otherwise replaces the item in place) |
| `Food { count, heal, stamina_cost }` | if hunger isn't maxed and `stamina_cost` (always 5) can be paid: restores `heal` hunger, consumes one unit |
| `Armor { armor, stamina_cost, .. }` | if no armor currently worn and `stamina_cost` (always 9) can be paid: equips (`cur_armor = Some(item.clone())`, `armor_points = armor * MAX_ARMOR`), consumes one unit from the stack (the worn copy is a clone, so unequipping doesn't need to "put it back" — see §10 for the resulting duplication-looking-but-intentional behavior) |
| `Clothing { player_col, .. }` | recolors the player shirt if different from current, consumes one unit |
| `Potion { ptype, .. }` | drinks it — `apply_potion(g, player, ptype, true)`, toggling the effect (see below) |
| `Book { book, has_title_page }` | opens `BookDisplay` with the asset text (`None` = blank book) |
| `Furniture { furniture, placed }` | if `tiles::may_pass` allows it at `(xt,yt)`: places a clone of `furniture` into the level at that tile's center, and either re-clones a fresh instance (creative mode — infinite placements) or marks `placed = true` (consumes the item) |
| everything else (`PowerGlove`, plain `Stackable`/`Unknown`) | `false` (no-op) |

### 7.1 Fishing (`player_behavior::go_fishing`)

The fishing wave replaced the Java flat table with **invisible fish**: no fish mobs —
the water itself tells you where they are. The rod's `interactOn` gate above decides
*whether* a tile is fishable and pays one durability per cast (stamina is paid by the
normal attack flow, unchanged); `go_fishing(g, player, x, y, xt, yt)` then rolls the
cast against the `(xt, yt)` tile the line landed on:

- **Odds** — per-cast catch chance = `fishing_catch_chance(presence, raining)`, a pure
  fn over `weather::fish_presence(world_seed, xt, yt)` (the same field that draws the
  rising bubbles) and `weather::is_raining`:
  - base `0.18` (the Java table's ~16/90 feel) on ordinary water;
  - at/above `FISH_PRESENCE_THRESHOLD` — exactly the bubble edge — a flat **3x**;
  - below it, `0.25 + 1.2 * presence` (dead water bottoms out at ~0.25x);
  - raining: a further **1.3x** (fish bite in the rain); total capped at `0.95`.
- **Flavor cues** (deduped against the last notification): "Something stirs here..."
  on a bubbling cast; "The rain has the fish biting..." on a rainy, non-bubbling one.
- **Catch tables**, picked by where the line landed:

| Water | Table (d100) |
|---|---|
| `water` / submerged Tidal Flat (`CastWater::Regular`) | 65% `Raw Fish`, 29% `Slime`, 6% `Leather Armor` — the Java trio, fish-forward |
| `Deep Water` (`CastWater::Deep`, cast from a raft or the shore edge) | 78% `Raw Fish`, 17% `Big Fish`, 5% treasure (2% `gem`, 3% `Iron`) |
| any `depth < 0` pool (`CastWater::Cave`) | 85% `Cave Eel`, 15% `Slime` |

- **New foods** (registry one-liners; oven recipes `X + coal` like Cooked Fish):
  `Big Fish` (heal 2) → `Cooked Big Fish` (**5** — the fisherman's payoff), `Cave Eel`
  (1) → `Cooked Cave Eel` (3). TODO(art): dedicated cells — placeholders recolor the
  fish cell `(24,4)`.
- The FISHNORRIS console easter egg survives on a missed cast, message verbatim.
- Tests: `tests/fishing.rs` — hotspot-vs-dead-water odds, the rain bonus, both new
  tables, the rod gate/durability on every water kind, tide-gated flats, and the
  cook-and-heal chain.

Potions (`src/item/potion_type.rs`) have **11** variants (`None` is the "does nothing"
base potion you get from the enchanter before adding an effect ingredient):

| `PotionType` | Duration (ticks) | Special `toggle_effect` behavior |
|---|---|---|
| `None` | 0 | never applies |
| `Speed` | 4200 | `move_speed += 1` on, `-= 1` off (floored at 1.0) |
| `Light` | 6000 | default (radius handled elsewhere by checking `potioneffects`) |
| `Swim` | 4800 | default |
| `Energy` | 8400 | default — but see `pay_stamina`: while active, **all** stamina costs are free (checked directly in `pay_stamina`, not via `toggle_effect`) |
| `Regen` | 1800 | default |
| `Health` | 0 (one-shot) | instant `heal(g, player, 5)` on apply, no lingering effect |
| `Time` | 1800 | default |
| `Lava` | 7200 | default |
| `Shield` | 5400 | default |
| `Haste` | 4800 | default |

"default" means `toggle_effect`'s catch-all arm just returns `true` — the gameplay effect
for those types is read elsewhere (e.g. render/movement code checking
`player.potioneffects.contains_key(&PotionType::X)`), not implemented as a state mutation
in `toggle_effect` itself.

## 8. The Raft and deep-water rule

Cross-referencing TERRAIN.md §4's `deep_water_may_pass` note from the item side, verified
by reading every `Raft` occurrence in the codebase (exactly three files reference it:
`registry.rs`, `recipe.rs`, `depth.rs` — there is no fourth "player_behavior.rs check";
the mechanic lives entirely in the tile module).

- **Obtained**: `Raft` is a plain `Stackable` item (`registry.rs`:
  `stackable("Raft", Sprite::new1x1(28, 4, ...))`) — no special `ItemKind`. Crafted at the
  **Workbench**: `workbench.push(Recipe::new("Raft_1", &["Wood_10", "Cord_2"]));`
  (`recipe.rs`). It is not personally craftable and has no tool-tier variants (unlike the
  tool ladder, it's a single flat item).
- **The check, exact code** (`deep_water_may_pass`, `src/level/tile/depth.rs`):
  ```rust
  pub fn deep_water_may_pass(g: &Game, e: &Entity) -> bool {
      match &e.kind {
          EntityKind::Player(_) => {
              let inv = &e.player().inventory;
              g.is_mode("creative")
                  || inv.items().iter().any(|i| i.get_name().eq_ignore_ascii_case("Raft"))
          }
          EntityKind::ItemEntity(_) => true,
          _ => false,
      }
  }
  ```
  This is **"any item in the inventory named Raft (case-insensitive)"** —
  `inv.items().iter().any(...)` scans the *entire* inventory list, not the player's
  `active_item`/currently-equipped slot. Carrying a Raft anywhere in your inventory (not
  holding it, not it being the selected hotbar item) is sufficient. This confirms
  TERRAIN.md's description precisely; there is no separate "equipped" check anywhere.
- **Not consumed on use**: `deep_water_may_pass` only reads the inventory — it never
  calls `remove_items`/`remove`. Nothing else in the codebase interacts with a `Raft`
  item's count (it has no `ItemKind::Tool` durability to pay, no `interactOn` arm in
  §7's dispatch table — it's a bare `Stackable`, so "using" it does nothing; it just
  needs to exist in the inventory). A player can walk onto Deep Water indefinitely with a
  single Raft in their pack, or even drop it while standing there (walking off would then
  refuse re-entry until it's picked back up or another is crafted).
- **Cosmetic raft visual is independent of the gate**: `player_behavior.rs`'s `render`
  draws the two-tile raft glyph under the player whenever `tile_at(...).id == Deep
  Water.id`, with no `Raft`-item or creative check — so a creative player standing on Deep
  Water (allowed via the `g.is_mode("creative")` branch above, no `Raft` item required)
  still gets the raft sprite. TERRAIN.md already notes this; confirmed here from the item
  side that the visual and the `may_pass` gate read two entirely different conditions
  (tile-under-feet vs. inventory-contains-Raft-or-creative).

## 9. HOW TO EXTEND

### 9.1 Add a new item

1. Decide if it needs a new `ItemKind` variant (§1) or fits an existing one
   (`Stackable`/`Food`/`Armor`/... cover most raw materials, foods, and equipment).
2. Add a prototype push in `build_registry` (`src/item/registry.rs`), in the block for
   its family, using the matching helper (`stackable`, `food`, `armor`, `clothing`,
   `tile_item`, or a bespoke `new_*_item` constructor if it needs a genuinely new
   `ItemKind`). List order is the creative-inventory order — keep it near its family.
3. Give it a sprite (see ADDING_CONTENT.md's "Sprite-sheet geography" section — same
   sheet, same 8x8-cell/4-shade-palette rules apply to items as tiles).
4. Make it obtainable: a recipe (§9.2), a tile drop (§9.4), a mob drop
   (`mobai_drop_items` in the mob's `die`), or a chest loot table
   (`src/entity/furniture/dungeon_chest*.rs`).
5. If it needs to *do* something when used, add an `item_interact_on_tile` arm (§7) — a
   plain `Stackable`/`Food`/`Armor`/etc. with no special behavior needs nothing further,
   the existing arm for its `ItemKind` already handles it generically.
6. Verify: `--debug` + `SHIFT-G` (fill creative inventory) to confirm it registers and
   renders; `cargo test` runs `tests/crafting_chain.rs`'s registry sweep if you also gave
   it a recipe.

### 9.2 Add a new recipe

1. Push a `Recipe::new("Product_amount", &["Cost_n", ...])` onto the right station list
   in `Recipes::new()` (`src/item/recipe.rs`) — `craft` for bare-handed/personal,
   otherwise the `CrafterType`-matching field (`workbench`, `oven`, `furnace`, `anvil`,
   `enchant`, `loom`).
2. Every name (product and every cost) is looked up case-insensitively against the
   registry at *craft time*, not recipe-registration time — a typo'd name still compiles
   and registers, it just never resolves to a real item (`registry::get` prints a warning
   and returns `UnknownItem`). `tests/crafting_chain.rs`'s
   `all_recipe_names_resolve_in_registry` test is what actually catches this — always run
   `cargo test` after adding a recipe.
3. Respect the station progression (ADDING_CONTENT.md's "7-Days-style gathering chain"
   comment atop `Recipes::new()` — quoted and expanded in §6 here): personal crafting
   stays bare-handed/no-station-appropriate (fiber/stick/stone-tier items and Crude
   tools only); Workbench assembles wood/rock tools with the handle+lashing+head
   pattern; Anvil is for metal tiers and Claymore upgrades; Loom is cloth/wool; Oven is
   cooking; Furnace is smelting; Enchanter is potions.
4. No manual wiring is needed to make it show up in a `CraftingDisplay` — every station's
   menu is built directly from its `Recipes` field each time the display opens (§5.3), so
   pushing the `Recipe` is the entire "show up in the crafting menu" step. Verify by
   opening that station (or `Z` for personal) in a debug run and confirming the "Cost:"
   panel shows the right ingredients and amounts.

### 9.3 Add a new tool type

1. Add a variant to `ToolType` (`src/item/tool_type.rs`): extend the enum, `VALUES`,
   `sprite()` (a sheet row), `durability()` (the base value multiplied by tier, §2.2),
   and `name()`.
2. No registry.rs change is needed for the tier ladder itself — the `for ttype in
   ToolType::VALUES { for lvl in 0..=5 { ... } }` loop in `build_registry` (§2.2)
   auto-generates all 6 tiers for any new non-`FishingRod` variant. If the new type
   should behave like `FishingRod` (a single non-tiered item), special-case it the same
   way that loop does (`if ttype == ToolType::X { continue; }` plus a manual
   `new_tool_item(ttype, 0)` push).
3. Give it tile behavior: add a `ttype == ToolType::YourType` check in every tile's
   `interact` that should respond to it (follow the pattern in §3's table — destructure
   `ItemKind::Tool { ttype, level: tool_level, .. }`, check `ttype`, pay stamina with
   your chosen `BASE - tool_level` formula, pay durability, do the tile effect).
4. If it should also affect mob combat, add a match arm to
   `get_attack_damage_bonus` (`src/entity/mob/player_behavior.rs`, §2.2) — omitting it
   just means the new type falls into the `_ => 1` flat-damage default, which is a valid
   choice for a non-combat tool (like `Shovel`/`Hoe`/`Pickaxe` today).
5. Add recipes for whichever tiers should be craftable (§9.2) following the existing
   per-`ToolType` recipe rows in `workbench`/`anvil`.

### 9.4 Add a new drop to an existing tile

1. Find the tile's module in `src/level/tile/` and its `hurt_by` (generic damage,
   including mob/explosion sources) and/or `interact` (tool-specific) functions —
   whichever paths should produce the new drop.
2. Match the file's existing drop-chance convention: single-roll chances are
   `g.random.next_int_bound(N) == k` (dropping on a specific one of N outcomes, e.g.
   grass's `next_int_bound(4) == 0` for a 1-in-4 fiber chance); ranged counts use
   `drop_items_counted(g, lvl, x, y, min, max, &[item])`, which itself rolls
   `min + next_int_bound(max - min + 1)` copies. Use `crate::level::drop_item` for a
   single guaranteed/chance drop, `drop_items_counted` for a min..=max range.
3. Decide whether the drop needs tool-gating: if it should only happen via a specific
   `ToolType`, put the roll inside that tile's `interact` (which already destructures
   `ItemKind::Tool { ttype, .. }`); if it should happen from *any* damage source
   (punching, explosions, mob collateral), put it in `hurt_by`/the shared `hurt_dmg`
   helper the tile uses, matching Rock's `drops_coal: bool` pattern (§6) if the two paths
   need genuinely different drop tables from the same tile.
4. Register the dropped item in `build_registry` first if it's new (§9.1).

## 10. Invariants & gotchas

- **`tool_level` and the tier `level` are the same number, always.** There is no
  separate "tool level" system distinct from the 0..5 `TOOL_LEVEL_NAMES` index — every
  `4 - tool_level`/`6 - tool_level`/`2 - tool_level` stamina formula in `src/level/tile/`
  and TERRAIN.md's dig-state-machine writeup are reading the exact same `ItemKind::Tool
  { level, .. }` field, just locally renamed at the destructuring site for readability.
  Don't introduce a genuinely separate "tool tier" concept when extending this system.
- **Armor does not share the tool tier ladder** (§2.3) — it's five hand-authored items
  with independent `level` numbers (1–5) that only coincidentally overlap the tool
  ladder's numeric range. A new armor tier is a new `armor()` call, not a loop iteration.
- **Item names must not contain an underscore.** `Recipe::new`'s `"Name_amount"` parsing
  is a literal `split('_').collect()` with `sep[1].parse().unwrap()` — a name containing
  `_` produces more than two pieces and either panics on `.unwrap()` or silently reads the
  wrong piece as the amount. No current item has an underscore in its name, but nothing
  enforces this; check before naming new content.
- **Item name lookups are case-insensitive by comparison, not by storage.** Unlike tile
  names (`TileDef.name` is stored uppercase — TERRAIN.md §8), item names are stored
  exactly as authored in `Item::new` (e.g. `"Grass Fibers"`, mixed case) —
  `registry::get_opt` uppercases *both* the query and does `eq_ignore_ascii_case` against
  the stored (non-uppercased) name, so the comparison is case-insensitive without the
  storage convention TileDef uses. `ItemKind::TileItem`'s `model`/`valid_tiles` fields
  *are* uppercased at construction time (`tile_item()` helper) because those reference
  *tile* names, which do follow the uppercase-storage convention.
- **`Recipe::new` panics at `Game::new` time on a malformed string**, not lazily at craft
  time — `sep[1].parse().unwrap()` has no fallback. Contrast with `registry::get_opt`'s
  item-name parsing, which uses `unwrap_or(1)` and never panics. A bad recipe string is a
  crash-on-boot bug, not a silent-failure one.
- **A recipe with an unresolvable item name doesn't fail to compile or register** — it
  just never becomes craftable (its cost check always fails, or its product resolves to
  `UnknownItem`). Only `tests/crafting_chain.rs`'s `all_recipe_names_resolve_in_registry`
  catches this; there is no runtime assertion.
- **Stacking merges into the *first* matching slot**, not the last or a "best fit" slot —
  if an inventory somehow has two separate stacks of the same stackable item (this
  shouldn't normally happen since `add_base` always merges into an existing match first,
  but can arise from load-time reconstruction or direct `add_at` calls with an explicit
  slot), only the first is grown; the second stays separate until something moves it.
- **Creative-mode inventory add/remove behavior is a UX affordance layered on the same
  `Vec`**, not a different data structure (§4.2) — `player_inv && creative` is checked at
  the top of `add_at`/`remove` and silently changes semantics (single-count slots,
  auto-reinsertion at index 0 on "removal"). Code that manipulates a player's inventory
  directly (bypassing `add`/`remove`, e.g. via `items_mut()` if it existed, though today
  there is no such accessor) would skip this and could produce a creative inventory that
  looks like a survival one.
- **Ore mining has no tier gate.** Any `Pickaxe` tier can mine any ore vein at any depth
  (§3's table) — the only tier effect is the `6 - tool_level` stamina cost getting
  cheaper at higher tiers. There is no "need an Iron Pickaxe to mine Gold" restriction
  anywhere in the code, unlike some Minicraft-family games' conventions.
- **The stale attack-damage comments in `player_behavior.rs`** (§2.2) describe pre-Crude
  -tier damage numbers; trust the formula and the code, not those two comment lines, when
  reasoning about current Axe/Sword/Claymore damage.
- **`ItemEntity`/dropped-item physics live in [ENTITIES.md](ENTITIES.md#10-item-entities-and-projectiles),
  not here.** This document only covers `Item` as inert data; don't duplicate the
  pickup-delay/despawn/magnetism details in this file if extending it later.

## 11. Test coverage map

| Test file | Locks in |
|---|---|
| `tests/crafting_chain.rs` (`all_recipe_names_resolve_in_registry`) | Every recipe (all 7 station lists) has a product and every cost resolving to a real registry item, not `UnknownItem` — catches typo'd recipe strings. |
| `tests/crafting_chain.rs` (`personal_crafting_offers_the_survival_chain`) | Personal (`craft`) crafting offers exactly the bare-handed chain (Cord, Sharp Stone, Stick, Crude Axe, Crude Pickaxe, Fishing Rod, Workbench) and does **not** offer any `Wood *` tool; every `Wood *` tool's Workbench recipe requires Cord. |
| `tests/crafting_chain.rs` (`early_loop_crafts_a_crude_axe`) | End-to-end: gathered Grass Fibers/Stone/Wood craft (via the personal `craft` list) into Stick → Cord → Sharp Stone → Crude Axe, consuming exact quantities (1 Wood → 2 Sticks, one consumed by the axe, one left over); asserts the resulting item is `ItemKind::Tool { ttype: Axe, level: 0, .. }` with positive durability. |
| `tests/crafting_chain.rs` (`crude_axe_outchops_fists_and_grass_yields_fibers`) | A Crude Axe interact deals more tile damage than a bare-fist `hurt_by` roll, pays durability (`dur < ToolType::Axe.durability()` afterward), and punching Tall Grass bare-handed yields ≥2 `Grass Fibers` `ItemEntity` drops. |
| `tests/weapons_and_food.rs` | The survival-weapons wave: new recipes sit at the intended stations and every one crafts headlessly from exactly its listed costs; the forage foods resolve with the agreed names/heal values; Crossbow/Slingshot/Throwing Knife each damage a zombie in a live-fire headless world; the spear SHIFT-throw lands as a pickup and round-trips with durability intact; a Bandage heals 3 health (not hunger) and is consumed. |
| `tests/multi_level_terrain.rs` (`deep_water_needs_a_raft`) | `deep_water_may_pass` blocks a raftless player on Deep Water and allows one with a `Raft` item added to inventory (§8). |
| `tests/save_load_roundtrip.rs` | Inventory save/load round-trips item names + counts + the active-item-becomes-slot-0 convention (asserts a saved `Wood Pickaxe` active item and a `Wood_10` stack reload correctly by name and count). |
| `tests/keymap_check.rs` | `CRAFT`/`INVENTORY` key mappings resolve and actually open their respective displays in a running `Game` (`e_opens_inventory_in_game`; the analogous craft-key assertion lives alongside it). |
| `tests/display_flow.rs` (`inventory_esc_closes`) | The inventory display (which renders `Item`s via `render_inventory`) opens and closes correctly through the display stack. |

Run everything item/crafting-related with:

```
cargo test --test crafting_chain --test multi_level_terrain --test save_load_roundtrip \
  --test keymap_check --test display_flow
```

(`crafting_chain`, `save_load_roundtrip`, `keymap_check`, and `display_flow` don't match a
narrower name-based filter, so run them explicitly by `--test` file name; there is no
inline `#[cfg(test)]` module anywhere under `src/item/` today — all item/crafting
coverage lives in the `tests/` integration suite.) See [DEV_GUIDE.md](DEV_GUIDE.md) for
`--debug`/`SHIFT-G` (fill creative inventory) and other manual-verification cheat keys
useful when eyeballing a new item or recipe interactively.
