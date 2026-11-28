//! Confetti: the crack's burst and the mark it leaves.
//! Cheerleader: a star. A party popper: a bright pop, tumbling paper chips, curling streamers,
//! gold stars and sequins, all falling slowly.

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    let p = &mut fx.particles;
    const PAPER: [u32; 6] = [0xFF8CC6, 0xFFD25A, 0x7FD4FF, 0x8FE3A0, 0xFFFFFF, 0xC59CFF];
    // the pop
    let mut pop = ring(1, at, (0.0, 0.0), (0.12, 0.12), (13.0, 13.0), &[0xFFF2A8], Shape::Star, 0.0);
    pop[0].spin = 0.0;
    p.extend(pop);
    // paper: thrown fast, then slows to a drift and tumbles down
    let n0 = p.len();
    p.extend(ring(40, at, (260.0, 680.0), (0.6, 0.9), (3.0, 5.0), &PAPER, Shape::Chip, 560.0));
    for q in &mut p[n0..] {
        q.drag = 0.935;
        q.spin = rnd(-260.0, 260.0);
    }
    // streamers: long thin ribbons that trail and fall
    let n1 = p.len();
    p.extend(ring(8, at, (300.0, 600.0), (0.55, 0.85), (1.8, 2.6), &PAPER, Shape::Spark, 700.0));
    for q in &mut p[n1..] {
        q.drag = 0.94;
    }
    // stars and sequins
    p.extend(ring(5, at, (60.0, 200.0), (0.45, 0.7), (5.0, 8.0), &[0xFFF2A8, 0xFFD25A], Shape::Star, 60.0));
    let n2 = p.len();
    p.extend(ring(10, at, (150.0, 460.0), (0.5, 0.8), (1.4, 2.4), &[0xFFFFFF, 0xFFF2A8, 0x7FD4FF], Shape::Dot, 400.0));
    for q in &mut p[n2..] {
        q.drag = 0.94;
    }
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    use std::f32::consts::{PI, TAU};
    let (x, y) = s.at;
    let rot = r(0.0, TAU);
    let star = |scale: f32, dx: f32, dy: f32| -> Vec<V> {
        let mut v: Vec<V> = (0..10).map(|k| {
            let (ang, rad) = (rot + k as f32 * PI / 5.0, if k % 2 == 0 { 32.0 * scale } else { 14.0 * scale });
            (x + dx + ang.cos() * rad, y + dy + ang.sin() * rad)
        }).collect();
        v.push(v[0]);
        v
    };
    // a soft shadow, a gold body with a lighter core, a pink outline and a white glint
    fill_poly(pm, &star(1.0, 1.5, 2.5), 0x0A1024, a * 0.35);
    fill_poly(pm, &star(1.0, 0.0, 0.0), 0xFFD25A, a * 0.85);
    fill_poly(pm, &star(0.6, 0.0, 0.0), 0xFFF2A8, a * 0.7);
    stroke_line(pm, &star(1.0, 0.0, 0.0), 0xFF8CC6, a, 3.0);
    disc(pm, (x - 4.0, y - 4.0), 2.4, 0xFFFFFF, a * 0.9);
    // paper bits that landed beside it
    const BITS: [u32; 4] = [0xFF8CC6, 0x7FD4FF, 0x8FE3A0, 0xFFD25A];
    for k in 0..6 {
        let ang = r(0.0, TAU);
        let d = r(38.0, 54.0);
        let (px, py) = (x + ang.cos() * d, y + ang.sin() * d * 0.6);
        let tilt = r(-1.2, 1.2);
        let (ux, uy) = (tilt.cos() * 3.0, tilt.sin() * 3.0);
        stroke_line(pm, &[(px - ux, py - uy), (px + ux, py + uy)], BITS[k % 4], a * 0.9, 2.4);
    }
}
