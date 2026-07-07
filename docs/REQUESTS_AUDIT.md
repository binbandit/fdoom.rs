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
