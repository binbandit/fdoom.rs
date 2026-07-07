//! Port of `fdoom.level.tile.RockTile`.

use super::{ConnectorSprite, TileDef, TileKind, dirt, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::entity::mob::player_behavior::pay_stamina;
use crate::entity::particle::{new_smash_particle, new_text_particle};
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ItemKind, ToolType};
use crate::level::drop_items_counted;

// JAVA: `RockTile.coallvl` was instance state on the singleton tile — set to 1 on the
// first pickaxe interact (or creative break) and never reset, so whether *any* rock ever
// dropped coal depended on process-global history that leaked across worlds and saves.
// FIX: the intent ("rocks mined with a pickaxe drop coal, rocks smashed by mobs or
// explosions just crumble to stone") is derived per break: `hurt_dmg_inner` takes a
// `drops_coal` flag — true from the pickaxe interact (and creative breaks), false from
// mob damage and the generic `hurt_dmg` dispatch entry (explosions). No global state.

/// Java `RockTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Rock);
    def.csprite = Some(ConnectorSprite::new(
        Sprite::new(4, 0, 3, 3, color::get4(111, 444, 555, 321), 3),
        Sprite::new(7, 0, 2, 2, color::get4(111, 444, 555, 321), 3),
        // dedicated fractured-plate texture (artgen `stone_texture`, cells 25..28,3):
        // 0 = lit plate edges, 1 = stone face, 2 = cracks, 3 = deep pits
        Sprite::dots_at(25, 3, color::get4(555, 444, 333, 111)),
    ));
    def
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    // Edge shade blends into the surrounding ground instead of always dirt-brown
    // (a lone rock in a meadow used to get a brown halo). Sample the four neighbors
    // and match the dominant ground family.
    let mut grass_n = 0;
    let mut sand_n = 0;
    let mut snow_n = 0;
    for (nx, ny) in [(x, y - 1), (x, y + 1), (x - 1, y), (x + 1, y)] {
        let t = g.tile_at(lvl, nx, ny);
        if matches!(t.kind, TileKind::Snow) {
            snow_n += 1;
        } else if t.connects_to_sand {
            sand_n += 1;
        } else if t.connects_to_grass {
            grass_n += 1;
        }
    }
    let bg = if snow_n >= grass_n.max(sand_n) && snow_n > 0 {
        color::hex("#e8eef4")
    } else if sand_n > grass_n {
        550
    } else if grass_n > 0 {
        141
    } else {
        dirt::d_col(g.level(lvl).depth)
    };
    let col = color::get4(111, 444, 555, bg);
    let full = def.csprite.as_ref().map(|cs| cs.full.color).unwrap_or(0);
    dispatch::csprite_render(g, screen, def, lvl, x, y, Some((col, col, full)));
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    // JAVA: a commented-out debug branch let creative-mode arrows break rock.
    false
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    _source: &mut Entity,
    _dmg: i32,
    _attack_dir: Direction,
) -> bool {
    // Mob smashing: stone only, no coal (see the coallvl note above).
    hurt_dmg_inner(g, def, lvl, x, y, 1, false);
    true
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    if let ItemKind::Tool {
        ttype,
        level: tool_level,
        ..
    } = &item.kind
    {
        let (ttype, tool_level) = (*ttype, *tool_level);
        if ttype == ToolType::Pickaxe
            && pay_stamina(player, 4 - tool_level)
            && item.pay_durability(g.is_mode("creative"))
        {
            let dmg = g.random.next_int_bound(10) + tool_level * 5 + 10;
            // Pickaxe mining: eligible for coal drops.
            hurt_dmg_inner(g, def, lvl, xt, yt, dmg, true);
            return true;
        }
    }
    false
}

/// Generic damage entry (dispatch/explosions) — smashed rock drops stone, not coal.
pub fn hurt_dmg(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    hurt_dmg_inner(g, def, lvl, x, y, dmg, false);
}

fn hurt_dmg_inner(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    dmg: i32,
    drops_coal: bool,
) {
    let mut dmg = dmg;
    let mut damage = g.level(lvl).get_data(x, y) + dmg;
    let mut drops_coal = drops_coal;
    let rock_health = 50;
    if g.is_mode("creative") {
        dmg = rock_health;
        damage = rock_health;
        drops_coal = true;
    }
    g.play_sound(Sound::MonsterHurt); // JAVA: the SmashParticle constructor plays this.
    g.level_mut(lvl)
        .add(new_smash_particle(x * 16, y * 16), lvl);
    let text = new_text_particle(
        &dmg.to_string(),
        x * 16 + 8,
        y * 16 + 8,
        color::RED,
        &mut g.random,
    );
    g.level_mut(lvl).add(text, lvl);
    if damage >= rock_health {
        // JAVA: unused `int count = random.nextInt(1) + 0;` — consumes a random value.
        let _count = g.random.next_int_bound(1);
        if !drops_coal {
            let stone = crate::item::registry::get(g, "Stone");
            drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 4, &[stone]);
        } else {
            let stone = crate::item::registry::get(g, "Stone");
            drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[stone]);
            let mut mincoal = 0;
            let mut maxcoal = 1;
            if g.settings.get("diff").as_str() != "Hard" {
                mincoal += 1;
                maxcoal += 1;
            }
            let coal = crate::item::registry::get(g, "coal");
            drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, mincoal, maxcoal, &[coal]);
        }
        let dirt = g.tiles.get("dirt");
        g.set_tile_default(lvl, x, y, &dirt);
    } else {
        g.level_mut(lvl).set_data(x, y, damage);
    }
}

pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let damage = g.level(lvl).get_data(xt, yt);
    if damage > 0 {
        g.level_mut(lvl).set_data(xt, yt, damage - 1);
    }
}
