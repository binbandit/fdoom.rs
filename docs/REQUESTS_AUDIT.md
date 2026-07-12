# Request Audit — every user request this project cycle

Status: ✅ done & verified · 🔄 in flight (agent working now) · 📋 planned (spec'd task)

## Foundation
- ✅ Full Java→Rust 1:1 port, tagged `v0.1.0` (byte-identical worldgen vs JVM at the tag)
- ✅ Commit everything in logical conventional commits; .gitignore; version tag
- ✅ Remove JavaRandom (xoshiro256++, deterministic per seed)
- ✅ Codebase cleanup: Settings-as-widgets anti-pattern, =debug keymap hack, dead code
- ✅ Bug/stability passes (wall dmg, invisible fences, gravestone state, soak tests…)
- ✅ Better default controls (E inventory, Space attack, sane text entry)
- ✅ Easier content addition (docs/ADDING_CONTENT.md + registries + artgen)
- ✅ DX overhaul: README, docs/, justfile ("fun to build in")
- ✅ Document EVERYTHING (TERRAIN/ENTITIES/ITEMS_AND_CRAFTING/RENDERING_AND_UI/
     CORE_AND_SAVES + ROADMAP; sonnet agents)

## World
- ✅ Infinite chunked worlds (dig-descent between layers; no pre-placed stairs)
- ✅ Large Minecraft-scale biomes + more natural biomes (10 incl. DeepOcean)
- ✅ Patchy biome edges (domain-warped transitions; snow freckles, sand fades)
- ✅ Per-biome ground tint (in the lighting pass; patchy domain-warped edges)
- ✅ Multi-level terrain: dig-down pits→chasm→ladder; deep water + raft
- ✅ Highland rock v1: raised double-hard rock bands under the summit snowline
- ✅ Summit snow: highest peaks whiten in any climate
- ✅ Structures: ruins, cemeteries (aging graves→zombies), standing stones, camps,
     old trails, destroyed villages, boulders
- ✅ Mud tile + mud pits + inland ponds; ocean waves; marsh mud rims
- ✅ Tall grass: paddock-core blocking only; meadows; slow growth (days/stage)
- ✅ Tree species per biome (pine/dead/willow/palm/flat-crown; snow pine existing)
- ✅ Dried dead bushes (desert/savanna)
- ✅ Seaweed/coral shallows; reeds; snow-capped mountains
- ✅ Forests denser + more common (user report fixed)
- ✅ Natural food: berries (regrow), mushrooms (surface+mines), apples, cactus fruit,
     coconuts; pumpkins drop items
- ✅ Jack-o-lanterns: craftable (pumpkin+torch) + spawn in cemeteries/villages
- ✅ Rare world events: Hollow Night, Aurora, Ember Rain, Whisper Fog, The Caravan
- ✅ Seasonal events: Halloween & Christmas windows (real calendar, mockable)
- ✅ New worlds spawn at seed-random time of day
- ✅ Day-cycle option: Classic / Long / Realtime (24h)
- ✅ Survival-only (Creative = --debug tool); no world size/type/theme options
- ✅ Sandbox pivot: sky world, Air Wizard, win condition, Score mode all removed

## Game systems
- ✅ Crafting overhaul: gather chains (fiber/stick/cord/knapping → crude tools)
- ✅ Weapons: spear (+throw), crossbow (part assembly), throwing knives, slingshot
- ✅ More recipes: cooking chain, bandages, fruit medley, rafts, jack-o-lantern
- ✅ Mob roster: creeper/slime/skeleton/wizard REMOVED (incl. their art cells +
     bossdeath.wav); snake kept & fixed; 4 originals (lurker, hound pack, golem, wisp)
- ✅ Mob-life wave: snake family (grass snake/adder/coiled rattler/cave serpent),
     ghost (grave-rising, Hollow Night swarms, phase pulses), roosting/spooking
     firefly swarms, grass stealth + glowing night eyes, movement personalities
- ✅ Fossicking overhaul: prospector's pan + richness field, mineral-seep stains,
     vein-chasing sparkles, cracked/dense rock, cave-ins + timber props, skerries
- ✅ QOL: durability bar, death screen (time/score, Respawn/Main Menu), inventory
     info strip, readable notifications, Load World fixed (was completely broken),
     R-save crash fixed
- 📋 Save toast small in bottom-right corner (lands right after the fire wave)

- ✅ Weather: deterministic rain/snowfall, pixel-art streaks, wet dimming, desert
     gate, crop/berry growth boost while raining, fish-presence bubbles
- ✅ Water tides: creeping coastal waterline twice a day, beachcombing at low tide
- ✅ Windows + glass + light occlusion: light beams through panes/doorways, walls
     stop it; sealed rooms hold their light
- ✅ Structures: +46% density, 13 layout variants (towers, walled cemeteries,
     avenues, dolmens, cold camps, crossroads villages); desert cemeteries
- ✅ Events complete: Ember Rain, Whisper Fog, The Caravan + Halloween/Christmas
- ✅ Worldview inspector bin (biome/tile modes, structure counts, R rerolls)
- 🔄 Fire & campfire wave (in flight): fuel, smoke, rest, cooking, fire spread
- 🔄 Fishing wave (in flight): invisible fish, bubble hotspots, deep-water table
- 📋 Beauty sweep (final): remove all 300 JAVA comments, tile interact dedup,
     --debug dev console

## Presentation
- ✅ Hybrid RGB pipeline → fresh original art + generator (`artgen`)
- ✅ Player art: pixel-traced from the original (after 3 redesign attempts failed)
- ✅ Classic mob art traced pixel-for-pixel (pig/knight/cow/sheep/snake)
- ✅ Terrain textures redesigned twice to user standard: calm base + sparse
     clustered detail (tufts/ripples/drifts/clods/cracks), daytime A/B verified
- ✅ Full sheet-art logo: FOSSICKERS + DOOM wordmarks (verified on title)
- ✅ Item icon audit; flora readability; growth-stage clarity; footprints;
     grave variety incl. wooden crosses; connected tree clusters; one-sheet
     asset consolidation; mushroom redesigned (forage cluster)
- ✅ Full sheet-art FOSSICKERS DOOM lockup; Continue on title; glass menus;
     name+seed world creation; readable books; TestWorld harness + 1-line
     item/recipe declarations; guides rewritten; stale translations dropped
- ✅ Lighting system: sunsets, real cave darkness, dithered warm torchlight, aurora
     curtains, ~96µs/frame, always-on (no Classic toggle per user)
- ✅ Title: drone-flyover world (shared splash/title), smooth pan, gradient text bands
- ✅ Splash modernized; dead menu entries removed
- ✅ Smoked-glass menus; slate frames; hints only on title
- ✅ World creation: name+seed only, floats over flyover, seed hint
- ✅ Title flow: Continue (most recent world) / New World / Load World flat menu
- ✅ Instructions/book pages readable (opaque paper under glass system)
- ✅ Options screen matches UI style (floats, no wear-suit row)
- ✅ Snow/sand footprints color-matched (snow prints blue-gray)
- ✅ Tile edge blending (rock halo, grass dirt-rim fixes; corner nubs with art agent)

## Meta
- ✅ Sub-agents used aggressively with correct sequencing/file ownership
- ✅ Creative roadmap committed (docs/ROADMAP.md): mystical layer (leylines, shrines),
     bountiful layer (hives, fishing schools) — future waves

## Product-manager era (this cycle: taste doc + wave board; commits cited)
- ✅ DayZ/7DtD north star + Minecraft approachability written into CLAUDE.md taste
     (eebe302, 494000d); world coherence = "oddities are bugs"
- ✅ Snow never beside sand: climate-gated biome adjacency + property test (17af522)
- ✅ Square holes → ragged organic pits/chasms (5364a72); merged excavations, channel
     flooding that assumes pit depth, base-in-hole (a628082)
- ✅ Tiles-not-merging sweep: ODDITIES.md audit of 27 findings (e1c6c00); O1-O3 light
     seams/rock backings/tint identity fixed (be7b198); water family O4/O5 + all
     waterline axes (39c7970)
- ✅ Tiny mushrooms 3-5 per tile (asked twice) + flower species + wet sand + dedicated
     icons + roof-support timber prop (3ff1fe5)
- ✅ Trees in groups read as little forests (canopy connectors, 4937964)
- ✅ Crops/cooking/food recipes: 4 world-seeded crops, campfire roasting, oven pot
     dishes, Queasy raw-food risk (8aec45c)
- ✅ Rain→snow in cold biomes + snowfall converting tiles one at a time + thaw
     (24b700b — snow visits the cold fringe, tundra keeps it)
- ✅ Morning fog / afternoon haze / regional fog banks (bbc59ed), capped below the
     Whisper Fog event
- ✅ Towns of different shapes/sizes/ages (overgrown→settled) + hamlets; searchable
     containers; water bottles / food cans / can→tin chain (5df1fec)
- ✅ Heat/cold system: 7 bands, fur coats, straw hats, tree shade, campfire warmth,
     3-heart mercy floor (2e837da)
- ✅ Pixel studio kept improving for post-AI self-sufficiency: real canvas mode,
     new-sprite flow, in-context previews, full manual (ff86b95)
- ✅ HUD redo + one survival screen + original crafting identity: design 634dbbd,
     frameless HUD 5fb146c, survival screen 2e553cd, wear slots 4ba7f3e,
     stations/containers deb066c, THE BENCH 14dfa5c
- ✅ Text overflow "hard to read": draw_fit ellipsis rule across all panes + pixel
     guard-band tests (deb066c; recipe list follow-up in 14dfa5c)
- ✅ Crash holding item after crafting (stale pack rows) fixed structurally (e8473c3)
- 🔄 Resizable window that shows MORE (dynamic logical resolution 288x192→640x400,
     integer scaling, denser lists) — gpt-5.5 migration in flight
- ✅ Playtest program: docs/PLAYTEST.md top-10 fully landed (combat juice e1427d4,
     notifications 4dacdd9, saves 0a460f3, cave-ins 3e38de0, map e525614, village
     light 94bae2d, first-day thread 660f09f); README + hero shots (b1609fe)
\n