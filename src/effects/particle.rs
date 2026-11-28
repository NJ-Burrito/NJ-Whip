//! Drawing one particle. Every shape is built from a few soft layers (glow, shade, body,
//! highlight) so a burst has depth, and the flat ones flip and sway as they fall.

use super::*;
use std::f32::consts::PI;

/// A Star in this colour is drawn as a six-armed snow crystal (Snow's big flakes).
pub(super) const CRYSTAL: u32 = 0xEAF4FF;

/// Like `ring`, but the particles leave inside the angle range `arc` (radians, 0 = right, up is negative).
pub(super) fn fan(n: usize, at: V, arc: V, speed: V, life: V, size: V, colours: &[u32], shape: Shape, grav: f32) -> Vec<Particle> {
    (0..n)
        .map(|k| {
            let a = rnd(arc.0, arc.1);
            let s = rnd(speed.0, speed.1);
            Particle {
                pos: at,
                vel: (a.cos() * s, a.sin() * s),
                grav,
                drag: 0.9,
                age: 0.0,
                life: rnd(life.0, life.1),
                size: rnd(size.0, size.1),
                grow: 0.0,
                rot: rnd(0.0, 360.0),
                spin: rnd(-720.0, 720.0),
                rgb: colours[k % colours.len()],
                shape,
            }
        })
        .collect()
}

/// `rgb` moved toward white (`k` > 0) or black (`k` < 0).
pub(super) fn shade(rgb: u32, k: f32) -> u32 {
    let m = |c: u32| {
        let c = c as f32;
        let v = if k >= 0.0 { c + (255.0 - c) * k } else { c * (1.0 + k) };
        v.round().clamp(0.0, 255.0) as u32
    };
    (m(rgb >> 16 & 255) << 16) | (m(rgb >> 8 & 255) << 8) | m(rgb & 255)
}

fn fill(pm: &mut Pixmap, path: Option<tiny_skia::Path>, rgb: u32, a: f32, tf: Transform) {
    if let Some(p) = path {
        pm.fill_path(&p, &paint(rgb, a), FillRule::Winding, tf, None);
    }
}

fn petal_path(s: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, 0.0);
    pb.cubic_to(s * 0.6, -s * 0.7, s * 1.6, -s * 0.6, s * 2.0, 0.0);
    pb.cubic_to(s * 1.6, s * 0.6, s * 0.6, s * 0.7, 0.0, 0.0);
    pb.finish()
}

fn drop_path(s: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, -s * 1.6);
    pb.cubic_to(s * 0.9, -s * 0.4, s, s * 0.5, 0.0, s);
    pb.cubic_to(-s, s * 0.5, -s * 0.9, -s * 0.4, 0.0, -s * 1.6);
    pb.finish()
}

pub(super) fn draw_particle(pm: &mut Pixmap, q: &Particle) {
    let t = q.age / q.life;
    let a = (1.0 - t).clamp(0.0, 1.0);
    // flat pieces stay solid, then fade over the last third
    let hold = ((1.0 - t) / 0.35).clamp(0.0, 1.0);
    let (x, y) = q.pos;
    let s = q.size;
    let ph = q.rot.to_radians();
    match q.shape {
        Shape::Dot => {
            let tw = 0.85 + 0.15 * (q.age * 20.0 + ph).sin();
            disc(pm, q.pos, s * 2.0, q.rgb, hold * 0.09);
            disc(pm, q.pos, s * tw, q.rgb, hold);
        }
        Shape::Puff => {
            // a soft ball: haze, body, a shaded lower right, a highlight
            let r = s * (1.0 + 1.2 * t);
            let al = a * 0.6;
            disc(pm, q.pos, r * 1.25, q.rgb, al * 0.15);
            disc(pm, q.pos, r, q.rgb, al);
            disc(pm, (x + r * 0.2, y + r * 0.26), r * 0.68, shade(q.rgb, -0.22), al * 0.45);
            disc(pm, (x - r * 0.3, y - r * 0.34), r * 0.42, shade(q.rgb, 0.45), al * 0.7);
        }
        Shape::Chip => {
            // tumbles: the face narrows to an edge and the back is darker
            let f = ph.cos();
            let k = f.abs().max(0.14);
            let tf = Transform::from_translate(x, y).pre_rotate(q.rot * 0.3).pre_scale(1.0, k);
            let body = if f >= 0.0 { q.rgb } else { shade(q.rgb, -0.3) };
            fill(pm, Rect::from_xywh(-s, -s * 0.5, s * 2.0, s).map(PathBuilder::from_rect), body, hold, tf);
            if f > 0.35 {
                fill(pm, Rect::from_xywh(-s, -s * 0.5, s * 2.0, s * 0.3).map(PathBuilder::from_rect), shade(q.rgb, 0.5), hold * 0.5, tf);
            }
        }
        Shape::Petal => {
            // two-tone, with a vein; turns over and sways as it drifts
            let f = ph.cos();
            let k = 0.4 + 0.6 * f.abs();
            let sway = (q.age * 5.0 + ph).sin() * s * 0.5;
            let tf = Transform::from_translate(x + sway, y).pre_rotate(q.rot * 0.5).pre_scale(1.0, k);
            let body = if f >= 0.0 { q.rgb } else { shade(q.rgb, -0.16) };
            fill(pm, petal_path(s), body, hold, tf);
            fill(pm, petal_path(s * 0.66), shade(q.rgb, 0.4), hold * 0.5, tf.pre_translate(s * 0.35, 0.0));
            let mut pb = PathBuilder::new();
            pb.move_to(s * 0.15, 0.0);
            pb.line_to(s * 1.7, 0.0);
            if let Some(v) = pb.finish() {
                pm.stroke_path(&v, &paint(shade(q.rgb, -0.35), hold * 0.45), &Stroke { width: 0.9, ..Stroke::default() }, tf, None);
            }
        }
        Shape::Drop => {
            // the tail trails the way it flies; faster drops are longer and thinner
            let sp = q.vel.0.hypot(q.vel.1);
            let th = (-q.vel.0).atan2(q.vel.1).to_degrees();
            let st = 1.0 + (sp / 1100.0).min(0.7);
            let tf = Transform::from_translate(x, y).pre_rotate(th).pre_scale(1.0 / st.sqrt(), st);
            fill(pm, drop_path(s), shade(q.rgb, -0.3), hold, tf);
            fill(pm, drop_path(s * 0.8), q.rgb, hold, tf.pre_translate(0.0, s * 0.05));
            disc(pm, (x - s * 0.3, y - s * 0.2), s * 0.26, 0xFFFFFF, hold * 0.75);
        }
        Shape::Star => {
            let pulse = 0.78 + 0.22 * (q.age * 24.0 + ph).sin();
            if q.rgb == CRYSTAL {
                let ang = ph * 0.4;
                let mut pb = PathBuilder::new();
                for k in 0..3 {
                    let (dx, dy) = ((ang + k as f32 * PI / 3.0).cos(), (ang + k as f32 * PI / 3.0).sin());
                    pb.move_to(x - dx * s, y - dy * s);
                    pb.line_to(x + dx * s, y + dy * s);
                }
                for k in 0..6 {
                    let b = ang + k as f32 * PI / 3.0;
                    for side in [-0.6f32, 0.6] {
                        let (bx, by) = (x + b.cos() * s * 0.62, y + b.sin() * s * 0.62);
                        pb.move_to(bx, by);
                        pb.line_to(bx + (b + side).cos() * s * 0.32, by + (b + side).sin() * s * 0.32);
                    }
                }
                disc(pm, q.pos, s * 1.5, q.rgb, a * 0.14);
                if let Some(p) = pb.finish() {
                    let st = Stroke { width: (s * 0.2).max(1.0), line_cap: LineCap::Round, ..Stroke::default() };
                    pm.stroke_path(&p, &paint(q.rgb, a), &st, Transform::identity(), None);
                }
                disc(pm, q.pos, s * 0.22, 0xFFFFFF, a);
                return;
            }
            let r = s * pulse;
            let mut pb = PathBuilder::new();
            for k in 0..8 {
                let rr = if k % 2 == 0 { r } else { r * 0.3 };
                let ang = k as f32 * PI / 4.0;
                if k == 0 { pb.move_to(ang.cos() * rr, ang.sin() * rr) } else { pb.line_to(ang.cos() * rr, ang.sin() * rr) }
            }
            pb.close();
            // a soft bloom: three discs, widest faintest, so it has no hard edge
            disc(pm, q.pos, r * 2.1, q.rgb, a * 0.05);
            disc(pm, q.pos, r * 1.5, q.rgb, a * 0.08);
            disc(pm, q.pos, r * 1.0, q.rgb, a * 0.12);
            fill(pm, pb.finish(), q.rgb, a, Transform::from_translate(x, y).pre_rotate(q.rot));
            disc(pm, q.pos, r * 0.3, shade(q.rgb, 0.85), a);
        }
        Shape::Spark => {
            // a streak that flickers: glow, body in its colour, hot core, bright head
            let v = q.vel;
            let d = v.0.hypot(v.1).max(1e-3);
            let len = (d * 0.034).clamp(3.0, 18.0);
            let al = a * (0.8 + 0.2 * (q.age * 70.0 + ph).sin());
            let tail = (x - v.0 / d * len, y - v.1 / d * len);
            let mid = (x - v.0 / d * len * 0.45, y - v.1 / d * len * 0.45);
            stroke_line(pm, &[q.pos, tail], q.rgb, al * 0.22, s * 3.4);
            stroke_line(pm, &[q.pos, tail], q.rgb, al * 0.9, s);
            stroke_line(pm, &[q.pos, mid], shade(q.rgb, 0.75), al, s * 0.6);
            disc(pm, q.pos, s * 0.75, shade(q.rgb, 0.9), al);
        }
    }
}
