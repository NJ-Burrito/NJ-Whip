use super::*;
use crate::whips::CATALOGUE;

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
/// writes target/marks-<secs>.png (catalogue order, five to a row, one second after the crack;
/// `NJW_MARK_SECS=2.5` for another moment).
#[test]
#[ignore]
fn marks_sheet() {
    let mut pm = Pixmap::new(1100, 540).unwrap();
    pm.fill(crate::ink::col(0x0A1024, 1.0));
    let mut fx = Fx::default();
    for (k, w) in CATALOGUE.iter().enumerate() {
        fx.burst(w.effect, (110.0 + (k % 5) as f32 * 220.0, 90.0 + (k / 5) as f32 * 180.0), (1100.0, 540.0));
    }
    let secs: f32 = std::env::var("NJW_MARK_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    for _ in 0..(secs * 60.0) as usize {
        fx.step(1.0 / 60.0);
    }
    fx.draw(&mut pm);
    pm.save_png(format!("target/marks-{secs}.png")).unwrap();
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