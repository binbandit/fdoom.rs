//! Port of `fdoom.level.tile.WaterTile`, plus the sandbox-era shoreline work: the
//! waterline connector art is tinted per neighboring ground family (sand foam, mud
//! wet-lap, icy snow margins), tide-aware against Tidal Flats, and the shallow side
//! of the ocean→deep boundary feathers out through the same hashed contour bands as
//! `depth::deep_water_render`.

use super::{ConnectorSprite, TileDef, TileKind, dirt, dispatch, tidal};
use crate::core::game::Game;
use crate::entity::Entity;
use crate::entity::behavior::can_swim;
use crate::gfx::{Screen, Sprite, color};
use crate::level::infinite_gen::hash;

/// Java `WaterTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Water);
    def.csprite = Some(ConnectorSprite::simple(
        Sprite::new(14, 0, 3, 3, color::get4(3, 105, 211, 321), 3),
        Sprite::dots(color::get4(5, 105, 115, 115)),
    ));
    def.connects_to_sand = true;
    def.connects_to_water = true;
    def
}

/// Java anonymous `ConnectorSprite.connectsTo` override.
pub fn connects_to(_def: &TileDef, other: &TileDef, _is_side: bool) -> bool {
    other.connects_to_water
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, e: &Entity) -> bool {
    can_swim(e)
}

/// Waterline palette against one neighboring ground, when that ground family wants
/// its own lap tint. Slot 2 is the wet shadow ring, slot 3 the lap line the water
/// leaves on the bank (see `tiles/water_sparse.png`): sand gets foam-yellow, snow an
/// icy margin, mud a dark wet lap that melts into the bog — so shorelines read as
/// the same treatment everywhere, tinted by what the water touches.
fn shore_palette(tile: &TileDef) -> Option<i32> {
    match tile.kind {
        TileKind::Snow => Some(color::get4(3, 105, 334, 455)),
        TileKind::Mud => Some(color::get4(3, 105, 100, 210)),
        // the exposed intertidal flat is damp sand: a softer foam than dry beach
        TileKind::TidalFlat => Some(color::get4(3, 105, 431, 543)),
        _ if !tile.connects_to_water && tile.connects_to_sand => {
            Some(color::get4(3, 105, 440, 550))
        }
        _ => None,
    }
}

/// Java anonymous `ConnectorSprite.getSparseColor` override.
pub fn get_sparse_color(_def: &TileDef, tile: &TileDef, orig_col: i32) -> i32 {
    shore_palette(tile).unwrap_or(orig_col)
}

/// Does the tile at `(x, y)` read as open water *right now*? Like
/// `connects_to_water`, but tide-aware: a Tidal Flat only counts while submerged.
fn waterish(g: &Game, lvl: usize, x: i32, y: i32) -> bool {
    let t = g.tile_at(lvl, x, y);
    match t.kind {
        TileKind::TidalFlat => tidal::is_submerged(g, x, y),
        _ => t.connects_to_water,
    }
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    // Ripple animation seed. The i32 math (truncating / 10) before widening to i64 is
    // load-bearing: it quantizes the phase so the surface shimmers in steps.
    let int_part = g
        .tile_tick_count
        .wrapping_add((x / 2 - y).wrapping_mul(4311))
        / 10;
    let seed = (int_part as i64)
        .wrapping_mul(54687121)
        .wrapping_add(x as i64 * 3271612)
        .wrapping_add(y as i64 * 3412987161);

    let mut tmp = def.clone();
    let cs = tmp.csprite.as_mut().expect("water has a csprite");
    cs.full = Sprite::random_dots(seed, cs.full.color);
    let full = cs.full.color;
    let sparse = color::get4(3, 105, 211, dirt::d_col(g.level(lvl).depth));
    // two-sprite ConnectorSprite: sides share the sparse sprite, so the side color
    // must follow the sparse recolor
    dispatch::csprite_render(g, screen, &tmp, lvl, x, y, Some((sparse, sparse, full)));

    let g: &Game = g;

    // Tide waterline: the static connector pass reads a Tidal Flat neighbor's fixed
    // flag and treats it as open water, so the tide's edge got no shoreline at all
    // (ODDITIES O13). An *exposed* flat is a walkable bank — redraw the quadrants it
    // touches with the waterline art, in the damp-sand tint.
    let exposed = |dx: i32, dy: i32| {
        let t = g.tile_at(lvl, x + dx, y + dy);
        matches!(t.kind, TileKind::TidalFlat) && !tidal::is_submerged(g, x + dx, y + dy)
    };
    if exposed(0, -1) || exposed(0, 1) || exposed(-1, 0) || exposed(1, 0) {
        let wet = |dx: i32, dy: i32| waterish(g, lvl, x + dx, y + dy);
        let (u, d, l, r) = (wet(0, -1), wet(0, 1), wet(-1, 0), wet(1, 0));
        // per-quadrant sparse color, chained exactly like `csprite_render` (the
        // horizontal neighbor's palette wins), but skipping submerged flats
        let shore_dyn = |dx: i32, dy: i32, prev: i32| {
            if wet(dx, dy) {
                return prev;
            }
            shore_palette(&g.tile_at(lvl, x + dx, y + dy)).unwrap_or(prev)
        };
        let sp = &tmp.csprite.as_ref().expect("water has a csprite").sparse;
        let mut quad = |n: (i32, i32), h: (i32, i32), cx: i32, cy: i32, px: i32, py: i32| {
            if !(exposed(n.0, n.1) || exposed(h.0, h.1)) {
                return; // quadrant unaffected by the tide: the static pass was right
            }
            let col = shore_dyn(h.0, h.1, shore_dyn(n.0, n.1, sparse));
            sp.render_pixel_color(cx, cy, screen, (x << 4) + px, (y << 4) + py, col);
        };
        let (iu, id) = (if u { 1 } else { 2 }, if d { 1 } else { 0 });
        let (il, ir) = (if l { 1 } else { 2 }, if r { 1 } else { 0 });
        if !(u && l) {
            quad((0, -1), (-1, 0), il, iu, 0, 0);
        }
        if !(u && r) {
            quad((0, -1), (1, 0), ir, iu, 8, 0);
        }
        if !(d && l) {
            quad((0, 1), (-1, 0), il, id, 0, 8);
        }
        if !(d && r) {
            quad((0, 1), (1, 0), ir, id, 8, 8);
        }
    }

    // Shallow side of the ocean→deep boundary: the deep side already feathers its
    // darkening out through hashed contour bands (`depth::deep_water_render`); creep
    // the first band raggedly into the shallow tile too, so the boundary wanders
    // across the tile seam instead of stair-stepping along the grid (ODDITIES O14).
    // Only for genuine shallow-water map tiles — this render fn also paints the base
    // for deep water and submerged flats, which pass the shared water def.
    if matches!(g.tile_at(lvl, x, y).kind, TileKind::Water) {
        let deep =
            |dx: i32, dy: i32| matches!(g.tile_at(lvl, x + dx, y + dy).kind, TileKind::DeepWater);
        let (dn, ds, dw, de) = (deep(0, -1), deep(0, 1), deep(-1, 0), deep(1, 0));
        // corner blobs where the deep region only touches diagonally — the hard
        // 90-degree staircase corners
        let dnw = !dn && !dw && deep(-1, -1);
        let dne = !dn && !de && deep(1, -1);
        let dsw = !ds && !dw && deep(-1, 1);
        let dse = !ds && !de && deep(1, 1);
        let deep_sides = [dn, ds, dw, de].iter().filter(|&&b| b).count();
        if deep_sides >= 3 {
            // deep water on (nearly) every side: this is a shallow speck inside the
            // blended deep boundary's tile checker. Per-edge feathers here would
            // draw a glowing frame — wash the whole tile toward the deep tone
            // instead so the checker melts together.
            screen.darken_rect(x * 16, y * 16, 16, 16, 44);
        } else if dn || ds || dw || de || dnw || dne || dsw || dse {
            let wseed = g.world_seed;
            // How far the deep darkening reaches into this tile at one boundary
            // pixel: a coarse 4px-cluster component plus fine per-pixel wobble,
            // 0..=7px. Keyed on absolute position along the boundary so the contour
            // runs continuously across neighboring shallow tiles — clustered fingers
            // of dark water crossing the seam, not a comb.
            let reach = |salt: u64, along: i32, across: i32| -> i32 {
                let coarse = (hash(wseed, salt, along.div_euclid(4), across) % 4) as i32 * 2;
                coarse + (hash(wseed, salt ^ 0x5A5A, along, across) % 2) as i32
            };
            // depth-into-finger -> darken amount: the deep side's own edge pixels
            // stay bright (depth.rs feathers 0/30/62/96 from the seam inward), so a
            // finger must NOT slam dark right at the seam — it eases in at the
            // deep side's first-band tone and stays there, reading as the 30-band
            // poking across rather than outlining the tile
            let amount = |d: i32, r: i32| -> i32 {
                if d > r {
                    0
                } else if d == 0 {
                    16
                } else {
                    30
                }
            };
            for py in 0..16 {
                for px in 0..16 {
                    let mut a = 0;
                    if dn {
                        a = a.max(amount(py, reach(0xD3E9_0011, x * 16 + px, y)));
                    }
                    if ds {
                        a = a.max(amount(15 - py, reach(0xD3E9_0012, x * 16 + px, y)));
                    }
                    if dw {
                        a = a.max(amount(px, reach(0xD3E9_0013, y * 16 + py, x)));
                    }
                    if de {
                        a = a.max(amount(15 - px, reach(0xD3E9_0014, y * 16 + py, x)));
                    }
                    // staircase corners: soften with a Chebyshev-distance blob
                    if dnw {
                        a = a.max(amount(px.max(py), reach(0xD3E9_0015, x * 16 + px, y)));
                    }
                    if dne {
                        a = a.max(amount(
                            (15 - px).max(py),
                            reach(0xD3E9_0016, x * 16 + px, y),
                        ));
                    }
                    if dsw {
                        a = a.max(amount(px.max(15 - py), reach(0xD3E9_0017, x * 16 + px, y)));
                    }
                    if dse {
                        a = a.max(amount(
                            (15 - px).max(15 - py),
                            reach(0xD3E9_0018, x * 16 + px, y),
                        ));
                    }
                    if a > 0 {
                        screen.darken_rect(x * 16 + px, y * 16 + py, 1, 1, a);
                    }
                }
            }
        }
    }
}

pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let mut xn = xt;
    let mut yn = yt;

    if g.random.next_boolean() {
        xn += g.random.next_int_bound(2) * 2 - 1;
    } else {
        yn += g.random.next_int_bound(2) * 2 - 1;
    }

    if g.tile_at(lvl, xn, yn).same_tile(&g.tiles.get("hole")) {
        g.set_tile_default(lvl, xn, yn, def);
    }
    if g.tile_at(lvl, xn, yn).same_tile(&g.tiles.get("lava")) {
        // water spreading into lava quenches it to a stone-brick floor
        let t = g.tiles.get("Stone Bricks");
        g.set_tile_default(lvl, xn, yn, &t);
    }
    // excavation flooding: water pours into an adjacent dug pit or chasm and assumes
    // its depth (shallow dig -> water, bottomed-out pit or chasm -> deep water)
    super::depth::try_flood(g, lvl, xn, yn);
}
