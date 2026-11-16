//! The heat meter (how close the streak is to the whip's full screen effect) and the combo count.

use std::time::Instant;

/// Cracks from empty to full.
pub const CRACKS_TO_FULL: u32 = 4;
/// The meter holds this long after a crack, then drains.
const HOLD: f32 = 1.2;
/// Drain per second once the hold is over.
const DRAIN: f32 = 0.45;
/// The full effect ends when the level falls below this.
const FULL_OFF: f32 = 0.6;
/// How fast the shown bar chases the real level (per second): quick up, smooth down.
const RISE: f32 = 14.0;
const FALL: f32 = 5.0;
/// How fast the full effect fades in and out (per second).
const GLOW_RATE: f32 = 6.0;

#[derive(Default)]
pub struct Heat {
    level: f32,
    shown: f32,
    full: bool,
    glow: f32,
    since_crack: f32,
}

impl Heat {
    pub fn crack(&mut self) {
        self.level = (self.level + 1.0 / CRACKS_TO_FULL as f32).min(1.0);
        self.since_crack = 0.0;
        if self.level >= 1.0 {
            self.full = true;
        }
    }

    pub fn step(&mut self, dt: f32) {
        self.since_crack += dt;
        if self.since_crack > HOLD {
            self.level = (self.level - DRAIN * dt).max(0.0);
        }
        if self.level < FULL_OFF {
            self.full = false;
        }
        let k = if self.level > self.shown { RISE } else { FALL };
        self.shown += (self.level - self.shown) * (1.0 - (-dt * k).exp());
        if self.level == 0.0 && self.shown < 0.002 {
            self.shown = 0.0;
        }
        let target = if self.full { 1.0 } else { 0.0 };
        self.glow += (target - self.glow) * (1.0 - (-dt * GLOW_RATE).exp());
        if !self.full && self.glow < 0.004 {
            self.glow = 0.0;
        }
    }

    /// The bar as drawn, 0..1 (smoothed).
    pub fn shown(&self) -> f32 {
        self.shown
    }

    pub fn full(&self) -> bool {
        self.full
    }

    /// The full effect's strength, 0..1: it fades in when the meter fills and out when it ends.
    pub fn glow(&self) -> f32 {
        self.glow
    }

    /// Nothing left to show or animate.
    pub fn cold(&self) -> bool {
        self.level == 0.0 && self.shown == 0.0 && self.glow == 0.0
    }
}

/// A new crack within this time continues the combo.
const COMBO_GAP: f32 = 1.4;
/// The combo label stays this long after its crack.
pub const COMBO_SHOW: f32 = 1.5;

/// Counts on the wall clock, not on frames: the engine sleeps between cracks.
#[derive(Default)]
pub struct Combo {
    count: u32,
    last: Option<Instant>,
}

impl Combo {
    /// Seconds from the last crack to `now` (None before the first one).
    fn age(&self, now: Instant) -> Option<f32> {
        self.last.map(|t| now.saturating_duration_since(t).as_secs_f32())
    }

    /// Registers a crack at `now` and returns the combo count after it.
    pub fn crack(&mut self, now: Instant) -> u32 {
        if self.age(now).is_none_or(|a| a > COMBO_GAP) {
            self.count = 0;
        }
        self.count += 1;
        self.last = Some(now);
        self.count
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    /// A streak gets its label from the 2nd crack on.
    pub fn labelled(&self) -> bool {
        self.count >= 2
    }

    /// Seconds since the last crack while the label is still up.
    pub fn visible(&self, now: Instant) -> Option<f32> {
        self.age(now).filter(|&a| self.labelled() && a < COMBO_SHOW)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const DT: f32 = 1.0 / 60.0;

    fn run(h: &mut Heat, secs: f32) {
        for _ in 0..(secs / DT) as u32 {
            h.step(DT);
        }
    }

    #[test]
    fn fills_on_the_fourth_crack() {
        let mut h = Heat::default();
        for _ in 0..3 {
            h.crack();
        }
        assert!(!h.full());
        h.crack();
        assert!(h.full());
    }

    #[test]
    fn bar_rises_fast_and_falls_smoothly() {
        let mut h = Heat::default();
        h.crack();
        run(&mut h, 0.3);
        assert!((h.shown() - 0.25).abs() < 0.02, "rose to {}", h.shown());
        // during the hold nothing drains
        run(&mut h, 0.8);
        assert!((h.shown() - 0.25).abs() < 0.01);
        // then it drains with no jump between frames
        let mut last = h.shown();
        for _ in 0..240 {
            h.step(DT);
            assert!(h.shown() <= last + 1e-6 && last - h.shown() < 0.02, "{last} -> {}", h.shown());
            last = h.shown();
        }
        assert!(h.cold());
    }

    #[test]
    fn full_effect_ends_when_the_streak_stops() {
        let mut h = Heat::default();
        for _ in 0..4 {
            h.crack();
        }
        run(&mut h, 1.0);
        assert!(h.full(), "still inside the hold");
        run(&mut h, 2.0);
        assert!(!h.full());
    }

    #[test]
    fn a_streak_keeps_it_full() {
        let mut h = Heat::default();
        for _ in 0..12 {
            h.crack();
            run(&mut h, 0.5);
        }
        assert!(h.full());
    }

    #[test]
    fn combo_counts_shows_and_expires() {
        let mut c = Combo::default();
        let t0 = Instant::now();
        let at = |secs: f32| t0 + Duration::from_secs_f32(secs);
        assert_eq!(c.crack(t0), 1);
        assert!(c.visible(at(0.1)).is_none(), "a single crack is not a combo");
        assert_eq!(c.crack(at(0.5)), 2);
        assert!((c.visible(at(0.8)).unwrap() - 0.3).abs() < 1e-3, "age counts from the last crack");
        assert!(c.visible(at(0.6 + COMBO_SHOW)).is_none(), "the label disappears");
        assert_eq!(c.crack(at(0.6 + COMBO_SHOW)), 1, "and the next crack starts over");
    }
}
