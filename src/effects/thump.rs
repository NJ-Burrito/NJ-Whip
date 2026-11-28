//! Thump: the crack's burst and the mark it leaves.
//! Elephant: a cracked dent. A heavy burst: dust rolling out along the ground, chunks of rock
//! thrown up that fall hard, and the shock marks.

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    use std::f32::consts::PI;
    let p = &mut fx.particles;
    let st = &mut fx.stamps;
    let stamp = |kind, to: V, life: f32, rgb: u32| Stamp { kind, at, to, age: 0.0, life, rgb };
    // dust rolls out sideways, low and wide, in two sizes
    let n0 = p.len();
    const DUST: [u32; 3] = [0x9A9286, 0x7E776D, 0xB8B0A2];
    p.extend(fan(6, at, (-0.3, 0.0), (160.0, 320.0), (0.45, 0.7), (6.0, 10.0), &DUST, Shape::Puff, -20.0));
    p.extend(fan(6, at, (PI, PI + 0.3), (160.0, 320.0), (0.45, 0.7), (6.0, 10.0), &DUST, Shape::Puff, -20.0));
    p.extend(ring(3, at, (20.0, 60.0), (0.5, 0.7), (13.0, 18.0), &[0x8A8378, 0xA89F91], Shape::Puff, -50.0));
    for q in &mut p[n0..] {
        q.drag = 0.955;
    }
    // rock chunks: thrown up, then they fall hard
    let n1 = p.len();
    p.extend(fan(9, at, (-2.5, -0.65), (220.0, 480.0), (0.4, 0.7), (4.0, 7.0), &[0x8A8378, 0x6B655C, 0xA39A8C, 0x7A7367], Shape::Chip, 1300.0));
    for q in &mut p[n1..] {
        q.drag = 0.96;
        q.spin *= 0.7;
    }
    // grit: fine dust thrown up
    p.extend(fan(10, at, (-2.8, -0.35), (120.0, 320.0), (0.3, 0.55), (1.0, 1.8), &[0xB8B0A2, 0x9A9286], Shape::Dot, 500.0));
    st.push(stamp(StampKind::Shock, at, 0.25, 0x454950));
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, r: &mut dyn FnMut(f32, f32) -> f32) {
    use std::f32::consts::TAU;
    let c @ (x, y) = s.at;
    // dust settled round the rim, a lit rim, a deep dark bowl with a darker floor
    oval(pm, (x, y + 3.0), 46.0, 20.0, 0x8A8378, a * 0.18);
    oval(pm, (x, y + 2.0), 38.0, 17.5, 0x15171B, a * 0.4);
    oval(pm, c, 37.0, 16.5, 0xB8B0A2, a * 0.55);
    oval(pm, (x, y + 1.5), 34.0, 14.0, 0x2A2C31, a * 0.8);
    oval(pm, (x + 2.0, y + 4.0), 26.0, 8.5, 0x15171B, a * 0.6);
    oval(pm, (x - 4.0, y - 6.0), 20.0, 2.6, 0xB8B0A2, a * 0.28);
    for k in 0..6 {
        let ang = k as f32 * TAU / 6.0 + r(-0.3, 0.3);
        let len = r(40.0, 56.0);
        let ray = [(x + ang.cos() * 20.0, y + ang.sin() * 8.0), (x + ang.cos() * len * 0.6, y + ang.sin() * len * 0.3 + r(-2.0, 2.0)), (x + ang.cos() * len, y + ang.sin() * len * 0.45)];
        stroke_line(pm, &ray, 0xB8B0A2, a * 0.5, 4.5);
        stroke_line(pm, &ray, 0x2A2C31, a * 0.85, 2.5);
    }
    // thrown pebbles
    for _ in 0..5 {
        let ang = r(0.0, TAU);
        let (px, py) = (x + ang.cos() * r(40.0, 55.0), y + ang.sin() * r(14.0, 24.0));
        let rad = r(1.5, 3.0);
        disc(pm, (px + 0.8, py + 1.2), rad, 0x15171B, a * 0.5);
        disc(pm, (px, py), rad, 0x6B655C, a * 0.9);
    }
}
