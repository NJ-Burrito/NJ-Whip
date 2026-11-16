//! Feel harness (tests only): the real rope, weighted handle, swing detector and kick, driven
//! by mouse traces at 60 Hz, with numbers for "does the lash behave like a whip".
//!
//! `cargo test --release --offline feel_report -- --ignored --nocapture` prints the table and,
//! with WHIP_FEEL_DIR=<existing folder>, writes picture strips of the lash there.

use crate::physics::{Params, Rope, V};
use crate::swing::{Event, Swing};
use crate::whips::{self, WhipId};
use std::f32::consts::{PI, TAU};

const B: (f32, f32, f32, f32) = (0.0, 0.0, 1920.0, 1080.0);
/// Steps the mouse rests before a trace moves (the spawn pose settles) and after it stops.
const WARM: usize = 240;
const REST: usize = 360;
/// A point that moves less than this per step is not redrawn (app.rs `SHOW_MOVE`).
const STILL: f32 = 0.3;

/// The lightest, the normal and the heaviest whip.
const WHIPS: [WhipId; 3] = [WhipId::Zen, WhipId::Classic, WhipId::Elephant];

struct Trace {
    name: &'static str,
    mouse: Vec<V>,
    /// First step at which the mouse is still for good.
    stop: usize,
}

fn trace(name: &'static str, moves: Vec<V>) -> Trace {
    let mut mouse = vec![moves[0]; WARM];
    mouse.extend(&moves);
    let stop = mouse.len();
    mouse.extend(vec![moves[moves.len() - 1]; REST]);
    Trace { name, mouse, stop }
}

fn traces() -> Vec<Trace> {
    let y = 600.0;
    let circle = |r: f32, hz: f32, steps: usize| (0..=steps).map(|i| {
        let a = i as f32 * TAU * hz / 60.0;
        (960.0 + r * a.cos(), y + r * a.sin())
    }).collect::<Vec<V>>();
    vec![
        // 250 px to the left in 10 steps, then hard right at 3000 px/s
        trace("snap", (0..=10).map(|i| (1000.0 - 25.0 * i as f32, y)).chain((1..=12).map(|i| (750.0 + 50.0 * i as f32, y))).collect()),
        // six strokes of 400 px, alternating, 10 steps each
        trace("streak", (0..=60).map(|i| {
            let (k, f) = (i / 10, (i % 10) as f32 * 40.0);
            (if k % 2 == 0 { 760.0 + f } else { 1160.0 - f }, y)
        }).collect()),
        // 300 px each way, once a second, for 4 s
        trace("lazy", (0..=240).map(|i| (960.0 + 300.0 * (i as f32 * TAU / 60.0).cos(), y)).collect()),
        trace("circle", circle(200.0, 1.0, 180)),
        trace("fast circle", circle(250.0, 1.5, 120)),
        // 900 px in 12 steps, then nothing
        trace("dash-stop", (0..=12).map(|i| (500.0 + 75.0 * i as f32, y)).collect()),
        trace("jitter", (0..120).map(|i| (960.0 + if i % 2 == 0 { 3.0 } else { -3.0 }, y)).collect()),
    ]
}

struct Sim {
    /// The rope after every step.
    frames: Vec<Vec<V>>,
    /// Steps at which the swing cracked.
    cracks: Vec<usize>,
}

/// The chain as the app runs it: mouse -> handle -> rope, swing on the handle's speed, a crack
/// kicks the lash. `weighted` off = a handle pinned to the mouse; `kick` off = cracks leave the
/// rope alone (both off is the rope as it was before 2026-11-16).
fn sim(p: &Params, weighted: bool, kick: bool, mouse: &[V]) -> Sim {
    let p = if weighted { p.clone() } else { Params { weight: 0.1, ..p.clone() } };
    let mut rope = Rope::spawn(&p, mouse[0]);
    let mut swing = Swing::default();
    let (mut frames, mut cracks) = (Vec::with_capacity(mouse.len()), Vec::new());
    for (i, &at) in mouse.iter().enumerate() {
        rope.step(&p, at, B, false);
        if let Some(Event::Crack { dir, .. }) = swing.step(rope.hand_vel().0) {
            cracks.push(i);
            if kick {
                rope.kick(dir);
            }
        }
        frames.push(rope.pts.clone());
    }
    Sim { frames, cracks }
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

/// How far point `i` swings around the hand over `frames`, in turns: the spread of its
/// unwrapped angle. 1.0 = it went once round the hand.
fn winding(frames: &[Vec<V>], i: usize) -> f32 {
    let ang = |f: &Vec<V>| (f[i].1 - f[0].1).atan2(f[i].0 - f[0].0);
    let (mut prev, mut cum, mut lo, mut hi) = (ang(&frames[0]), 0.0f32, 0.0f32, 0.0f32);
    for f in &frames[1..] {
        let a = ang(f);
        cum += wrap(a - prev);
        prev = a;
        lo = lo.min(cum);
        hi = hi.max(cum);
    }
    (hi - lo) / TAU
}

/// How curled the lash is, in turns: the direction of its last segment relative to its first,
/// followed along the rope. 1.0 = the rope makes a full loop on itself.
fn curl(pts: &[V]) -> f32 {
    let dir = |i: usize| (pts[i + 1].1 - pts[i].1).atan2(pts[i + 1].0 - pts[i].0);
    (1..pts.len() - 1).map(|i| wrap(dir(i) - dir(i - 1))).sum::<f32>() / TAU
}

/// Number of places where the rope lies across itself.
fn crossings(pts: &[V]) -> usize {
    let side = |a: V, b: V, c: V| (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let cross = |a: V, b: V, c: V, d: V| side(a, b, c) * side(a, b, d) < 0.0 && side(c, d, a) * side(c, d, b) < 0.0;
    let n = pts.len() - 1;
    (0..n).flat_map(|i| (i + 2..n).map(move |j| (i, j))).filter(|&(i, j)| cross(pts[i], pts[i + 1], pts[j], pts[j + 1])).count()
}

/// Largest distance any point moves between two frames.
fn moved(a: &[V], b: &[V]) -> f32 {
    a.iter().zip(b).map(|(p, q)| (p.0 - q.0).hypot(p.1 - q.1)).fold(0.0, f32::max)
}

struct Feel {
    cracks: usize,
    /// Tip and mid-lash winding around the hand over the whole trace, and the tip's after the mouse stopped (turns).
    tip_turns: f32,
    mid_turns: f32,
    tip_turns_after: f32,
    /// Most curled the lash ever was (turns), and for how many steps it lay across itself.
    curl: f32,
    crossed: usize,
    /// Steps from the mouse stopping until the whip is still for good.
    settle: usize,
    /// Fastest the tip moved (px/step).
    tip_peak: f32,
}

fn feel(s: &Sim, stop: usize) -> Feel {
    let f = &s.frames[WARM - 1..];
    let n = f[0].len();
    let still_from = (stop..s.frames.len()).rev().find(|&k| moved(&s.frames[k], &s.frames[k - 1]) > STILL).map_or(stop, |k| k + 1);
    Feel {
        cracks: s.cracks.len(),
        tip_turns: winding(f, n - 1),
        mid_turns: winding(f, n / 2),
        tip_turns_after: winding(&s.frames[stop - 1..], n - 1),
        curl: f.iter().map(|p| curl(p).abs()).fold(0.0, f32::max),
        crossed: f.iter().filter(|p| crossings(p) > 0).count(),
        settle: still_from - stop,
        tip_peak: f.windows(2).map(|w| (w[1][n - 1].0 - w[0][n - 1].0).hypot(w[1][n - 1].1 - w[0][n - 1].1)).fold(0.0, f32::max),
    }
}

/// How far the tip travels in the 6 steps after the first crack (the visible snap), in px.
fn snap6(s: &Sim) -> f32 {
    let Some(&c) = s.cracks.first() else { return 0.0 };
    let n = s.frames[0].len() - 1;
    (c..c + 6).map(|k| (s.frames[k + 1][n].0 - s.frames[k][n].0).hypot(s.frames[k + 1][n].1 - s.frames[k][n].1)).sum()
}

/// "Like a real whip", for the lightest, the normal and the heaviest whip: the lash trails the
/// hand and snaps out along the swing line instead of going round the hand, stops soon after
/// the hand does, and a crack still throws the tip.
#[test]
fn whip_trails_and_snaps_instead_of_circling() {
    // how far the tip went in the 6 steps after a crack before 2026-11-16's fix; keep 70% of it
    let snap_before = [206.0, 197.0, 174.0];
    let ts = traces();
    let run = |p: &Params, name: &str| {
        let t = ts.iter().find(|t| t.name == name).unwrap();
        let s = sim(p, true, true, &t.mouse);
        (feel(&s, t.stop), snap6(&s))
    };
    for (id, before) in WHIPS.into_iter().zip(snap_before) {
        let def = whips::def(id);
        let p = def.physics;
        let (lazy, _) = run(&p, "lazy");
        assert!(lazy.tip_turns < 1.0, "{}: lazy sweep, tip went {} turns round the hand", def.name, lazy.tip_turns);
        assert!(lazy.curl < 1.0, "{}: lazy sweep, lash curled {} turns into a ring", def.name, lazy.curl);
        for name in ["snap", "streak"] {
            let (f, _) = run(&p, name);
            assert!(f.tip_turns < 1.0 && f.mid_turns < 1.0, "{}: {name}, tip {} and mid-lash {} turns round the hand", def.name, f.tip_turns, f.mid_turns);
        }
        let (stop, _) = run(&p, "dash-stop");
        assert!(stop.settle <= 90, "{}: still moving {} steps after a dead stop", def.name, stop.settle);
        let (_, snap) = run(&p, "snap");
        assert!(snap >= 0.7 * before, "{}: a crack moves the tip {snap} px in 6 steps, was {before}", def.name);
    }
}

#[test]
#[ignore]
fn feel_brief() {
    println!("{:<9} {:<11} {:>6} {:>8} {:>8} {:>9} {:>6} {:>7} {:>7} {:>8} {:>6}",
        "whip", "trace", "cracks", "tip trn", "mid trn", "tip after", "curl", "crossed", "settle", "tip pk", "snap6");
    for id in WHIPS {
        let def = whips::def(id);
        let p = def.physics;
        for t in traces() {
            let s = sim(&p, true, true, &t.mouse);
            let f = feel(&s, t.stop);
            println!("{:<9} {:<11} {:>6} {:>8.2} {:>8.2} {:>9.2} {:>6.2} {:>7} {:>7} {:>8.1} {:>6.0}",
                def.name, t.name, f.cracks, f.tip_turns, f.mid_turns, f.tip_turns_after, f.curl, f.crossed, f.settle, f.tip_peak, snap6(&s));
        }
    }
}

/// The diagnostic table, and picture strips when WHIP_FEEL_DIR names an existing folder.
#[test]
#[ignore]
fn feel_report() {
    let dir = std::env::var_os("WHIP_FEEL_DIR").map(std::path::PathBuf::from).filter(|d| d.is_dir());
    println!("{:<9} {:<11} {:<12} {:>6} {:>8} {:>8} {:>9} {:>6} {:>7} {:>7} {:>8}",
        "whip", "trace", "model", "cracks", "tip trn", "mid trn", "tip after", "curl", "crossed", "settle", "tip pk");
    for id in WHIPS {
        let def = whips::def(id);
        let p = def.physics;
        for t in traces() {
            for (model, weighted, kick) in [("pinned", false, false), ("pinned+kick", false, true), ("weighted", true, false), ("today", true, true)] {
                let s = sim(&p, weighted, kick, &t.mouse);
                let f = feel(&s, t.stop);
                println!("{:<9} {:<11} {:<12} {:>6} {:>8.2} {:>8.2} {:>9.2} {:>6.2} {:>7} {:>7} {:>8.1}",
                    def.name, t.name, model, f.cracks, f.tip_turns, f.mid_turns, f.tip_turns_after, f.curl, f.crossed, f.settle, f.tip_peak);
                if let (Some(dir), "today" | "pinned") = (&dir, model) {
                    // from just before the first crack (or the first move), every 2nd step
                    let from = s.cracks.first().map_or(WARM, |c| c - 6);
                    strip(&s, from, 2, def, &dir.join(format!("{}-{}-{model}.png", def.name, t.name.replace(' ', "-"))));
                }
            }
        }
    }
}

/// 20 frames of a run side by side (left to right, top to bottom), each a 1400x960 window of
/// the screen at 0.35 scale; frames on which the swing cracked are tinted.
fn strip(s: &Sim, from: usize, every: usize, def: &whips::WhipDef, path: &std::path::Path) {
    use tiny_skia::{FilterQuality, Paint, Pixmap, PixmapPaint, Rect, Transform};
    let (vw, vh, k, cols, rows) = (1400u32, 960u32, 0.35f32, 5u32, 4u32);
    let (cw, ch) = ((vw as f32 * k) as u32, (vh as f32 * k) as u32);
    let mut out = Pixmap::new(cw * cols, ch * rows).unwrap();
    for c in 0..cols * rows {
        let i = from + c as usize * every;
        let Some(pts) = s.frames.get(i) else { break };
        let cracked = s.cracks.iter().any(|&x| x > i - every && x <= i);
        let mut cell = Pixmap::new(vw, vh).unwrap();
        cell.fill(crate::ink::col(if cracked { 0xFFD9D9 } else if (c + c / cols) % 2 == 0 { 0xEEF6FB } else { 0xE2ECF2 }, 1.0));
        let pts: Vec<V> = pts.iter().map(|p| (p.0 - 260.0, p.1 - 120.0)).collect();
        crate::whipdraw::draw_whip(&mut cell, &pts, def, None);
        let mut dot = Paint::default();
        dot.set_color(crate::ink::col(0xE0202A, 1.0));
        cell.fill_rect(Rect::from_xywh(pts[0].0 - 7.0, pts[0].1 - 7.0, 14.0, 14.0).unwrap(), &dot, Transform::identity(), None);
        let tf = Transform::from_scale(k, k).post_translate((c % cols * cw) as f32, (c / cols * ch) as f32);
        out.draw_pixmap(0, 0, cell.as_ref(), &PixmapPaint { quality: FilterQuality::Bilinear, ..PixmapPaint::default() }, tf, None);
    }
    out.save_png(path).unwrap();
}
