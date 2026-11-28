use super::*;
use crate::physics::Rope;
use crate::whips::{def, WhipId, CATALOGUE};

fn full() -> Rect {
    Rect::from_xywh(0.0, 0.0, 800.0, 600.0).unwrap()
}

fn rope() -> Rope {
    Rope::spawn(&def(WhipId::Classic).physics, (100.0, 500.0))
}

fn with_whip(r: &Rope) -> Scene<'_> {
    Scene { whip: Some((&r.pts[..], def(WhipId::Classic), None)), ..Scene::none() }
}

#[test]
fn hiding_the_whip_leaves_nothing_behind() {
    let r = rope();
    let mut c = Canvas::new(800, 600).unwrap();
    c.draw(&with_whip(&r), full());
    c.draw(&Scene::none(), full());
    assert!(c.pm.pixels().iter().all(|p| p.alpha() == 0));
}

#[test]
fn rope_pixels_opaque() {
    let r = rope();
    let pm = draw_once(800, 600, &with_whip(&r), full());
    let (x, y) = r.pts[5];
    assert!(pm.pixel(x as u32, y as u32).unwrap().alpha() > 200);
}

#[test]
fn background_alpha_one() {
    let r = rope();
    let pm = draw_once(800, 600, &with_whip(&r), full());
    assert_eq!(pm.pixel(799, 0).unwrap().alpha(), 1);
}

#[test]
fn glow_is_at_the_edges_in_the_whips_colour() {
    let r = rope();
    let pm = draw_once(800, 600, &Scene { glow: 1.0, hue: 0x33D6FF, ..with_whip(&r) }, full());
    let (edge, centre) = (pm.pixel(0, 300).unwrap(), pm.pixel(400, 300).unwrap());
    assert!(edge.alpha() > centre.alpha(), "edge {} centre {}", edge.alpha(), centre.alpha());
    assert!(edge.blue() > edge.red(), "cyan, not red");
}

#[test]
fn offset_shifts_the_glow() {
    let r = rope();
    let pm = draw_once(800, 600, &Scene { glow: 1.0, offset: (10.0, 0.0), ..with_whip(&r) }, full());
    // the left strip moved right: x=2 is now outside it, x=14 inside
    assert!(pm.pixel(14, 300).unwrap().alpha() > pm.pixel(2, 300).unwrap().alpha());
}

#[test]
fn click_area_is_the_work_area_only() {
    let r = rope();
    // Work area = everything but a 40 px taskbar strip at the bottom.
    let work = Rect::from_xywh(0.0, 0.0, 800.0, 560.0).unwrap();
    let pm = draw_once(800, 600, &Scene { glow: 1.0, ..with_whip(&r) }, work);
    assert_eq!(pm.pixel(600, 300).unwrap().alpha(), 1, "inside the work area, clear of the glow");
    assert_eq!(pm.pixel(700, 580).unwrap().alpha(), 0, "taskbar strip lets clicks through");
    assert!(pm.pixel(700, 558).unwrap().alpha() > 1, "the glow hugs the work area");
}

#[test]
fn meter_fills_from_the_left_at_the_top() {
    let pm = draw_once(800, 600, &Scene { meter: Some((0.5, false)), hue: 0x33D6FF, ..Scene::none() }, full());
    let (x, y) = meter_origin(full(), 1.0);
    let (cy, filled, empty) = ((y + METER.1 / 2.0 + 2.0) as u32, (x + METER.0 * 0.2) as u32, (x + METER.0 * 0.85) as u32);
    let (f, e) = (pm.pixel(filled, cy).unwrap(), pm.pixel(empty, cy).unwrap());
    assert!(f.blue() > 150 && f.alpha() == 255, "filled part is the hue: {f:?}");
    assert!(e.blue() < 120, "empty part is the dark track: {e:?}");
    assert_eq!(pm.pixel(400, 300).unwrap().alpha(), 0, "nothing else on screen");
}

#[test]
fn merge_joins_boxes_that_overlap_enough() {
    // one inside the other, two that share an edge, and a far one
    let mut v = vec![(0, 0, 100, 100), (10, 10, 50, 50), (100, 0, 200, 100), (500, 500, 510, 510)];
    merge(&mut v);
    v.sort();
    assert_eq!(v, vec![(0, 0, 200, 100), (500, 500, 510, 510)]);
    // two edge strips: their bounding box is the whole screen, so they stay apart
    let mut strips = vec![(0, 0, 1000, 90), (0, 0, 90, 800)];
    merge(&mut strips);
    assert_eq!(strips.len(), 2);
}

/// The whip never paints outside the boxes it reports (a stray pixel would stay on screen):
/// every whip, swung hard and cracked.
#[test]
fn whip_stays_inside_its_boxes() {
    let work = Rect::from_xywh(0.0, 0.0, 900.0, 700.0).unwrap();
    for w in &CATALOGUE {
        let p = w.physics;
        let mut r = Rope::spawn(&p, (450.0, 450.0));
        for f in 0..36 {
            let t = f as f32 / 60.0;
            r.step(&p, (450.0 + 300.0 * (t * 9.0).sin(), 420.0 + 120.0 * (t * 5.0).cos()), (0.0, 0.0, 900.0, 700.0), false);
            if f == 8 {
                r.kick(1);
            }
            if f % 3 != 0 {
                continue;
            }
            let s = Scene { whip: Some((&r.pts[..], w, None)), flash: 1.0, ..Scene::none() };
            let boxes = &layers(&s, 900, 700, work)[L_WHIP].0;
            let pm = draw_once(900, 700, &s, work);
            for (i, px) in pm.data().chunks_exact(4).enumerate() {
                if px != [0, 0, 0, 1] {
                    let (x, y) = ((i % 900) as i32, (i / 900) as i32);
                    assert!(boxes.iter().any(|b| x >= b.0 && x < b.2 && y >= b.1 && y < b.3), "{} step {f}: pixel ({x}, {y}) outside {boxes:?}", w.name);
                }
            }
        }
    }
}

/// The incrementally updated canvas must equal a from-scratch drawing of the same scene.
fn assert_fresh(c: &Canvas, s: &Scene, work: Rect, what: &str) {
    let (w, h) = c.size();
    let fresh = draw_once(w, h, s, work);
    if c.pm.data() != fresh.data() {
        let i = c.pm.data().iter().zip(fresh.data()).position(|(a, b)| a != b).unwrap() / 4;
        panic!("{what}: pixel ({}, {}) differs", i % w as usize, i / w as usize);
    }
}

/// The heart of the partial redraw: frame after frame, the canvas must equal a from-scratch
/// drawing, for every whip: with everything moving at once, then with the whip still under
/// a draining meter (only the meter is redrawn), then with nothing changing (no redraw).
#[test]
fn partial_redraw_equals_full_redraw() {
    let work = Rect::from_xywh(0.0, 0.0, 900.0, 660.0).unwrap();
    let bounds = (0.0, 0.0, 900.0, 700.0);
    let label = PathBuilder::from_rect(Rect::from_xywh(0.0, -70.0, 320.0, 70.0).unwrap());
    for w in &CATALOGUE {
        let p = w.physics;
        let mut r = Rope::spawn(&p, (450.0, 450.0));
        let mut fx = Fx::default();
        let mut c = Canvas::new(900, 700).unwrap();
        for f in 0..40 {
            let t = f as f32 / 60.0;
            let hand = (450.0 + 300.0 * (t * 9.0).sin(), 420.0 + 120.0 * (t * 5.0).cos());
            r.step(&p, hand, bounds, false);
            if f == 8 {
                r.kick(1);
                fx.burst(w.effect, *r.pts.last().unwrap(), (900.0, 700.0));
            }
            fx.step(1.0 / 60.0);
            // the whip's own screen effect (between them the whips use every variant), with
            // a jolt from the crack at frame 8
            let crack = (f >= 8).then(|| ((f - 8) as f32 / 60.0, 0.7));
            let (level, offset, edges) = w.vibe.screen.look(t * 4.0, crack);
            let s = Scene {
                whip: (f < 30).then_some((&r.pts[..], w, None)),
                flash: if (8..14).contains(&f) { 0.7 } else { 0.0 },
                glow: if f > 12 { 0.8 * level } else { 0.0 },
                edges,
                hue: w.vibe.hue,
                offset,
                // the display scaling changes mid-run
                scale: if f < 26 { 1.0 } else { 1.5 },
                meter: (f > 4).then_some((t, f > 20)),
                clock: t,
                combo: (10..34).contains(&f).then(|| ComboView {
                    path: &label,
                    width: 320.0,
                    at: *r.pts.last().unwrap(),
                    age: (f - 10) as f32 / 60.0,
                    count: if f < 22 { 3 } else { 5 },
                    dir: [1, -1, 0][(f / 8 % 3) as usize],
                }),
                burst: Some(&fx),
            };
            c.draw(&s, work);
            assert_fresh(&c, &s, work, &format!("{} frame {f}", w.name));
        }

        // The whip hangs still, clear of the meter, while the meter drains.
        let hand = (300.0, 320.0);
        let mut r = Rope::spawn(&p, hand);
        for _ in 0..300 {
            r.step(&p, hand, bounds, false);
        }
        let still = |level: f32| Scene { whip: Some((&r.pts[..], w, None)), meter: Some((level, false)), hue: w.vibe.hue, ..Scene::none() };
        c.draw(&still(0.9), work);
        let whip_boxes = layers(&still(0.9), 900, 700, work)[L_WHIP].0.clone();
        for k in 1..4 {
            let (s, what) = (still(0.9 - 0.2 * k as f32), format!("{} still frame {k}", w.name));
            let dirty = c.draw(&s, work);
            assert!(!dirty.is_empty(), "{what}: the meter changed");
            assert!(dirty.iter().all(|&d| whip_boxes.iter().all(|&b| !overlap(d, b))), "{what}: {dirty:?} touches the whip {whip_boxes:?}");
            assert_fresh(&c, &s, work, &what);
            assert!(c.draw(&s, work).is_empty(), "{what} again: nothing changed, nothing redrawn");
            assert_fresh(&c, &s, work, &what);
        }
    }

    // One field at a time on an otherwise still scene: a layer whose key misses the field
    // would keep its old pixels.
    let combo = |count: u32, dir: i8| Some(ComboView { path: &label, width: 320.0, at: (450.0, 400.0), age: 0.5, count, dir });
    let base = || Scene { glow: 0.6, meter: Some((0.4, false)), combo: combo(3, 0), ..Scene::none() };
    let mut c = Canvas::new(900, 700).unwrap();
    for (what, s) in [
        ("top edge", Scene { edges: [0.5, 1.0, 1.0, 1.0], ..base() }),
        ("bottom edge", Scene { edges: [1.0, 0.5, 1.0, 1.0], ..base() }),
        ("left edge", Scene { edges: [1.0, 1.0, 0.5, 1.0], ..base() }),
        ("right edge", Scene { edges: [1.0, 1.0, 1.0, 0.5], ..base() }),
        ("scale", Scene { scale: 1.25, ..base() }),
        ("glow offset", Scene { offset: (7.0, -4.0), ..base() }),
        ("hue", Scene { hue: 0xFFD84A, ..base() }),
        ("full meter", Scene { meter: Some((0.4, true)), ..base() }),
        ("combo to the right", Scene { combo: combo(3, 1), ..base() }),
        ("combo to the left", Scene { combo: combo(3, -1), ..base() }),
        ("fifth combo", Scene { combo: combo(5, 0), ..base() }),
    ] {
        c.draw(&base(), work);
        assert_fresh(&c, &base(), work, &format!("back from before {what}"));
        assert!(!c.draw(&s, work).is_empty(), "{what}: the change is seen");
        assert_fresh(&c, &s, work, what);
    }
}

/// A whip brushing the bottom glow repaints that edge only, not the other three.
#[test]
fn a_whip_on_one_edge_repaints_only_that_edge() {
    let work = Rect::from_xywh(0.0, 0.0, 900.0, 700.0).unwrap();
    fn scene(pts: &[V]) -> Scene<'_> {
        Scene { whip: Some((pts, def(WhipId::Classic), None)), glow: 1.0, ..Scene::none() }
    }
    // hanging straight down mid-screen, its tip inside the bottom glow
    let pose = |x: f32| (0..28).map(|i| (x, 380.0 + 8.0 * i as f32)).collect::<Vec<V>>();
    let (a, b) = (pose(450.0), pose(453.0));
    let mut c = Canvas::new(900, 700).unwrap();
    c.draw(&scene(&a), work);
    let dirty = c.draw(&scene(&b), work);
    assert_fresh(&c, &scene(&b), work, "whip moved");
    let l = layers(&scene(&b), 900, 700, work);
    assert!(dirty.iter().any(|&d| l[1].0.iter().any(|&e| overlap(d, e))), "the bottom edge is repainted: {dirty:?}");
    for e in [0, 2, 3] {
        assert!(dirty.iter().all(|&d| l[e].0.iter().all(|&b| !overlap(d, b))), "edge {e} is left alone: {dirty:?} vs {:?}", l[e].0);
    }
}

/// Not a check, pictures to look at (whip + half meter, full meter + glow, combo ×2 and ×5,
/// over grey and over white): `cargo test --release --offline look_frames -- --ignored`.
#[test]
#[ignore]
fn look_frames() {
    let out = std::path::Path::new("target/look-out");
    std::fs::create_dir_all(out).unwrap();
    let glyphs = crate::glyphs::Glyphs::load().expect("the Thmanyah face is installed");
    let work = Rect::from_xywh(0.0, 0.0, 1280.0, 680.0).unwrap();
    for id in [WhipId::Classic, WhipId::Neon, WhipId::Elephant, WhipId::Zen, WhipId::Storm, WhipId::Chef, WhipId::Ninja, WhipId::Penguin] {
        let w = def(id);
        let (p, vibe) = (w.physics, w.vibe);
        // the hand sweeps right, high up, so the whole lash trails inside the picture
        let mut r = Rope::spawn(&p, (300.0, 130.0));
        for f in 0..60 {
            r.step(&p, (300.0 + 7.0 * f as f32, 130.0), (0.0, 0.0, 1280.0, 720.0), false);
        }
        let tip = *r.pts.last().unwrap();
        let (_, offset, edges) = vibe.screen.look(0.2, None);
        let labels = [2, 5].map(|n| glyphs.line(&format!("{} ×{n}", vibe.shout)).unwrap());
        let combo = |i: usize, count: u32, dir: i8| Some(ComboView { path: &labels[i].0, width: labels[i].1, at: tip, age: 0.3, count, dir });
        let whip = || Scene { whip: Some((&r.pts[..], w, None)), hue: vibe.hue, ..Scene::none() };
        for (shot, s) in [
            ("half", Scene { meter: Some((0.5, false)), ..whip() }),
            ("full", Scene { meter: Some((1.0, true)), glow: 1.0, edges, offset, ..whip() }),
            ("combo2", Scene { meter: Some((0.5, false)), flash: 0.8, combo: combo(0, 2, 1), ..whip() }),
            ("combo5", Scene { meter: Some((1.0, true)), combo: combo(1, 5, -1), ..whip() }),
        ] {
            let pm = draw_once(1280, 720, &s, work);
            for (ground, rgb) in [("grey", 0x5A6270), ("white", 0xFFFFFF)] {
                let mut page = Pixmap::new(1280, 720).unwrap();
                page.fill(col(rgb, 1.0));
                page.draw_pixmap(0, 0, pm.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
                page.save_png(out.join(format!("{}_{shot}_{ground}.png", w.name.to_lowercase()))).unwrap();
            }
        }
    }
}

const GOLDEN: &str = "target/engine-golden/frames.txt";

/// A scripted 120-frame run on one
/// canvas, for four whips: the whip swinging, cracks with their bursts and tip flash, the
/// meter filling to full and draining, the glow on with every screen effect in turn, the
/// combo label (×2, then the white ×5), then the whip hidden while the effects play out.
fn scripted_run(mut each: impl FnMut(&WhipDef, usize, &Pixmap)) {
    use crate::whips::Screen;
    let glyphs = crate::glyphs::Glyphs::load().expect("the Thmanyah face is installed");
    let work = Rect::from_xywh(0.0, 0.0, 1280.0, 672.0).unwrap();
    let bounds = (0.0, 0.0, 1280.0, 720.0);
    let screens = [Screen::Glow, Screen::Flicker, Screen::Wobble, Screen::Jolt, Screen::Rise, Screen::Fall, Screen::Sides];
    for (k, id) in [WhipId::Classic, WhipId::Neon, WhipId::Cheerleader, WhipId::Storm].into_iter().enumerate() {
        fastrand::seed(2026 + k as u64);
        let w = def(id);
        let (p, vibe) = (w.physics, w.vibe);
        let labels = [2, 5].map(|n| glyphs.line(&format!("{} ×{n}", vibe.shout)).unwrap());
        let mut r = Rope::spawn(&p, (640.0, 400.0));
        let mut fx = Fx::default();
        let mut c = Canvas::new(1280, 720).unwrap();
        let (mut crack_at, mut cracked) = ((0.0, 0.0), 0);
        for f in 0..120usize {
            let t = f as f32 / 60.0;
            // a wide swing, a rest at the bottom edge (frames 60..75), a swing again
            let hand = if (60..75).contains(&f) { (700.0, 600.0) } else { (640.0 + 420.0 * (t * 7.0).sin(), 380.0 + 200.0 * (t * 4.0).cos()) };
            r.step(&p, hand, bounds, false);
            if [10, 40, 92].contains(&f) {
                r.kick(if f == 40 { -1 } else { 1 });
                crack_at = *r.pts.last().unwrap();
                cracked = f;
                fx.burst(w.effect, crack_at, (1280.0, 720.0));
            }
            fx.step(1.0 / 60.0);
            let crack = Some(((f - cracked.min(f)) as f32 / 60.0, 0.7 + k as f32));
            let (level, offset, edges) = screens[(f / 11 + k) % 7].look(t * 3.0, crack);
            let heat = ((f as f32 - 28.0) / 12.0).clamp(0.0, 1.0) * (1.0 - (f as f32 - 104.0) / 14.0).clamp(0.0, 1.0);
            let s = Scene {
                whip: (f < 95).then_some((&r.pts[..], w, None)),
                flash: if f >= cracked && cracked > 0 { (1.0 - (f - cracked) as f32 / 9.0).max(0.0) } else { 0.0 },
                glow: heat * level,
                edges,
                hue: vibe.hue,
                offset,
                scale: [1.0, 1.25, 1.5, 1.0][k],
                meter: Some(((f as f32 / 45.0).min(1.0) * (1.0 - (f as f32 - 100.0) / 20.0).clamp(0.0, 1.0), (45..100).contains(&f))),
                clock: t,
                combo: (cracked > 10).then(|| {
                    let i = (cracked == 92) as usize;
                    ComboView { path: &labels[i].0, width: labels[i].1, at: crack_at, age: (f - cracked) as f32 / 60.0, count: [2, 5][i], dir: if cracked == 40 { -1 } else { 1 } }
                }),
                burst: Some(&fx),
            };
            c.draw(&s, work);
            each(w, f, &c.pm);
        }
    }
}

/// One line per frame of the scripted run: "whip frame hash-of-the-pixmap".
fn golden_frames() -> String {
    let mut out = String::new();
    scripted_run(|w, f, pm| {
        // FNV-1a over the pixmap, eight bytes at a time
        let hash = pm.data().chunks_exact(8).fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from_le_bytes(b.try_into().unwrap())).wrapping_mul(0x100000001b3));
        out += &format!("{} {f:03} {hash:016x}\n", w.name);
    });
    out
}

/// Every whip with its name on the Polar Night background (the README's gallery):
/// `cargo test --release --offline gallery -- --ignored` writes target/gallery.png.
#[test]
#[ignore]
fn gallery() {
    let glyphs = crate::glyphs::Glyphs::load().expect("the Thmanyah face is installed");
    let (cw, ch, cols) = (400.0, 270.0, 5);
    let mut page = Pixmap::new(cw as u32 * cols as u32, ch as u32 * 3).unwrap();
    page.fill(col(0x0A1024, 1.0));
    for (i, w) in CATALOGUE.iter().enumerate() {
        let (x, y) = ((i % cols) as f32 * cw, (i / cols) as f32 * ch);
        let pts = crate::whipdraw::preview_points(w.physics.points, (x + 70.0, y + 128.0), 0.55);
        crate::whipdraw::draw_whip(&mut page, &pts, w, None);
        let (path, width) = glyphs.line(&w.name.to_uppercase()).unwrap();
        let k = 20.0 / EM;
        let tf = Transform::from_translate(x + (cw - width * k) / 2.0, y + ch - 22.0).pre_scale(k, k);
        page.fill_path(&path, &paint(w.vibe.hue, 1.0), FillRule::Winding, tf, None);
    }
    page.save_png("target/gallery.png").unwrap();
}

/// The scripted run as pictures over the Polar Night background (the README's demo):
/// `cargo test --release --offline demo_frames -- --ignored` writes target/demo/NNNN.png.
#[test]
#[ignore]
fn demo_frames() {
    let out = std::path::Path::new("target/demo");
    std::fs::create_dir_all(out).unwrap();
    let mut n = 0;
    scripted_run(|_, _, pm| {
        let mut page = Pixmap::new(pm.width(), pm.height()).unwrap();
        page.fill(col(0x0A1024, 1.0));
        page.draw_pixmap(0, 0, pm.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
        page.save_png(out.join(format!("{n:04}.png"))).unwrap();
        n += 1;
    });
}

/// Stores the golden hashes: run once, before a change that must not alter a pixel.
/// `cargo test --release --offline golden_write -- --ignored`
#[test]
#[ignore]
fn golden_write() {
    let path = std::path::Path::new(GOLDEN);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, golden_frames()).unwrap();
}

/// Every frame of the scripted run still hashes as stored.
/// `cargo test --release --offline golden_check -- --ignored`
#[test]
#[ignore]
fn golden_check() {
    let (want, got) = (std::fs::read_to_string(GOLDEN).expect("run golden_write first"), golden_frames());
    let differ: Vec<&str> = got.lines().zip(want.lines()).filter(|(g, w)| g != w).map(|(g, _)| g).collect();
    assert!(differ.is_empty() && got.len() == want.len(), "{} of {} frames differ, first: {:?}", differ.len(), got.lines().count(), differ.first());
}

/// Not a check, a measurement: `cargo test --release --offline bench_frames -- --ignored --nocapture`.
/// Each case times `Canvas::draw` and the RGBA -> BGRA copy of the boxes it returned (what
/// `Overlay::present` does before the push). 4K runs at 150% display scaling.
#[test]
#[ignore]
fn bench_frames() {
    use crate::whips::{Effect, Screen};
    use std::time::Instant;
    let whip = def(WhipId::Classic);
    let p = whip.physics;
    const CASES: [&str; 7] = [
        "whip swinging",
        "whip still, meter drains",
        "whip swinging, glow 1.0",
        "whip still, glow breathes",
        "whip still, confetti",
        "whip on / off each frame",
        "nothing changes",
    ];
    println!("{:<10} | {:<25} | {:>7} | {:>7} | {:>7} | {:>8} | {:>14}", "size", "case", "draw ms", "copy ms", "mean ms", "worst ms", "dirty px/frame");
    for (pw, ph, scale) in [(1920u32, 1080u32, 1.0f32), (3840, 2160, 1.5)] {
        let (fw, fh) = (pw as f32, ph as f32);
        let work = Rect::from_xywh(0.0, 0.0, fw, fh - 48.0 * scale).unwrap();
        let centre = (fw / 2.0, fh / 2.0);
        for (ci, case) in CASES.iter().enumerate() {
            fastrand::seed(7);
            let mut r = Rope::spawn(&p, centre);
            for _ in 0..600 {
                r.step(&p, centre, (0.0, 0.0, fw, fh), false);
            }
            let mut fx = Fx::default();
            let mut c = Canvas::new(pw, ph).unwrap();
            let mut dib = vec![0u8; (pw * ph * 4) as usize];
            let (mut draw, mut copy, mut worst, mut px) = (0.0f64, 0.0f64, 0.0f64, 0i64);
            const FRAMES: usize = 300;
            // frame 0 is the whole-pixmap first frame: drawn, not counted
            for f in 0..=FRAMES {
                let t = f as f32 / 60.0;
                if ci == 0 || ci == 2 {
                    r.step(&p, (centre.0 + 300.0 * (t * 9.0).sin(), centre.1 + 120.0 * (t * 5.0).cos()), (0.0, 0.0, fw, fh), false);
                }
                if ci == 4 && f % 60 == 1 {
                    fx.burst(Effect::Confetti, *r.pts.last().unwrap(), (fw, fh));
                }
                fx.step(1.0 / 60.0);
                let glow = match ci {
                    2 => 1.0,
                    3 => Screen::Glow.look(t, None).0,
                    _ => 0.0,
                };
                let s = Scene {
                    whip: (ci != 5 || f % 2 == 0).then_some((&r.pts[..], whip, None)),
                    glow,
                    meter: Some((if ci == 1 { 1.0 - t / 5.0 } else { 0.5 }, false)),
                    hue: whip.vibe.hue,
                    scale,
                    burst: Some(&fx),
                    ..Scene::none()
                };
                let t0 = Instant::now();
                let dirty = c.draw(&s, work);
                let t1 = Instant::now();
                for &d in &dirty {
                    crate::overlay::to_bgra(&mut dib, c.pm.data(), pw as i32, d);
                }
                let (d_ms, c_ms) = ((t1 - t0).as_secs_f64() * 1e3, t1.elapsed().as_secs_f64() * 1e3);
                if f > 0 {
                    draw += d_ms;
                    copy += c_ms;
                    worst = worst.max(d_ms + c_ms);
                    px += dirty.iter().map(|&d| area(d)).sum::<i64>();
                }
            }
            let n = FRAMES as f64;
            println!("{:<10} | {case:<25} | {:>7.3} | {:>7.3} | {:>7.3} | {:>8.3} | {:>14}", format!("{pw}x{ph}"), draw / n, copy / n, (draw + copy) / n, worst, px / FRAMES as i64);
        }
    }

    // What a frame costs before it draws anything.
    let per = |n: u32, t0: Instant| t0.elapsed().as_secs_f64() * 1e6 / n as f64;
    let mut r = Rope::spawn(&p, (960.0, 540.0));
    let t0 = Instant::now();
    for _ in 0..20_000 {
        r.step(&p, (960.0, 540.0), (0.0, 0.0, 1920.0, 1080.0), false);
    }
    println!("physics step: {:.2} us", per(20_000, t0));
    let mut ov = crate::overlay::Overlay::new().unwrap();
    ov.move_to_cursor_monitor();
    let t0 = Instant::now();
    for _ in 0..20_000 {
        ov.move_to_cursor_monitor();
    }
    println!("monitor query: {:.2} us", per(20_000, t0));
    let t0 = Instant::now();
    for _ in 0..20_000 {
        ov.cursor_local();
    }
    println!("cursor read: {:.2} us", per(20_000, t0));
    // The copy + push of `Overlay::present`, on the overlay window (never shown here).
    let (w, h) = ov.size();
    let pm = Pixmap::new(w, h).unwrap();
    ov.present(&pm, &[]);
    let t0 = Instant::now();
    for _ in 0..20 {
        ov.present(&pm, &[(0, 0, w as i32, h as i32)]);
    }
    println!("present, whole {w}x{h}: {:.3} ms", per(20, t0) / 1e3);
    let t0 = Instant::now();
    for _ in 0..200 {
        ov.present(&pm, &[(100, 100, 700, 500)]);
    }
    println!("present, one 600x400 box: {:.3} ms", per(200, t0) / 1e3);
}