# ODDITIES — Visual-Coherence Audit (2026-07-12)

Method: staged seam matrix (every ground-family pair, both axes + corners),
natural biome-border sweep (17 pairs x seeds 9 and 42), structure visits, big
uniform fields, night/lighting/weather/tide scenes. 122 screenshots under
`target/verify/oddities/` (`<name>.png` = 1x, `<name>_big.png` = 6x); every shot
was read at 6x, and every finding below was verified by eye + traced to a
suspected code site (read-only). Staged scenes: `TestWorld` seed 9, painted
around tile (64,64); natural scenes list their seed + tile coords (repro:
`tp x y` in the dev console, or `TestWorld::teleport`).

Prior art in `docs/PLAYTEST.md` is not re-reported (flora square grass bases in
snow/desert, pasted-on boulders-with-pale-outline, marsh identity, dark
windows, dawn darkness, notification issues). In-flight fixes excluded:
snow-beside-sand, square dug holes, giant mushrooms.

Concurrency note: `structures_gen.rs` gained Hamlet + TownAge decay
(Overgrown/Weathered/Settled) while this audit ran; the village shots predate
it. Village decay observations are framed as dressing notes, not bugs.

---

## Ranked list

Status: O1-O3 FIXED in be7b198 (screen-blend glow, real ground under rock,
edge-scaled carry + tint clamp). O4/O5/O13/O14/O15 in flight (water-family lane).

Breaks-immersion:
1.  O1  [FIXED be7b198] Emitter light pools split in half at ground seams
2.  O2  [FIXED be7b198] Mountain-border rock/heath renders as smudges, flat square backings, translucent cliffs
3.  O3  [FIXED be7b198] Biome tint bleaches/recolors ground so far it flips terrain identity
4.  O4  Swimming ring is an opaque black rectangle on tidal flats / deep water
5.  O5  Ponds are hard rectangles with a warm mud rim on every ground (incl. snow)

Noticeable:
6.  O6  Cemetery gravestones + overgrowth stamp grass-green squares on the dirt plot
7.  O7  Flora bases ignore the ground they stand on at border bands (pine snow-squares on grass; cacti in grass)
8.  O8  Item drops on water sit flat with black shadows; shadow is a black sprite-copy
9.  O9  Precipitation identity is player-global: rain falls on snowfields across borders
10. O10 Waterlines render as periodic sparse dots, and differ by axis (brown bank south, dots east/west)
11. O11 mud|water boundary has zero shore treatment
12. O12 Heath is excluded from all blending — hard zipper edges vs every ground
13. O13 Tidal flat: hard material cliff vs dry sand + bare 90-degree staircase vs open water
14. O14 Ocean-to-deep boundary is a tile-quantized darkening staircase
15. O15 Fireflies don't glow — dim gray rings, land-only
16. O16 Mountains rock interior is a wall-to-wall quilted grid
17. O17 Desert ripples align into unbroken ruled lines
18. O18 Day water palette reads as night sky
19. O19 Boulders bisected by structure footprints (half-boulders in walls/floors)

Nitpick:
20. O20 Wood Planks floor reads as wicker grid
21. O21 Ruins: worn-dirt patches fringe onto adjacent pavement tiles
22. O22 mud|sand seam renders as a loud dark sawtooth
23. O23 Dry bush on grass reads as a glowing neon-yellow ball
24. O24 Snowfield streaks form a uniform diagonal lattice
25. O25 Halo stipple rings read mechanical on bright grounds
26. O26 Beach sand is indistinguishable from desert sand
27. O27 Village decay dressing reads as bugs (dead-end paths, clean orphan paving)

Counts: 5 breaks-immersion, 14 noticeable, 8 nitpick.

---

## Breaks-immersion

### O1. Emitter light pools split in half at ground seams
- Shots: `light_night_grass_sand_seam.png`, `light_night_snow_water_shore.png`
- Repro: seed 9, staged grass|sand seam at tile (64,64), campfire straddling
  the seam, night.
- Wrong: the campfire/torch glow is bright warm orange on the sand side and
  near-dark green on the grass side, cut by a razor line exactly on the tile
  seam — the halo reads as a half-moon. Same at any shore (peach pool on snow,
  dim purple haze on water). Any emitter near any ground seam breaks.
- Suspected site: `src/gfx/lighting.rs` — final pixel = `grade(pixel) *
  max(ambient, light)` (`build_luts` ~:803, `compose` ~:1142); light multiplies
  tile albedo, so dark grounds can never brighten.
- Fix in one line: add a small additive (screen-blend) warm term inside emitter
  radius so glow survives albedo changes across seams.

### O2. Mountain-border rock/heath renders as smudges, flat backings, translucent cliffs
- Shots: `nb9_mountains_plains.png` (seed 9, tile (49,72)),
  `nb9_mountains_desert.png` ((37,-48)), `nb42_mountains_*` , `st9_Camp.png`
  ((-209,-275), the "smoky blotch"), `st42_StandingStones.png`
- Wrong (three stacked mechanisms, verified):
  (a) rock tiles fill their base with a *flat* approximation color of the
  dominant neighbor — a flat square against textured sand/grass reads as a
  backing board behind every boulder/crag, with brown L-shaped hooks at convex
  corners; (b) heath's olive-gray clod texture under the desert-side biome tint
  turns sand-yellow, so heath tiles read as translucent smoke stains — isolated
  ones in open sand have no visible core at all; (c) neither rock nor heath
  blends (Other family), so all edges are checker-dither zippers. Border
  screens read wholesale broken.
- Confirmed sites: `src/level/tile/rock.rs:55-95` (flat `bg` = 550/141/hex
  fills), `src/level/tile/heath.rs` render + `src/gfx/lighting.rs:577`
  ground-blend tint, `tile_ground()` :536 (no Heath/Rock family).
- Fix in one line: give rock a textured base sampled from the neighbor tile's
  actual art (not a flat shade), and exempt heath/rock from the neighbor-biome
  tint (or clamp it — see O3).

### O3. Biome tint bleaches/recolors ground far enough to flip terrain identity
- Shots: `nb42_forest_tundra.png` (seed 42, tile (487,336)) — washed
  near-white rectangles over grass/sand; `nb9_savanna_desert.png` ((359,-168))
  — camouflage patchwork with isolated glowing bright-yellow squares;
  `pair_grass_sand_q.png` — same grass tile dark-saturated in one quadrant,
  pale mint in another, in a single frame.
- Wrong: the bilinear biome-factor multiply is strong enough that (a) sand and
  grass near tundra bleach to near-white with visible per-tile grid seams,
  (b) grass near desert reads as sand (walkable-terrain misread), (c) single
  tiles in blend bands become "glow squares" brighter than both neighbors.
- Confirmed site: `src/gfx/lighting.rs:577` `ground_blend_pass`
  (`SAND_F=[271,256,230]`, `SNOW_F=[256,264,281]`, bilinear corner-averaged
  `biome_factor`).
- Fix in one line: clamp the per-channel factor swing (or shrink blend radius)
  so no tile's hue crosses into a different ground family's band.

### O4. Swimming ring is an opaque black rectangle on tidal flats / deep water
- Shot: `tide_evening_shore.png` (seed 9, shore at tile (-8,36), evening high
  tide)
- Wrong: the swimming player sits inside a solid black rounded box with hard
  edges on open water.
- Confirmed site: `src/entity/mob/player_behavior.rs:1245` — `liquid_color = 0`
  (black) unless the standing tile id is exactly `"water"`/`"lava"`; submerged
  Tidal Flat and Deep Water fall through.
- Fix in one line: match water-family TileKind (Water | DeepWater | submerged
  Tidal) instead of the "water" tile id.

### O5. Ponds are hard rectangles with a warm mud rim on every ground
- Shots: `watershape_grass.png`, `watershape_snow.png`, `marsh_day.png`
  (staged at (64,64); natural pool in marsh interior seed 42 (112,128))
- Wrong: every pond edge is a 90-degree tile-grid rectangle traced by a thick
  warm-brown bank rim — on snow, a warm mud border around an indigo pond reads
  plainly wrong; diagonal-touching ponds stay two sealed boxes; a 1-tile pond
  is a brown ring with a blue dot. (Adjacent to the in-flight square-hole fix,
  but the ground-blind rim color and missing corner-cut connector cells are
  their own problems.)
- Suspected site: water connector side/corner cell art + `csprite_render`
  (`src/level/tile/dispatch.rs:546`); rim color is not ground-aware.
- Fix in one line: tint the bank rim from the neighboring ground family (as
  `rock.rs` samples neighbors) and add diagonal corner-cut cells.

---

## Noticeable

### O6. Cemetery gravestones + overgrowth stamp grass-green squares on the dirt plot
- Shots: `st9_Cemetery.png` (seed 9, tile (183,153)), `st42_Cemetery.png`
  ((89,158)) — every gravestone/cross AND the overgrown tall-grass tufts sit on
  bright grass-green squares punched into the brown plot.
- Suspected site: `grave_stone.rs` renders a grass base under the stone;
  `tall_grass.rs` base tile is hardcoded `"grass"` (`make_tall_grass_tile(_,
  "grass", _)`).
- Fix in one line: render prop/overgrowth bases from the actual stamped ground
  (same fix family as PLAYTEST #10, extended to props).

### O7. Flora bases ignore the stamped ground at border bands
- Shots: `nb9_plains_tundra.png` (seed 9 (363,768)): pines on the *grass* side
  stamp square SNOW patches (inverse of the known bug — pine base is hardcoded
  snow); `nb42_plains_desert.png` (seed 42 (809,800)): cacti standing in the
  grass-side blend band.
- Suspected sites: `src/level/tile/tree_species.rs` render (base ground chosen
  by species, not by terrain), flora placement biome-gating in
  `infinite_gen` (cactus placed where biome=Desert even when the local tile
  interleave landed grass).
- Fix in one line: species trees render base from the underlying generated
  ground; flora placement checks the actual tile, not just the biome.

### O8. Item drops on water sit flat with black shadows; shadow is a black sprite-copy
- Shots: `drops_grounds.png` (staged quadrants at (64,64)),
  `tide_noon_shore.png` + `wx_rain_shore.png` (natural: plant items floating
  on flat/water with black twins)
- Wrong: drops on water render exactly as on land — no float/sink/ripple, hard
  black shadow painted on the water; and the shadow is a full black copy of the
  item sprite offset down, so on light grounds (snow, tidal flat) it reads as a
  second dead object.
- Confirmed site: `src/entity/item_entity_behavior.rs:53` `render` — no
  ground-type conditional; shadow = sprite re-render in black.
- Fix in one line: on water-family tiles clip/sink the item + swim ripple, no
  shadow; elsewhere dither/alpha the shadow.

### O9. Precipitation identity is player-global
- Shot: `wx_rain_plains_tundra.png` (seed 9, border at (363,768), day-1 rain
  slice)
- Wrong: standing on the plains side, blue rain streaks fall on the snowfield
  across the border; stepping across flips the whole sky to snow at once.
- Confirmed site: `src/core/weather.rs:159` `precip_at_clock` (Rain/Snow chosen
  at the player) + `src/gfx/lighting.rs:300` (one pass over the full frame).
- Fix in one line: choose streaks-vs-flecks per screen region from
  `biome_at_blended` (already used by the ground blend).

### O10. Waterlines render as periodic sparse dots, and differ by axis
- Shots: `pair_sand_water_v.png` (evenly spaced single yellow pixels down the
  waterline), `pair_grass_water_v.png` / `pair_snow_water_v.png` (dirt-brown
  specks reading as floating debris — brown crumbs on a snow shoreline),
  `pair_grass_water_h.png` (south-facing shore gets a *solid brown bank line*
  instead — axis-inconsistent treatment)
- Confirmed sites: `src/level/tile/water.rs:31` `get_sparse_color` (sand-foam
  special) + csprite sparse cells (`dispatch.rs:483`); side-vs-top cell art
  differs.
- Fix in one line: continuous 2px foam/wet band tinted per neighbor family,
  same treatment on all four edges.

### O11. mud|water boundary has zero shore treatment
- Shot: `pair_mud_water_v.png` (staged at (64,64))
- Wrong: razor-straight hard line — marsh pools read as cutouts.
- Confirmed site: `src/level/tile/mud.rs:15` `connects_to_water = true` (water
  draws full cells against mud) + mud renders flat with no connector.
- Fix in one line: drop the flag (giving water its edge cells) or add a
  mud-tinted water edge.

### O12. Heath is excluded from all blending
- Shots: `pair_heath_grass_v.png`, `pair_heath_snow_v.png` (staged); all
  nb-mountain shots (natural)
- Wrong: heath meets grass/snow/sand with a bare dither zipper and zero carry,
  unlike every other soft ground; feeds the O2 mess.
- Confirmed site: `src/gfx/lighting.rs:536` `tile_ground()` — no Heath arm
  (falls to `Other`); `heath.rs` sets only `connects_to_grass`.
- Fix in one line: add `GroundFam::Heath` + `fam_color` to the carry families.

### O13. Tidal flat: hard material cliff vs dry sand, bare staircase vs open water
- Shots: `pair_sand_tidal_v.png`, `nb9_beach_ocean.png` (seed 9 (-8,36)),
  `nb42_beach_ocean.png` (seed 42 (-160,155)) — the flat→water coast is a raw
  90-degree tile staircase with no rim/foam/dots at all
- Wrong: exposed flat ("sand darkened 28") reads as a different gray-brown
  material with a hard straight dry-line; the flat→water line — the softest
  boundary in nature — has zero edge treatment (tidal `connects_to_water`
  suppresses water's edge cells); glint dashes sit in a regular diagonal
  lattice across whole screens.
- Confirmed site: `src/level/tile/tidal.rs:57-100` (`is_submerged`, exposed
  render), `connects_to_water/sand` flags.
- Fix in one line: ramp the darken by distance to the dry line (`land_at`
  gradient), give the waterline a foam edge, hash-jitter glints into clusters.

### O14. Ocean-to-deep boundary is a tile-quantized darkening staircase
- Shots: `nb9_ocean_deep.png` (seed 9 (0,-126)), `pair_water_deep_v.png`
- Wrong: the deep darken lands per-tile, so the boundary is a staircase of
  16px dark squares reading as square shadow puddles on the water (seed 42's
  boundary reads organic only because its region edge happens to be blobby).
- Confirmed site: `src/level/tile/depth.rs:62` `deep_water_render` — flat
  darken(96) per tile, `connects_to_water = true` so no edge cells.
- Fix in one line: feather the darken across 1-2 tiles with a dither ramp
  (same Bayer approach as the seam carry).

### O15. Fireflies don't glow
- Shot: `marsh_night.png` (seed 42, marsh interior (112,128), night)
- Wrong: fireflies render as dim desaturated gray-blue rings with no warm
  color, no emitter halo, and none over the pool — unreadable as lights; the
  marsh's signature night cue is inert.
- Suspected site: `src/entity/fireflies.rs` render color + not registered in
  `lighting::stamp_emitters` (glowworms are; fireflies appear not to be).
- Fix in one line: warm true-color dot + tiny emitter radius (r<20 so no halo
  ring), and allow hover over water tiles near shore.

### O16. Mountains rock interior is a wall-to-wall quilted grid
- Shot: `field_mountains.png` (seed 9, tile (-96,120))
- Wrong: the rock mass renders as pale-gray pillow blocks with crack ticks in a
  uniform lattice and darker squares in a checker — reads as bathroom tiling
  across the entire screen.
- Suspected site: `src/level/tile/rock.rs` render + rock texture cells (single
  crack motif per tile, no variants).
- Fix in one line: hash-select 2-3 crack/texture variants per tile and break
  the darker-block cadence.

### O17. Desert ripples align into unbroken ruled lines
- Shot: `field_desert.png` (seed 9, tile (200,-192))
- Wrong: ripple dashes line up across tile boundaries into continuous
  horizontal stripes spanning the screen — ruled paper, violates the
  calm-base rule.
- Suspected site: sand texture art (`assets/sprites/tiles/sand.png`) — ripple
  rows at identical y in every tile; `sand.rs::render` applies no per-tile
  variation.
- Fix in one line: hash-offset/flip the sand texture per tile so lines break.

### O18. Day water palette reads as night sky
- Shots: every staged water pair, `field_ocean.png`, `field_deepocean.png`
- Wrong: at full noon brightness water is deep indigo with white/pink glitter
  points — open water reads as a starfield; at night, shorelines genuinely read
  as land-meets-sky. Taste call, but it undermines every shoreline read above.
- Suspected site: water base colors (`water.rs` color ramp) + day glitter
  color (`ambience.rs:215`).
- Fix in one line: day-gate a lighter mid-blue water band so day and night
  water differ.

### O19. Boulders bisected by structure footprints
- Shots: `st42_Village_edge.png` (boulder half under the NE wall bricks),
  `st9_Village_edge.png` (boulder embedded in a wood floor)
- Wrong: 2x2 boulders straddling a structure footprint are half-overwritten,
  leaving half-boulders fused into walls/floors.
- Confirmed mechanism: boulders stamp first, structures overwrite only their
  own footprint tiles (`structures_gen.rs` stamping order).
- Fix in one line: suppress `boulder_at` within `kind_radius + 2` of any
  placement footprint.

---

## Nitpicks

### O20. Wood Planks floor reads as wicker grid
- Shot: `pair_floor_wood_grass_v.png`. Every 8px cell has its own dark border —
  a plank field is a relentless small-square grid with no board direction.
  Site: floor texture cells (`floor.rs` Wood). Fix: long horizontal boards.

### O21. Ruins: worn-dirt patches fringe onto adjacent pavement
- Shots: `st9_Ruins.png` (seed 9 (-118,-95)), `st42_Ruins.png` ((90,-424)).
  Brown dither from interior worn-dirt tiles intrudes on neighboring gray
  floor tiles (floor is Other and should never receive carry — medium
  confidence on mechanism; may be worn-floor art reading as spill). Site to
  check: `ground_blend_pass` family check vs the ruins dirt/floor mix.

### O22. mud|sand seam renders as a loud dark sawtooth
- Shot: `pair_mud_sand_v.png`. The seam is a dark comb unlike any other pair
  (mud flat render + sand connector interplay). Site: `mud.rs`/`sand.rs`
  connector flags. Fix: give mud a proper connector or include it in sand's.

### O23. Dry bush on grass reads as a glowing neon-yellow ball
- Shots: `st9_village_out0.png` (top-right), `field_savanna.png`,
  `tide_*` frame corners. The tan lattice ball, tinted by the sand-side biome
  factor, floats on grass as a bright dithered "glow". Site: dry bush art +
  O3's tint. Fix: mostly falls out of the O3 clamp; else darken the lattice.

### O24. Snowfield streaks form a uniform diagonal lattice
- Shots: `pair_grass_snow_v.png`, `field_tundra.png` (mild). Same-orientation
  comma streaks on a fixed lattice. Site: snow texture cells. Fix: mirrored
  variant per tile hash.

### O25. Halo stipple rings read mechanical on bright grounds
- Shot: `light_night_snow_water_shore.png`. The emitter halo band is two crisp
  concentric dotted circles on snow. Site: `stamp_falloff` halo band
  (`HALO_W=8`). Fix: noise-jitter the band radius like `fog_noise`.

### O26. Beach sand is indistinguishable from desert sand
- Shot: `nb42_desert_beach.png` (seed 42 (-544,-531)) — no visual cue at all
  where desert ends and beach begins. Fix: shells/driftwood scatter or a damp
  tone band on beach.

### O27. Village decay dressing reads as bugs
- Shots: `st9_village_out0.png` (path dead-ends in open grass; clean orphan
  2x2/2x4 paving groups mid-path), `st9_Village_edge.png` (wall side fully
  gone, lone floating window tile).
- Note: TownAge decay (path_gap/paving/crumble) landed mid-audit and likely
  *intends* these; but clean, undamaged floor squares and paths that stop dead
  without taper read as stamping errors. Fix: dress decay (cracked variants,
  rubble at path ends, no lone window tiles on missing walls).

---

## Verified-clean categories
- Land-land seam carry (grass|sand, grass|snow, dirt|grass, mud|grass,
  dirt|sand, dirt|snow, sand|snow): fires on both axes and at corners; no
  X-blob artifacts at four-corner meets; h and v treated the same.
- Blend containment on floors/rock: no carry bleeding onto Stone Bricks, Wood
  Planks, or rock in any staged pair (O21 is the one suspected exception, in
  ruins' mixed dirt/floor interiors).
- Water|deep-water staged seam corners: smooth, no artifacts (the issue is the
  natural boundary shape, O14).
- Raft on deep water renders correctly under the player (`field_deepocean`).
- Sprite scale sweep: all tiles/furniture render 16px; suspect source PNGs
  (mushroom cluster, berry bush, dry bush) are 16x16 on disk — no further
  giant-sprite class bugs found beyond the known mushroom fix.
- Dead trees and cacti in deep desert sit directly on sand (`field_desert`).
- Forest, plains, tundra interiors: calm, no loud repetition
  (`field_forest/plains/tundra`).
- Structure trail torches, camp layouts, standing-stone edge scenes, cemetery
  surroundings: coherent (`st9_Camp_edge`, `st42_Camp*`, `st*_edge` shots).

## Not verified (honesty notes)
- Golden-hour long shadows: both dusk attempts landed in scheduled rain slices
  (long shadows are precip-gated off) and dawn is too dark to judge — the
  feature was never observed in this audit; needs a calm-slice retest.
- Mob contact shadows on snow vs grass: no mobs wandered into staged frames.
- Ocean-side shots at low tide show flat only (tide-out can exceed a full
  screen) — "waterline at mid-tide" states beyond the O13 staircase were not
  sampled across the tide clock.
- Village window light at night: skipped (known-dead per PLAYTEST.md; TownAge
  "Settled" lanterns landed after the audit shots).
