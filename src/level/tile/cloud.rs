//! Port of `fdoom.level.tile.CloudTile`.

use super::{ConnectorSprite, TileDef, TileKind};
use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::sprite::make_sprite;
use crate::gfx::{Sprite, color};
use crate::item::{Item, ItemKind, ToolType};

/// Java `CloudTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Cloud);
    def.csprite = Some(ConnectorSprite::new(
        Sprite::new(4, 0, 3, 3, color::get4(333, 444, 555, -1), 3),
        Sprite::new(7, 0, 2, 2, color::get4(333, 444, 555, -1), 3),
        make_sprite(
            2,
            2,
            color::get4(444, 444, 555, 444),
            0,
            false,
            &[19, 18, 20, 19],
        ),
    ));
    def
}

/// Java anonymous `ConnectorSprite.connectsTo` override — connects to everything except
/// Infinite Fall.
pub fn connects_to(_def: &TileDef, other: &TileDef, _is_side: bool) -> bool {
    other.name != "INFINITE FALL"
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    true
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
    if let ItemKind::Tool { ttype, .. } = item.kind {
        // JAVA: forgot the payDurability call every other shovelable tile makes, so
        // shoveling cloud was free. FIX: charge durability like sand/snow/grass do.
        if ttype == ToolType::Shovel
            && crate::entity::mob::player_behavior::pay_stamina(player, 5)
            && item.pay_durability(g.is_mode("creative"))
        {
            // JAVA: "would allow you to shovel cloud, I think."
            let infinite_fall = g.tiles.get("Infinite Fall");
            g.set_tile_default(lvl, xt, yt, &infinite_fall);
            let cloud = crate::item::registry::get(g, "cloud");
            crate::level::drop_items_counted(g, lvl, xt * 16 + 8, yt * 16 + 8, 1, 3, &[cloud]);
            return true;
        }
    }
    false
}
