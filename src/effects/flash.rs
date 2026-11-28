//! Flash: the crack's burst and the mark it leaves.
//! Neon: a flash burnt into the screen, a white core with rays, strobing for its first second.
//! The burst is a clean light show: an overexposed core, evenly spaced rays and motes of light.

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    use std::f32::consts::TAU;
    let p = &mut fx.particles;
    fx.flash = Some(ScreenFlash { colours: vec![0xFFFFFF, 0x33D6FF, 0xFF5BD7, 0xFFFFFF, 0x33D6FF, 0xFF5BD7, 0xFFFFFF], alpha: 0.45, age: 0.0, life: 0.5 });
    // the overexposed core: a big pink bloom under a big cyan one under a white one
    for (size, life, rgb) in [(34.0, 0.16, 0xFF5BD7), (24.0, 0.2, 0x33D6FF), (14.0, 0.26, 0xFFFFFF)] {
        let mut c = ring(1, at, (0.0, 0.0), (life, life), (size, size), &[rgb], Shape::Star, 0.0);
        c[0].spin = 0.0;
        p.extend(c);
    }
    // sixteen rays, evenly spaced, alternating long and short, cyan and pink
    let n0 = p.len();
    p.extend(ring(16, at, (500.0, 900.0), (0.22, 0.42), (2.2, 3.4), &[0x33D6FF, 0xFFFFFF, 0xFF5BD7, 0xFFFFFF], Shape::Spark, 0.0));
    let rot = rnd(0.0, TAU);
    for (k, q) in p[n0..].iter_mut().enumerate() {
        let sp = q.vel.0.hypot(q.vel.1) * if k % 2 == 0 { 1.0 } else { 0.55 };
        let ang = rot + k as f32 * TAU / 16.0;
        q.vel = (ang.cos() * sp, ang.sin() * sp);
        q.drag = 0.93;
    }
    // twinkling stars
    p.extend(ring(8, at, (90.0, 240.0), (0.35, 0.55), (4.0, 7.0), &[0xFFFFFF, 0x9BEFFF, 0xFFB3EE], Shape::Star, 0.0));
    // motes of light that drift and go out
    let n1 = p.len();
    p.extend(ring(12, at, (60.0, 220.0), (0.35, 0.55), (2.2, 3.8), &[0x33D6FF, 0xFF5BD7, 0xFFFFFF], Shape::Dot, 0.0));
    for q in &mut p[n1..] {
        q.drag = 0.95;
    }
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    use std::f32::consts::TAU;
    let c @ (x, y) = s.at;
    let a = if s.age < 1.0 && mark_frame(s) % 2 == 1 { a * 0.55 } else { a };
    let rot = r(0.0, TAU);
    // a burnt halo, then a thin ring the rays stand on
    disc(pm, c, 30.0, 0x33D6FF, a * 0.08);
    disc(pm, c, 22.0, 0xFF5BD7, a * 0.1);
    hoop(pm, c, 19.0, 0x33D6FF, a * 0.55, 1.4);
    for k in 0..12 {
        let ang = rot + k as f32 * TAU / 12.0;
        let (len, rgb) = if k % 2 == 0 { (r(44.0, 56.0), if k % 4 == 0 { 0x33D6FF } else { 0xFF5BD7 }) } else { (r(20.0, 28.0), 0xFFFFFF) };
        let ray = [(x + ang.cos() * 8.0, y + ang.sin() * 8.0), (x + ang.cos() * len, y + ang.sin() * len)];
        stroke_line(pm, &ray, rgb, a * 0.22, 9.0);
        stroke_line(pm, &ray, rgb, a * 0.35, 5.0);
        stroke_line(pm, &ray, rgb, a * 0.95, 2.2);
        stroke_line(pm, &ray, 0xFFFFFF, a * 0.7, 0.9);
        // a dot at the end of the long rays, like a lit tip
        if k % 2 == 0 {
            disc(pm, (x + ang.cos() * (len + 3.0), y + ang.sin() * (len + 3.0)), 1.8, 0xFFFFFF, a * 0.9);
        }
    }
    disc(pm, c, 16.0, 0x33D6FF, a * 0.25);
    disc(pm, c, 10.0, 0xFFFFFF, a * 0.6);
    disc(pm, c, 5.5, 0xFFFFFF, a);
}
