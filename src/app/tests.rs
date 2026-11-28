use super::*;
use crate::physics::Params;

#[test]
fn pace_sleeps_only_when_flush_returned_early() {
    let ms = Duration::from_millis;
    assert_eq!(pace_sleep(ms(10), ms(11)), Duration::ZERO, "a real wait is left alone");
    assert_eq!(pace_sleep(ms(0), ms(3)), ms(13), "instant flush: pad to 16 ms since frame start");
    assert_eq!(pace_sleep(ms(2), ms(5)), Duration::ZERO, "a short wait on a fast screen is real too");
    assert_eq!(pace_sleep(Duration::from_micros(300), ms(20)), Duration::ZERO, "frame already took longer");
}

/// The whole chain as the app runs it (frames of any length -> fixed steps -> weighted
/// handle -> swing) reads a swing the same at every refresh rate, with uneven frames:
/// a lazy sweep never cracks, a snap cracks exactly once.
#[test]
fn swing_reads_the_same_at_every_frame_rate() {
    const B: (f32, f32, f32, f32) = (0.0, 0.0, 1920.0, 1080.0);
    // The mouse's x at time t (s), the motions of swing.rs's own whole-chain test.
    // lazy: 300 px each way, once a second
    let lazy = |t: f32| 960.0 + 300.0 * (t * std::f32::consts::TAU).cos();
    // snap: 250 px to the left in 10 steps, then hard right at 3000 px/s for 12 steps
    let snap = |t: f32| {
        let s = t * 60.0;
        if s <= 10.0 { 1000.0 - 25.0 * s } else { 750.0 + 50.0 * (s - 10.0).min(12.0) }
    };
    let mut rng = fastrand::Rng::with_seed(2026);
    for weight in [0.6, 1.0, 1.8] {
        let p = Params { weight, ..Params::default() };
        for hz in [60.0, 75.0, 144.0, 240.0] {
            let mut cracks = |mouse: &dyn Fn(f32) -> f32, secs: f32| {
                let mut prev = (mouse(0.0), 600.0);
                let mut rope = Rope::spawn(&p, prev);
                let (mut swing, mut acc, mut t, mut n) = (Swing::default(), 0.0, 0.0, 0);
                while t < secs {
                    let dt = 1.0 / hz + (rng.f32() - 0.5) * 0.001; // +-0.5 ms of frame jitter
                    t += dt;
                    let at = (mouse(t), 600.0);
                    each_step(&mut acc, dt, prev, at, |hand| {
                        rope.step(&p, hand, B, false);
                        if let Some(Event::Crack { .. }) = swing.step(rope.hand_vel().0) {
                            n += 1;
                        }
                    });
                    prev = at;
                }
                n
            };
            assert_eq!(cracks(&lazy, 8.0), 0, "weight {weight} at {hz} Hz: lazy sweep");
            assert_eq!(cracks(&snap, 1.0), 1, "weight {weight} at {hz} Hz: snap");
        }
    }
}
