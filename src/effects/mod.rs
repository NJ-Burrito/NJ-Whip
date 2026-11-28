//! Crack effects: short particle bursts, screen flashes and stamped shapes, one recipe per whip,
//! and the mark each crack leaves where it landed (a puddle, a bullet hole, a cut...), which fades
//! over `MARK_LIFE`.
//! Pure stepping (tested) plus drawing into the overlay pixmap.

use crate::whips::Effect;
use crate::ink::paint;
use tiny_skia::{FillRule, LineCap, PathBuilder, Pixmap, Rect, Stroke, Transform};


type V = (f32, f32);
mod particle;
use particle::*;
mod stamps;
use stamps::*;
mod sparks;
mod flash;
mod thump;
mod lasso;
mod confetti;
mod exclaim;
mod ripple;
mod slash;
mod spray;
mod sizzle;
mod magic;
mod splash;
mod petals;
mod snow;
mod lightning;

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
}

/// The mark a crack leaves fades away over this many seconds.
pub const MARK_LIFE: f32 = 3.0;
/// A mark only fades in place, so it is redrawn this many times a second, not every frame
/// (the Ninja's cut runs across the whole screen).
const MARK_FPS: f32 = 15.0;

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

    /// Starts the effect for one crack at the whip tip `at` on a `screen` (w, h).
    pub fn burst(&mut self, effect: Effect, at: V, screen: V) {
        use Effect::*;
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
        self.stamps.push(Stamp { kind: StampKind::Mark(effect), at: from, to, age: 0.0, life: MARK_LIFE, rgb: 0 });
        match effect {
            Sparks => sparks::burst(self, at, screen),
            Flash => flash::burst(self, at, screen),
            Thump => thump::burst(self, at, screen),
            Lasso => lasso::burst(self, at, screen),
            Confetti => confetti::burst(self, at, screen),
            Exclaim => exclaim::burst(self, at, screen),
            Ripple => ripple::burst(self, at, screen),
            Slash => slash::burst(self, at, screen),
            Spray => spray::burst(self, at, screen),
            Sizzle => sizzle::burst(self, at, screen),
            Magic => magic::burst(self, at, screen),
            Splash => splash::burst(self, at, screen),
            Petals => petals::burst(self, at, screen),
            Snow => snow::burst(self, at, screen),
            Lightning => lightning::burst(self, at, screen),
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
    let (x, y) = s.at;
    let mut rng = fastrand::Rng::with_seed(((x as u64) << 20) ^ y as u64 ^ 0x9E37);
    let mut r = |lo: f32, hi: f32| lo + (hi - lo) * rng.f32();
    match effect {
        Effect::Sparks => sparks::mark(pm, s, a, &mut r),
        Effect::Flash => flash::mark(pm, s, a, &mut r),
        Effect::Thump => thump::mark(pm, s, a, &mut r),
        Effect::Lasso => lasso::mark(pm, s, a, &mut r),
        Effect::Confetti => confetti::mark(pm, s, a, &mut r),
        Effect::Exclaim => exclaim::mark(pm, s, a, &mut r),
        Effect::Ripple => ripple::mark(pm, s, a, &mut r),
        Effect::Slash => slash::mark(pm, s, a, &mut r),
        Effect::Spray => spray::mark(pm, s, a, &mut r),
        Effect::Sizzle => sizzle::mark(pm, s, a, &mut r),
        Effect::Magic => magic::mark(pm, s, a, &mut r),
        Effect::Splash => splash::mark(pm, s, a, &mut r),
        Effect::Petals => petals::mark(pm, s, a, &mut r),
        Effect::Snow => snow::mark(pm, s, a, &mut r),
        Effect::Lightning => lightning::mark(pm, s, a, &mut r),
    }
}


#[cfg(test)]
mod tests;
