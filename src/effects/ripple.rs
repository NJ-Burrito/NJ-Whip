//! Ripple: the crack's burst and the mark it leaves.
//! Zen: a brushed ink circle, left open

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    let p = &mut fx.particles;
    let st = &mut fx.stamps;
    let stamp = |kind, to: V, life: f32, rgb: u32| Stamp { kind, at, to, age: 0.0, life, rgb };
    st.push(stamp(StampKind::Ripple, at, 0.6, 0x4F86C6));
    p.push(Particle { pos: (at.0 - 20.0, at.1 - 60.0), vel: (30.0, 40.0), grav: 20.0, drag: 0.99, age: 0.0, life: 0.85, size: 9.0, grow: 0.0, rot: 20.0, spin: 120.0, rgb: 0xFFB7C9, shape: Shape::Petal });
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    use std::f32::consts::{PI, TAU};
    let c @ (x, y) = s.at;
    let start = r(0.0, TAU);
    // an ink circle in one stroke: a wet pale ground, then the brush thinning as it runs dry
    disc(pm, c, 28.0, 0xFFFFFF, a * 0.05);
    let (w0, w1, sweep) = (8.5, 2.4, 1.8 * PI);
    let n = 8;
    for i in 0..n {
        let (u0, u1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
        let w = w0 + (w1 - w0) * (u0 + u1) * 0.5;
        let rad = 32.0 - 3.0 * u0;
        let seg = arc(c, rad, 1.0, start + sweep * u0, start + sweep * u1 + 0.02, 6);
        stroke_line(pm, &seg, 0xFFFFFF, a * 0.3, w + 3.0);
    }
    for i in 0..n {
        let (u0, u1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
        let w = w0 + (w1 - w0) * (u0 + u1) * 0.5;
        let rad = 32.0 - 3.0 * u0;
        stroke_line(pm, &arc(c, rad, 1.0, start + sweep * u0, start + sweep * u1 + 0.02, 6), 0x2B2F3D, a * 0.92, w);
    }
    // dry-brush streaks near the tail and a pooled blot where the brush landed
    stroke_line(pm, &arc(c, 29.0, 1.0, start + 0.4 * sweep, start + 0.9 * sweep, 20), 0x2B2F3D, a * 0.4, 1.6);
    stroke_line(pm, &arc(c, 35.0, 1.0, start + 0.55 * sweep, start + 0.95 * sweep, 20), 0x2B2F3D, a * 0.35, 1.2);
    let p0 = (x + start.cos() * 32.0, y + start.sin() * 32.0);
    disc(pm, p0, 6.0, 0x2B2F3D, a * 0.95);
    disc(pm, (p0.0 - 1.5, p0.1 - 1.5), 1.8, 0xFFFFFF, a * 0.3);
    for _ in 0..3 {
        let (ang, d) = (start + r(-0.5, 0.5), r(8.0, 15.0));
        disc(pm, (p0.0 + ang.cos() * d, p0.1 + ang.sin() * d), r(0.9, 1.9), 0x2B2F3D, a * 0.7);
    }
}
