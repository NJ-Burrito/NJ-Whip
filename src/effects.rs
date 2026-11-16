//! Crack effects: short particle bursts, screen flashes and stamped shapes, one recipe per whip,
//! and the mark each crack leaves where it landed (a puddle, a bullet hole, a cut...), which fades
//! over `MARK_LIFE`.
//! Pure stepping (tested) plus drawing into the overlay pixmap.

use crate::whips::Effect;
use crate::ink::paint;
use tiny_skia::{FillRule, LineCap, PathBuilder, Pixmap, Rect, Stroke, Transform};

type V = (f32, f32);

/// Every burst is over within this time (the mark it leaves stays for `MARK_LIFE`).
#[cfg(test)]
const MAX_LIFE: f32 = 0.9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape { Dot, Chip, Petal, Star, Drop, Puff, Spark }

#[derive(Clone, Debug)]
pub struct Particle {
    pub pos: V,
    pub vel: V,
    pub grav: f32,
    pub drag: f32,
    pub age: f32,
    pub life: f32,
    pub size: f32,
    pub grow: f32,
    pub rot: f32,
    pub spin: f32,
    pub rgb: u32,
    pub shape: Shape,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StampKind {
    Lasso,
    Exclaim,
    Ripple,
    Slash,
    Magic,
    Bolt,
    Shock,
    /// What a crack of this effect leaves where it landed.
    Mark(Effect),
}

#[derive(Clone, Debug)]
pub struct Stamp {
    pub kind: StampKind,
    pub at: V,
    pub to: V,
    pub age: f32,
    pub life: f32,
    pub rgb: u32,
}

#[derive(Clone, Debug)]
pub struct ScreenFlash {
    pub colours: Vec<u32>,
    pub alpha: f32,
    pub age: f32,
    pub life: f32,
}

#[derive(Default)]
pub struct Fx {
    pub particles: Vec<Particle>,
    pub stamps: Vec<Stamp>,
    pub flash: Option<ScreenFlash>,
    /// Where the last trail blob was left (None: the next `trail` call only marks the spot).
    trail_at: Option<V>,
}

/// A trail blob fades away over this many seconds.
pub const TRAIL_LIFE: f32 = 3.0;
/// The mark a crack leaves fades away over this many seconds.
pub const MARK_LIFE: f32 = 3.0;
/// A mark only fades in place, so it is redrawn this many times a second, not every frame
/// (the Ninja's cut runs across the whole screen).
const MARK_FPS: f32 = 15.0;
/// One blob per this many pixels the tip travels.
const TRAIL_GAP: f32 = 11.0;
/// A jump longer than this (the whip was put away and taken out elsewhere) leaves no line of blobs.
const TRAIL_JUMP: f32 = 500.0;

fn rnd(a: f32, b: f32) -> f32 {
    a + (b - a) * fastrand::f32()
}

fn ring(n: usize, at: V, speed: (f32, f32), life: (f32, f32), size: (f32, f32), colours: &[u32], shape: Shape, grav: f32) -> Vec<Particle> {
    (0..n)
        .map(|k| {
            let a = rnd(0.0, std::f32::consts::TAU);
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

impl Fx {
    pub fn alive(&self) -> bool {
        !self.particles.is_empty() || !self.stamps.is_empty() || self.flash.is_some()
    }

    /// A whip that leaves a trail: call every frame with its tip. Blobs are dropped along the
    /// way the tip came, sag a little and fade over `TRAIL_LIFE`.
    pub fn trail(&mut self, tip: V, rgb: u32) {
        let Some(from) = self.trail_at else {
            self.trail_at = Some(tip);
            return;
        };
        let (dx, dy) = (tip.0 - from.0, tip.1 - from.1);
        let d = dx.hypot(dy);
        if d > TRAIL_JUMP {
            self.trail_at = Some(tip);
            return;
        }
        let n = (d / TRAIL_GAP) as usize;
        for k in 1..=n {
            let t = k as f32 * TRAIL_GAP / d;
            self.particles.push(Particle {
                pos: (from.0 + dx * t + rnd(-2.0, 2.0), from.1 + dy * t + rnd(-2.0, 2.0)),
                vel: (0.0, rnd(2.0, 10.0)),
                grav: 14.0,
                drag: 0.985,
                age: 0.0,
                life: TRAIL_LIFE,
                size: rnd(3.0, 6.5),
                grow: 0.0,
                rot: 0.0,
                spin: 0.0,
                rgb,
                shape: Shape::Dot,
            });
        }
        if n > 0 {
            let t = n as f32 * TRAIL_GAP / d;
            self.trail_at = Some((from.0 + dx * t, from.1 + dy * t));
        }
    }

    /// The whip is away: the next trail starts fresh.
    pub fn trail_end(&mut self) {
        self.trail_at = None;
    }

    /// Starts the effect for one crack at the whip tip `at` on a `screen` (w, h).
    pub fn burst(&mut self, effect: Effect, at: V, screen: V) {
        use Effect::*;
        let p = &mut self.particles;
        let st = &mut self.stamps;
        // The mark is under everything else the crack throws. The Ninja's cut runs through the
        // crack point from edge to edge; every other mark sits on the point.
        let (from, to) = match effect {
            Slash => {
                let reach = screen.0.hypot(screen.1);
                let d = (0.832 * reach, -0.555 * reach);
                ((at.0 - d.0, at.1 - d.1), (at.0 + d.0, at.1 + d.1))
            }
            _ => (at, at),
        };
        st.push(Stamp { kind: StampKind::Mark(effect), at: from, to, age: 0.0, life: MARK_LIFE, rgb: 0 });
        let stamp = |kind, to: V, life: f32, rgb: u32| Stamp { kind, at, to, age: 0.0, life, rgb };
        match effect {
            Sparks => p.extend(ring(14, at, (260.0, 520.0), (0.18, 0.32), (2.0, 3.5), &[0xFFFFFF, 0x7FC4EC, 0x2F86C9], Shape::Spark, 0.0)),
            Flash => {
                self.flash = Some(ScreenFlash { colours: vec![0x33D6FF], alpha: 0.32, age: 0.0, life: 0.38 });
                p.extend(ring(18, at, (300.0, 600.0), (0.2, 0.35), (1.5, 2.5), &[0x33D6FF, 0xFF5BD7, 0xFFFFFF], Shape::Spark, 0.0));
            }
            Thump => {
                p.extend(ring(12, at, (60.0, 160.0), (0.4, 0.7), (8.0, 16.0), &[0x9A9286, 0x7E776D, 0xB8B0A2], Shape::Puff, -40.0));
                st.push(stamp(StampKind::Shock, at, 0.25, 0x454950));
            }
            Lasso => {
                st.push(stamp(StampKind::Lasso, at, 0.45, 0xB97A43));
                p.extend(ring(8, at, (50.0, 130.0), (0.4, 0.6), (6.0, 11.0), &[0xD8C29A, 0xC9AE7E], Shape::Puff, -30.0));
                p.extend(ring(6, at, (150.0, 300.0), (0.4, 0.6), (3.0, 5.0), &[0xE8C86A], Shape::Chip, 600.0));
            }
            Confetti => {
                p.extend(ring(48, at, (200.0, 520.0), (0.6, 0.9), (3.0, 5.0), &[0xFF8CC6, 0xFFD25A, 0x7FD4FF, 0x8FE3A0, 0xFFFFFF], Shape::Chip, 700.0));
                p.extend(ring(4, at, (40.0, 90.0), (0.4, 0.6), (5.0, 8.0), &[0xFFF2A8], Shape::Star, 0.0));
            }
            Exclaim => {
                st.push(stamp(StampKind::Exclaim, at, 0.5, 0xE03A3A));
                st.push(stamp(StampKind::Shock, at, 0.2, 0xE03A3A));
            }
            Ripple => {
                st.push(stamp(StampKind::Ripple, at, 0.6, 0x3A3F55));
                p.push(Particle { pos: (at.0 - 20.0, at.1 - 60.0), vel: (30.0, 40.0), grav: 20.0, drag: 0.99, age: 0.0, life: 0.85, size: 9.0, grow: 0.0, rot: 20.0, spin: 120.0, rgb: 0xFFB7C9, shape: Shape::Petal });
            }
            Slash => {
                st.push(Stamp { kind: StampKind::Slash, at: (at.0 - 90.0, at.1 + 60.0), to: (at.0 + 90.0, at.1 - 60.0), age: 0.0, life: 0.22, rgb: 0xFFFFFF });
                p.extend(ring(8, at, (30.0, 90.0), (0.4, 0.6), (8.0, 13.0), &[0x2B2D33, 0x3B3E46], Shape::Puff, -30.0));
            }
            Spray => {
                p.extend(ring(14, at, (220.0, 420.0), (0.45, 0.7), (2.5, 4.0), &[0xCFEFFF, 0xFFFFFF, 0x7FC8F0], Shape::Drop, 900.0));
                p.extend(ring(5, at, (30.0, 70.0), (0.4, 0.6), (8.0, 12.0), &[0x9AA0A8], Shape::Puff, -20.0));
            }
            Sizzle => {
                p.extend(ring(14, at, (200.0, 420.0), (0.2, 0.4), (1.5, 2.5), &[0xFF9A3C, 0xFFFFFF, 0xFFD25A], Shape::Spark, 300.0));
                p.extend(ring(6, at, (10.0, 40.0), (0.6, 0.85), (8.0, 12.0), &[0xF4F4F4], Shape::Puff, -120.0));
            }
            Magic => {
                st.push(stamp(StampKind::Magic, at, 0.45, 0xB89CFF));
                p.extend(ring(20, at, (120.0, 320.0), (0.4, 0.7), (3.0, 5.0), &[0xC7A8FF, 0xFFD25A, 0xFFFFFF], Shape::Star, 0.0));
            }
            Splash => {
                p.extend(ring(16, at, (180.0, 420.0), (0.45, 0.75), (2.5, 4.5), &[0xD93A22, 0xB52A15, 0xFF6A4D], Shape::Drop, 900.0));
                p.extend(ring(4, at, (20.0, 60.0), (0.5, 0.7), (3.0, 4.0), &[0x5FA84A, 0xFFF3E0], Shape::Chip, 500.0));
                self.flash = Some(ScreenFlash { colours: vec![0xFFC27A], alpha: 0.12, age: 0.0, life: 0.45 });
            }
            Petals => p.extend(ring(12, at, (60.0, 180.0), (0.7, 0.9), (5.0, 8.0), &[0xFF9EC0, 0xFFC2D6, 0x8FCB7E], Shape::Petal, 120.0)),
            Snow => {
                p.extend(ring(24, at, (60.0, 220.0), (0.6, 0.9), (1.5, 3.0), &[0xFFFFFF, 0xE6F2FF], Shape::Dot, 160.0));
                p.extend(ring(4, at, (20.0, 50.0), (0.4, 0.6), (9.0, 13.0), &[0xF4F8FF], Shape::Puff, -20.0));
            }
            Lightning => {
                let to = if at.1 < screen.1 * 0.5 { (at.0 + rnd(-120.0, 120.0), screen.1) } else { (at.0 + rnd(-120.0, 120.0), 0.0) };
                st.push(Stamp { kind: StampKind::Bolt, at, to, age: 0.0, life: 0.3, rgb: 0xFFE680 });
                self.flash = Some(ScreenFlash { colours: vec![0xFFE08A], alpha: 0.22, age: 0.0, life: 0.18 });
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        for q in &mut self.particles {
            q.age += dt;
            let k = q.drag.powf(dt * 60.0);
            q.vel = (q.vel.0 * k, q.vel.1 * k + q.grav * dt);
            q.pos = (q.pos.0 + q.vel.0 * dt, q.pos.1 + q.vel.1 * dt);
            q.rot += q.spin * dt;
            q.size += q.grow * dt;
        }
        self.particles.retain(|q| q.age < q.life);
        for s in &mut self.stamps {
            s.age += dt;
        }
        self.stamps.retain(|s| s.age < s.life);
        if let Some(f) = &mut self.flash {
            f.age += dt;
            if f.age >= f.life {
                self.flash = None;
            }
        }
    }

    /// Is anything on the move (not only marks fading in place)?
    pub fn moving(&self) -> bool {
        !self.particles.is_empty() || self.flash.is_some() || self.stamps.iter().any(|s| !matches!(s.kind, StampKind::Mark(_)))
    }

    /// Changes when a mark is added, goes, or takes its next fade step.
    pub fn mark_step(&self) -> u64 {
        self.stamps.iter().fold(0, |h, s| h.wrapping_mul(31).wrapping_add(mark_frame(s) as u64 ^ ((s.at.0.to_bits() as u64) << 20)))
    }

    /// The screen flash right now: colour and strength. Drawn by the renderer as an edge glow
    /// (never a full-screen fill: cheap to draw, easy on the eyes).
    pub fn flash_now(&self) -> Option<(u32, f32)> {
        let f = self.flash.as_ref()?;
        let t = f.age / f.life;
        let idx = ((f.age / 0.06) as usize).min(f.colours.len() - 1);
        Some((f.colours[idx], (f.alpha * 1.4).min(0.45) * (1.0 - t).powf(1.5)))
    }

    /// Box (left, top, right, bottom) around everything `draw` paints this frame.
    pub fn bounds(&self) -> Option<(f32, f32, f32, f32)> {
        let mut b: Option<(f32, f32, f32, f32)> = None;
        let mut add = |x: f32, y: f32, pad: f32| {
            let r = (x - pad, y - pad, x + pad, y + pad);
            b = Some(b.map_or(r, |o| (o.0.min(r.0), o.1.min(r.1), o.2.max(r.2), o.3.max(r.3))));
        };
        for q in &self.particles {
            add(q.pos.0, q.pos.1, q.size * 2.2 + 18.0);
        }
        for s in &self.stamps {
            add(s.at.0, s.at.1, 120.0);
            add(s.to.0, s.to.1, 40.0);
        }
        b
    }

    pub fn draw(&self, pm: &mut Pixmap) {
        for s in &self.stamps {
            draw_stamp(pm, s);
        }
        for q in &self.particles {
            draw_particle(pm, q);
        }
    }
}

fn draw_particle(pm: &mut Pixmap, q: &Particle) {
    let t = q.age / q.life;
    let a = (1.0 - t).clamp(0.0, 1.0);
    let tf = Transform::from_translate(q.pos.0, q.pos.1).pre_rotate(q.rot);
    let s = q.size;
    let path = match q.shape {
        Shape::Dot | Shape::Puff => PathBuilder::from_circle(0.0, 0.0, if q.shape == Shape::Puff { s * (1.0 + t) } else { s }),
        Shape::Chip => Rect::from_xywh(-s, -s * 0.5, s * 2.0, s).map(PathBuilder::from_rect),
        Shape::Petal => {
            let mut pb = PathBuilder::new();
            pb.move_to(0.0, 0.0);
            pb.cubic_to(s * 0.6, -s * 0.6, s * 1.6, -s * 0.5, s * 2.0, 0.0);
            pb.cubic_to(s * 1.6, s * 0.5, s * 0.6, s * 0.6, 0.0, 0.0);
            pb.finish()
        }
        Shape::Drop => {
            let mut pb = PathBuilder::new();
            pb.move_to(0.0, -s * 1.6);
            pb.cubic_to(s * 0.9, -s * 0.4, s, s * 0.5, 0.0, s);
            pb.cubic_to(-s, s * 0.5, -s * 0.9, -s * 0.4, 0.0, -s * 1.6);
            pb.finish()
        }
        Shape::Star => {
            let mut pb = PathBuilder::new();
            for k in 0..8 {
                let r = if k % 2 == 0 { s } else { s * 0.35 };
                let ang = k as f32 * std::f32::consts::PI / 4.0;
                if k == 0 { pb.move_to(ang.cos() * r, ang.sin() * r) } else { pb.line_to(ang.cos() * r, ang.sin() * r) }
            }
            pb.close();
            pb.finish()
        }
        Shape::Spark => {
            let v = q.vel;
            let len = (v.0.hypot(v.1) * 0.03).clamp(3.0, 14.0);
            let d = v.0.hypot(v.1).max(1e-3);
            let mut pb = PathBuilder::new();
            pb.move_to(q.pos.0, q.pos.1);
            pb.line_to(q.pos.0 - v.0 / d * len, q.pos.1 - v.1 / d * len);
            if let Some(path) = pb.finish() {
                let st = Stroke { width: s, line_cap: LineCap::Round, ..Stroke::default() };
                pm.stroke_path(&path, &paint(q.rgb, a), &st, Transform::identity(), None);
            }
            return;
        }
    };
    let alpha = if q.shape == Shape::Puff { a * 0.55 } else { a };
    if let Some(path) = path {
        pm.fill_path(&path, &paint(q.rgb, alpha), FillRule::Winding, tf, None);
    }
}

fn stroke_line(pm: &mut Pixmap, pts: &[V], rgb: u32, a: f32, w: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        pb.line_to(p.0, p.1);
    }
    if let Some(path) = pb.finish() {
        let st = Stroke { width: w, line_cap: LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Stroke::default() };
        pm.stroke_path(&path, &paint(rgb, a), &st, Transform::identity(), None);
    }
}

/// The fade step a mark is on.
fn mark_frame(s: &Stamp) -> u32 {
    (s.age * MARK_FPS) as u32
}

fn disc(pm: &mut Pixmap, c: V, r: f32, rgb: u32, a: f32) {
    if let Some(p) = PathBuilder::from_circle(c.0, c.1, r) {
        pm.fill_path(&p, &paint(rgb, a), FillRule::Winding, Transform::identity(), None);
    }
}

fn hoop(pm: &mut Pixmap, c: V, r: f32, rgb: u32, a: f32, w: f32) {
    if let Some(p) = PathBuilder::from_circle(c.0, c.1, r) {
        pm.stroke_path(&p, &paint(rgb, a), &Stroke { width: w, ..Stroke::default() }, Transform::identity(), None);
    }
}

fn oval(pm: &mut Pixmap, c: V, rx: f32, ry: f32, rgb: u32, a: f32) {
    if let Some(p) = Rect::from_xywh(c.0 - rx, c.1 - ry, rx * 2.0, ry * 2.0).and_then(PathBuilder::from_oval) {
        pm.fill_path(&p, &paint(rgb, a), FillRule::Winding, Transform::identity(), None);
    }
}

fn fill_poly(pm: &mut Pixmap, pts: &[V], rgb: u32, a: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(rgb, a), FillRule::Winding, Transform::identity(), None);
    }
}

/// Points on a circle of radius `r` (or an oval `r` by `r * flat`) from angle `a0` to `a1` (radians).
fn arc(c: V, r: f32, flat: f32, a0: f32, a1: f32, n: usize) -> Vec<V> {
    (0..=n).map(|i| {
        let u = a0 + (a1 - a0) * i as f32 / n as f32;
        (c.0 + u.cos() * r, c.1 + u.sin() * r * flat)
    }).collect()
}

/// What a crack leaves where it landed, at strength `a`. Everything stays within 60 px of the
/// point (the Ninja's cut runs from `s.at` to `s.to`). Its random parts come from the point, so
/// a mark looks the same on every frame.
fn draw_mark(pm: &mut Pixmap, effect: Effect, s: &Stamp, a: f32) {
    use std::f32::consts::{PI, TAU};
    let c @ (x, y) = s.at;
    let mut rng = fastrand::Rng::with_seed(((x as u64) << 20) ^ y as u64 ^ 0x9E37);
    let mut r = |lo: f32, hi: f32| lo + (hi - lo) * rng.f32();
    match effect {
        // Classic: the lash's scar
        Effect::Sparks => {
            let ang = r(-0.7, 0.7);
            let (dx, dy) = (ang.cos() * 40.0, ang.sin() * 40.0);
            let pts = [(x - dx, y - dy), (x - dx * 0.3, y - dy * 0.3 - 5.0), (x + dx * 0.4, y + dy * 0.4 + 4.0), (x + dx, y + dy)];
            stroke_line(pm, &pts, 0x0E1730, a * 0.7, 6.0);
            stroke_line(pm, &pts, 0x7FC4EC, a * 0.9, 2.0);
        }
        // Neon: a ring of light
        Effect::Flash => {
            hoop(pm, c, 30.0, 0x33D6FF, a * 0.22, 10.0);
            hoop(pm, c, 30.0, 0x33D6FF, a * 0.9, 3.0);
            hoop(pm, c, 20.0, 0xFF5BD7, a * 0.7, 2.0);
        }
        // Elephant: a cracked dent
        Effect::Thump => {
            oval(pm, c, 37.0, 16.5, 0xB8B0A2, a * 0.5);
            oval(pm, c, 34.0, 14.0, 0x2A2C31, a * 0.75);
            for k in 0..6 {
                let ang = k as f32 * TAU / 6.0 + r(-0.3, 0.3);
                let len = r(40.0, 56.0);
                stroke_line(pm, &[(x + ang.cos() * 20.0, y + ang.sin() * 8.0), (x + ang.cos() * len, y + ang.sin() * len * 0.45)], 0xB8B0A2, a * 0.5, 4.5);
                stroke_line(pm, &[(x + ang.cos() * 20.0, y + ang.sin() * 8.0), (x + ang.cos() * len, y + ang.sin() * len * 0.45)], 0x2A2C31, a * 0.8, 2.5);
            }
        }
        // Cowboy: a horseshoe print
        Effect::Lasso => {
            let shoe = arc(c, 24.0, 1.15, 0.75 * PI, 2.25 * PI, 24);
            stroke_line(pm, &shoe, 0x3A2A1C, a * 0.6, 10.0);
            stroke_line(pm, &shoe, 0xB97A43, a * 0.9, 6.0);
        }
        // Cheerleader: a star
        Effect::Confetti => {
            let rot = r(0.0, TAU);
            let mut star: Vec<V> = (0..10).map(|k| {
                let (ang, rad) = (rot + k as f32 * PI / 5.0, if k % 2 == 0 { 32.0 } else { 14.0 });
                (x + ang.cos() * rad, y + ang.sin() * rad)
            }).collect();
            star.push(star[0]);
            fill_poly(pm, &star, 0xFFD25A, a * 0.35);
            stroke_line(pm, &star, 0xFF8CC6, a, 3.0);
        }
        // Sauce: a puddle
        Effect::Splash => {
            oval(pm, (x, y + 6.0), 40.0, 16.0, 0xB52A15, a * 0.85);
            oval(pm, (x - 4.0, y + 4.0), 30.0, 11.0, 0xD93A22, a);
            for (dx, dy, rad) in [(38.0, 12.0, 7.0), (-36.0, 2.0, 5.0), (12.0, 22.0, 6.0)] {
                disc(pm, (x + dx, y + dy), rad, 0xD93A22, a);
            }
            oval(pm, (x - 10.0, y), 10.0, 3.0, 0xFFB3A0, a * 0.7);
        }
        // Drill Sergeant: a bullet hole
        Effect::Exclaim => {
            for _ in 0..6 {
                let (ang, len) = (r(0.0, TAU), r(20.0, 36.0));
                stroke_line(pm, &[(x + ang.cos() * 11.0, y + ang.sin() * 11.0), (x + ang.cos() * len, y + ang.sin() * len)], 0xC8CCD2, a * 0.6, 3.2);
                stroke_line(pm, &[(x + ang.cos() * 11.0, y + ang.sin() * 11.0), (x + ang.cos() * len, y + ang.sin() * len)], 0x1E1F24, a * 0.8, 1.6);
            }
            hoop(pm, c, 11.0, 0x9AA0A8, a * 0.85, 2.5);
            disc(pm, c, 9.5, 0x0B0C0E, a * 0.95);
        }
        // Zen: a brushed ink circle, left open
        Effect::Ripple => {
            let start = r(0.0, TAU);
            stroke_line(pm, &arc(c, 32.0, 1.0, start, start + 1.8 * PI, 40), 0xFFFFFF, a * 0.4, 9.0);
            stroke_line(pm, &arc(c, 32.0, 1.0, start, start + 1.8 * PI, 40), 0x2B2F3D, a * 0.9, 6.0);
            stroke_line(pm, &arc(c, 29.0, 1.0, start + 0.3, start + 1.5 * PI, 30), 0x2B2F3D, a * 0.45, 2.5);
        }
        // Ninja: a cut across the screen
        Effect::Slash => {
            stroke_line(pm, &[s.at, s.to], 0x0B0C0E, a * 0.5, 5.0);
            stroke_line(pm, &[s.at, s.to], 0xFFFFFF, a * 0.9, 1.6);
        }
        // Pirate: X marks the spot
        Effect::Spray => {
            for sign in [-1.0, 1.0] {
                stroke_line(pm, &[(x - 24.0, y - 24.0 * sign), (x + 24.0, y + 24.0 * sign)], 0x5A120E, a * 0.5, 10.0);
                stroke_line(pm, &[(x - 24.0, y - 24.0 * sign), (x + 24.0, y + 24.0 * sign)], 0xC0271E, a * 0.95, 6.0);
            }
        }
        // Chef: a fried egg
        Effect::Sizzle => {
            oval(pm, c, 30.0, 22.0, 0xFFFDF5, a * 0.95);
            disc(pm, (x + 22.0, y + 10.0), 12.0, 0xFFFDF5, a * 0.95);
            disc(pm, (x - 20.0, y - 8.0), 11.0, 0xFFFDF5, a * 0.95);
            disc(pm, (x + 3.0, y - 1.0), 11.0, 0xFFB21E, a);
            disc(pm, (x - 1.0, y - 5.0), 3.0, 0xFFE9A8, a);
        }
        // Wizard: a rune
        Effect::Magic => {
            hoop(pm, c, 34.0, 0xB89CFF, a * 0.8, 1.8);
            hoop(pm, c, 27.0, 0xB89CFF, a * 0.5, 1.0);
            let rot = r(0.0, TAU);
            let tri: Vec<V> = (0..=3).map(|k| {
                let ang = rot + k as f32 * TAU / 3.0;
                (x + ang.cos() * 27.0, y + ang.sin() * 27.0)
            }).collect();
            stroke_line(pm, &tri, 0xFFD25A, a * 0.8, 1.6);
        }
        // Vine: leaves and a flower
        Effect::Petals => {
            for k in 0..3 {
                let ang = -PI / 2.0 + (k as f32 - 1.0) * 0.9 + r(-0.15, 0.15);
                let (tip, mid) = ((x + ang.cos() * 36.0, y + ang.sin() * 36.0), (x + ang.cos() * 18.0, y + ang.sin() * 18.0));
                let side = (-ang.sin() * 10.0, ang.cos() * 10.0);
                let mut pb = PathBuilder::new();
                pb.move_to(x, y);
                pb.quad_to(mid.0 + side.0, mid.1 + side.1, tip.0, tip.1);
                pb.quad_to(mid.0 - side.0, mid.1 - side.1, x, y);
                if let Some(leaf) = pb.finish() {
                    pm.fill_path(&leaf, &paint(0x5FA84A, a * 0.9), FillRule::Winding, Transform::identity(), None);
                }
                stroke_line(pm, &[c, tip], 0x2F6B2A, a * 0.7, 1.2);
            }
            disc(pm, c, 6.0, 0xFF9EC0, a);
            disc(pm, c, 2.5, 0xFFE9A8, a);
        }
        // Penguin: a pile of snow
        Effect::Snow => {
            let mut pb = PathBuilder::new();
            pb.move_to(x - 36.0, y + 10.0);
            pb.cubic_to(x - 30.0, y - 20.0, x + 30.0, y - 20.0, x + 36.0, y + 10.0);
            pb.close();
            if let Some(pile) = pb.finish() {
                pm.fill_path(&pile, &paint(0xF4F8FF, a * 0.95), FillRule::Winding, Transform::identity(), None);
            }
            stroke_line(pm, &[(x - 36.0, y + 10.0), (x + 36.0, y + 10.0)], 0xB9CCE6, a * 0.8, 2.0);
            disc(pm, (x - 44.0, y + 7.0), 4.0, 0xF4F8FF, a * 0.95);
            disc(pm, (x + 46.0, y + 8.0), 3.0, 0xF4F8FF, a * 0.95);
        }
        // Storm: a scorch
        Effect::Lightning => {
            let burn: Vec<V> = (0..12).map(|k| {
                let (ang, rad) = (k as f32 * TAU / 12.0, if k % 2 == 0 { r(28.0, 40.0) } else { r(12.0, 18.0) });
                (x + ang.cos() * rad, y + ang.sin() * rad)
            }).collect();
            fill_poly(pm, &burn, 0x15171C, a * 0.8);
            let mut edge = burn.clone();
            edge.push(burn[0]);
            stroke_line(pm, &edge, 0xFF9A3C, a * 0.6, 1.5);
            hoop(pm, c, 12.0, 0xFF9A3C, a * 0.7, 2.5);
            disc(pm, c, 4.0, 0xFFE680, a * 0.8);
        }
    }
}

fn draw_stamp(pm: &mut Pixmap, s: &Stamp) {
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
            let pop = 1.0 + 0.25 * (1.0 - t).powi(3);
            let tf = Transform::from_translate(x + 20.0, y - 70.0).pre_scale(pop, pop);
            if let Some(r) = Rect::from_xywh(-7.0, -30.0, 14.0, 34.0) {
                pm.fill_rect(r, &paint(s.rgb, a), tf, None);
            }
            if let Some(c) = PathBuilder::from_circle(0.0, 14.0, 7.5) {
                pm.fill_path(&c, &paint(s.rgb, a), FillRule::Winding, tf, None);
            }
        }
        StampKind::Ripple => {
            for k in 0..3 {
                let r = 10.0 + (t * 80.0) - k as f32 * 18.0;
                if r > 2.0 {
                    if let Some(c) = PathBuilder::from_circle(x, y, r) {
                        let halo = Stroke { width: 3.5, ..Stroke::default() };
                        pm.stroke_path(&c, &paint(0xFFFFFF, a * 0.45), &halo, Transform::identity(), None);
                        let st = Stroke { width: 2.2, ..Stroke::default() };
                        pm.stroke_path(&c, &paint(s.rgb, a * 0.8), &st, Transform::identity(), None);
                    }
                }
            }
        }
        StampKind::Lasso => {
            let turns = 1.5 * t.min(1.0) + 0.05;
            let n = 48;
            let pts: Vec<V> = (0..=n)
                .map(|i| {
                    let u = i as f32 / n as f32 * turns * std::f32::consts::TAU;
                    let r = 46.0 * (1.0 - 0.6 * (u / (1.5 * std::f32::consts::TAU)));
                    (x + u.cos() * r * 1.4, y - 30.0 + u.sin() * r * 0.55)
                })
                .collect();
            stroke_line(pm, &pts, s.rgb, a, 3.0);
        }
        StampKind::Slash => {
            let k = (t * 3.0).min(1.0);
            let end = (s.at.0 + (s.to.0 - s.at.0) * k, s.at.1 + (s.to.1 - s.at.1) * k);
            stroke_line(pm, &[s.at, end], 0x1E1F24, a * 0.45, 7.0);
            stroke_line(pm, &[s.at, end], 0xBFE6FF, a * 0.5, 9.0);
            stroke_line(pm, &[s.at, end], s.rgb, a, 3.0);
        }
        StampKind::Magic => {
            let r = 40.0 + 20.0 * t;
            if let Some(c) = PathBuilder::from_circle(x, y, r) {
                let st = Stroke { width: 2.0, ..Stroke::default() };
                pm.stroke_path(&c, &paint(s.rgb, a), &st, Transform::identity(), None);
            }
            let rot = t * 120.0_f32.to_radians();
            let star: Vec<V> = (0..=6)
                .map(|i| {
                    let ang = rot + (i * 2 % 6) as f32 * std::f32::consts::TAU / 6.0 + if i * 2 >= 6 { std::f32::consts::PI / 6.0 * 0.0 } else { 0.0 };
                    (x + ang.cos() * r * 0.85, y + ang.sin() * r * 0.85)
                })
                .collect();
            stroke_line(pm, &star, 0xFFD25A, a, 1.6);
        }
        StampKind::Bolt => {
            let segs = 9;
            let mut pts = vec![s.at];
            let mut rng = fastrand::Rng::with_seed(((s.at.0 as u64) << 16) ^ s.at.1 as u64);
            for i in 1..segs {
                let u = i as f32 / segs as f32;
                let jitter = -28.0 + 56.0 * rng.f32();
                pts.push((s.at.0 + (s.to.0 - s.at.0) * u + jitter, s.at.1 + (s.to.1 - s.at.1) * u));
            }
            pts.push(s.to);
            let flick = if (s.age * 30.0) as i32 % 2 == 0 { 1.0 } else { 0.6 };
            stroke_line(pm, &pts, 0x2E3A58, a * 0.55 * flick, 8.0);
            stroke_line(pm, &pts, 0xFFC400, a * 0.55, 12.0);
            stroke_line(pm, &pts, s.rgb, a * flick, 4.0);
            stroke_line(pm, &pts, 0xFFFFFF, a * flick, 1.6);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::whips::CATALOGUE;

    #[test]
    fn a_trail_follows_the_tip_and_fades_in_three_seconds() {
        let mut fx = Fx::default();
        fx.trail((100.0, 100.0), 0xD93A22);
        assert!(!fx.alive(), "the first call only marks the spot");
        fx.trail((100.0 + TRAIL_GAP * 10.5, 100.0), 0xD93A22);
        assert_eq!(fx.particles.len(), 10);
        fx.trail((5000.0, 100.0), 0xD93A22);
        assert_eq!(fx.particles.len(), 10, "a jump leaves no line");
        for _ in 0..(TRAIL_LIFE * 60.0) as usize - 2 {
            fx.step(1.0 / 60.0);
        }
        assert!(fx.alive());
        for _ in 0..4 {
            fx.step(1.0 / 60.0);
        }
        assert!(!fx.alive());
    }

    #[test]
    fn every_whip_effect_starts_and_ends_in_time() {
        for w in &CATALOGUE {
            let mut fx = Fx::default();
            fx.burst(w.effect, (500.0, 400.0), (1920.0, 1080.0));
            assert!(fx.alive(), "{}", w.name);
            let mut t = 0.0;
            while fx.moving() && t < 2.0 {
                fx.step(1.0 / 60.0);
                t += 1.0 / 60.0;
            }
            assert!(!fx.moving() && t <= MAX_LIFE + 0.02, "{} lasted {t}", w.name);
        }
    }

    #[test]
    fn every_crack_leaves_a_mark_that_fades_in_three_seconds() {
        for w in &CATALOGUE {
            let mut fx = Fx::default();
            fx.burst(w.effect, (500.0, 400.0), (1920.0, 1080.0));
            let mut steps = std::collections::HashSet::new();
            for _ in 0..(MARK_LIFE * 60.0) as usize - 2 {
                fx.step(1.0 / 60.0);
                steps.insert(fx.mark_step());
            }
            assert!(fx.alive(), "{}: the mark is gone early", w.name);
            // only the mark is left, and it is drawn: fading in MARK_FPS steps a second
            assert!(!fx.moving(), "{}", w.name);
            let mut pm = Pixmap::new(1920, 1080).unwrap();
            fx.draw(&mut pm);
            assert!(pm.pixels().iter().any(|p| p.alpha() > 0), "{}: nothing drawn", w.name);
            assert!((40..=46).contains(&steps.len()), "{}: {} fade steps", w.name, steps.len());
            for _ in 0..4 {
                fx.step(1.0 / 60.0);
            }
            assert!(!fx.alive(), "{}: the mark stays", w.name);
        }
    }

    /// Every whip's mark on one sheet, to look at: `cargo test --release marks_sheet -- --ignored`
    /// writes target/marks.png (catalogue order, five to a row, one second after the crack).
    #[test]
    #[ignore]
    fn marks_sheet() {
        let mut pm = Pixmap::new(1100, 540).unwrap();
        pm.fill(crate::ink::col(0x0A1024, 1.0));
        let mut fx = Fx::default();
        for (k, w) in CATALOGUE.iter().enumerate() {
            fx.burst(w.effect, (110.0 + (k % 5) as f32 * 220.0, 90.0 + (k / 5) as f32 * 180.0), (1100.0, 540.0));
        }
        for _ in 0..60 {
            fx.step(1.0 / 60.0);
        }
        fx.draw(&mut pm);
        pm.save_png("target/marks.png").unwrap();
    }

    #[test]
    fn effects_stay_inside_their_bounds() {
        for w in &CATALOGUE {
            let mut fx = Fx::default();
            fx.burst(w.effect, (600.0, 500.0), (1200.0, 1000.0));
            for _ in 0..6 {
                fx.step(1.0 / 60.0);
                let Some((l, t, r, b)) = fx.bounds() else { continue };
                let mut pm = Pixmap::new(1200, 1000).unwrap();
                fx.draw(&mut pm);
                for (i, px) in pm.pixels().iter().enumerate() {
                    if px.alpha() > 0 {
                        let (x, y) = ((i % 1200) as f32, (i / 1200) as f32);
                        assert!(x >= l - 1.0 && x <= r + 1.0 && y >= t - 1.0 && y <= b + 1.0, "{}: ({x},{y}) outside {:?}", w.name, (l, t, r, b));
                    }
                }
            }
        }
    }

    #[test]
    fn effects_draw_something() {
        for w in &CATALOGUE {
            let mut fx = Fx::default();
            fx.burst(w.effect, (300.0, 300.0), (600.0, 600.0));
            fx.step(0.05);
            let mut pm = Pixmap::new(600, 600).unwrap();
            fx.draw(&mut pm);
            assert!(pm.pixels().iter().any(|p| p.alpha() > 0) || fx.flash_now().is_some(), "{}", w.name);
        }
    }
}
