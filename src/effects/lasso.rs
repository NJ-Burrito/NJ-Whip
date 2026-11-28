//! Lasso: the crack's burst and the mark it leaves.
//! Cowboy: a horseshoe print

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    let p = &mut fx.particles;
    let st = &mut fx.stamps;
    let stamp = |kind, to: V, life: f32, rgb: u32| Stamp { kind, at, to, age: 0.0, life, rgb };
    st.push(stamp(StampKind::Lasso, at, 0.45, 0xB97A43));
    p.extend(ring(8, at, (50.0, 130.0), (0.4, 0.6), (6.0, 11.0), &[0xD8C29A, 0xC9AE7E], Shape::Puff, -30.0));
    p.extend(ring(6, at, (150.0, 300.0), (0.4, 0.6), (3.0, 5.0), &[0xE8C86A], Shape::Chip, 600.0));
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, _r: &mut dyn FnMut(f32, f32) -> f32) {
    use std::f32::consts::PI;
    let c @ (x, y) = s.at;
    // pressed ground, then a raised horseshoe with a shadow, a lit edge and nail holes
    oval(pm, c, 34.0, 28.0, 0x3A2A1C, a * 0.18);
    oval(pm, c, 29.0, 23.0, 0xD8C29A, a * 0.1);
    let (a0, a1) = (0.75 * PI, 2.25 * PI);
    let shoe = arc(c, 24.0, 1.15, a0, a1, 24);
    let low = arc((x + 1.5, y + 2.5), 24.0, 1.15, a0, a1, 24);
    stroke_line(pm, &low, 0x000000, a * 0.3, 11.0);
    stroke_line(pm, &shoe, 0x3A2A1C, a * 0.75, 10.5);
    stroke_line(pm, &shoe, 0xB97A43, a * 0.95, 7.0);
    stroke_line(pm, &arc((x - 0.8, y - 1.2), 25.2, 1.15, a0 + 0.35, a0 + 1.9, 12), 0xE6B07A, a * 0.75, 2.0);
    stroke_line(pm, &arc(c, 21.0, 1.15, a0 + 0.2, a1 - 0.2, 24), 0x6E4524, a * 0.5, 1.6);
    for i in 0..6 {
        let ang = a0 + 0.45 + (a1 - a0 - 0.9) * i as f32 / 5.0;
        disc(pm, (x + ang.cos() * 24.0, y + ang.sin() * 24.0 * 1.15), 1.5, 0x1E130A, a * 0.9);
    }
}
