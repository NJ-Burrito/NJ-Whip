//! Petals: the crack's burst and the mark it leaves.
//! Vine: leaves and a flower. A gentle burst: two-tone petals that turn over as they drift,
//! green leaves, and drifting pollen. The mark is three veined leaves round a five-petal flower.

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    let p = &mut fx.particles;
    // petals: they float out, then sink slowly
    let n0 = p.len();
    p.extend(ring(11, at, (80.0, 220.0), (0.7, 0.9), (6.5, 10.0), &[0xFF9EC0, 0xFFC2D6, 0xFF7FAA, 0xFFE0EA], Shape::Petal, 110.0));
    // leaves
    p.extend(ring(5, at, (60.0, 170.0), (0.7, 0.9), (6.0, 9.0), &[0x8FCB7E, 0x5FA84A, 0x6DB85A], Shape::Petal, 150.0));
    for q in &mut p[n0..] {
        q.drag = 0.95;
        q.spin = rnd(-300.0, 300.0);
    }
    // pollen: tiny gold motes that rise and hang
    let n1 = p.len();
    p.extend(ring(12, at, (30.0, 150.0), (0.5, 0.85), (1.2, 2.2), &[0xFFE9A8, 0xFFD25A, 0xFFF4C8], Shape::Dot, -25.0));
    for q in &mut p[n1..] {
        q.drag = 0.94;
    }
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    use std::f32::consts::{PI, TAU};
    let c @ (x, y) = s.at;
    let leaf = |pm: &mut Pixmap, ang: f32, len: f32, rgb: u32, al: f32| {
        let (tip, mid) = ((x + ang.cos() * len, y + ang.sin() * len), (x + ang.cos() * len * 0.5, y + ang.sin() * len * 0.5));
        let side = (-ang.sin() * len * 0.28, ang.cos() * len * 0.28);
        let mut pb = PathBuilder::new();
        pb.move_to(x, y);
        pb.quad_to(mid.0 + side.0, mid.1 + side.1, tip.0, tip.1);
        pb.quad_to(mid.0 - side.0, mid.1 - side.1, x, y);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(rgb, al), FillRule::Winding, Transform::identity(), None);
        }
        // the vein and two side veins
        stroke_line(pm, &[c, tip], 0x2F6B2A, al * 0.7, 1.2);
        for (f, sd) in [(0.4f32, 1.0f32), (0.4, -1.0), (0.65, 1.0), (0.65, -1.0)] {
            let b = (x + ang.cos() * len * f, y + ang.sin() * len * f);
            stroke_line(pm, &[b, (b.0 + (ang + sd * 0.8).cos() * len * 0.2, b.1 + (ang + sd * 0.8).sin() * len * 0.2)], 0x2F6B2A, al * 0.45, 0.9);
        }
    };
    // leaves: a darker one underneath each, shifted, then the lit leaf
    for k in 0..3 {
        let ang = -PI / 2.0 + (k as f32 - 1.0) * 0.9 + r(-0.15, 0.15);
        leaf(pm, ang + 0.06, 37.0, 0x3E7E34, a * 0.5);
        leaf(pm, ang, 36.0, if k == 1 { 0x6DB85A } else { 0x5FA84A }, a * 0.9);
    }
    // the flower: a shadow, five petals with a lit centre line, a gold heart
    let rot = r(0.0, TAU);
    for k in 0..5 {
        let ang = rot + k as f32 * TAU / 5.0;
        let pc = (x + ang.cos() * 7.0, y + ang.sin() * 7.0);
        oval(pm, (pc.0 + 0.8, pc.1 + 1.4), 5.2, 5.2, 0xA64A68, a * 0.35);
    }
    for k in 0..5 {
        let ang = rot + k as f32 * TAU / 5.0;
        let pc = (x + ang.cos() * 7.0, y + ang.sin() * 7.0);
        disc(pm, pc, 5.4, 0xFF9EC0, a);
        disc(pm, (pc.0 + ang.cos() * 1.5, pc.1 + ang.sin() * 1.5), 3.0, 0xFFC2D6, a * 0.8);
    }
    disc(pm, c, 4.6, 0xFFD25A, a);
    disc(pm, c, 2.6, 0xFFE9A8, a);
    disc(pm, (x - 1.0, y - 1.0), 0.9, 0xFFFFFF, a * 0.9);
}
