//! Port of `fdoom.level.tile.SnowTile`.

use super::dispatch;
use super::{ConnectorSprite, TileDef, TileKind, tool_use};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::sprite::Px;
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ToolType};

/// Java static `steppedOn` sprite.
fn stepped_on_sprite() -> Sprite {
    let pixels = vec![
        vec![Px::new(3, 1, 0), Px::new(14, 3, 0)],
        vec![Px::new(15, 3, 0), Px::new(3, 1, 0)],
    ];
    // cool blue-gray prints so tracks read as compressed snow, not tan smudges
    Sprite::from_pixels(
        pixels,
        color::get4(
            color::hex("#ffffff"),
            color::hex("#ffffff"),
            color::hex("#dde6f0"),
            color::hex("#b9c8d8"),
        ),
    )
}

/// Java `SnowTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Snow);
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
        // dedicated drift-and-glint texture (artgen `snow_texture`, cells 13..16,3):
        // 1 = snow field, 2 = soft drift shading, 3 = deep drift edge / glints
        Sprite::dots_at(
            13,
            3,
            color::get4(
                color::hex("#ffffff"),
                color::hex("#ffffff"),
                color::hex("#dde6f0"),
                color::hex("#b9c8d8"),
            ),
        ),
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

    // the def is shared, so render a modified copy rather than mutating it in place
    let mut def = def.clone();
    if let Some(cs) = def.csprite.as_mut() {
        if stepped_on {
            cs.full = stepped_on_sprite();
        } else {
            cs.full = Sprite::dots_at(
                13,
                3,
                color::get4(
                    color::hex("#ffffff"),
                    color::hex("#ffffff"),
                    color::hex("#dde6f0"),
                    color::hex("#b9c8d8"),
                ),
            );
        }
    }

    dispatch::csprite_render(g, screen, &def, lvl, x, y, None);
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
    if tool_use(g, player, item, ToolType::Shovel, 4).is_some() {
        let grass = g.tiles.get("grass");
        g.set_tile_default(lvl, xt, yt, &grass);
        g.play_sound(Sound::MonsterHurt);
        if g.random.next_int_bound(5) == 0 {
            let seeds = crate::item::registry::get(g, "seeds");
            for _ in 0..2 {
                crate::level::drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, seeds.clone());
            }
        }
        // success even when no seeds drop — the snow was still cleared
        return true;
    }
    false
}
