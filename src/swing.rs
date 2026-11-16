//! Swing detection: a whip cracks when the hand winds up one way and is snapped back the other.
//!
//! Fed the hand's horizontal speed once per physics step (px per 1/60 s). A swing in one
//! direction builds up travel; a reversal after enough wind-up arms a crack, and the crack
//! fires only when the hand reaches snap speed in the new direction within a short window.
//! So: lazy back-and-forth swishes (a whoosh, no crack); one snap = exactly one crack; the
//! next crack needs a new reversal, so cracks always alternate right / left.

/// Slower than this (px/step) is "not moving".
const DEAD: f32 = 1.5;
/// A change of direction counts from this speed: a hand resting at the turn wobbles a pixel or
/// two the other way, and that must not use up the wind-up.
const REVERSE: f32 = 4.0;
/// Steps of stillness after which a wind-up is forgotten.
const IDLE_STEPS: u32 = 12;
/// A swing announces itself (whoosh) at this speed, once it has travelled this far.
const SWING_SPEED: f32 = 10.0;
const WHOOSH_TRAVEL: f32 = 40.0;
/// Wind-up travel (px) needed before a reversal can crack.
const CHARGE: f32 = 140.0;
/// The snap: this speed in the new direction, reached within the window after the reversal.
/// The window is the same for every weight: a heavy handle already answers late, and a longer
/// window would let its lazy sweeps crack.
const SNAP_SPEED: f32 = 24.0;
const SNAP_WINDOW: u32 = 6;
/// Steps between two cracks at least: scribbling cracks a few times a second, not on every wiggle.
const CRACK_GAP: u32 = 8;
/// Speed that counts as full power.
const FULL_POWER: f32 = 60.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// The whip is cutting the air (`dir`: -1 left, 1 right; `power` 0..1).
    Whoosh { dir: i8, power: f32 },
    Crack { dir: i8, power: f32 },
}

#[derive(Default)]
pub struct Swing {
    dir: i8,
    travel: f32,
    peak: f32,
    whooshed: bool,
    idle: u32,
    /// A charged reversal waiting for its snap: (new direction, steps left).
    armed: Option<(i8, u32)>,
    /// Steps until the next crack is allowed.
    cooldown: u32,
}

impl Swing {
    /// One physics step: `vx` is the handle's horizontal speed (px per step, sign = direction).
    pub fn step(&mut self, vx: f32) -> Option<Event> {
        let speed = vx.abs();
        let d: i8 = if vx > DEAD { 1 } else if vx < -DEAD { -1 } else { 0 };
        self.cooldown = self.cooldown.saturating_sub(1);
        if d != 0 && d != self.dir && (self.dir == 0 || speed >= REVERSE) {
            self.armed = (self.dir != 0 && self.travel >= CHARGE && self.peak >= SWING_SPEED).then_some((d, SNAP_WINDOW));
            self.dir = d;
            self.travel = speed;
            self.peak = speed;
            self.whooshed = false;
            self.idle = 0;
        } else if d != 0 && d == self.dir {
            self.idle = 0;
            self.travel += speed;
            self.peak = self.peak.max(speed);
        } else {
            // still, or a wobble against the swing too small to be a reversal
            self.idle += 1;
            if self.idle > IDLE_STEPS {
                *self = Swing { cooldown: self.cooldown, ..Swing::default() };
            }
        }
        if let Some((nd, left)) = self.armed {
            if d == nd && speed >= SNAP_SPEED {
                self.armed = None;
                self.whooshed = true; // the crack is this swing's sound
                if self.cooldown == 0 {
                    self.cooldown = CRACK_GAP;
                    return Some(Event::Crack { dir: nd, power: (speed / FULL_POWER).clamp(0.3, 1.0) });
                }
                return None;
            }
            self.armed = (left > 0).then(|| (nd, left - 1));
        }
        // No whoosh while a crack is armed: the crack is this swing's sound; if the snap never
        // comes and the arm expires, the swing may whoosh then.
        if self.armed.is_none() && d != 0 && d == self.dir && !self.whooshed && speed >= SWING_SPEED && self.travel >= WHOOSH_TRAVEL {
            self.whooshed = true;
            return Some(Event::Whoosh { dir: d, power: (speed / FULL_POWER).clamp(0.2, 1.0) });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &mut Swing, speeds: impl IntoIterator<Item = f32>) -> Vec<Event> {
        speeds.into_iter().filter_map(|v| s.step(v)).collect()
    }

    fn cracks(ev: &[Event]) -> Vec<i8> {
        ev.iter().filter_map(|e| if let Event::Crack { dir, .. } = e { Some(*dir) } else { None }).collect()
    }

    /// `n` steps one way at `v`, then a hard snap the other way for `n` steps.
    fn windup_and_snap(v: f32, n: usize) -> Vec<f32> {
        (0..n).map(|_| v).chain((0..n).map(|_| -v * 2.0)).collect()
    }

    #[test]
    fn left_then_snap_right_cracks_right_once() {
        let mut s = Swing::default();
        let ev = run(&mut s, windup_and_snap(-20.0, 12));
        assert_eq!(cracks(&ev), vec![1], "{ev:?}");
    }

    #[test]
    fn no_whoosh_between_the_reversal_and_the_crack() {
        let mut s = Swing::default();
        // wind up left, reverse with a ramp (whoosh speed reached before snap speed), snap
        let ramp = [6.0, 12.0, 18.0, 22.0, 30.0];
        let mut ev = run(&mut s, (0..12).map(|_| -20.0));
        let mut between = Vec::new();
        for v in ramp {
            let e = s.step(v);
            between.extend(e);
            ev.extend(e);
        }
        assert_eq!(cracks(&ev), vec![1], "{ev:?}");
        let crack_at = between.iter().position(|e| matches!(e, Event::Crack { .. })).unwrap();
        assert!(!between[..crack_at].iter().any(|e| matches!(e, Event::Whoosh { .. })), "{between:?}");
        assert!(!between.iter().any(|e| matches!(e, Event::Whoosh { .. })), "{between:?}");
    }

    #[test]
    fn one_swing_never_cracks_twice() {
        let mut s = Swing::default();
        // wind up left, snap right, then keep flying right for a long time
        let ev = run(&mut s, (0..12).map(|_| -20.0).chain((0..200).map(|_| 45.0)));
        assert_eq!(cracks(&ev).len(), 1, "{ev:?}");
    }

    #[test]
    fn streak_alternates_sides() {
        let mut s = Swing::default();
        let mut speeds = Vec::new();
        for k in 0..6 {
            let v = if k % 2 == 0 { -40.0 } else { 40.0 };
            speeds.extend((0..10).map(|_| v));
        }
        let c = cracks(&run(&mut s, speeds));
        assert_eq!(c, vec![1, -1, 1, -1, 1]);
    }

    #[test]
    fn lazy_back_and_forth_whooshes_but_never_cracks() {
        let mut s = Swing::default();
        // a 1 s sine sweep, 300 px each way: fast in the middle, soft at the turns
        let ev = run(&mut s, (0..600).map(|i| 31.0 * (i as f32 * std::f32::consts::TAU / 60.0).sin()));
        assert!(cracks(&ev).is_empty(), "{ev:?}");
        assert!(ev.iter().any(|e| matches!(e, Event::Whoosh { .. })));
    }

    #[test]
    fn a_wobble_at_the_turn_keeps_the_windup() {
        let mut s = Swing::default();
        // wind up left, rest with a pixel or two of noise both ways, then snap right
        let ev = run(&mut s, (0..12).map(|_| -20.0).chain([2.0, -2.0, 3.0, 0.5, -2.5]).chain((0..10).map(|_| 45.0)));
        assert_eq!(cracks(&ev), vec![1], "{ev:?}");
    }

    #[test]
    fn scribbling_is_rate_limited() {
        let mut s = Swing::default();
        // 5-step strokes (a 6 Hz scribble, 200 px each way) for 2 s
        let ev = run(&mut s, (0..120).map(|i| if (i / 5) % 2 == 0 { -40.0 } else { 40.0 }));
        let n = cracks(&ev).len();
        assert!((6..=120 / CRACK_GAP as usize).contains(&n), "{n} cracks");
    }

    /// The whole chain as the app runs it: mouse -> weighted handle -> swing, for the lightest,
    /// the normal and the heaviest whip.
    #[test]
    fn every_weight_cracks_once_on_a_snap_and_never_on_a_lazy_sweep() {
        use crate::physics::{Params, Rope};
        const B: (f32, f32, f32, f32) = (0.0, 0.0, 1920.0, 1080.0);
        for weight in [0.6, 1.0, 1.8] {
            let p = Params { weight, ..Params::default() };
            let feed = |xs: Vec<f32>| {
                let mut r = Rope::spawn(&p, (xs[0], 600.0));
                let mut s = Swing::default();
                let ev: Vec<Event> = xs.iter().filter_map(|&x| {
                    r.step(&p, (x, 600.0), B, false);
                    s.step(r.hand_vel().0)
                }).collect();
                cracks(&ev)
            };
            // snap: 250 px to the left in 10 steps, then hard right at 3000 px/s
            let snap: Vec<f32> = (0..=10).map(|i| 1000.0 - 25.0 * i as f32).chain((1..=12).map(|i| 750.0 + 50.0 * i as f32)).collect();
            assert_eq!(feed(snap), vec![1], "weight {weight}: snap");
            // lazy: 300 px each way, once a second, for 4 s
            let lazy: Vec<f32> = (0..240).map(|i| 960.0 + 300.0 * (i as f32 * std::f32::consts::TAU / 60.0).cos()).collect();
            assert!(feed(lazy).is_empty(), "weight {weight}: lazy sweep");
            // pointing: glide 900 px to a target, overshoot 40 px, settle back
            let point: Vec<f32> = (0..=30).map(|i| 400.0 + 940.0 * (0.5 - 0.5 * (i as f32 / 30.0 * std::f32::consts::PI).cos()))
                .chain((1..=8).map(|i| 1340.0 - 5.0 * i as f32)).chain((0..30).map(|_| 1300.0)).collect();
            assert!(feed(point).is_empty(), "weight {weight}: pointing");
        }
    }

    #[test]
    fn no_windup_no_crack() {
        let mut s = Swing::default();
        // a short twitch left (30 px) then a hard move right
        let ev = run(&mut s, (0..3).map(|_| -10.0).chain((0..20).map(|_| 50.0)));
        assert!(cracks(&ev).is_empty(), "{ev:?}");
    }

    #[test]
    fn jitter_and_slow_moves_are_silent() {
        let mut s = Swing::default();
        let ev = run(&mut s, (0..300).map(|i| if i % 2 == 0 { 3.0 } else { -3.0 }));
        assert!(ev.is_empty(), "{ev:?}");
        let ev = run(&mut s, (0..300).map(|_| 5.0));
        assert!(ev.is_empty(), "{ev:?}");
    }

    #[test]
    fn a_pause_forgets_the_windup() {
        let mut s = Swing::default();
        let ev = run(&mut s, (0..12).map(|_| -20.0).chain((0..30).map(|_| 0.0)).chain((0..12).map(|_| 50.0)));
        assert!(cracks(&ev).is_empty(), "{ev:?}");
    }
}
