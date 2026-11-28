//! Sizzle: the crack's burst and the mark it leaves.
//! Chef: a fried egg. Hot oil: orange sparks that pop and arc, golden oil drops, bubbles that
//! rise and burst, and steam curling up. The mark is a fried egg with a crisp edge.

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    use std::f32::consts::PI;
    let p = &mut fx.particles;
    // oil pops: spatter thrown up and out, it arcs back down
    let n0 = p.len();
    p.extend(fan(14, at, (-PI + 0.15, -0.15), (200.0, 480.0), (0.2, 0.4), (1.5, 2.5), &[0xFF9A3C, 0xFFFFFF, 0xFFD25A, 0xFF7A1F], Shape::Spark, 520.0));
    for q in &mut p[n0..] {
        q.drag = 0.94;
    }
    // golden drops of oil
    let n1 = p.len();
    p.extend(fan(8, at, (-PI + 0.3, -0.3), (140.0, 340.0), (0.3, 0.5), (1.8, 2.8), &[0xFFC04A, 0xE8902A, 0xFFD98A], Shape::Drop, 800.0));
    for q in &mut p[n1..] {
        q.drag = 0.95;
    }
    // bubbles drift up and pop
    let n2 = p.len();
    p.extend(fan(8, at, (-PI + 0.4, -0.4), (20.0, 90.0), (0.4, 0.6), (2.0, 3.5), &[0xFFE9A8, 0xFFFFFF], Shape::Dot, -120.0));
    for q in &mut p[n2..] {
        q.drag = 0.95;
    }
    // steam: wisps that rise and spread
    let n3 = p.len();
    p.extend(fan(7, at, (-PI + 0.7, -0.7), (20.0, 80.0), (0.6, 0.85), (6.0, 10.0), &[0xFFFFFF, 0xF0F0F0], Shape::Puff, -420.0));
    for q in &mut p[n3..] {
        q.drag = 0.96;
        q.grow = 6.0;
    }
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    let c @ (x, y) = s.at;
    // a shadow and a sheen of oil, then the white with a browned, crisp edge
    oval(pm, (x + 1.0, y + 5.0), 36.0, 26.0, 0x0A1024, a * 0.3);
    oval(pm, (x, y + 2.0), 40.0, 29.0, 0xFFC04A, a * 0.16);
    let blobs = [(c, 30.0, 22.0), ((x + 22.0, y + 10.0), 12.0, 12.0), ((x - 20.0, y - 8.0), 11.0, 11.0)];
    for (c, rx, ry) in blobs {
        oval(pm, c, rx + 2.0, ry + 2.0, 0xD9A24A, a * 0.55);
    }
    for (c, rx, ry) in blobs {
        oval(pm, c, rx, ry, 0xFFFDF5, a * 0.95);
    }
    // a soft shade on the lower right of the white
    oval(pm, (x + 8.0, y + 8.0), 16.0, 8.0, 0xE9E2CF, a * 0.45);
    // little bubbles in the white
    for _ in 0..4 {
        let (bx, by) = (x + r(-22.0, 26.0), y + r(-14.0, 18.0));
        disc(pm, (bx, by), r(1.0, 1.8), 0xE9E2CF, a * 0.7);
    }
    // the yolk: dark rim, body, glossy highlight
    disc(pm, (x + 3.0, y - 1.0), 12.0, 0xD98A0A, a);
    disc(pm, (x + 3.0, y - 1.0), 11.0, 0xFFB21E, a);
    disc(pm, (x + 5.0, y + 1.0), 6.5, 0xF59A0C, a * 0.5);
    disc(pm, (x - 1.0, y - 5.0), 3.2, 0xFFE9A8, a);
    disc(pm, (x - 2.5, y - 6.0), 1.2, 0xFFFFFF, a);
}
