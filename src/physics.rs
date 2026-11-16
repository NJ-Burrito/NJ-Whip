//! Whip rope simulation (NJ-Whip's own model).
//!
//! The lash is a chain of points integrated with Verlet steps. Each step:
//! 1. integrate (velocity from previous position, gravity, air drag that grows toward the tip
//!    and the leather's own friction, so the lash trails the hand and a loop dies),
//! 2. pin the first point to the hand (mouse),
//! 3. pull the first few points toward a "rest pose" aimed up-and-out from the hand
//!    (the stiff part of a real whip near the handle),
//! 4. relax distance constraints (segments taper toward the tip) and a smoothing
//!    bend constraint whose stiffness fades from handle to tip,
//! 5. keep points inside the screen with a soft bounce.
//!
//! One step is one 1/60 s tick. Parameters vary per whip type.

pub type V = (f32, f32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub points: usize,
    /// Length of the first segment in px; later segments shrink toward `TAPER` × this.
    pub seg_len: f32,
    pub gravity: f32,
    /// Fraction of velocity kept per step (1 = none lost).
    pub damping: f32,
    /// The same at the tip: air drag grows along the lash, so a loop dies instead of orbiting.
    pub tip_damping: f32,
    /// Bend stiffness at the handle end and at the tip (0..1).
    pub stiff_handle: f32,
    pub stiff_tip: f32,
    /// How heavy the whip feels (1 = normal): a heavy handle trails the hand and smooths it.
    pub weight: f32,
}

// The same for every whip.
/// Segments shrink toward the tip, down to this share of the first one.
const TAPER: f32 = 0.55;
/// Speed (px/step) every point loses per step: the leather's own friction, which stops a
/// slow swing dead instead of letting it ring on like a pendulum.
const FRICTION: f32 = 0.25;
/// Constraint relaxation passes per step.
const ITERATIONS: u32 = 16;
/// How many points near the hand follow the rest pose, and how strongly.
const POSE_POINTS: usize = 4;
const POSE_STRENGTH: f32 = 0.55;
/// Rest aim in radians (0 = right, negative = up). It does not follow the hand: an aim
/// that swings with the hand's speed steers the lash round the hand in circles.
const AIM: f32 = -1.05;
/// Hard cap on a stretched segment, as a multiple of its rest length.
const MAX_STRETCH: f32 = 1.15;
/// Wall bounce: fraction of speed kept.
const BOUNCE: f32 = 0.45;

impl Default for Params {
    fn default() -> Self {
        Params::DEFAULT
    }
}

impl Params {
    /// The Classic whip; every other whip is this with a few fields changed (whips.rs).
    pub const DEFAULT: Params = Params {
        points: 26,
        seg_len: 24.0,
        gravity: 1.25,
        damping: 0.965,
        tip_damping: 0.94,
        stiff_handle: 0.85,
        stiff_tip: 0.06,
        weight: 1.0,
    };

    pub fn seg(&self, i: usize) -> f32 {
        let n = (self.points.max(2) - 1) as f32;
        let t = i as f32 / n;
        self.seg_len * (1.0 - t * (1.0 - TAPER))
    }
}

pub struct Rope {
    pub pts: Vec<V>,
    prev: Vec<V>,
    hand_prev: V,
    hand_vel: V,
}

/// Fraction of the gap to the mouse a normal-weight handle closes per step.
const FOLLOW: f32 = 0.62;
/// Tip speed (px/step) a crack adds to the end of the lash.
const KICK: f32 = 56.0;
/// A whip whose hand is still and whose every point moved less than this (px) in a step is at rest.
const SLEEP: f32 = 0.3;

impl Rope {
    /// A rope at rest along the aim direction, curling down toward the tip.
    pub fn spawn(p: &Params, hand: V) -> Rope {
        let mut pts = Vec::with_capacity(p.points);
        let (mut x, mut y) = hand;
        let mut a = AIM;
        pts.push((x, y));
        for i in 0..p.points.saturating_sub(1) {
            x += a.cos() * p.seg(i);
            y += a.sin() * p.seg(i);
            pts.push((x, y));
            a += 0.09; // gentle droop toward the tip
        }
        Rope { prev: pts.clone(), pts, hand_prev: hand, hand_vel: (0.0, 0.0) }
    }

    /// The (weighted) handle's velocity in px per step.
    pub fn hand_vel(&self) -> V {
        self.hand_vel
    }

    /// A crack: throw the last third of the lash along the swing line (-1 left, 1 right,
    /// 0 = where it points). Sideways only, so the lash shoots out and comes back instead of
    /// being lifted over the hand.
    pub fn kick(&mut self, dir: i8) {
        let n = self.pts.len();
        let dir = if dir != 0 { dir as f32 } else { (self.pts[n - 1].0 - self.pts[0].0).signum() };
        let from = n - (n / 3).max(1);
        for i in from..n {
            let t = (i - from + 1) as f32 / (n - from) as f32;
            self.prev[i].0 -= dir * KICK * t * t;
        }
    }

    pub fn fallen_off(&self, bottom: f32) -> bool {
        self.pts.iter().all(|p| p.1 > bottom + 60.0)
    }

    /// One 1/60 s step. `bounds` = (left, top, right, bottom). While `dropping`, the hand lets go.
    pub fn step(&mut self, p: &Params, hand: V, bounds: (f32, f32, f32, f32), dropping: bool) {
        let n = self.pts.len();
        // 1. integrate
        for i in 0..n {
            let (x, y) = self.pts[i];
            let (px, py) = self.prev[i];
            let d = p.damping + (p.tip_damping - p.damping) * i as f32 / (n - 1) as f32;
            let (vx, vy) = (x - px, y - py);
            let s = vx.hypot(vy);
            let d = if s > FRICTION { d * (1.0 - FRICTION / s) } else { 0.0 };
            let (vx, vy) = (vx * d, vy * d);
            self.prev[i] = (x, y);
            self.pts[i] = (x + vx, y + vy + p.gravity);
        }
        if !dropping {
            // 2. hand: the handle chases the mouse; a heavier whip trails it (and filters jitter)
            let k = (FOLLOW / p.weight.max(0.1)).clamp(0.15, 1.0);
            let hand = (self.hand_prev.0 + (hand.0 - self.hand_prev.0) * k, self.hand_prev.1 + (hand.1 - self.hand_prev.1) * k);
            self.pts[0] = hand;
            self.prev[0] = hand;
            self.hand_vel = (hand.0 - self.hand_prev.0, hand.1 - self.hand_prev.1);
            self.hand_prev = hand;
            // 3. rest pose near the hand
            let a = AIM;
            let k = POSE_POINTS.min(n - 1);
            let (mut x, mut y) = hand;
            for i in 1..=k {
                x += a.cos() * p.seg(i - 1);
                y += a.sin() * p.seg(i - 1);
                let w = POSE_STRENGTH * (1.0 - (i - 1) as f32 / k.max(1) as f32 * 0.5);
                let q = &mut self.pts[i];
                q.0 += (x - q.0) * w;
                q.1 += (y - q.1) * w;
            }
        }
        // 4. constraints
        for _ in 0..ITERATIONS {
            for i in 0..n - 1 {
                let rest = p.seg(i);
                let (a, b) = (self.pts[i], self.pts[i + 1]);
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let d = dx.hypot(dy).max(1e-4);
                let diff = (d - rest) / d;
                if i == 0 && !dropping {
                    self.pts[1].0 -= dx * diff;
                    self.pts[1].1 -= dy * diff;
                } else {
                    self.pts[i].0 += dx * diff * 0.5;
                    self.pts[i].1 += dy * diff * 0.5;
                    self.pts[i + 1].0 -= dx * diff * 0.5;
                    self.pts[i + 1].1 -= dy * diff * 0.5;
                }
            }
            for i in 1..n - 1 {
                let t = i as f32 / (n - 1) as f32;
                let s = p.stiff_handle + (p.stiff_tip - p.stiff_handle) * t;
                let (a, c) = (self.pts[i - 1], self.pts[i + 1]);
                let m = ((a.0 + c.0) * 0.5, (a.1 + c.1) * 0.5);
                let q = &mut self.pts[i];
                q.0 += (m.0 - q.0) * s * 0.25;
                q.1 += (m.1 - q.1) * s * 0.25;
            }
        }
        // hard stretch cap, walking out from the hand
        for i in 0..n - 1 {
            let max = p.seg(i) * MAX_STRETCH;
            let (a, b) = (self.pts[i], self.pts[i + 1]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let d = dx.hypot(dy);
            if d > max {
                self.pts[i + 1] = (a.0 + dx / d * max, a.1 + dy / d * max);
            }
        }
        if !dropping {
            // at rest: leather does not swing on like a pendulum once it has all but stopped
            let slow = |(a, b): (&V, &V)| (a.0 - b.0).hypot(a.1 - b.1) < SLEEP;
            if self.hand_vel.0.hypot(self.hand_vel.1) < 0.05 && self.pts.iter().zip(&self.prev).all(slow) {
                self.pts.clone_from(&self.prev);
                return;
            }
        }
        // 5. walls (not while dropping, so the whip can fall off screen)
        if !dropping {
            let (l, t, r, b) = bounds;
            for i in 1..n {
                let (x, y) = self.pts[i];
                let (px, py) = self.prev[i];
                let (cx, cy) = (x.clamp(l, r), y.clamp(t, b));
                if cx != x {
                    self.prev[i].0 = cx + (x - px) * BOUNCE;
                }
                if cy != y {
                    self.prev[i].1 = cy + (y - py) * BOUNCE;
                }
                self.pts[i] = (cx, cy);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const B: (f32, f32, f32, f32) = (0.0, 0.0, 1920.0, 1080.0);

    #[test]
    fn spawn_has_all_points() {
        let p = Params::default();
        assert_eq!(Rope::spawn(&p, (500.0, 500.0)).pts.len(), p.points);
    }

    #[test]
    fn handle_follows_hand() {
        let p = Params::default();
        let mut r = Rope::spawn(&p, (500.0, 500.0));
        r.step(&p, (520.0, 480.0), B, false);
        assert!(r.pts[0].0 > 500.0 && r.pts[0].0 < 520.0, "one step closes part of the gap");
        for _ in 0..60 {
            r.step(&p, (520.0, 480.0), B, false);
        }
        assert!((r.pts[0].0 - 520.0).abs() < 0.01 && (r.pts[0].1 - 480.0).abs() < 0.01);
    }

    #[test]
    fn never_overstretches_under_jerks() {
        let p = Params::default();
        let mut r = Rope::spawn(&p, (900.0, 600.0));
        for k in 0..600 {
            let hand = if k % 2 == 0 { (500.0, 300.0) } else { (1300.0, 800.0) };
            r.step(&p, hand, B, false);
            for i in 0..r.pts.len() - 1 {
                let (a, b) = (r.pts[i], r.pts[i + 1]);
                let d = (b.0 - a.0).hypot(b.1 - a.1);
                assert!(d <= p.seg(i) * MAX_STRETCH + 0.5, "seg {i} = {d}");
            }
        }
    }

    /// One more step with the hand at `hand`: the largest distance any point moves in it.
    fn moved(r: &mut Rope, p: &Params, hand: V) -> f32 {
        let before = r.pts.clone();
        r.step(p, hand, B, false);
        r.pts.iter().zip(&before).map(|(a, b)| (a.0 - b.0).hypot(a.1 - b.1)).fold(0.0, f32::max)
    }

    #[test]
    fn settles_when_hand_is_still() {
        let p = Params::default();
        let mut r = Rope::spawn(&p, (900.0, 600.0));
        for _ in 0..900 {
            r.step(&p, (900.0, 600.0), B, false);
        }
        let m = moved(&mut r, &p, (900.0, 600.0));
        assert!(m < 0.05, "moved {m}");
    }

    /// A lash lying on the bottom edge or pressed against the right one is at rest too: its
    /// points stop moving, whatever the wall bounce keeps in `prev`.
    #[test]
    fn settles_against_a_screen_edge() {
        let p = Params::default();
        for hand in [(900.0, 1000.0), (1900.0, 600.0)] {
            let mut r = Rope::spawn(&p, hand);
            for _ in 0..3000 {
                r.step(&p, hand, B, false);
            }
            let m = moved(&mut r, &p, hand);
            assert!(m < 0.05, "hand {hand:?}: moved {m}");
        }
    }

    #[test]
    fn heavy_handle_trails_more() {
        let light = Params { weight: 0.6, ..Params::default() };
        let heavy = Params { weight: 1.8, ..Params::default() };
        let (mut a, mut b) = (Rope::spawn(&light, (500.0, 500.0)), Rope::spawn(&heavy, (500.0, 500.0)));
        a.step(&light, (600.0, 500.0), B, false);
        b.step(&heavy, (600.0, 500.0), B, false);
        assert!(a.pts[0].0 > b.pts[0].0 + 20.0, "light {} heavy {}", a.pts[0].0, b.pts[0].0);
    }

    #[test]
    fn kick_throws_the_tip_and_rest_is_still() {
        let p = Params::default();
        let mut r = Rope::spawn(&p, (900.0, 600.0));
        for _ in 0..900 {
            r.step(&p, (900.0, 600.0), B, false);
        }
        let m = moved(&mut r, &p, (900.0, 600.0));
        assert!(m < 0.05, "at rest: {m}");
        let x = r.pts[r.pts.len() - 1].0;
        r.kick(-1);
        r.step(&p, (900.0, 600.0), B, false);
        assert!(r.pts[r.pts.len() - 1].0 < x - 5.0, "tip went left");
    }

    #[test]
    fn dropped_rope_falls_off() {
        let p = Params::default();
        let mut r = Rope::spawn(&p, (900.0, 600.0));
        for _ in 0..600 {
            r.step(&p, (0.0, 0.0), B, true);
        }
        assert!(r.fallen_off(1080.0));
    }
}
