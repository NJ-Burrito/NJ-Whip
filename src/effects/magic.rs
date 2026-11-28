//! Magic: the crack's burst and the mark it leaves.
//! Wizard: a rune

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    let p = &mut fx.particles;
    let st = &mut fx.stamps;
    let stamp = |kind, to: V, life: f32, rgb: u32| Stamp { kind, at, to, age: 0.0, life, rgb };
    st.push(stamp(StampKind::Magic, at, 0.45, 0xB89CFF));
    p.extend(ring(20, at, (120.0, 320.0), (0.4, 0.7), (3.0, 5.0), &[0xC7A8FF, 0xFFD25A, 0xFFFFFF], Shape::Star, 0.0));
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    use std::f32::consts::{PI, TAU};
    let c @ (x, y) = s.at;
    // a faint glow, a double ring with rune ticks between, a six-point star and a bright centre
    disc(pm, c, 37.0, 0xB89CFF, a * 0.1);
    hoop(pm, c, 34.0, 0xB89CFF, a * 0.22, 5.0);
    hoop(pm, c, 34.0, 0xB89CFF, a * 0.85, 1.8);
    hoop(pm, c, 27.0, 0xB89CFF, a * 0.55, 1.0);
    let rot = r(0.0, TAU);
    let ticks = (0..18).map(|i| {
        let ang = rot * 0.5 + i as f32 * TAU / 18.0;
        let (r0, r1) = (28.5, if i % 3 == 0 { 32.8 } else { 31.0 });
        ((x + ang.cos() * r0, y + ang.sin() * r0), (x + ang.cos() * r1, y + ang.sin() * r1))
    });
    segs(pm, ticks, 0xD9C8FF, a * 0.75, 1.2);
    for tri in 0..2 {
        let pts: [V; 4] = std::array::from_fn(|k| {
            let ang = rot + tri as f32 * PI / 3.0 + k as f32 * TAU / 3.0;
            (x + ang.cos() * 26.0, y + ang.sin() * 26.0)
        });
        stroke_line(pm, &pts, 0xFFD25A, a * 0.2, 3.6);
        stroke_line(pm, &pts, 0xFFD25A, a * if tri == 0 { 0.9 } else { 0.55 }, 1.4);
    }
    for k in 0..6 {
        let ang = rot + k as f32 * TAU / 6.0;
        disc(pm, (x + ang.cos() * 26.0, y + ang.sin() * 26.0), 1.9, 0xFFFFFF, a * 0.85);
    }
    disc(pm, c, 5.0, 0xFFD25A, a * 0.3);
    disc(pm, c, 2.4, 0xFFFFFF, a * 0.85);
}
