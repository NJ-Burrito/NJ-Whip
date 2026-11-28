//! Snow: the crack's burst and the mark it leaves.
//! Penguin: a puddle of snow that gathers itself into a snowball. A powder burst: soft puffs of
//! powder, fine flakes that drift down, and a few large six-armed crystals that turn slowly.

use super::*;

pub(super) fn burst(fx: &mut Fx, at: V, _screen: V) {
    let p = &mut fx.particles;
    // powder: soft puffs thrown up and out
    let n0 = p.len();
    p.extend(ring(6, at, (40.0, 130.0), (0.45, 0.7), (8.0, 13.0), &[0xF4F8FF, 0xDCE8F8, 0xFFFFFF], Shape::Puff, 10.0));
    for q in &mut p[n0..] {
        q.drag = 0.92;
    }
    // flakes: many, small, they drift down
    let n1 = p.len();
    p.extend(ring(24, at, (70.0, 260.0), (0.6, 0.9), (1.4, 2.8), &[0xFFFFFF, 0xE6F2FF, 0xCFE4FA], Shape::Dot, 150.0));
    for q in &mut p[n1..] {
        q.drag = 0.935;
    }
    // crystals: big, slow, turning
    let n2 = p.len();
    p.extend(ring(5, at, (40.0, 150.0), (0.7, 0.9), (4.5, 7.5), &[CRYSTAL], Shape::Star, 90.0));
    for q in &mut p[n2..] {
        q.drag = 0.95;
        q.spin = rnd(-160.0, 160.0);
    }
}

/// What this crack leaves where it landed, at strength `a` (see `draw_mark`).
pub(super) fn mark(pm: &mut Pixmap, s: &Stamp, a: f32, _r: &mut dyn FnMut(f32, f32) -> f32) {
    let (x, y) = s.at;
    let k = ((s.age / s.life - 0.2) / 0.45).clamp(0.0, 1.0);
    let k = k * k * (3.0 - 2.0 * k);
    let ground = y + 12.0;
    let (rx, ry) = (40.0 + (21.0 - 40.0) * k, 8.0 + (21.0 - 8.0) * k);
    let ball = (x, ground - ry);
    // a contact shadow under it all, then what is left of the puddle, shrinking as the ball takes the snow
    oval(pm, (x + 2.0, ground + 2.0), 46.0 - 20.0 * k, 8.0 - 3.0 * k, 0x1B2D52, a * 0.35);
    oval(pm, (x, ground), 44.0 - 18.0 * k, 7.0 - 3.0 * k, 0xB9CCE6, a * (0.7 - 0.35 * k));
    oval(pm, (x, ground - 1.5), 40.0 - 16.0 * k, 4.5 - 2.0 * k, 0xE6F0FC, a * (0.6 - 0.3 * k));
    oval(pm, ball, rx, ry, 0xF4F8FF, a * 0.97);
    // a cool rim on the lower edge, shade on the lower right and a highlight on the upper left, stronger as it rounds up
    oval(pm, (ball.0 + rx * 0.08, ball.1 + ry * 0.12), rx * 0.96, ry * 0.9, 0xC3D5EE, a * 0.3 * k);
    oval(pm, ball, rx * 0.93, ry * 0.88, 0xF4F8FF, a * 0.97 * k);
    oval(pm, (ball.0 + rx * 0.22, ball.1 + ry * 0.3), rx * 0.62, ry * 0.5, 0xD3E1F4, a * 0.35 * k);
    disc(pm, (ball.0 - rx * 0.35, ball.1 - ry * 0.4), 3.0 + 3.5 * k, 0xFFFFFF, a * 0.9);
    for (dx, rad) in [(-48.0, 3.5), (47.0, 3.0), (30.0, 2.0)] {
        disc(pm, (x + dx * (1.0 - 0.6 * k), ground - 1.0), rad * (1.0 - 0.5 * k), 0xF4F8FF, a * 0.9);
    }
    // sparkles on the snow
    for (dx, dy) in [(-16.0, -3.0), (18.0, 1.0), (4.0, -8.0)] {
        disc(pm, (x + dx * (1.0 - 0.5 * k), ground + dy * (1.0 - k) + dy * 0.2 * k), 0.9, 0xFFFFFF, a * 0.85);
    }
}
