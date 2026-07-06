//! Behaviors of `fdoom.entity.particle.Particle` and `TextParticle`.

use crate::core::game::Game;
use crate::entity::{Entity, EntityKind, behavior};
use crate::gfx::Screen;

/// Java `Particle.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    let EntityKind::Particle(p) = &mut e.kind else {
        return;
    };
    p.time += 1;
    if p.time > p.lifetime {
        behavior::remove_entity(g, e);
    }
}

/// Java `Particle.render(screen)`.
pub fn render(_g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let EntityKind::Particle(p) = &e.kind else {
        return;
    };
    p.sprite.render(screen, e.c.x, e.c.y);
}

/// Java `TextParticle.tick()`.
pub fn text_tick(g: &mut Game, e: &mut Entity) {
    {
        let EntityKind::TextParticle(t) = &mut e.kind else {
            return;
        };
        t.particle.time += 1;
        if t.particle.time > t.particle.lifetime {
            behavior::remove_entity(g, e);
            return;
        }

        // move the particle according to the acceleration
        t.xx += t.xa;
        t.yy += t.ya;
        t.zz += t.za;
        if t.zz < 0.0 {
            t.zz = 0.0;
            t.za *= -0.5;
            t.xa *= 0.6;
            t.ya *= 0.6;
        }
        t.za -= 0.15;
        e.c.x = t.xx as i32;
        e.c.y = t.yy as i32;
    }
}

/// Java `TextParticle.render(screen)`.
pub fn text_render(_g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let EntityKind::TextParticle(t) = &e.kind else {
        return;
    };
    let style = t
        .style
        .clone()
        .set_x_pos(e.c.x - t.msg.chars().count() as i32 * 4)
        .set_y_pos(e.c.y - t.zz as i32);
    style.draw(&t.msg, screen);
}

/// Java `TextParticle.getData()`.
pub fn text_get_data(e: &Entity) -> String {
    let EntityKind::TextParticle(t) = &e.kind else {
        return String::new();
    };
    format!("{}:{}", t.msg, t.style.get_color())
}
