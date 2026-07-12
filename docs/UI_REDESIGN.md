# UI REDESIGN — HUD, the Survival Screen, and the Crafting Identity

Status: **design, not implemented**. Product owner directive: "redo the hud... a single
screen / display that all of this can be achieved in... a more powerful crafting box...
unless we can come up with a better idea than copying Minecraft's crafting table...
clean up the hud, make the game easier to play / view, and understand."

Method: read RENDERING_AND_UI / PLAYTEST / ODDITIES + the screen sources, then **played
the current build** via scripted FDOOM_DEMO runs (world `UIX`, seed-random tundra spawn,
debug console). Every claim below is backed by a screenshot in
`target/verify/ui_mock/` (`before_*.png` = current build at 1x, `*_big.png` = 4x
upscales; `mock_*.png` = proposed frames, composed at exactly 288x192 from the game's
own font glyphs, palette math (`color::upgrade` ported), icon crops from real frames,
and the real smoked-glass darken (185) — every coordinate in the mockups is a real
coordinate).

Decisions in one breath: **frameless corner-anchored HUD with need-to-know meters;
one survival screen on E with four tabs (PACK / WEAR / CRAFT / SELF) navigated
left/right; equipping becomes a visible slot action instead of a hidden
attack-the-ground ritual; crafting identity = THE BENCH — one upgradeable prospector's
bench with physical modules, heat stays in the oven/furnace; field-notes journals are
the runner-up, folded in later as additive variants only.**

---

## 1. Audit — what is janky today, exactly

Numbers refer to `src/core/renderer.rs::render_gui` (cell-coord frame boxes
`(0,0,10,4)`, `(11,0,25,2)`, `(26,0,35,2)`) and the screen sources.

### 1.1 The HUD (`before_hud_noon.png`, `before_hud_held_armor.png`)

- **J1 — The whole top edge is furniture.** Three heavy 9-slice frame boxes span all
  288px of the top edge, 24-40px tall (up to 21% of the screen). The middle box (the
  held item) displays a lone `-` for the entire early game; the right box shows
  `X0` arrows forever even if no bow exists in the world. Empty chrome outweighs
  content most of the session.
- **J2 — The temperature dot reads as a glitch.** The (just-landed, good) system
  paints its 7x7 dot at `(84,13)` — *on the border seam* between two frame boxes
  (`before_hud_noon`, blue square). An instrument this important (it predicts damage)
  looks like a stuck pixel.
- **J3 — Held-item text self-destructs.** `1 LEATHER..` (`before_hud_held_armor`):
  the clip that landed with playtest #9 spends its budget on the count prefix and
  ellipsis; the actual noun is gone. Long tool names still crowd the arrow box.
- **J4 — The armor meter lives alone at the bottom.** Hearts/stamina/hunger stack
  top-left; armor pips render at `y = H-24`, bottom-left, in nobody's eye line —
  most players never learn armor *has* a meter.
- **J5 — Meters are always-on noise.** Ten bright bolts + ten hearts + ten drumsticks
  glow at full saturation from minute zero, violating the project's own established
  principle (the temp dot hides in the comfort band; nothing else earned that polish).

### 1.2 The menus (`before_inv_full.png`, `before_inv_echo.png`, `before_craft.png`, `before_chest.png`)

- **J6 — The selected-row echo breaks every panel.** `inventory_menu::render_selected_info`
  re-renders the selected row pinned onto the *bottom border*, left-shifted outside
  the panel edge (`before_inv_full` bottom-left, `before_inv_echo`, `before_chest`).
  It duplicates a row that is already flanked by `>` `<` cursors 60px above — double
  emphasis, zero information, plus a visible layout bug.
- **J7 — Counts are baked into names.** Rows read `30 PLANK`, `8 TORCH` — the number
  is part of the label string, so nothing aligns and scanning quantities means
  reading every line.
- **J8 — Two panel languages.** The crafting screen wears a red-brown 2px frame;
  inventory wears gray-blue; the HAVE:/COST: readouts are *two extra floating framed
  boxes* below the crafting panel (`before_craft`), anchored to the recipe menu's
  bottom edge, covering the world and jumping as you scroll.
- **J9 — Six stations, one screen, six titles.** Workbench, Oven, Furnace, Anvil,
  Enchanter, Loom (`crafter.rs::CrafterType`) all open the identical
  `CraftingDisplay` with a different recipe list. Station identity is a title
  string; the world objects are interchangeable menus on legs.
- **J10 — The container dance.** `ContainerDisplay` pins the *focused* pane flush
  against the screen edge and nudges panes around on every focus switch
  (`before_chest`: INVENTORY title crowding the right bezel, mismatched pane
  widths, echo row bleeding into the neighbor pane).
- **J11 — Invisible modality on E.** E opens your inventory *unless* furniture is in
  front, in which case it opens the furniture (`player_behavior.rs:443`). You cannot
  check your pack while standing at a bench, and nothing on screen explains why E
  changed meaning.

### 1.3 Equipping — the flow that failed four scripted attempts

To wear armor today you must: open inventory → ENTER to *hold* the armor →
close → face an **empty** tile → press ATTACK (`interact.rs:224`). This run of
blind-scripted attempts — written with the source open — failed **four times**:

- Any mob/critter overlapping the facing tile makes the attack **silently do
  nothing** (`player_behavior.rs:831` skips item interaction if the target tile has
  entities). `before_hud_equip_fail.png`: armor held, open field, SPACE pressed —
  nothing happened, no message, no sound.
- Success gives no feedback either (no toast; the only trace is the armor meter
  appearing at the bottom of the screen — see J4).
- The same flow with hidden preconditions governs *everything*: tile items demand
  `Dig a hole first!` (`interact.rs:124`, `before_chest` ticker in run 3), furniture
  demands a passable tile. A kid — or a scripted adult — has no chance of deducing
  "put on a coat" = "attack the ground with the coat."

### 1.4 What the playtest already flagged (still true)

Empty inventory now says "Empty — gather something" (good, landed), craft menu dims
unaffordable recipes (landed), notification tiers landed. Remaining from
PLAYTEST.md: menus stack *over* the ticker bands (`before_hud_armor_worn`: GAVE
bands under the panel), and the held-item panel empty state is a bare `-`.

---

## 2. The proposed HUD

Mockups: **`mock_hud_calm.png`** (the frame most of play should look like) and
**`mock_hud_alert.png`** (worst-case: every system demanding attention at once —
the density test). Both are exact 288x192 compositions.

Principle (generalizing the temp-dot rule already in the codebase): **a meter that
needs nothing from you does not exist.** Corner-anchored, frameless, fixed slots.

```
+----------------------------------------------------+
| ambient ticker (unchanged)              (debug F3) |
|                                                    |
|                  [warning band, unchanged]         |
|                                                    |
| badges: temp-dot COLD / effect pips     ITEM NAME* |
| hearts      (hidden at 10/10)                 [n]* |
| stamina     (hidden at 10/10)               +----+ |
| food        (hidden at 10/10)               |icon| |
| water       (future slot, hidden at full)   +----+ |
|                                             ▀▀▀▀ dura
+----------------------------------------------------+
```

Element by element:

- **Vitals, bottom-left, fixed rows** at y = 158 (hearts), 166 (stamina),
  174 (food), 182 (water — reserved). Frameless: icons on a light `darken(90)`
  strip. Fixed slots preserve spatial memory — a row is always where it was, it is
  just *absent* while full. Appear rules: below max → visible; changed in the last
  ~90 ticks → visible; at/below 30% → visible + 1px white pulse underline
  (`mock_hud_alert` hearts). The bottom edge is the calm part of the frame — threats
  come from the world, center-up; DayZ/7DtD keep vitals low for the same reason.
- **Temperature** keeps its landed colors/pulse cadence (`renderer.rs:487`,
  `temperature.rs` bands) but moves off the border seam into the **badge slot**
  (y=146) above the vitals, and gains a one-word label at ±2 steps and beyond:
  `COLD` / `HOT` in the dot's color (`mock_hud_alert`). Comfort band: absent,
  exactly as today.
- **Effect pips** (potions) share the badge row as 8px icons right of the temp slot;
  details live on the SELF tab (§3.4). The `P` full-screen overlay retires.
- **Armor** stops being a fourth always-on meter: a small shield pip + hit count sits
  right of the hearts row *only while armor is worn*, and flashes when it absorbs a
  hit. Detail lives on WEAR.
- **Held item, bottom-right plate** (18x18 bordered slot at (266,170)): icon,
  count badge above (only for stackables/ammo — kills the permanent `X0`),
  2px durability bar underneath (green → amber at 50% → red at 20%; replaces the
  numeric `%` readout), and the item NAME as a right-aligned label above the plate
  for ~90 ticks after switching, then it fades (`mock_hud_alert`: THROWING KNIFE).
  Truncation stops being possible because the persistent state is icon+bar, not text.
  Empty hands: dim fist glyph instead of `-`.
- **Ammo/arrow counter**: contextual — attaches to the plate only when the held item
  consumes a counted resource (bow → arrows, knives → stack). No bow, no counter.
- **Thirst (probable follow-up system)**: row y=182, five droplets
  (`mock_hud_alert`), same hide-at-full rule; gentle by design — 5 units, halves
  later if needed. Until the stat ships, the row simply never renders; the slot is
  the design commitment. (Water bottles today restore stamina, `interact.rs:316` —
  they become the thirst refill when the stat lands.)
- **Unchanged**: ambient ticker top-left, centered warning band, save toast
  bottom-right (one interaction note: while the held-item name label is up, the
  toast lifts ~20px — both are transient, collision is rare).
- **Creative mode**: only the held plate renders (today it still draws frame chrome).

Why this reads better at 288x192: the calm frame (`mock_hud_calm`) gives back the
entire top edge to the world and shows exactly two things — one meter that wants
attention (8/10 food) and what's in your hand. The alert frame proves the full load
(5 meters + temp + warning + ammo + durability) fits in two corners and stays
legible.

---

## 3. The Survival Screen — one key, four tabs

Mockups: `mock_pack.png`, `mock_wear.png`, `mock_craft.png`, `mock_self.png`.
(Contents — recipe costs, flavor lines — are illustrative; layouts and coordinates
are the spec.)

One panel (8,8)-(280,184), smoked-glass 200, tab strip on top, key legend on the
bottom, two-column body (list x=12..146 | divider 148 | detail x=154..276).
**E opens it. Always.** Furniture context changes the *content*, never the key's
meaning (fixes J11). Z still jumps straight to the CRAFT tab; X and ESC close.

Key map (inside the screen — all existing actions, no new bindings):

| Key | Action |
|---|---|
| LEFT / RIGHT | switch tab (wraps; already how `display_tick_default` switches sub-menus) |
| UP / DOWN | move in the focused list (wrap at ends — fixes the no-wrap friction) |
| ENTER (or SPACE) | context action: hold / wear / craft / move stack |
| Q / SHIFT-Q | drop one / drop stack (container: move one / move stack) |
| E, X, ESC | close |

### 3.1 PACK (`mock_pack.png`)

- Items grouped under dim-yellow category headers (TOOLS / MATERIALS / FOOD / GEAR —
  derived from `ItemKind`, no data changes needed), **counts right-aligned in their
  own column** (fixes J7), 1px scrollbar on the divider.
- Right: detail card for the selection — 2x icon, name, durability bar + %, one or
  two flavor lines (optional `registry` string; card ships fine without), and the
  action legend (ENTER HOLD IT / Q DROP ONE).
- The selected-row echo (J6) is deleted, not fixed — the detail card is its
  replacement.

### 3.2 WEAR (`mock_wear.png`) — equipping becomes visible

- Left: **slot list** — HEAD, BODY, HELD, CHARM (reserved, ships disabled) — each a
  16px box with the worn item's icon, name, and stat line. Below: totals
  (ARMOR 30 HITS / WARMTH +0).
- Right: dark-tinted player portrait (render the real `MobSprite`, palette-correct)
  and the "FITS ON <slot>" list: every pack item wearable in the selected slot with
  its one-line effect (+2 WARMTH, 30 HITS, DYES SHIRT).
- **ENTER wears instantly. Q takes off.** No world interaction, no facing tile, no
  silent failure (fixes §1.3 outright). The legacy use-to-wear path keeps working
  for muscle memory but now emits feedback both ways ("WORN — LEATHER ARMOR" /
  "CAN'T — SOMETHING IS IN THE WAY").
- System changes this pane pulls in: split `cur_armor` into `worn_head` +
  `worn_body` (Straw Hat and Fur Coat currently *both* ride the single armor slot,
  so hat+coat can't stack — `temperature.rs` mitigation shifts already distinguish
  them). Clothing (shirt dye) stays instant-apply from the list. Save format gains
  the two slots, version-gated.

### 3.3 CRAFT (`mock_craft.png`)

- Left list split by affordability into **YOU CAN MAKE** / **MISSING PARTS**
  (extends the landed dimming; the sort already exists in `recipe_menu`).
- Right: product card — 2x icon, `NEEDS 1/1 STICK` lines (satisfied = white,
  short = red), `YOU HAVE n`, ENTER CRAFT. The two floating HAVE:/COST: boxes (J8)
  are deleted.
- Discovery hints, not wikis: grayed `THE BENCH` teaser in the personal list, and a
  dim "MORE RECIPES AT OVEN, FURNACE, AND THE BENCH" line on the card.
- Crafting keeps you on the pane and refreshes counts (already the behavior).

### 3.4 SELF (`mock_self.png`)

Day + time-of-day + biome/layer; numeric meters (including the ones the HUD is
currently hiding because they're full — the HUD hides, SELF always tells);
**WARMTH gauge**: the 7 temperature bands as cells with a marker and a plain-words
advice line ("WEAR A COAT OR FIND FIRE."); active effects with timers (absorbs the
`P` overlay and the old `InfoDisplay`).

### 3.5 Edge cases

- **Chests/containers** (`mock_chest.png`): the same shell, tab strip replaced by
  the two list titles (CHEST | PACK), equal-width fixed panes — no more
  pin-to-screen-edge dance (J10). LEFT/RIGHT switches side; ENTER moves stack,
  Q moves one (same semantics as today, container_display keeps its
  creative-duplication rule). Tabs are unavailable while a container is open —
  ESC returns to the world. Death chests and dungeon chests use the same variant.
- **Stations**: interacting with (or pressing E at) an oven/furnace/bench opens the
  survival screen on **CRAFT with the station's context** — station name as a
  sub-header (`mock_bench.png`), station recipe list. The other tabs stay live:
  you can flip to PACK to shuffle materials without stepping away (fixes J11's
  worst sting). ESC returns to the world, not to a nested menu.
- **Menus over notifications**: the survival screen suppresses the ticker lanes
  while open (warning band still punches through) — closes PLAYTEST bug #2's
  remaining half.
- **Creative**: unchanged initially (creative fills the PACK list; equip actions
  work the same). A searchable catalog is out of scope here.
- Title/pause/options/death/book/map screens: untouched by this program.

Implementation reality check (why this fits the codebase): `DisplayBase` already
holds multiple `Menu`s with LEFT/RIGHT cross-menu navigation and selectable flags;
the tab strip is a `Menu`-per-tab arrangement plus a header renderer. The take-out
display pattern, smoked-glass `render_frame`, `ItemEntry`/`RecipeEntry` all reuse.
The biggest new code is the detail-card renderer and the slot model in §3.2.

---

## 4. Crafting identity — the decision

### The pick: **THE BENCH** (the Prospector's Bench) — `mock_bench.png`

One buildable station. Crafted early and cheap (planks + cord — it slots into the
existing craft-chain tutorial right after the crude tools). It starts as a plain
woodworking bench, and you **outfit it with modules** — physical, holdable items
found while scavenging or crafted late:

| Module | Absorbs today's | How you get it |
|---|---|---|
| SAW (built-in) | Workbench list | comes with the bench |
| VICE | Anvil list (metal tools, armor) | ruins/mine chest loot, or craft (iron-heavy) |
| SPINDLE | Loom list (wool, clothes, bed) | village loot, or craft |
| ASSAY KIT | Enchanter list (potions, gold apple — reflavored as prospector's assaying) | dungeon/ruins loot, or craft (gold+gem) |

Fit a module once (ENTER with it held at the bench, or from the bench screen) and
its recipe family unlocks forever on that bench — the rack across the top of the
bench screen shows filled modules lit and empty sockets as dim `?` silhouettes with
a hint line ("SPINDLE FITS HERE"). Progression is *visible on the furniture*, not
in a menu: show, don't tell.

**Heat stays in the world.** The Oven and Furnace remain separate stations — the
candidate the PM floated, and the right call: fire is spatial and dangerous,
cooking/smelting at a flame is instantly legible to a child, and villages/houses
already generate ovens — merging heat into the bench would turn it into a god-box
and strip world structures of their remaining function. The bench absorbs the
*bench-shaped* stations only (workbench/anvil/loom/enchanter — the four menus on
legs from J9).

Legacy migration with flavor: old saves keep their placed anvils/looms/enchanters
working as-is (old-save tolerance rule), and each can be **broken down into its
module** — your grandfathered anvil literally becomes the VICE you bolt onto the
new bench. New worlds stop generating the standalone stations in loot recipes and
seed module items into ruins/village/dungeon chest tables instead.

Why it wins:

1. **It answers the actual complaint.** Four near-identical station menus become
   one screen with one growing recipe list — menu proliferation was the disease,
   stations-as-menus was the cause.
2. **It's ours.** "Outfit your camp bench like a prospector's kit" is fossicking
   identity; nobody ships vice/spindle/assay modules. A Minecraft crafting table is
   a 3x3 grid; this is a workbench that visibly accretes tools. Inspired-by, not
   copied.
3. **Exploration → capability, with zero RNG lock.** Every module also has a
   (deliberately expensive) recipe displayed grayed-out at the bench itself, so a
   kid who never finds the loot still sees the path and can grind it. Finding the
   module in a ruin is the shortcut, not the gate.
4. **It fits the machine.** `CrafterType` + per-station recipe lists already exist;
   modules are a `Vec<Module>` on the bench furniture (saved with furniture data),
   and the recipe list is a concat of unlocked families. No knowledge-base, no
   per-player unlock bookkeeping.

### The runner-up: field-notes learning (journals)

Scavenged prospectors' journals teaching recipes lose on the PM's own risk:
knowledge-gating is invisible until you have it, RNG-gating a kid's progression
violates the approachability rule, and mitigating it ("core chain always known,
notes only add") shrinks it into... exactly the variants layer. So that's what it
becomes: **a later, additive content wave** — journals found in ruins grant
*variant* recipes (a cheaper Fur Coat stitch, a longer-burning torch, cosmetic
dye colors), never family unlocks. It keeps the exploration→knowledge romance,
gates nothing, and needs no UI beyond a "NEW VARIANT LEARNED" toast and an
annotation on the recipe card. Not in this program's lanes; queued behind them.

---

## 5. Implementation plan — lanes

Order: L1 ∥ L2 → L3 → L4 → L5. L6 whenever the system is wanted. One lane = one
conventional commit with the explicit file list; `just check` green before each;
every lane ships before/after screenshots into `target/verify/` (rule 7).
Never two agents in `item/registry.rs` at once (L3/L5 both touch it — serialize).

### L1 — `feat(hud): frameless corner HUD` (independent)
Files: `src/core/renderer.rs` (rewrite `render_gui`, relocate temp-dot draw,
delete frame boxes, held plate + badges), `tests/headless_render.rs` additions.
Acceptance: no `render_frame` calls remain in the HUD path; calm frame shows only
non-full meters + plate (screenshot match vs `mock_hud_calm` layout); alert-state
staging (TestWorld: hurt+cold+low-dura) matches `mock_hud_alert` slots; long item
names never draw outside the plate label band; `X0` impossible without a bow;
creative shows plate only; `just check` green.

### L2 — `feat(screen): survival screen shell + PACK/SELF` (independent of L1)
Files: `src/screen/survival_display.rs` (new), `src/screen/mod.rs`,
`src/entity/mob/player_behavior.rs` (E/Z/P routing),
`src/screen/inventory_menu.rs` (delete `render_selected_info` echo; counts column),
`src/screen/info_display.rs` (retire into SELF), `tests/display_flow.rs`.
Acceptance: E opens tabbed screen from open world; LEFT/RIGHT cycles 4 tabs with
wrap; PACK categories + right detail card render; SELF shows day/meters/warmth
band/effects; Z lands on CRAFT tab (even before L4 fills it, it hosts the existing
personal recipe menu); list UP/DOWN wraps; drops (Q/SHIFT-Q) work from PACK;
ticker suppressed while open; screenshots of all four tabs eyeballed.

### L3 — `feat(player): wear slots + instant equip` (after L2)
Files: `src/entity/mob/player.rs` (`worn_head`/`worn_body` replacing lone
`cur_armor` semantics), `src/item/interact.rs` (armor/clothing feedback + slot
routing), `src/core/temperature.rs` (read both slots), `src/screen/survival_display.rs`
(WEAR pane), `src/saveload/save.rs` + `load.rs` (version-gated slots),
`src/item/registry.rs` (hat items tagged HEAD), `tests/` (equip roundtrip,
old-save load keeps armor).
Acceptance: ENTER on WEAR equips with zero world interaction; hat + coat stack
warmth per `temperature.rs` shifts; legacy use-to-wear emits success/blocked
notifications; old saves load worn armor into BODY; save/load roundtrip test green.

### L4 — `feat(screen): craft pane, station context, container restyle` (after L2)
Files: `src/screen/crafting_display.rs` (fold into CRAFT pane; delete HAVE/COST
satellite boxes), `src/screen/recipe_menu.rs` (CAN-MAKE/MISSING split),
`src/screen/container_display.rs` (fixed equal panes inside the shell),
`src/entity/furniture/crafter_behavior.rs` (stations open survival screen CRAFT
context), `tests/display_flow.rs`.
Acceptance: personal + all six station lists render in the pane with cost card;
crafting stays open and refreshes; PACK reachable while at a station; container
matches `mock_chest` layout with today's move semantics; no floating sub-boxes
anywhere; screenshots.

### L5 — `feat(craft)!: THE BENCH — modular station identity` (after L4)
Files: `src/entity/furniture/crafter.rs` + `crafter_behavior.rs` (Bench type,
module state, breakdown of legacy stations), `src/item/registry.rs` (bench +
4 module items + icons under `assets/sprites/items/`), `src/item/recipe.rs`
(family regrouping; bench recipe into the personal chain; module recipes),
`src/level/structures_gen.rs` (module loot seeding; stop generating standalone
stations' craft recipes for new worlds), `src/saveload/*` (bench module data,
version-gated), `src/screen/survival_display.rs` (module rack UI),
`docs/ITEMS_AND_CRAFTING.md`, tests (module fit persists across save/load; legacy
anvil still opens its list and breaks down into VICE; a new world can reach every
recipe family with zero loot finds).
Acceptance: bench screen matches `mock_bench` (rack, hint line, grouped families);
all four absorb-targets reachable via modules; oven/furnace untouched; old-save
world with placed stations fully playable; `just check` green.

### L6 — `feat(survival): thirst` (deferred — design slot reserved)
`player.thirst` (5 units), water bottle/standing-water drink wiring
(`interact.rs:316` reroute), HUD row y=182, SELF row, tuning doc. Ships only when
the survival pacing wants it; the HUD and SELF layouts above already hold its seat.

Mockup caveat for implementers: recipe costs, flavor copy, and module names in the
mocks are illustrative; layout geometry, colors (game palette via `color::get4`
words), and interaction rules are the spec.

---

## 6. What explicitly does NOT change

- **The tiered notification system** (ambient ticker top-left, centered warning
  band, save toast bottom-right) — just landed (playtest #2), works, stays. The
  survival screen only adds "suppress ticker while open."
- **The first-day cue thread and the craft-chain tutorial content** — the personal
  recipe list *is* the tutorial; CRAFT pane re-dresses it without touching a recipe.
- **Recipe contents and balance** — every list keeps its items; L5 regroups
  ownership, not costs.
- **Container semantics** — ENTER moves stack / Q moves one, creative keeps copies.
- **The smoked-glass panel language, caps-only 8px font, and packed-palette
  rendering** — the redesign is built *from* these primitives (the mockups use the
  exact `darken(185/200)` math and glyphs).
- **Map (M), pause, options, title, death, book screens, dev console** — the map
  was rebuilt in its own lane; the rest are out of scope.
- **Key bindings** — E/Z/X/Q/ESC keep their meanings; no new bindings are
  introduced; `P` is freed (SELF absorbs it), kept as an alias that opens SELF.
- **The temperature model** — bands, mitigation shifts, dot colors and pulse
  cadence are untouched; the dot only moves house and gains a word.

---

## Appendix — screenshot index (`target/verify/ui_mock/`)

Before (current build): `before_hud_fresh` (dawn spawn murk), `before_hud_noon`
(three frame boxes + temp dot on the seam), `before_hud_held` / `before_hud_held_armor`
(`1 LEATHER..` truncation), `before_hud_equip_fail` / `before_hud_armor_worn`
(silent armor failure, ticker stack), `before_hud_durability` (menu + echo row), `before_inv_empty` /
`before_inv_full` / `before_inv_echo` (echo overlap, baked-in counts),
`before_inv_after_select`, `before_craft` (odd frame + floating HAVE/COST),
`before_furnace` (station-held state; station menus share `before_craft`'s screen),
`before_chest` (edge-pinned container), `before_menu_x`.

Proposed: `mock_hud_calm` ★, `mock_hud_alert` ★, `mock_pack`, `mock_wear`,
`mock_craft`, `mock_self`, `mock_bench` ★, `mock_chest`. (★ = the frames that sell
the redesign.) All also present as `*_big.png` 4x.
