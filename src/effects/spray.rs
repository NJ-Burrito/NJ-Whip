//! Spray: the crack's burst and the mark it leaves.
//! Pirate: X marks the spot. Sea water: a fan of clear drops, a fine mist that hangs, and puffs
//! of foam. The mark is a red X painted on, with a wet ring and drips.

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    use std::f32::consts::PI;
    let p = &mut fx.particles;
    // clear drops, thrown up and out in a fan
    let n0 = p.len();
    p.extend(fan(20, at, (-PI + 0.2, -0.2), (260.0, 620.0), (0.45, 0.7), (1.8, 3.0), &[0xCFEFFF, 0xFFFFFF, 0x7FC8F0, 0xA8DCF5], Shape::Drop, 800.0));
    for q in &mut p[n0..] {
        q.drag = 0.94;
    }
    // mist: hundreds of tiny drops, in practice a few dozen, that hang and sink
    let n1 = p.len();
    p.extend(ring(22, at, (100.0, 380.0), (0.3, 0.6), (0.9, 1.7), &[0xFFFFFF, 0xCFEFFF], Shape::Dot, 160.0));
    for q in &mut p[n1..] {
        q.drag = 0.9;
    }
    // foam at the base
    let n2 = p.len();
    p.extend(ring(6, at, (40.0, 120.0), (0.4, 0.7), (7.0, 11.0), &[0xDFF3FF, 0xBFD9EC], Shape::Puff, -15.0));
    for q in &mut p[n2..] {
        q.drag = 0.9;
    }
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    use std::f32::consts::TAU;
    let (x, y) = s.at;
    // a wet ring round the spot, with drops that bounced off it
    hoop(pm, (x, y + 4.0), 40.0, 0x7FC8F0, a * 0.3, 1.6);
    hoop(pm, (x, y + 4.0), 34.0, 0xCFEFFF, a * 0.18, 1.0);
    for k in 0..8 {
        let ang = k as f32 * TAU / 8.0 + r(-0.2, 0.2);
        disc(pm, (x + ang.cos() * r(40.0, 54.0), y + 4.0 + ang.sin() * r(34.0, 48.0)), r(1.2, 2.2), 0xCFEFFF, a * 0.7);
    }
    // the X: a shadow under it, dark red paint, bright red on top, a pale highlight along each stroke
    for sign in [-1.0, 1.0] {
        let line = [(x - 24.0, y - 24.0 * sign), (x + 24.0, y + 24.0 * sign)];
        let shifted = [(line[0].0 + 1.5, line[0].1 + 2.5), (line[1].0 + 1.5, line[1].1 + 2.5)];
        stroke_line(pm, &shifted, 0x0A1024, a * 0.4, 10.0);
        stroke_line(pm, &line, 0x5A120E, a * 0.8, 10.0);
        stroke_line(pm, &line, 0xC0271E, a * 0.95, 6.5);
        let hi = [(line[0].0 - 1.0, line[0].1 - 1.5 * sign), (line[1].0 - 1.0, line[1].1 - 1.5 * sign)];
        stroke_line(pm, &hi, 0xFF7A66, a * 0.5, 1.5);
    }
    // paint drips under the ends of the strokes
    for (dx, len) in [(-24.0, 9.0), (24.0, 13.0), (0.0, 7.0)] {
        let top = y + if dx == 0.0 { 3.0 } else { 26.0 };
        stroke_line(pm, &[(x + dx, top), (x + dx, top + len)], 0xC0271E, a * 0.9, 2.2);
        disc(pm, (x + dx, top + len + 1.0), 2.0, 0xC0271E, a * 0.9);
    }
}
