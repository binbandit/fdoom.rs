//! Port of `fdoom.level.tile.SnowTile`.

use super::dispatch;
use super::{ConnectorSprite, TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::sprite::Px;
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ItemKind, ToolType};

/// Java static `steppedOn` sprite.
fn stepped_on_sprite() -> Sprite {
    let pixels = vec![
        vec![Px::new(3, 1, 0), Px::new(1, 0, 0)],
        vec![Px::new(1, 0, 0), Px::new(3, 1, 0)],
    ];
    Sprite::from_pixels(
        pixels,
        color::get4(
            color::hex("#2c2c2c"),
            color::hex("#ffffff"),
            color::hex("#d3d3d3"),
            440,
        ),
    )
}

/// Java `SnowTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Snow);
    // JAVA: the ConnectorSprite is constructed with GrassTile.class as owner, but its
    // connectsTo is overridden (see connects_to below).
    def.csprite = Some(ConnectorSprite::simple(
        Sprite::new(
            11,
            0,
            3,
            3,
            color::get4(
                color::hex("#ffffff"),
                color::hex("#ffffff"),
                color::hex("#ffffff"),
                321,
            ),
            3,
        ),
        Sprite::dots(color::get4(
            color::hex("#2c2c2c"),
            color::hex("#ffffff"),
            color::hex("#d3d3d3"),
            321,
        )),
    ));
    def.may_spawn = true;
    def.connects_to_snow = true;
    def
}

/// Java anonymous `ConnectorSprite.connectsTo` override.
pub fn connects_to(_def: &TileDef, other: &TileDef, is_side: bool) -> bool {
    if !is_side {
        return true;
    }
    other.connects_to_snow
}

pub fn stepped_on(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32, e: &mut Entity) {
    if e.mob().is_some() {
        g.level_mut(lvl).set_data(xt, yt, 10);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let stepped_on = g.level(lvl).get_data(x, y) > 0;

    // JAVA: mutates the shared static sprite.full before rendering; here we render a
    // modified copy of the def instead.
    let mut def = def.clone();
    if let Some(cs) = def.csprite.as_mut() {
        if stepped_on {
            cs.full = stepped_on_sprite();
        } else {
            cs.full = Sprite::dots(color::get4(
                color::hex("#2c2c2c"),
                color::hex("#ffffff"),
                color::hex("#d3d3d3"),
                321,
            ));
        }
    }

    dispatch::csprite_render(g, screen, &def, lvl, x, y, None);
}

/// Java `tick` — entirely commented out in this fork.
pub fn tick(_g: &mut Game, _def: &TileDef, _lvl: usize, _xt: i32, _yt: i32) {
    // JAVA: "TODO revise this method." — the snow-spreading logic is commented out.
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    if let ItemKind::Tool { ttype, level, .. } = item.kind {
        if ttype == ToolType::Shovel
            && crate::entity::mob::player_behavior::pay_stamina(player, 4 - level)
            && item.pay_durability(g.is_mode("creative"))
        {
            let grass = g.tiles.get("grass");
            g.set_tile_default(lvl, xt, yt, &grass);
            g.play_sound(Sound::MonsterHurt);
            if g.random.next_int_bound(5) == 0 {
                // JAVA: dropItem(x, y, count, item) — exactly 2 seeds.
                let seeds = crate::item::registry::get(g, "seeds");
                for _ in 0..2 {
                    crate::level::drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, seeds.clone());
                }
                return true;
            }
            // JAVA: falls through to return false when no seeds drop.
        }
    }
    false
}
