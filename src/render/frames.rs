//! The off-screen frame renderer: any whip's swing or any crack effect to PNG frames and a
//! contact sheet, with no window, no overlay and no keystrokes. See design/renderer.md.
//!
//! ```text
//! NJ_WHIP_NAME=ninja NJ_WHIP_FRAMES=target/frames \
//!   cargo test --release --locked render::frames::frames -- --ignored --exact --nocapture
//! ```
//!
//! `NJ_WHIP_NAME`: a whip name (`Classic`, `Ninja`...), an effect name (`Sparks`, `Slash`...) or
//! `all` (default). `NJ_WHIP_FRAMES`: output folder (default `target/frames`). `NJ_WHIP_N`:
//! frames per run (default 12, at least 6). Each run writes `<folder>/<kind>-<name>/NNN.png`
//! and `<folder>/<kind>-<name>/sheet.png`; `<folder>/index.txt` lists what was written.

use super::*;
use crate::physics::Rope;
use crate::whips::{Effect, CATALOGUE};

const W: u32 = 960;
const H: u32 = 540;
const BACKGROUND: u32 = 0x0A1024; // Polar Night
const COLS: usize = 4;

fn slug(name: &str) -> String {
    name.to_lowercase().replace(' ', "-")
}

/// One frame over the background, as the screen shows it.
fn on_background(pm: &Pixmap) -> Pixmap {
    let mut page = Pixmap::new(pm.width(), pm.height()).unwrap();
    page.fill(col(BACKGROUND, 1.0));
    page.draw_pixmap(0, 0, pm.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
    page
}

/// Writes the frames and the contact sheet (frames at a third of their size, `COLS` per row).
fn write_run(dir: &std::path::Path, frames: &[Pixmap]) {
    std::fs::create_dir_all(dir).unwrap();
    let (cw, ch) = (W / 3, H / 3);
    let rows = frames.len().div_ceil(COLS) as u32;
    let mut sheet = Pixmap::new(cw * COLS as u32, ch * rows).unwrap();
    sheet.fill(col(BACKGROUND, 1.0));
    for (i, f) in frames.iter().enumerate() {
        f.save_png(dir.join(format!("{i:03}.png"))).unwrap();
        let (x, y) = ((i % COLS) as u32 * cw, (i / COLS) as u32 * ch);
        let tf = Transform::from_translate(x as f32, y as f32).pre_scale(1.0 / 3.0, 1.0 / 3.0);
        sheet.draw_pixmap(0, 0, f.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() }, tf, None);
    }
    sheet.save_png(dir.join("sheet.png")).unwrap();
}

/// A whip swung across the screen, one crack a third of the way in.
fn swing(w: &WhipDef, n: usize) -> Vec<Pixmap> {
    fastrand::seed(2026);
    let p = w.physics;
    let work = Rect::from_xywh(0.0, 0.0, W as f32, H as f32).unwrap();
    let bounds = (0.0, 0.0, W as f32, H as f32);
    let mut rope = Rope::spawn(&p, (W as f32 / 2.0, H as f32 * 0.7));
    let mut fx = Fx::default();
    let mut canvas = Canvas::new(W, H).unwrap();
    let crack = n / 3;
    let mut cracked = None;
    let mut out = Vec::new();
    for i in 0..n {
        for s in 0..4 {
            let t = (i * 4 + s) as f32 / 60.0;
            let hand = (W as f32 / 2.0 + 300.0 * (t * 4.0).sin(), H as f32 * 0.65 + 90.0 * (t * 2.5).cos());
            rope.step(&p, hand, bounds, false);
            fx.step(1.0 / 60.0);
        }
        if i == crack {
            rope.kick(1);
            let at = *rope.pts.last().unwrap();
            fx.burst(w.effect, at, (W as f32, H as f32));
            cracked = Some(i);
        }
        let flash = cracked.map_or(0.0, |c| (1.0 - (i - c) as f32 / 3.0).max(0.0));
        let scene = Scene { whip: Some((&rope.pts[..], w, None)), flash, hue: w.vibe.hue, burst: Some(&fx), ..Scene::none() };
        canvas.draw(&scene, work);
        out.push(on_background(&canvas.pm));
    }
    out
}

/// One crack effect at the middle of the screen: the burst in steps of 70 ms, then the mark it
/// leaves at 1.0, 2.0 and 2.9 s.
fn effect(e: Effect, n: usize) -> Vec<Pixmap> {
    fastrand::seed(2026);
    let work = Rect::from_xywh(0.0, 0.0, W as f32, H as f32).unwrap();
    let mut fx = Fx::default();
    let mut canvas = Canvas::new(W, H).unwrap();
    fx.burst(e, (W as f32 / 2.0, H as f32 / 2.0), (W as f32, H as f32));
    let tail = [1.0f32, 2.0, 2.9];
    let mut clock = 0.0f32;
    let mut out = Vec::new();
    for i in 0..n {
        let want = if i + tail.len() >= n { tail[i + tail.len() - n] } else { i as f32 * 0.07 };
        while clock + 1.0 / 120.0 < want {
            fx.step(1.0 / 60.0);
            clock += 1.0 / 60.0;
        }
        canvas.draw(&Scene { burst: Some(&fx), ..Scene::none() }, work);
        out.push(on_background(&canvas.pm));
    }
    out
}

/// Every whip swing and every crack effect (or the one named in `NJ_WHIP_NAME`) as frames and a sheet.
#[test]
#[ignore]
fn frames() {
    let root = std::path::PathBuf::from(std::env::var("NJ_WHIP_FRAMES").unwrap_or_else(|_| "target/frames".into()));
    let want = std::env::var("NJ_WHIP_NAME").unwrap_or_else(|_| "all".into()).to_lowercase();
    let n: usize = std::env::var("NJ_WHIP_N").ok().and_then(|v| v.parse().ok()).unwrap_or(12).max(6);
    let all = want == "all";
    let mut index = String::new();
    for w in CATALOGUE.iter().filter(|w| all || w.name.to_lowercase() == want) {
        let dir = root.join(format!("whip-{}", slug(w.name)));
        write_run(&dir, &swing(w, n));
        index += &format!("whip {} -> {}\n", w.name, dir.join("sheet.png").display());
    }
    let mut seen: Vec<Effect> = Vec::new();
    for e in CATALOGUE.iter().map(|w| w.effect) {
        if seen.contains(&e) || !(all || format!("{e:?}").to_lowercase() == want) {
            continue;
        }
        seen.push(e);
        let dir = root.join(format!("effect-{}", slug(&format!("{e:?}"))));
        write_run(&dir, &effect(e, n));
        index += &format!("effect {e:?} -> {}\n", dir.join("sheet.png").display());
    }
    assert!(!index.is_empty(), "no whip or effect is named {want:?}; try a name from design/renderer.md or `all`");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("index.txt"), &index).unwrap();
    println!("{index}");
}
