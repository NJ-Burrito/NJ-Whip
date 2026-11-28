//! Drawing the short-lived stamps (shock, exclaim, ripple, lasso, slash, magic, bolt).

use super::*;
use std::f32::consts::{PI, TAU};
use tiny_skia::Path;

/// Mix colour `a` toward `b` by `t` (0 = a, 1 = b).
pub(super) fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |sh: u32| {
        let (x, y) = (((a >> sh) & 255) as f32, ((b >> sh) & 255) as f32);
        (x + (y - x) * t).round().clamp(0.0, 255.0) as u32
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// Many separate segments as one path (one stroke call).
pub(super) fn segs(pm: &mut Pixmap, list: impl Iterator<Item = (V, V)>, rgb: u32, a: f32, w: f32) {
    let mut pb = PathBuilder::new();
    for (p, q) in list {
        pb.move_to(p.0, p.1);
        pb.line_to(q.0, q.1);
    }
    if let Some(path) = pb.finish() {
        let st = Stroke { width: w, line_cap: LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Stroke::default() };
        pm.stroke_path(&path, &paint(rgb, a), &st, Transform::identity(), None);
    }
}

/// A blade-shaped stroke from `a` to `b`, pointed at both ends and `hw` wide at its belly, bowed
/// by `bend` and shifted sideways by `off`; only the part from `u0` to `u1` of the way is drawn.
pub(super) fn taper(pm: &mut Pixmap, a: V, b: V, bend: f32, hw: f32, (u0, u1): (f32, f32), off: f32, rgb: u32, al: f32) {
    const N: usize = 10;
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let (nx, ny) = (-dy / len, dx / len);
    let mut poly = [(0.0f32, 0.0f32); 2 * (N + 1)];
    for i in 0..=N {
        let v = i as f32 / N as f32;
        let u = u0 + (u1 - u0) * v;
        let w = hw * (PI * v).sin().max(0.0).powf(0.7);
        let sh = bend * len * u * (1.0 - u) + off;
        let (cx, cy) = (a.0 + dx * u + nx * sh, a.1 + dy * u + ny * sh);
        poly[i] = (cx + nx * w, cy + ny * w);
        poly[2 * N + 1 - i] = (cx - nx * w, cy - ny * w);
    }
    fill_poly(pm, &poly, rgb, al);
}

fn exclaim_bar() -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(-9.0, -34.0);
    pb.line_to(9.0, -34.0);
    pb.line_to(5.5, 6.0);
    pb.line_to(-5.5, 6.0);
    pb.close();
    pb.finish()
}

fn quad(pts: [V; 4]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    pb.finish()
}

pub(super) fn draw_stamp(pm: &mut Pixmap, s: &Stamp) {
    if let StampKind::Mark(effect) = s.kind {
        // a mark fades in steps (see MARK_FPS)
        let t = (mark_frame(s) as f32 / MARK_FPS / s.life).min(1.0);
        return draw_mark(pm, effect, s, 1.0 - t);
    }
    let t = (s.age / s.life).clamp(0.0, 1.0);
    let a = 1.0 - t;
    let (x, y) = s.at;
    match s.kind {
        StampKind::Mark(_) => {}
        StampKind::Shock => {
            for k in 0..3 {
                let ang = -0.6 + k as f32 * 0.6;
                let (r0, r1) = (18.0 + 30.0 * t, 34.0 + 40.0 * t);
                stroke_line(pm, &[(x + ang.cos() * r0, y - 20.0 + ang.sin() * r0), (x + ang.cos() * r1, y - 20.0 + ang.sin() * r1)], s.rgb, a, 3.0);
            }
        }
        StampKind::Exclaim => {
            let hold = (a * 2.2).min(1.0);
            let u = (t / 0.2).min(1.0);
            let back = 1.0 + 2.70158 * (u - 1.0).powi(3) + 1.70158 * (u - 1.0).powi(2);
            let sc = 0.5 + 0.5 * back;
            let wob = (t * 38.0).sin() * 7.0 * (1.0 - t) * (1.0 - t);
            let (cx, cy) = (x + 20.0, y - 54.0);
            // alarm rays round the glyph, flying outward and thinning
            let (r0, r1) = (36.0 + 6.0 * t, 45.0 - 6.0 * t);
            let rays = || {
                (0..8).map(move |k| {
                    let ang = k as f32 * TAU / 8.0 + 0.39;
                    ((cx + ang.cos() * r0, cy - 14.0 + ang.sin() * r0), (cx + ang.cos() * r1, cy - 14.0 + ang.sin() * r1))
                })
            };
            segs(pm, rays(), 0x4A0C0C, hold * 0.5, 5.5);
            segs(pm, rays(), mix(s.rgb, 0xFFB0A0, 0.35), hold * 0.95, 2.8);
            let tf = |dx: f32, dy: f32| Transform::from_translate(cx + dx, cy + dy).pre_rotate(-8.0 + wob).pre_scale(sc, sc);
            let (Some(bar), Some(dot)) = (exclaim_bar(), PathBuilder::from_circle(0.0, 17.0, 7.5)) else { return };
            let round = Stroke { width: 6.0, line_join: tiny_skia::LineJoin::Round, ..Stroke::default() };
            let edge = 0x4A0C0C;
            // contact shadow, dark outline, body, light and shade
            let sh = tf(5.0, 7.0);
            pm.stroke_path(&bar, &paint(0x000000, hold * 0.3), &round, sh, None);
            pm.fill_path(&bar, &paint(0x000000, hold * 0.3), FillRule::Winding, sh, None);
            pm.fill_path(&dot, &paint(0x000000, hold * 0.3), FillRule::Winding, sh, None);
            let t0 = tf(0.0, 0.0);
            pm.stroke_path(&bar, &paint(edge, hold), &round, t0, None);
            if let Some(c) = PathBuilder::from_circle(0.0, 17.0, 10.5) {
                pm.fill_path(&c, &paint(edge, hold), FillRule::Winding, t0, None);
            }
            pm.fill_path(&bar, &paint(s.rgb, hold), FillRule::Winding, t0, None);
            pm.fill_path(&dot, &paint(s.rgb, hold), FillRule::Winding, t0, None);
            if let Some(p) = quad([(-7.0, -31.0), (-2.0, -31.0), (-1.0, 2.0), (-4.5, 2.0)]) {
                pm.fill_path(&p, &paint(mix(s.rgb, 0xFFFFFF, 0.5), hold * 0.85), FillRule::Winding, t0, None);
            }
            if let Some(p) = quad([(4.0, -31.0), (7.5, -31.0), (4.5, 4.0), (2.0, 4.0)]) {
                pm.fill_path(&p, &paint(mix(s.rgb, 0x000000, 0.35), hold * 0.7), FillRule::Winding, t0, None);
            }
            if let Some(c) = PathBuilder::from_circle(-2.6, 14.4, 2.4) {
                pm.fill_path(&c, &paint(0xFFFFFF, hold * 0.75), FillRule::Winding, t0, None);
            }
        }
        StampKind::Ripple => {
            let hold = (a * 1.6).min(1.0);
            // a faint pool the rings spread over, and the dimple where the crack landed
            disc(pm, (x, y), 12.0 + t * 82.0, 0xBFD8FF, hold * 0.09);
            disc(pm, (x, y), 5.0 * (1.0 - t), 0x1B2030, hold * 0.5);
            for k in 0..3 {
                let r = 10.0 + (t * 80.0) - k as f32 * 18.0;
                if r > 2.0 {
                    let fade = (1.0 - r / 96.0).clamp(0.2, 1.0) * hold;
                    let w = (4.6 - r / 30.0).max(1.4);
                    hoop(pm, (x, y), r, 0xFFFFFF, fade * 0.35, w + 3.0);
                    hoop(pm, (x, y), r, s.rgb, fade * 0.85, w);
                    hoop(pm, (x, y), r, 0xBFE0FF, fade * 0.55, w * 0.35);
                    // glint on the upper left of the ring
                    stroke_line(pm, &arc((x, y), r, 1.0, PI * 1.1, PI * 1.45, 6), 0xFFFFFF, fade * 0.9, (w * 0.5).max(1.2));
                }
            }
        }
        StampKind::Lasso => {
            let hold = (a * 2.2).min(1.0);
            let draw = (t / 0.65).min(1.0);
            let cinch = 1.0 - 0.28 * ((t - 0.65) / 0.35).clamp(0.0, 1.0);
            let turns = 1.5 * draw + 0.05;
            const N: usize = 48;
            let mut pts = [(0.0f32, 0.0f32); N + 1];
            for (i, p) in pts.iter_mut().enumerate() {
                let u = i as f32 / N as f32 * turns * TAU;
                let r = 46.0 * (1.0 - 0.6 * (u / (1.5 * TAU))) * cinch;
                *p = (x + u.cos() * r * 1.4, y - 30.0 + u.sin() * r * 0.55);
            }
            // shadow, dark outline, rope body, twist ticks, highlight
            let low: [V; N + 1] = std::array::from_fn(|i| (pts[i].0 + 2.0, pts[i].1 + 6.0));
            stroke_line(pm, &low, 0x000000, hold * 0.28, 7.0);
            stroke_line(pm, &pts, 0x3A2A1C, hold, 7.0);
            stroke_line(pm, &pts, s.rgb, hold, 4.8);
            let ticks = pts.windows(2).map(|w| {
                let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
                let l = (dx * dx + dy * dy).sqrt().max(0.01);
                let (nx, ny) = (-dy / l, dx / l);
                let m = ((w[0].0 + w[1].0) * 0.5, (w[0].1 + w[1].1) * 0.5);
                ((m.0 - nx * 2.2 - dx / l, m.1 - ny * 2.2 - dy / l), (m.0 + nx * 2.2 + dx / l, m.1 + ny * 2.2 + dy / l))
            });
            segs(pm, ticks, mix(s.rgb, 0x000000, 0.45), hold * 0.75, 1.1);
            let up: [V; N + 1] = std::array::from_fn(|i| (pts[i].0 - 0.5, pts[i].1 - 1.2));
            stroke_line(pm, &up, mix(s.rgb, 0xFFFFFF, 0.55), hold * 0.5, 1.2);
            // the honda knot where the rope begins, and a short tail
            let k0 = pts[0];
            stroke_line(pm, &[k0, (k0.0 + 12.0, k0.1 - 9.0)], 0x3A2A1C, hold, 5.8);
            stroke_line(pm, &[k0, (k0.0 + 12.0, k0.1 - 9.0)], s.rgb, hold, 3.4);
            disc(pm, k0, 6.4, 0x3A2A1C, hold);
            disc(pm, k0, 4.8, mix(s.rgb, 0x000000, 0.1), hold);
            disc(pm, (k0.0 - 1.4, k0.1 - 1.4), 1.8, mix(s.rgb, 0xFFFFFF, 0.6), hold * 0.8);
        }
        StampKind::Slash => {
            let k = (t * 3.0).min(1.0);
            let s0 = ((t - 0.45) * 1.8).clamp(0.0, 0.95).min(k - 0.05).max(0.0);
            let hold = (a * 1.5).min(1.0);
            // glow, dark underside, the blade, two thin echoes and a glint at the tip
            taper(pm, s.at, s.to, 0.12, 15.0, (s0, k), 0.0, 0xBFE6FF, hold * 0.22);
            taper(pm, s.at, s.to, 0.12, 8.0, (s0, k), 3.0, 0x1E1F24, hold * 0.4);
            taper(pm, s.at, s.to, 0.12, 5.0, (s0, k), 0.0, s.rgb, hold);
            taper(pm, s.at, s.to, 0.12, 1.6, (s0 * 0.8, k * 0.8), 11.0, 0xBFE6FF, hold * 0.75);
            taper(pm, s.at, s.to, 0.12, 1.2, (s0 * 0.7, k * 0.65), -12.0, 0xFFFFFF, hold * 0.55);
            let (dx, dy) = (s.to.0 - s.at.0, s.to.1 - s.at.1);
            let tip = (s.at.0 + dx * k, s.at.1 + dy * k);
            let g = 12.0 * (1.0 - t) + 4.0;
            segs(pm, [((tip.0 - g, tip.1), (tip.0 + g, tip.1)), ((tip.0, tip.1 - g), (tip.0, tip.1 + g))].into_iter(), 0xFFFFFF, hold * 0.9, 1.8);
            disc(pm, tip, 3.2, 0xFFFFFF, hold);
        }
        StampKind::Magic => {
            let hold = (a * 1.8).min(1.0);
            let r = 40.0 + 20.0 * t;
            disc(pm, (x, y), r * 1.05, s.rgb, hold * 0.08);
            disc(pm, (x, y), r * 0.7, s.rgb, hold * 0.1);
            hoop(pm, (x, y), r, s.rgb, hold * 0.3, 6.0);
            hoop(pm, (x, y), r, mix(s.rgb, 0xFFFFFF, 0.35), hold, 2.2);
            hoop(pm, (x, y), r * 0.78, 0xFFD25A, hold * 0.6, 1.2);
            // rune ticks turning against the star
            let spin = -t * 90.0_f32.to_radians();
            let ticks = (0..16).map(|i| {
                let ang = spin + i as f32 * TAU / 16.0;
                let (a0, a1) = (r * 0.8, r * if i % 2 == 0 { 0.96 } else { 0.9 });
                ((x + ang.cos() * a0, y + ang.sin() * a0), (x + ang.cos() * a1, y + ang.sin() * a1))
            });
            segs(pm, ticks, mix(s.rgb, 0xFFFFFF, 0.5), hold * 0.8, 1.4);
            // the six-point star: two triangles, glowing under a thin gold line
            let rot = t * 120.0_f32.to_radians();
            for tri in 0..2 {
                let base = rot + tri as f32 * PI / 3.0;
                let pts: [V; 4] = std::array::from_fn(|i| {
                    let ang = base + i as f32 * TAU / 3.0;
                    (x + ang.cos() * r * 0.76, y + ang.sin() * r * 0.76)
                });
                stroke_line(pm, &pts, 0xB89CFF, hold * 0.3, 4.5);
                stroke_line(pm, &pts, 0xFFD25A, hold, 1.7);
            }
            for i in 0..6 {
                let ang = rot + i as f32 * TAU / 6.0;
                disc(pm, (x + ang.cos() * r * 0.76, y + ang.sin() * r * 0.76), 2.4, 0xFFFFFF, hold * 0.95);
            }
            disc(pm, (x, y), 3.0 + 2.0 * (1.0 - t), 0xFFFFFF, hold * 0.8);
        }
        StampKind::Bolt => {
            const N: usize = 12;
            let mut rng = fastrand::Rng::with_seed(((s.at.0 as u64) << 16) ^ s.at.1 as u64);
            let mut pts = [s.at; N + 1];
            for (i, p) in pts.iter_mut().enumerate().skip(1) {
                let u = i as f32 / N as f32;
                let amp = 30.0 * (PI * u).sin().max(0.0).powf(0.6);
                let jitter = (-1.0 + 2.0 * rng.f32()) * amp;
                *p = if i == N { s.to } else { (s.at.0 + (s.to.0 - s.at.0) * u + jitter, s.at.1 + (s.to.1 - s.at.1) * u) };
            }
            let flick = if (s.age * 30.0) as i32 % 2 == 0 { 1.0 } else { 0.65 };
            let hold = (a * 2.0).min(1.0) * flick;
            // two forks off the main stroke
            let mut forks = [[(0.0f32, 0.0f32); 3]; 2];
            for (f, &i) in forks.iter_mut().zip([4usize, 8].iter()) {
                let side = if rng.f32() < 0.5 { -1.0 } else { 1.0 };
                let dir = ((s.to.0 - s.at.0) * 0.2 + side * (28.0 + 20.0 * rng.f32()), (s.to.1 - s.at.1) * 0.12);
                let o = pts[i];
                *f = [o, (o.0 + dir.0 * 0.5 + side * 6.0, o.1 + dir.1 * 0.5), (o.0 + dir.0, o.1 + dir.1)];
            }
            stroke_line(pm, &pts, 0x6F8CFF, hold * 0.16, 22.0);
            stroke_line(pm, &pts, 0x2E3A58, hold * 0.5, 9.0);
            stroke_line(pm, &pts, 0xFFC400, hold * 0.6, 9.6);
            for f in &forks {
                stroke_line(pm, f, 0xFFC400, hold * 0.5, 6.0);
                stroke_line(pm, f, s.rgb, hold, 2.4);
            }
            stroke_line(pm, &pts, s.rgb, hold, 4.5);
            stroke_line(pm, &pts, 0xFFFFFF, hold, 1.8);
            // where it lands: a bright burst and a glare on the ground
            let (hx, hy) = s.at;
            oval(pm, (hx, hy + 6.0), 34.0 * (1.0 - 0.4 * t), 11.0, 0xFFE680, hold * 0.22);
            disc(pm, s.at, 15.0, 0xFFE680, hold * 0.35);
            let spokes = (0..8).map(|k| {
                let ang = k as f32 * TAU / 8.0 + 0.2;
                let (r0, r1) = (7.0, 20.0 + 14.0 * (k % 2) as f32);
                ((hx + ang.cos() * r0, hy + ang.sin() * r0), (hx + ang.cos() * r1, hy + ang.sin() * r1))
            });
            segs(pm, spokes, 0xFFFFFF, hold * 0.85, 1.8);
            disc(pm, s.at, 6.0, 0xFFFFFF, hold);
        }
    }
}
