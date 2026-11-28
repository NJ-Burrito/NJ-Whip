//! Sparks: the crack's burst and the mark it leaves.
//! Classic: the lash's scar. A struck-steel burst: a white-hot flash, long streaks that die fast,
//! heavier embers that arc down, and a few glints.

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    let p = &mut fx.particles;
    // the flash the crack makes where it lands
    let mut flash = ring(1, at, (0.0, 0.0), (0.1, 0.1), (13.0, 13.0), &[0xFFFFFF], Shape::Star, 0.0);
    flash[0].spin = 0.0;
    p.extend(flash);
    // long hot streaks, thrown wide, barely falling
    let n0 = p.len();
    p.extend(ring(12, at, (560.0, 960.0), (0.14, 0.26), (2.2, 3.2), &[0xFFFFFF, 0xBFE6FA, 0xFFFFFF, 0x7FC4EC], Shape::Spark, 120.0));
    for q in &mut p[n0..] {
        q.drag = 0.95;
    }
    // embers: slower, bigger, they arc down
    let n1 = p.len();
    p.extend(ring(16, at, (160.0, 420.0), (0.22, 0.32), (1.6, 2.6), &[0x7FC4EC, 0x2F86C9, 0xA8DCF5], Shape::Spark, 620.0));
    for q in &mut p[n1..] {
        q.drag = 0.93;
    }
    // glints that hang in the air for a moment
    p.extend(ring(7, at, (30.0, 150.0), (0.2, 0.3), (1.2, 2.2), &[0xFFFFFF, 0xBFE6FA], Shape::Dot, 0.0));
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    let (x, y) = s.at;
    let ang = r(-0.7, 0.7);
    let (dx, dy) = (ang.cos() * 40.0, ang.sin() * 40.0);
    let pts = [(x - dx, y - dy), (x - dx * 0.3, y - dy * 0.3 - 5.0), (x + dx * 0.4, y + dy * 0.4 + 4.0), (x + dx, y + dy)];
    // scorched metal around the cut, then the cut: dark lip, blue glow, white-hot line
    oval(pm, (x, y + 1.0), 38.0, 10.0, 0x070B18, a * 0.3);
    stroke_line(pm, &pts, 0x070B18, a * 0.5, 9.0);
    stroke_line(pm, &pts, 0x0E1730, a * 0.8, 6.0);
    stroke_line(pm, &pts, 0x2F86C9, a * 0.5, 3.6);
    stroke_line(pm, &pts, 0x7FC4EC, a * 0.95, 2.0);
    stroke_line(pm, &pts[1..3], 0xFFFFFF, a * 0.8, 0.9);
    // two hairline cracks branching off, and pits where sparks bit in
    for (k, side) in [(1usize, -1.0f32), (2, 1.0)] {
        let (bx, by) = pts[k];
        let off = r(14.0, 22.0);
        let br = [(bx, by), (bx + side * off * 0.4, by + side * off), (bx + side * off * 0.5, by + side * off * 1.7)];
        stroke_line(pm, &br, 0x0E1730, a * 0.7, 2.6);
        stroke_line(pm, &br, 0x7FC4EC, a * 0.7, 1.0);
    }
    for _ in 0..5 {
        let (px, py) = (x + r(-34.0, 34.0), y + r(-14.0, 14.0));
        disc(pm, (px, py), r(0.9, 1.7), 0xA8DCF5, a * 0.8);
    }
}
