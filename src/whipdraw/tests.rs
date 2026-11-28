use super::*;
use crate::physics::Rope;
use crate::whips::CATALOGUE;

fn alpha_near(pm: &Pixmap, p: V, r: i32) -> u8 {
    let mut m = 0;
    for dy in -r..=r {
        for dx in -r..=r {
            let (x, y) = (p.0 as i32 + dx, p.1 as i32 + dy);
            if let Some(px) = pm.pixel(x as u32, y as u32) {
                m = m.max(px.alpha());
            }
        }
    }
    m
}

#[test]
fn every_whip_draws_handle_lash_and_tip() {
    for w in &CATALOGUE {
        let p = w.physics;
        let rope = Rope::spawn(&p, (300.0, 500.0));
        let mut pm = Pixmap::new(1000, 800).unwrap();
        draw_whip(&mut pm, &rope.pts, w, None);
        assert!(alpha_near(&pm, rope.pts[0], 3) > 200, "{} handle", w.name);
        assert!(alpha_near(&pm, rope.pts[rope.pts.len() / 2], 3) > 200, "{} lash", w.name);
        assert!(alpha_near(&pm, *rope.pts.last().unwrap(), 6) > 150, "{} tip", w.name);
    }
}

#[test]
fn custom_tip_is_drawn_past_the_lash_end() {
    let w = crate::whips::def(crate::whips::WhipId::Penguin);
    let rope = Rope::spawn(&w.physics, (300.0, 500.0));
    let mut img = Pixmap::new(40, 40).unwrap();
    img.fill(tiny_skia::Color::from_rgba8(0, 200, 0, 255));
    let mut pm = Pixmap::new(1000, 800).unwrap();
    draw_whip(&mut pm, &rope.pts, w, Some(&img));
    let (t, u, _) = frames(&rope.pts);
    let px = pm.pixel((t.0 + u.0 * 20.0) as u32, (t.1 + u.1 * 20.0) as u32).unwrap();
    assert!(px.green() > 150 && px.red() < 80, "{px:?}");
}

/// The pen is tiny-skia's `stroke_path` with reused buffers: the same pixels at every width,
/// on both sides of the hairline switch, with and without a transform.
#[test]
fn pen_strokes_like_tiny_skia() {
    let path = shape(&[('M', &[20.0, 30.0]), ('C', &[40.0, 5.0, 70.0, 60.0, 95.0, 25.0]), ('L', &[60.0, 80.0])]).unwrap();
    let pen = &mut Pen::new();
    for tf in [Transform::identity(), Transform::from_translate(8.0, 3.0).pre_rotate(33.0).pre_scale(0.6, 0.6), rot_tf((60.0, 20.0), 40.0)] {
        for w in [0.5, 0.8, 0.9, 1.0, 1.01, 1.7, 2.4, 13.0] {
            for a in [1.0, 0.8] {
                let (mut theirs, mut ours) = (Pixmap::new(260, 260).unwrap(), Pixmap::new(260, 260).unwrap());
                theirs.stroke_path(&path, &paint(0x7FC4EC, a), &stroke(w), tf, None);
                pen.stroke_path(&mut ours, &path, &paint(0x7FC4EC, a), &stroke(w), tf);
                assert!(theirs.data().iter().any(|&b| b != 0), "nothing drawn at width {w}");
                assert!(theirs.data() == ours.data(), "width {w}, alpha {a}, {tf:?}");
            }
        }
    }
}

/// Visual check: `cargo test atlas -- --ignored` writes target/atlas.png (all whips, light + dark).
#[test]
#[ignore]
fn atlas() {
    let (cw, ch) = (560u32, 300u32);
    let mut pm = Pixmap::new(cw * 3, ch * 10).unwrap();
    for (k, w) in CATALOGUE.iter().enumerate() {
        for dark in [false, true] {
            let col = (k % 3) as u32;
            let row = (k / 3) as u32 * 2 + dark as u32;
            let bg = if dark { 0x0A1024 } else { 0xEEF6FB };
            let r = tiny_skia::Rect::from_xywh((col * cw) as f32, (row * ch) as f32, cw as f32, ch as f32).unwrap();
            pm.fill_rect(r, &paint(bg, 1.0), Transform::identity(), None);
            let pts = preview_points(w.physics.points, ((col * cw + 130) as f32, (row * ch + 240) as f32), 0.95);
            draw_whip(&mut pm, &pts, w, None);
        }
    }
    pm.save_png("target/atlas.png").unwrap();
}

// ---- regression guard for the drawing speed work: golden images + a bench ----------------
// WHIP_GOLDEN_DIR=<folder> cargo test --release --offline golden_write -- --ignored   (once, on known-good code)
// WHIP_GOLDEN_DIR=<folder> cargo test --release --offline golden_compare -- --ignored --nocapture
// cargo test --release --offline bench_draw_whip -- --ignored --nocapture

const GW: u32 = 900;
const GH: u32 = 700;

fn golden_dir() -> std::path::PathBuf {
    std::env::var_os("WHIP_GOLDEN_DIR").map(Into::into).unwrap_or_else(|| "target/whip-golden".into())
}

/// Ten rope shapes per whip, from the real physics inside a GW x GH screen.
fn poses(w: &WhipDef) -> Vec<(&'static str, Vec<V>)> {
    let p = w.physics;
    let b = (0.0, 0.0, GW as f32, GH as f32);
    let run = |hand: V, settle: usize, kick: i8, mv: V, steps: usize| {
        let mut r = Rope::spawn(&p, hand);
        for _ in 0..settle {
            r.step(&p, hand, b, false);
        }
        if kick != 0 {
            r.kick(kick);
        }
        let mut h = hand;
        for _ in 0..steps {
            h = (h.0 + mv.0, h.1 + mv.1);
            r.step(&p, h, b, false);
        }
        r
    };
    // a lash folded back over itself: swung right, then snatched back left
    let mut fold = run((300.0, 380.0), 100, 0, (45.0, 0.0), 5);
    for k in 1..=4 {
        fold.step(&p, (525.0 - 55.0 * k as f32, 380.0 + 12.0 * k as f32), b, false);
    }
    let run = |hand: V, settle: usize, kick: i8, mv: V, steps: usize| run(hand, settle, kick, mv, steps).pts;
    vec![
        ("fold", fold.pts),
        ("wall", run((885.0, 300.0), 400, 0, (0.0, 0.0), 0)),
        ("spawn", run((300.0, 560.0), 0, 0, (0.0, 0.0), 0)),
        ("rest", run((300.0, 420.0), 400, 0, (0.0, 0.0), 0)),
        ("swing-left", run((640.0, 430.0), 120, 0, (-28.0, 3.0), 9)),
        ("swing-right", run((220.0, 430.0), 120, 0, (28.0, -4.0), 9)),
        ("edge-curl", run((850.0, 640.0), 400, 0, (0.0, 0.0), 0)),
        ("corner", run((45.0, 60.0), 200, 0, (0.0, 0.0), 0)),
        ("kick", run((380.0, 480.0), 300, -1, (0.0, 0.0), 3)),
        ("fast", run((150.0, 600.0), 60, 0, (70.0, -45.0), 6)),
    ]
}

fn render(pts: &[V], w: &WhipDef) -> Pixmap {
    let mut pm = Pixmap::new(GW, GH).unwrap();
    draw_whip(&mut pm, pts, w, None);
    pm
}

fn golden_name(w: &WhipDef, pose: &str) -> String {
    format!("{}-{pose}", w.name.replace(' ', ""))
}

/// Raw premultiplied RGBA, cropped to the rows that hold ink: [first row, row count] as u32 LE, then the rows.
fn pack(pm: &Pixmap) -> Vec<u8> {
    let row = GW as usize * 4;
    let inked: Vec<usize> = pm.data().chunks(row).enumerate().filter(|(_, r)| r.iter().any(|&b| b != 0)).map(|(y, _)| y).collect();
    let (y0, y1) = (inked.first().copied().unwrap_or(0), inked.last().map_or(0, |y| y + 1));
    let mut out = Vec::with_capacity(8 + (y1 - y0) * row);
    out.extend_from_slice(&(y0 as u32).to_le_bytes());
    out.extend_from_slice(&((y1 - y0) as u32).to_le_bytes());
    out.extend_from_slice(&pm.data()[y0 * row..y1 * row]);
    out
}

fn unpack(file: &[u8]) -> Vec<u8> {
    let row = GW as usize * 4;
    let y0 = u32::from_le_bytes(file[0..4].try_into().unwrap()) as usize;
    let mut full = vec![0u8; row * GH as usize];
    full[y0 * row..y0 * row + file.len() - 8].copy_from_slice(&file[8..]);
    full
}

#[test]
#[ignore]
fn golden_write() {
    let dir = golden_dir();
    std::fs::create_dir_all(&dir).unwrap();
    for w in &CATALOGUE {
        for (pose, pts) in poses(w) {
            let pm = render(&pts, w);
            let name = golden_name(w, pose);
            std::fs::write(dir.join(format!("{name}.rgba")), pack(&pm)).unwrap();
            pm.save_png(dir.join(format!("{name}.png"))).unwrap();
        }
    }
}

/// Per image: how many pixels differ from the golden and the largest per-channel difference.
/// Fails on any difference above 1/255; a changed image is saved beside its golden as `<name>.new.png`.
#[test]
#[ignore]
fn golden_compare() {
    let dir = golden_dir();
    if !dir.is_dir() {
        println!("golden_compare: no golden folder at {}, skipped (run golden_write first)", dir.display());
        return;
    }
    let (mut images, mut changed, mut worst, mut total_px) = (0, 0, 0u8, 0usize);
    for w in &CATALOGUE {
        for (pose, pts) in poses(w) {
            let name = golden_name(w, pose);
            let Ok(file) = std::fs::read(dir.join(format!("{name}.rgba"))) else {
                println!("{name:28} no golden, skipped");
                continue;
            };
            let pm = render(&pts, w);
            let old = unpack(&file);
            let (mut px, mut max) = (0usize, 0u8);
            for (a, b) in old.chunks(4).zip(pm.data().chunks(4)) {
                let d = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
                px += (d > 0) as usize;
                max = max.max(d);
            }
            images += 1;
            if px > 0 {
                changed += 1;
                total_px += px;
                worst = worst.max(max);
                println!("{name:28} {px:6} px differ, max {max}/255");
                pm.save_png(dir.join(format!("{name}.new.png"))).unwrap();
            }
        }
    }
    println!("golden_compare: {images} images, {} identical, {changed} changed ({total_px} px), max difference {worst}/255", images - changed);
    assert!(images > 0, "golden folder holds no images");
    assert!(worst <= 1, "max difference {worst}/255");
}

/// CPU cycles this thread has run for. Unlike the wall clock it does not count the time other
/// programs hold the core, so the bench reads the same on a busy machine.
fn thread_cycles() -> u64 {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThread() -> isize;
        fn QueryThreadCycleTime(thread: isize, cycles: *mut u64) -> i32;
    }
    let mut cycles = 0;
    unsafe { QueryThreadCycleTime(GetCurrentThread(), &mut cycles) };
    cycles
}

/// Time of `draw_whip` alone, per whip, mid-swing: 2,000 calls in 20 rounds, the quietest
/// round reported (thread cycles, turned into microseconds), plus rasteriser calls per frame.
#[test]
#[ignore]
fn bench_draw_whip() {
    use std::time::Instant;
    // cycles per second, from the cleanest of 200 two-millisecond spins
    let rate = (0..200).fold(0.0, |best: f64, _| {
        let (c, t) = (thread_cycles(), Instant::now());
        while t.elapsed().as_micros() < 2000 {
            std::hint::spin_loop();
        }
        best.max((thread_cycles() - c) as f64 / t.elapsed().as_secs_f64())
    });
    let mut pm = Pixmap::new(GW, GH).unwrap();
    let (mut sum, mut sum_calls) = (0.0, 0);
    println!("{:16} {:>9} {:>6}", "whip", "us/call", "calls");
    for w in &CATALOGUE {
        let all = poses(w);
        let pts = &all.iter().find(|(pose, _)| *pose == "swing-left").unwrap().1;
        CALLS.with(|c| c.set(0));
        draw_whip(&mut pm, pts, w, None);
        let calls = CALLS.with(|c| c.get());
        let mut best = u64::MAX;
        for _ in 0..20 {
            let c = thread_cycles();
            for _ in 0..100 {
                draw_whip(std::hint::black_box(&mut pm), std::hint::black_box(pts), w, None);
            }
            best = best.min(thread_cycles() - c);
        }
        let us = best as f64 / 100.0 / rate * 1e6;
        println!("{:16} {us:9.1} {calls:6}", w.name);
        sum += us;
        sum_calls += calls;
    }
    println!("{:16} {:9.1} {:6}", "mean", sum / CATALOGUE.len() as f64, sum_calls / CATALOGUE.len() as u32);
}