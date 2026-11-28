//! Slash: the crack's burst and the mark it leaves.
//! Ninja: a cut across the screen

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    let p = &mut fx.particles;
    let st = &mut fx.stamps;
    st.push(Stamp { kind: StampKind::Slash, at: (at.0 - 90.0, at.1 + 60.0), to: (at.0 + 90.0, at.1 - 60.0), age: 0.0, life: 0.22, rgb: 0xFFFFFF });
    p.extend(ring(8, at, (30.0, 90.0), (0.4, 0.6), (8.0, 13.0), &[0x2B2D33, 0x3B3E46], Shape::Puff, -30.0));
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    // a clean cut: a soft shadow, the dark gap pointed at both ends, a bright lit edge, and chips
    taper(pm, s.at, s.to, 0.05, 6.5, (0.0, 1.0), 1.5, 0x000000, a * 0.3);
    taper(pm, s.at, s.to, 0.05, 3.6, (0.0, 1.0), 0.0, 0x0B0C0E, a * 0.9);
    taper(pm, s.at, s.to, 0.05, 1.0, (0.02, 0.98), -2.6, 0xFFFFFF, a * 0.85);
    taper(pm, s.at, s.to, 0.05, 0.8, (0.1, 0.8), 3.4, 0x8FA2B8, a * 0.55);
    let (dx, dy) = (s.to.0 - s.at.0, s.to.1 - s.at.1);
    let l = (dx * dx + dy * dy).sqrt().max(1.0);
    let (nx, ny) = (-dy / l, dx / l);
    let mut chips = [((0.0f32, 0.0f32), (0.0f32, 0.0f32)); 5];
    for c in chips.iter_mut() {
        let u = r(0.2, 0.8);
        let side = if r(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
        let (px, py) = (s.at.0 + dx * u, s.at.1 + dy * u);
        let len = r(5.0, 10.0);
        let slide = r(-3.0, 3.0);
        *c = ((px + nx * side * 3.0, py + ny * side * 3.0), (px + nx * side * len + dx / l * slide, py + ny * side * len + dy / l * slide));
    }
    segs(pm, chips.iter().copied(), 0x0B0C0E, a * 0.7, 1.1);
}
