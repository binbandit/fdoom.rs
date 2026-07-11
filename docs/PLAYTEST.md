# PLAYTEST — Creative Director's Structured Pass

Date: 2026-07-11. Build: `main` @ 802a352, debug binary.
Method: scripted FDOOM_DEMO runs (blind key-driving + frame dumps), seed **9**,
world `PT9`, throwaway savedir. Rain and the Hollow Night were *scheduled by
arithmetic* — day 1 rains slices 2-4, day 2 is a Hollow Night for seed 9, computed
from the pure `hash(seed, salt, day, slice)` schedule. That this is possible at all
is a compliment to the systems design.

Screenshots: `target/verify/playtest/` (raw + `*_big.png` 6x upscales, all actually
read during the pass). Honesty notes at the bottom — a few scenario beats were not
reached and are flagged as such, not silently skipped.

---

## Per-scenario observations

### 1. Fresh spawn, first day (`s1_*.png`)
- Spawn lands on grass near origin (tile 64,64), at dawn — which is **nearly as dark
  as night** (`s1_spawn_asis`). A new player's first 4 real-time minutes are murk.
- Zero onboarding surface: no cue text, the held-item box is blank with no label,
  and the **empty inventory is a featureless black panel** (`s1_inventory`) — not
  even an "empty" line.
- The crafting menu is the real tutorial and it's *good* (`s1_craft`): Cord →
  Sharp Stone → crude tools reads as a chain, and `COST 3/0` on Cord points at
  fibers. But nothing points the player at **Z**, and affordable vs unaffordable
  recipes render identically.
- Punch feedback: swings have no readable arc, whiffs are silent, and eight air
  punches quietly drained 8 of 10 stamina bolts (`s2_treedrop` HUD). A drop is a
  static 6px gray dot with no pickup confirmation (`s1_afterpunch2`, `s1_inv2`).

### 2. Night one, no shelter (`s2_night*.png`, `s2_hurt`)
- The night grade itself is lovely — deep blue-teal, grass speckle preserved.
- **Threat legibility is the problem**: every mob in frame is a near-black blob on
  near-black ground (`s2_night3`). The tall-grass eye-glint (behavior.rs:941) never
  fires in the open, which is where the player actually meets night one.
- Player damage: one heart dims (`s2_hurt`), no flash/shake/direction. Damage
  numbers *do* exist (seen later, `s7_flats_noon2`) but are small and low-contrast.

### 3. Biome tour (`s3_*.png`)
- **Forest** (`s3_forest`): best-in-class. Dense canopies, mushroom mobs, berry
  bushes — alive and distinct.
- **Desert** (`s3_savanna` — coords landed in true desert): rippled dunes, flowering
  saguaros, sparse debris. Distinct. One dead tree sits on a square green grass
  patch — see seams below.
- **Tundra** (`s3_snow`): snow-capped pines, calm speckle base — charming, but
  several trees sit on **square green grass tiles** punched into the snowfield.
- **Mountains** (`s3_mountain`): thinnest identity — green grass + gray boulder
  blobs with pale outlines that read pasted-on. "Highland" needs its own ground.
- **Marsh**: not conclusively found. The biome-map's marsh region at (-280, 40)
  generated plain grass+trees (`s3_marsh*`; F3 shows TILE GRASS), no pools, no
  fireflies at night. Either marsh is patchy at region edges or it reads like
  plains — both are a distinctiveness problem for the tour promise.

### 4. Mining trip (`s4_*.png`)
- Dig-descent staging is legible: pit darkens per shovel stage (`s4_pit3`), the
  "hits solid rock" gate, chasm, guaranteed ladder — good design. Caveat: with no
  facing/target-tile indicator, aiming the dig is guesswork, and the notification
  band sat exactly on top of my pit through the whole sequence.
- Underground looks great (`s4_lantern`, `s4_m2`): warm lantern pool, beveled rock
  faces, hard black occlusion. Red ore-vein pips on mined faces are readable and
  enticing (`s4_m3`).
- **Stamina is a hard brake**: ~11 iron-pick swings drain the full bar; at zero,
  swings silently do nothing. Mining rhythm becomes "swing-swing-stand around".
- **No cave-in in four underground sessions.** By the constants
  (`COLLAPSE_OPEN_MIN=13` of 25, no prop within 3, then 1-in-4), organic corridor
  mining can *never* arm one — the marquee fossicking hazard, and the reason timber
  props exist, is effectively invisible in normal play.
- Timber prop sprite reads as a masonry crate, not a roof support (`s4_prop2`).

### 5. Rainy night at a campfire (`s5_*.png`)
- **The money shot of the playtest** (`s5_night_fire_rain2`, `s5_cozy2`): warm halo,
  drifting smoke puffs, rain streaks inside the light, player silhouette at the
  fire. The promised coziness is real; ship a screenshot of this.
- Nits: the halo is a clean radial gradient (reads slightly "flashlight"), and its
  edge grading goes yellow-green on grass rather than warm amber.

### 6. Cemetery + Hollow Night (`s6_*.png`)
- Cemetery set dressing by day is excellent: varied gravestone sprites, dirt plots,
  a grave-digger's tools prop, a torch (`s6_cem_day`).
- Dusk cue **"The evening is unnaturally still..."** is perfect tone (`s6_dusk_cue`).
- Night staging sings: purple-blue grade, gravestone silhouettes, and the glowing
  grave offerings acting as breadcrumbs through the dark (`s6_hollow4`).
- The restless dead then swarmed me 8 → 3 hearts while being **almost invisible**
  (dark green-black silhouettes on dark ground, no player hit feedback). The
  event's stakes are illegible exactly when they land.

### 7. Ocean, tides, fishing (`s7_*.png`)
- **Tides are quietly spectacular**: identical coordinates are walkable tidal flats
  with tide-pool rocks at noon low tide (`s7_shore`, `s7_pan1`) and swimming water
  at evening high tide (`s7_flood`). Wordless worldbuilding, best system surprise
  of the session.
- Swimming reads well (head-only sprite + ring), drowning at 0 stamina produces
  visible damage "1" popups.
- **Panning gave no visible result** in four uses on exposed TIDAL FLAT (pan
  equipped, `s7_pan1..3`): no notification, no sparkle, nothing. Whether the rolls
  missed or the spot wasn't pannable, the player can't tell — the signature
  mechanic has a silent failure mode.
- Fishing: never reached a catch in scripted runs; casts show no bobber/line in
  stills, and no visible bubble cluster at a computed `fish_presence≈0.87` hotspot.
  Flagged as "couldn't verify", but the *absence of cast feedback* is itself a
  finding.
- Raft-on-deep-water was not demonstrated (kept landing in shallow/tidal water).

### 8. Village / trail / ruins loot run (`s8_*.png`, `s3_ruins`)
- Ruins are great set-pieces: cracked floors, wall stubs, arch remnant, chest
  (`s3_ruins`). Village has good bones — well, radiating paths, farm plots, berry
  bushes, a full house with plank floor, spinning wheel, and loot chest
  (`s8_village_day3`).
- But it reads **abandoned**: no inhabitants, no animals, no light. "Village" and
  "ruins" are currently the same mood at different densities.

### 9. Window-lit house at night (`s9_*.png`)
- Village houses have real glass windows — and **no interior light source ever**.
  At night the windows are dead blue grids and whole buildings vanish
  (`s9_house_night1..3`). The scenario's promise (warm glow through glass, light
  occlusion showcase) cannot currently happen without the player building it.

### Extra: map screen (`smap.png`)
- Near-worthless: black void, a ~3-pixel reveal at the player, unreadable palette,
  no legend or coordinates. For an infinite seed-described world, this is the
  weakest tool in the kit.

---

## Bugs found while playing

1. **Held item lost on every save/load.** `write_inventory` (src/saveload/save.rs:349)
   writes the active item as the first Inventory line; `load_inventory`
   (src/saveload/load.rs:740) loads every line into the bag and never re-equips.
   Repro: equip anything, `R`, quit, reload → hands empty, item at inventory top.
   Observed all session; code-confirmed.
2. **Notifications render over open menus and collide with panel text**
   (`s5_invsel2`: two stale "GAVE..." bands bleeding through the inventory panel).
   They also pile up 3-deep mid-screen and sit exactly where the action is
   (renderer.rs:280 draws at 2/5 screen height, 120 ticks each, queue of 3).
3. **HUD held-item overflow**: long names spill into the arrow-counter box —
   "2 TIMBER PROP" (`s4_prop2`), "1 PROSPECTOR'S PAN" renders as garbage overlap
   (`s7_pan2`).
4. **Suspected: tool durability charged on null uses.** Fishing rod went 100→81%
   over four casts aimed at cave floor/rock (no water anywhere). Needs verify.
5. **Suspected: `/tp` no-op while swimming.** Three consecutive tp's while in water
   produced pixel-identical scenes (same boulder at same screen offset:
   `s7_raft2`, `s7_flats_noon2`, `s7_beach2`) while the notification claimed
   movement. Needs verify (dev-console only, but tp is also the test harness path).
6. **Art seam: flora on wrong ground.** Pines in tundra and dead trees in desert
   draw a square green grass base under themselves, punching holes in the snow/sand
   field (`s3_snow`, `s3_savanna`). Violates the calm-base texture rule.
7. Cosmetic: F3 overlay text and the notification band overlap each other
   (`s7_shore`); debug-only.

---

## TOP-10 ranked improvements

1. **Attack & interaction juice** — the single biggest gap; every minute of play
   touches it.
   *Problem:* no readable swing, silent whiffs that drain stamina, no impact
   particles, no player hurt flash/kick; at 0 stamina inputs die silently.
   *Fix:* short swing-arc sprite in the facing tile; dust/leaf/stone chip puff on
   tile hit (smoke-puff plumbing already exists — see campfire); 1-frame white
   flash + 2px kickback on player hurt (mobs already flash white, mirror it); a
   "too winded" cue (gray bolt shake + soft sound) when attacking at 0 stamina.
   *Files:* `src/entity/mob/player_behavior.rs` (attack path),
   `src/entity/behavior.rs`, `src/core/renderer.rs`. **Effort M. Impact: highest.**

2. **Notification system overhaul** — currently the loudest *and* least useful
   voice in the game.
   *Problem:* ALL-CAPS bands parked mid-screen at 2/5 height for 2s each, 3-queue
   backlog, covering the exact tile you're working (hid my dug pit, the well, the
   cemetery), rendering over open menus (bug #2).
   *Fix:* dock ambient/inventory messages top-left under the HUD as a small ticker
   (sentence case, ~90 ticks); reserve the centered band for warnings/event cues
   only ("The ceiling groans...", dusk cues); suppress while a menu is open.
   *Files:* `src/core/renderer.rs:280-310`, callers of `notify_all` for tiering.
   **Effort S-M. Impact: high — also fixes bug #2.**

3. **Night threat legibility** — night one and Hollow Night are the game's fear
   beats, and their monsters are invisible.
   *Problem:* hostiles are black blobs on black ground; eye-glint only exists
   inside tall grass.
   *Fix:* render the existing 2px warm eye-glint (behavior.rs EYES_POS) on **all**
   hostile mobs at night, always-on-top of the night grade (it's already a
   true-color cell that survives grading). Additive only — no anatomy changes.
   *Files:* `src/entity/behavior.rs::mobai_render` (~10 lines). **Effort S.
   Impact: high; the cheapest big win in this list.**

4. **First-day thread (onboarding via the game's own voice, not a tutorial).**
   *Problem:* nothing points at Z/craft, empty inventory is a black void, empty
   hands are unlabeled; the excellent craft-chain tutorial is undiscoverable.
   *Fix:* 3 one-time ambient cues (first minute: "The tall grass holds fibers.";
   first fiber: "Enough fibers could twist into cord [Z]."; first cord: "A sharp
   stone and a stick would make a tool."); "Empty — gather something" line in the
   empty inventory panel; "bare hands" label in the empty held-item box; dim
   unaffordable recipes in the craft menu.
   *Files:* `src/entity/mob/player_behavior.rs`, `src/screen/player_inv_display.rs`,
   `src/screen/inventory_menu.rs`, `src/screen/recipe_menu.rs`, a few one-shot
   flags. **Effort M. Impact: high for anyone's first 20 minutes.**

5. **Fix: held item survives save/load** (bug #1).
   *Fix:* tag the active-item line on save (e.g. leading marker, version-gated) or
   re-equip the first loaded item when the save wrote one; add a round-trip test.
   *Files:* `src/saveload/save.rs:345-357`, `src/saveload/load.rs:725-775`,
   `tests/save_load_roundtrip.rs`. **Effort S. Impact: correctness + daily feel.**

6. **Make cave-ins actually happen** — the identity hazard is unreachable.
   *Problem:* `COLLAPSE_OPEN_MIN=13` of 25 can't be met by corridor mining; I never
   armed one in four sessions of trying.
   *Fix:* add a corridor trigger: every rock broken >6 tiles from the nearest prop
   *or* wall-adjacent opening rolls a small arm chance (keep the groan → fuse →
   rubble telegraph, it's already well-designed); or drop OPEN_MIN to ~9 and
   retune odds. Keep props as the counter — now they matter.
   *Files:* `src/level/tile/fossick.rs` (collapse_check + constants), tests.
   **Effort S (tune) / M (heuristic). Impact: identity — pans, veins, and props
   only cohere if the mine bites back.**

7. **Map screen rebuild.**
   *Problem:* black void with a 3px reveal; no palette logic, legend, or coords.
   *Fix:* reveal by visited chunks with a real radius, color by biome (reuse
   worldview's `biome_color` — move it into `gfx`), player arrow + coordinates
   line, pips for visited structures.
   *Files:* `src/screen/map_menu.rs`, `src/bin/worldview.rs` (extract palette).
   **Effort M. Impact: exploration loop, seed-sharing culture.**

8. **Lit interiors + faint life for villages** (also delivers scenario 9).
   *Problem:* houses are pitch dark, windows never glow, villages read abandoned.
   *Fix:* stamp one lit lantern/hearth per generated house interior; light through
   the existing window tiles instantly showcases the occlusion system at night.
   Optional: 1-2 ambient critters per village.
   *Files:* `src/level/structures_gen.rs` (furniture stamp), no new art needed.
   **Effort S-M. Impact: world warmth; turns night houses into destinations.**

9. **HUD held-item panel: clip + empty state** (bug #3).
   *Fix:* truncate/ellipsize name to panel width (or 2px marquee), never paint
   outside the panel; show a small fist icon or "—" when empty.
   *Files:* `src/core/renderer.rs` (item bar block). **Effort S. Impact: polish on
   the most-looked-at UI element.**

10. **Ground-truth flora bases per biome** (bug #6).
    *Fix:* tree/dead-tree/cactus tiles render their base from the underlying biome
    ground (or the connector-sprite path) instead of hardcoded grass, so pines sit
    in snow and dead trees in sand.
    *Files:* `src/level/tile/tree.rs` (+ siblings), `dispatch.rs` csprite path.
    **Effort S-M. Impact: biome believability; enforces the house art rule.**

Near-misses (worth tickets, not top-10): silent panning results on flats (verify
`pan_outcome` on tidal flats + always emit a result line — "nothing but gray sand"
is fine and on-brand); fishing cast/bobber feedback; dawn-darkness at morning tick
0 (consider starting new worlds at mid-morning); mountains biome ground identity;
marsh distinctiveness/patchiness at region scale; craft-menu affordability dimming
(folded into #4).

---

## What already sings

- **The rainy-night campfire** — smoke, halo, rain inside the light. The cozy
  fantasy this game promises, delivered in one frame.
- **Tides.** Walkable flats at noon, open water at dusk, tide pools, a dry ridge
  holding out — landscape that breathes on the day clock, no text needed.
- **Hollow Night staging**: the "unnaturally still" dusk cue, glowing grave
  offerings as breadcrumbs, the purple grade. Event *writing* tone is exactly
  right throughout.
- **Lighting**: warm lantern/campfire/torch pools against hard cave blackness;
  the mine's beveled rock faces read almost dimensional.
- **Dig-descent** as a state machine (pit visibly deepens, rock gate, guaranteed
  ladder home) and the red vein pips that make you want one more rock.
- **The craft menu as tech-tree tutorial** — the crude-tool chain teaches the
  early game better than any popup could (it just needs a pointer to it, see #4).
- **Cemetery and ruins set dressing** — varied stones, grave tools, arch stubs,
  chests. Structures feel authored, not stamped.
- **Forest + the mushroom folk.** Original mob roster charm, on full display.
- **Determinism as a feature**: I scheduled rain and a Hollow Night with a
  20-line Python port of the hash. Speedrunners and seed-sharers will love this
  game's spine.

---

*Method limits, for honesty: all play was blind-scripted (no live window), so a
few beats weren't reached — no fishing catch, no armed cave-in, no confirmed raft
ride on deep water, and true marsh was never located. None of these were skipped
silently; each carries a finding above. Combat feel was probed via self-damage,
swarms, and whiffs rather than a controlled duel.*
