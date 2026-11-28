//! Splash: the crack's burst and the mark it leaves.
//! Sauce: a puddle. Thick red sauce: a few heavy blobs, many drops that stretch as they fly,
//! a fine mist, and a pinch of herbs. The mark is a glossy puddle with splatter round it.

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    let p = &mut fx.particles;
    const SAUCE: [u32; 3] = [0xD93A22, 0xB52A15, 0xFF6A4D];
    // heavy blobs: slow, big, they fall hard
    let n0 = p.len();
    p.extend(ring(6, at, (90.0, 270.0), (0.5, 0.75), (5.0, 7.5), &SAUCE, Shape::Drop, 1100.0));
    for q in &mut p[n0..] {
        q.drag = 0.95;
    }
    // drops: thrown fast, they stretch along their flight
    let n1 = p.len();
    p.extend(ring(16, at, (230.0, 560.0), (0.45, 0.7), (2.4, 4.0), &SAUCE, Shape::Drop, 950.0));
    for q in &mut p[n1..] {
        q.drag = 0.95;
    }
    // mist
    let n2 = p.len();
    p.extend(ring(14, at, (260.0, 560.0), (0.2, 0.4), (1.0, 1.8), &[0xFF6A4D, 0xFFB3A0], Shape::Dot, 300.0));
    for q in &mut p[n2..] {
        q.drag = 0.92;
    }
    p.extend(ring(4, at, (20.0, 60.0), (0.5, 0.7), (3.0, 4.0), &[0x5FA84A, 0xFFF3E0], Shape::Chip, 500.0));
    fx.flash = Some(ScreenFlash { colours: vec![0xFFC27A], alpha: 0.12, age: 0.0, life: 0.45 });
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    use std::f32::consts::TAU;
    let (x, y) = s.at;
    // splatter first: thin streaks out from the puddle, each ending in a drop
    for k in 0..7 {
        let ang = k as f32 * TAU / 7.0 + r(-0.35, 0.35);
        let (inner, mid, outer) = (r(30.0, 36.0), r(38.0, 46.0), r(48.0, 57.0));
        let (c, sn) = (ang.cos(), ang.sin() * 0.45);
        let at = |d: f32| (x + c * d, y + 6.0 + sn * d);
        stroke_line(pm, &[at(inner), at(mid)], 0xB52A15, a * 0.9, 3.0);
        stroke_line(pm, &[at(mid), at(outer)], 0xB52A15, a * 0.8, 1.2);
        disc(pm, at(outer + 3.0), r(1.2, 2.4), 0xD93A22, a * 0.9);
    }
    // the puddle: a dark edge, the sauce, a glossy highlight and a bright rim
    oval(pm, (x + 1.0, y + 9.0), 40.0, 16.0, 0x6E160B, a * 0.5);
    oval(pm, (x, y + 6.0), 40.0, 16.0, 0xB52A15, a * 0.9);
    oval(pm, (x - 4.0, y + 4.0), 31.0, 11.5, 0xD93A22, a);
    oval(pm, (x + 6.0, y + 7.0), 18.0, 5.0, 0xFF6A4D, a * 0.45);
    for (dx, dy, rad) in [(38.0, 12.0, 7.0), (-36.0, 2.0, 5.0), (12.0, 22.0, 6.0)] {
        disc(pm, (x + dx, y + dy + 1.5), rad, 0x6E160B, a * 0.45);
        disc(pm, (x + dx, y + dy), rad, 0xD93A22, a);
        disc(pm, (x + dx - rad * 0.3, y + dy - rad * 0.3), rad * 0.3, 0xFFD3C8, a * 0.8);
    }
    oval(pm, (x - 12.0, y + 1.0), 11.0, 3.0, 0xFFB3A0, a * 0.7);
    // a herb flake riding on top
    oval(pm, (x + 14.0, y + 5.0), 3.0, 1.8, 0x5FA84A, a * 0.9);
}
