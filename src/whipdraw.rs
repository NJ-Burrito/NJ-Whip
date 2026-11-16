//! Draws one whip (handle, tapered lash, pattern, tip) from its catalogue look.
//! Geometry follows the approved mockup.

use crate::ink::{paint, shape};
use crate::whips::{Look, Pattern, Tip, WhipDef};
use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, PathStroker, Pixmap, PixmapPaint, Stroke,
    Transform,
};

type V = (f32, f32);
const K: f32 = 1.9; // tip art scale (mockup units → px)
/// The light line round every lash.
const RIM: u32 = 0xF4FAFF;

#[cfg(test)]
thread_local!(static CALLS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) });

/// Counts rasteriser calls in tests (the bench prints calls per frame); free in the app.
#[inline(always)]
fn tick() {
    #[cfg(test)]
    CALLS.with(|c| c.set(c.get() + 1));
}

fn stroke(w: f32) -> Stroke {
    Stroke { width: w.max(0.5), line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() }
}

/// tiny-skia's own test (painter.rs, `treat_as_hairline`) for an anti-aliased stroke: at most a
/// pixel wide on screen, it is drawn as a hairline instead of an outline.
fn is_hairline(w: f32, ts: Transform) -> bool {
    let fast_len = |x: f32, y: f32| x.abs().max(y.abs()) + x.abs().min(y.abs()) * 0.5;
    fast_len(ts.sx * w, ts.ky * w) <= 1.0 && fast_len(ts.kx * w, ts.sy * w) <= 1.0
}

/// One stroker and one path buffer for every stroke of a frame; `Pixmap::stroke_path` builds a
/// fresh stroker (and its heap blocks) per call, and copies each transformed outline.
struct Pen {
    stroker: PathStroker,
    pb: PathBuilder,
}

impl Pen {
    fn new() -> Pen {
        Pen { stroker: PathStroker::new(), pb: PathBuilder::new() }
    }

    /// Pixel for pixel `pm.stroke_path(path, paint, st, tf, None)`: the same two branches
    /// (hairline, or stroke to an outline and fill it), with the stroker reused.
    fn stroke_path(&mut self, pm: &mut Pixmap, path: &Path, paint: &Paint, st: &Stroke, tf: Transform) {
        tick();
        if is_hairline(st.width, tf) {
            return pm.stroke_path(path, paint, st, tf, None);
        }
        let outline = self.stroker.stroke(path, st, PathStroker::compute_resolution_scale(&tf));
        let outline = if tf.is_identity() { outline } else { outline.and_then(|o| o.transform(tf)) };
        if let Some(o) = outline {
            pm.fill_path(&o, paint, FillRule::Winding, Transform::identity(), None);
        }
    }

    fn stroke(&mut self, pm: &mut Pixmap, paint: &Paint, st: &Stroke, tf: Transform, build: impl FnOnce(&mut PathBuilder)) {
        let mut pb = std::mem::replace(&mut self.pb, PathBuilder::new());
        build(&mut pb);
        if let Some(path) = pb.finish() {
            self.stroke_path(pm, &path, paint, st, tf);
            self.pb = path.clear();
        }
    }
}

fn band(pm: &mut Pixmap, pen: &mut Pen, a: V, b: V, rgb: u32, w: f32) {
    let st = Stroke { width: w, line_cap: LineCap::Butt, ..Stroke::default() };
    pen.stroke(pm, &paint(rgb, 1.0), &st, Transform::identity(), |pb| {
        pb.move_to(a.0, a.1);
        pb.line_to(b.0, b.1);
    });
}

/// The approved mockup's resting arc: `n` points from the hand at `origin`, scaled by `s`.
pub fn preview_points(n: usize, origin: V, s: f32) -> Vec<V> {
    let p = [(0.0f32, 0.0f32), (40.0, -180.0), (230.0, -210.0), (390.0, -110.0)];
    (0..n)
        .map(|i| {
            let t = i as f32 / (n - 1) as f32;
            let u = 1.0 - t;
            let x = u * u * u * p[0].0 + 3.0 * u * u * t * p[1].0 + 3.0 * u * t * t * p[2].0 + t * t * t * p[3].0;
            let y = u * u * u * p[0].1 + 3.0 * u * u * t * p[1].1 + 3.0 * u * t * t * p[2].1 + t * t * t * p[3].1;
            (origin.0 + x * s, origin.1 + y * s)
        })
        .collect()
}

fn catmull(pts: &[V], i: isize) -> V {
    let n = pts.len() as isize;
    if i < 0 {
        return (2.0 * pts[0].0 - pts[1].0, 2.0 * pts[0].1 - pts[1].1);
    }
    if i >= n {
        let (a, b) = (pts[n as usize - 2], pts[n as usize - 1]);
        return (2.0 * b.0 - a.0, 2.0 * b.1 - a.1);
    }
    pts[i as usize]
}

/// The lash as one cubic per link (start, two Catmull-Rom controls, end), worked out once per
/// frame for every pass.
fn links(pts: &[V]) -> Vec<[V; 4]> {
    (0..pts.len() - 1)
        .map(|i| {
            let p0 = catmull(pts, i as isize - 1);
            let (p1, p2) = (pts[i], pts[i + 1]);
            let p3 = catmull(pts, i as isize + 2);
            [p1, (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0), (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0), p2]
        })
        .collect()
}

fn cubic_to(pb: &mut PathBuilder, k: &[V; 4], off: V) {
    pb.cubic_to(k[1].0 + off.0, k[1].1 + off.1, k[2].0 + off.0, k[2].1 + off.1, k[3].0 + off.0, k[3].1 + off.1);
}

fn link_to(pb: &mut PathBuilder, k: &[V; 4], off: V) {
    pb.move_to(k[0].0 + off.0, k[0].1 + off.1);
    cubic_to(pb, k, off);
}

fn width_at(l: &Look, t: f32) -> f32 {
    l.w0 + (l.w1 - l.w0) * t.powf(0.8)
}

/// One continuous stroke along the whole lash at the mean width (for translucent layers, so
/// overlapping link caps don't show as dots).
fn smooth_pass(pm: &mut Pixmap, pen: &mut Pen, links: &[[V; 4]], l: &Look, rgb: u32, a: f32, mult: f32, grow: f32, off: V) {
    let w = width_at(l, 0.35) * mult + grow;
    pen.stroke(pm, &paint(rgb, a), &stroke(w), Transform::identity(), |pb| {
        pb.move_to(links[0][0].0 + off.0, links[0][0].1 + off.1);
        for k in links {
            cubic_to(pb, k, off);
        }
    });
}

/// One tapered pass along the lash, link by link: width(t) + grow.
///
/// Each link stays its own stroke on purpose. The time goes to anti-aliasing the outlines, not
/// to the number of calls: links batched into fewer fills that keep the pixels cost the same,
/// and all the links of a pass filled as one path (about a third off that pass) redraw the
/// lash edge where two round caps overlap, some 1,000 pixels by up to 67/255. Measured, dropped.
fn lash_pass(pm: &mut Pixmap, pen: &mut Pen, links: &[[V; 4]], l: &Look, rgb: u32, a: f32, grow: f32) {
    let n = links.len();
    let p = paint(rgb, a);
    for (i, k) in links.iter().enumerate() {
        let w = width_at(l, i as f32 / n as f32) + grow;
        pen.stroke(pm, &p, &stroke(w), Transform::identity(), |pb| link_to(pb, k, (0.0, 0.0)));
    }
}

fn line(pm: &mut Pixmap, pen: &mut Pen, a: V, b: V, rgb: u32, alpha: f32, w: f32, tf: Transform) {
    pen.stroke(pm, &paint(rgb, alpha), &stroke(w), tf, |pb| {
        pb.move_to(a.0, a.1);
        pb.line_to(b.0, b.1);
    });
}

fn fill(pm: &mut Pixmap, path: Option<Path>, rgb: u32, a: f32, tf: Transform) {
    // moved into place here: handed a transform, `fill_path` copies the path first
    if let Some(path) = path.and_then(|p| p.transform(tf)) {
        tick();
        pm.fill_path(&path, &paint(rgb, a), FillRule::Winding, Transform::identity(), None);
    }
}

fn outline(pm: &mut Pixmap, pen: &mut Pen, path: Option<Path>, rgb: u32, w: f32, tf: Transform) {
    if let Some(path) = path {
        pen.stroke_path(pm, &path, &paint(rgb, 1.0), &stroke(w), tf);
    }
}

fn circle(cx: f32, cy: f32, r: f32) -> Option<Path> {
    PathBuilder::from_circle(cx, cy, r)
}

fn oval(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    tiny_skia::Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0).map(PathBuilder::from_oval).flatten()
}

/// Draws the whole whip. `custom_tip` (if any) replaces the built-in tip art.
pub fn draw_whip(pm: &mut Pixmap, pts: &[V], w: &WhipDef, custom_tip: Option<&Pixmap>) {
    if pts.len() < 4 {
        return;
    }
    let l = &w.look;
    let n = pts.len() - 1;
    let pen = &mut Pen::new();
    let links = links(pts);
    // shadow, glow, rim, edge, body
    smooth_pass(pm, pen, &links, l, 0x000000, 0.14, 1.0, 3.0, (5.0, 9.0));
    if let Some(g) = l.glow {
        smooth_pass(pm, pen, &links, l, g, 0.2, 3.4, 0.0, (0.0, 0.0));
        smooth_pass(pm, pen, &links, l, g, 0.4, 1.9, 0.0, (0.0, 0.0));
    }
    lash_pass(pm, pen, &links, l, RIM, 0.9, 5.2);
    lash_pass(pm, pen, &links, l, l.edge, 1.0, 2.4);
    lash_pass(pm, pen, &links, l, l.body, 1.0, 0.0);
    draw_pattern(pm, pen, pts, l);
    // highlight along the upper side
    let hl = paint(l.hl, 1.0);
    for (i, k) in links.iter().enumerate().take(n.saturating_sub(4)) {
        let ww = width_at(l, i as f32 / n as f32);
        pen.stroke(pm, &hl, &stroke((ww * 0.18).max(0.8)), Transform::identity(), |pb| link_to(pb, k, (0.0, -ww * 0.26)));
    }
    draw_handle(pm, pen, pts, l);
    match custom_tip {
        Some(img) => draw_custom_tip(pm, pts, img),
        None => draw_tip(pm, pen, pts, w.tip, l),
    }
}

fn draw_pattern(pm: &mut Pixmap, pen: &mut Pen, pts: &[V], l: &Look) {
    let n = pts.len() - 1;
    let id = Transform::identity();
    // Braid marks a pixel wide or less are hairlines, which tiny-skia draws segment by segment:
    // every run of them at one width goes down in one call instead of one call each.
    let braid = paint(l.mark, 0.8);
    let mut thin = (0.0, PathBuilder::new());
    let flush = |pm: &mut Pixmap, pen: &mut Pen, thin: &mut (f32, PathBuilder)| {
        if let Some(path) = std::mem::replace(&mut thin.1, PathBuilder::new()).finish() {
            pen.stroke_path(pm, &path, &braid, &stroke(thin.0), id);
            thin.1 = path.clear();
        }
    };
    for i in 0..n {
        let ww = width_at(l, i as f32 / n as f32);
        let (a, b) = (pts[i], pts[i + 1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = dx.hypot(dy).max(1e-3);
        let (nx, ny) = (-dy / len, dx / len);
        let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        match l.pattern {
            Pattern::Braid if ww > 2.6 => {
                let e = ww * 0.4;
                let sw = (ww * 0.11).max(0.9);
                let hair = is_hairline(sw, id);
                if !hair || sw != thin.0 {
                    flush(pm, pen, &mut thin);
                    thin.0 = sw;
                }
                for from in [(a.0 + nx * e, a.1 + ny * e), (a.0 - nx * e, a.1 - ny * e)] {
                    if hair {
                        thin.1.move_to(from.0, from.1);
                        thin.1.line_to(m.0, m.1);
                    } else {
                        line(pm, pen, from, m, l.mark, 0.8, sw, id);
                    }
                }
            }
            Pattern::Rings if i % 2 == 0 && ww > 4.0 => {
                line(pm, pen, (m.0 - nx * ww * 0.45, m.1 - ny * ww * 0.45), (m.0 + nx * ww * 0.45, m.1 + ny * ww * 0.45), l.mark, 1.0, 1.5, id);
            }
            Pattern::Ribbon if ww > 3.0 => {
                let c = if i % 2 == 1 { l.mark } else { l.mark2 };
                line(pm, pen, (m.0 - nx * ww * 0.4, m.1 - ny * ww * 0.4), (m.0 + nx * ww * 0.4 + dx * 0.5, m.1 + ny * ww * 0.4 + dy * 0.5), c, 1.0, ww * 0.42, id);
            }
            Pattern::Chunks if i % 3 == 1 && ww > 3.0 => {
                let c = if i % 6 == 1 { l.mark } else { l.mark2 };
                let tf = Transform::from_translate(m.0, m.1).pre_rotate(i as f32 * 37.0);
                if let Some(r) = tiny_skia::Rect::from_xywh(-ww * 0.18, -ww * 0.15, ww * 0.36, ww * 0.3) {
                    tick();
                    pm.fill_rect(r, &paint(c, 1.0), tf, None);
                }
            }
            _ => {}
        }
    }
    flush(pm, pen, &mut thin);
}

fn draw_handle(pm: &mut Pixmap, pen: &mut Pen, pts: &[V], l: &Look) {
    let p0 = pts[0];
    let q = pts[3.min(pts.len() - 1)];
    let (dx, dy) = (p0.0 - q.0, p0.1 - q.1);
    let d = dx.hypot(dy).max(1e-3);
    let (ux, uy) = (dx / d, dy / d); // points away from the lash
    let (hw, hl) = (18.0, 100.0);
    let end = (p0.0 + ux * hl, p0.1 + uy * hl);
    let id = Transform::identity();
    line(pm, pen, (p0.0 + 5.0, p0.1 + 7.0), (end.0 + 5.0, end.1 + 7.0), 0x000000, 0.18, hw + 2.0, id);
    line(pm, pen, p0, end, l.edge, 1.0, hw + 3.0, id);
    line(pm, pen, p0, end, l.handle, 1.0, hw, id);
    // diagonal wraps
    let (nx, ny) = (-uy, ux);
    for k in 0..8 {
        let s = 18.0 + k as f32 * 9.0;
        let c = (p0.0 + ux * s, p0.1 + uy * s);
        let a = (c.0 - nx * (hw / 2.0 - 2.5) - ux * 3.0, c.1 - ny * (hw / 2.0 - 2.5) - uy * 3.0);
        let b = (c.0 + nx * (hw / 2.0 - 2.5) + ux * 3.0, c.1 + ny * (hw / 2.0 - 2.5) + uy * 3.0);
        line(pm, pen, a, b, l.wrap, 1.0, 2.4, id);
    }
    // ferrule at the lash end, knob at the butt
    band(pm, pen, (p0.0 + ux * 1.0, p0.1 + uy * 1.0), (p0.0 + ux * 11.0, p0.1 + uy * 11.0), l.edge, hw + 5.0);
    band(pm, pen, (p0.0 + ux * 2.0, p0.1 + uy * 2.0), (p0.0 + ux * 10.0, p0.1 + uy * 10.0), l.ferrule, hw + 3.0);
    fill(pm, circle(end.0, end.1, hw * 0.64 + 1.6), l.edge, 1.0, id);
    fill(pm, circle(end.0, end.1, hw * 0.64), l.knob, 1.0, id);
    fill(pm, circle(end.0 - hw * 0.2, end.1 - hw * 0.22, hw * 0.18), 0xFFFFFF, 0.35, id);
}

/// Tip frame helpers. `rot`: art rotated with the lash. `at`: art upright, its local point (ax, ay)
/// placed `dist` units past the lash end along the lash direction.
fn frames(pts: &[V]) -> (V, V, f32) {
    let t = pts[pts.len() - 1];
    let p = pts[pts.len() - 3];
    let (dx, dy) = (t.0 - p.0, t.1 - p.1);
    let d = dx.hypot(dy).max(1e-3);
    (t, (dx / d, dy / d), dy.atan2(dx).to_degrees())
}

fn rot_tf(t: V, ang: f32) -> Transform {
    Transform::from_translate(t.0, t.1).pre_rotate(ang).pre_scale(K, K)
}

fn at_tf(t: V, u: V, dist: f32, ax: f32, ay: f32) -> Transform {
    Transform::from_translate(t.0 + u.0 * dist * K, t.1 + u.1 * dist * K)
        .pre_scale(K, K)
        .pre_translate(-ax, -ay)
}

fn draw_tip(pm: &mut Pixmap, pen: &mut Pen, pts: &[V], tip: Tip, l: &Look) {
    let (t, u, ang) = frames(pts);
    match tip {
        Tip::Frayed => {
            let tf = rot_tf(t, ang);
            for (a, len) in [(-30.0f32, 20.0f32), (-10.0, 24.0), (12.0, 18.0)] {
                let r = a.to_radians();
                let path = shape(&[('M', &[0.0, 0.0]), ('Q', &[r.cos() * len * 0.5, r.sin() * len * 0.5 - 3.0, r.cos() * len, r.sin() * len + 6.0])]);
                if let Some(p) = path {
                    pen.stroke_path(pm, &p, &paint(l.cracker, 1.0), &stroke(0.9), tf.pre_scale(0.6, 0.6));
                }
            }
        }
        Tip::Bead => {
            let tf = rot_tf(t, ang);
            if let Some(g) = l.glow {
                fill(pm, circle(2.0, 0.0, 9.0), g, 0.35, tf);
            }
            fill(pm, circle(2.0, 0.0, 5.6), l.edge, 1.0, tf);
            fill(pm, circle(2.0, 0.0, 4.2), 0xFFFFFF, 1.0, tf);
        }
        Tip::Tail => {
            let tf = rot_tf(t, ang);
            fill(pm, shape(&[('M', &[-4.0, -3.2]), ('Q', &[2.0, -3.6, 6.0, -2.2]), ('L', &[6.0, 2.2]), ('Q', &[2.0, 3.6, -4.0, 3.2]), ('Z', &[])]), 0x8B8F96, 1.0, tf);
            for k in 0..15 {
                let a = (-34.0 + k as f32 * 68.0 / 14.0).to_radians();
                let len = 20.0 + ((k * 5) % 7) as f32;
                let c = [0x2B2E33, 0x3B3F46, 0x1E2024][k % 3];
                if let Some(p) = shape(&[('M', &[4.0, 0.0]), ('Q', &[a.cos() * len * 0.55, a.sin() * len * 0.55 + 1.5, a.cos() * len + 4.0, a.sin() * len + 3.0])]) {
                    pen.stroke_path(pm, &p, &paint(c, 1.0), &stroke(1.7), tf);
                }
            }
            fill(pm, oval(6.0, 0.0, 3.2, 3.6), 0x454950, 1.0, tf);
        }
        Tip::PomPom => {
            let tf = rot_tf(t, ang);
            fill(pm, circle(6.0, 2.0, 17.0), 0x000000, 0.12, tf);
            for k in 0..26 {
                let a = (-160.0 + k as f32 * 320.0 / 25.0).to_radians();
                let len = 16.0 + ((k * 7) % 9) as f32;
                let c = [0xFF8CC6, 0xFFD25A, 0xFFFFFF][k % 3];
                if let Some(p) = shape(&[('M', &[6.0, 0.0]), ('Q', &[a.cos() * len * 0.5 + 3.0, a.sin() * len * 0.5, a.cos() * len + 6.0, a.sin() * len])]) {
                    pen.stroke_path(pm, &p, &paint(c, 1.0), &stroke(3.0), tf);
                }
            }
            fill(pm, circle(6.0, 0.0, 4.0), 0xC93E86, 1.0, tf);
        }
        Tip::Drip => {
            // trail of drops behind the swing
            let mut k = 0;
            let mut idx = pts.len().saturating_sub(4);
            while idx > 4 {
                let (x, y) = pts[idx];
                let r = (3.4 - k as f32 * 0.28).max(1.4);
                let tf = Transform::from_translate(x, y + 8.0);
                let a = (0.95 - k as f32 * 0.08).max(0.35);
                fill(pm, shape(&[('M', &[0.0, -r * 1.6]), ('C', &[r * 0.9, -r * 0.4, r, r * 0.5, 0.0, r]), ('C', &[-r, r * 0.5, -r * 0.9, -r * 0.4, 0.0, -r * 1.6]), ('Z', &[])]), 0xD93A22, a, tf);
                fill(pm, circle(-r * 0.3, -r * 0.2, r * 0.3), 0xFFFFFF, 0.6 * a, tf);
                k += 1;
                idx = idx.saturating_sub(4);
            }
            let tf = at_tf(t, u, 0.0, 0.0, 0.0);
            let drop = || shape(&[('M', &[0.0, 0.0]), ('C', &[1.6, 3.0, 6.0, 7.0, 6.0, 11.5]), ('C', &[6.0, 15.5, 3.2, 18.0, 0.0, 18.0]), ('C', &[-3.2, 18.0, -6.0, 15.5, -6.0, 11.5]), ('C', &[-6.0, 7.0, -1.6, 3.0, 0.0, 0.0]), ('Z', &[])]);
            fill(pm, drop(), 0xD93A22, 1.0, tf);
            outline(pm, pen, drop(), 0x8E1F10, 0.9, tf);
            fill(pm, oval(-2.2, 10.0, 1.6, 2.6), 0xFFFFFF, 0.6, tf);
            fill(pm, circle(2.4, 13.6, 1.0), 0xFFF3E0, 1.0, tf);
            fill(pm, circle(-1.0, 15.0, 0.8), 0x5FA84A, 1.0, tf);
        }
        Tip::Bloom => {
            let tf = rot_tf(t, ang);
            for a in (0..360).step_by(60) {
                let r = (a as f32).to_radians();
                let (cx, cy) = (10.0 + r.cos() * 7.0, r.sin() * 7.0);
                let ptf = tf.pre_translate(cx, cy).pre_rotate(a as f32);
                fill(pm, oval(0.0, 0.0, 6.0, 4.0), 0xFF9EC0, 1.0, ptf);
                outline(pm, pen, oval(0.0, 0.0, 6.0, 4.0), 0xC94F7C, 1.0, ptf);
            }
            fill(pm, circle(10.0, 0.0, 4.0), 0xFFD25A, 1.0, tf);
            // leaves along the vine
            let n = pts.len();
            let mut k = 0;
            let mut i = 4;
            while i + 3 < n {
                let (a, b) = (pts[i], pts[i + 1]);
                let ang2 = (b.1 - a.1).atan2(b.0 - a.0).to_degrees() + if k % 2 == 1 { 55.0 } else { -55.0 };
                let s = (1.15 - k as f32 * 0.08).max(0.5);
                let ltf = Transform::from_translate(a.0, a.1).pre_rotate(ang2).pre_scale(s, s);
                let leaf = || shape(&[('M', &[0.0, 0.0]), ('C', &[6.0, -6.0, 16.0, -6.0, 22.0, 0.0]), ('C', &[16.0, 6.0, 6.0, 6.0, 0.0, 0.0]), ('Z', &[])]);
                fill(pm, leaf(), 0x5FAE5A, 1.0, ltf);
                outline(pm, pen, leaf(), 0x2F6B35, 1.2, ltf);
                k += 1;
                i += 5;
            }
        }
        Tip::Penguin => {
            let tf = at_tf(t, u, 13.0, 0.0, 0.0);
            let head = || shape(&[('M', &[-15.0, 2.0]), ('C', &[-16.0, -10.0, -8.0, -17.0, 0.0, -17.0]), ('C', &[9.0, -17.0, 16.0, -10.0, 15.0, 1.0]), ('C', &[15.0, 10.0, 9.0, 16.0, 0.0, 16.0]), ('C', &[-9.0, 16.0, -15.0, 11.0, -15.0, 2.0]), ('Z', &[])]);
            fill(pm, head(), 0x252B3E, 1.0, tf);
            outline(pm, pen, head(), 0x0E1120, 1.2, tf);
            fill(pm, oval(-5.0, -10.0, 6.0, 3.0), 0x4A5370, 0.8, tf);
            fill(pm, shape(&[('M', &[-10.0, 4.0]), ('C', &[-10.0, -3.0, -5.0, -7.0, 0.0, -7.0]), ('C', &[6.0, -7.0, 10.0, -3.0, 10.0, 4.0]), ('C', &[10.0, 10.0, 5.0, 13.0, 0.0, 13.0]), ('C', &[-5.0, 13.0, -10.0, 10.0, -10.0, 4.0]), ('Z', &[])]), 0xF4F7FB, 1.0, tf);
            for ex in [-5.0f32, 5.0] {
                fill(pm, oval(ex, -6.0, 4.2, 4.8), 0xFFFFFF, 1.0, tf);
                outline(pm, pen, oval(ex, -6.0, 4.2, 4.8), 0x0E1120, 0.8, tf);
                fill(pm, circle(ex + 0.8, -5.2, 2.2), 0x0E1120, 1.0, tf);
                fill(pm, circle(ex + 1.4, -6.0, 0.8), 0xFFFFFF, 1.0, tf);
            }
            let beak = || shape(&[('M', &[-7.0, 0.0]), ('C', &[-3.0, -2.5, 3.0, -2.5, 7.0, 0.0]), ('C', &[4.0, 5.0, -4.0, 5.0, -7.0, 0.0]), ('Z', &[])]);
            fill(pm, beak(), 0xF08A3A, 1.0, tf);
            outline(pm, pen, beak(), 0xB4560F, 0.9, tf);
            fill(pm, oval(-6.0, -12.0, 4.0, 2.0), 0xFFFFFF, 0.28, tf);
        }
        Tip::Cloud => {
            let tf = at_tf(t, u, 0.0, -22.0, 4.0);
            let bolt = || shape(&[('M', &[-1.0, 12.0]), ('L', &[-5.0, 19.0]), ('L', &[-1.0, 19.0]), ('L', &[-4.0, 27.0]), ('L', &[6.0, 16.0]), ('L', &[2.0, 16.0]), ('L', &[5.0, 12.0]), ('Z', &[])]);
            fill(pm, circle(1.0, 19.0, 9.0), 0xFFE066, 0.25, tf);
            fill(pm, bolt(), 0xFFD23A, 1.0, tf);
            outline(pm, pen, bolt(), 0xC99A0E, 0.8, tf);
            for x in [10.0f32, 15.0, -9.0] {
                line(pm, pen, (x, 13.0), (x - 1.5, 17.5), 0x8FB7E8, 1.0, 1.5, tf);
            }
            let cloud = || shape(&[('M', &[-19.0, 8.0]), ('C', &[-25.0, 8.0, -26.0, -1.0, -19.0, -2.0]), ('C', &[-20.0, -11.0, -9.0, -15.0, -4.0, -9.0]), ('C', &[-1.0, -17.0, 12.0, -17.0, 14.0, -8.0]), ('C', &[21.0, -10.0, 26.0, -2.0, 21.0, 3.0]), ('C', &[25.0, 6.0, 22.0, 12.0, 16.0, 11.0]), ('L', &[-16.0, 11.0]), ('C', &[-20.0, 11.0, -21.0, 9.0, -19.0, 8.0]), ('Z', &[])]);
            fill(pm, cloud(), 0xD5DDEA, 1.0, tf);
            fill(pm, oval(0.0, -4.0, 16.0, 7.0), 0xF4F7FC, 0.9, tf);
            outline(pm, pen, cloud(), 0x5B6B86, 1.2, tf);
            fill(pm, oval(-6.0, -8.0, 6.0, 2.6), 0xFFFFFF, 0.7, tf);
        }
        Tip::Kunai => {
            let tf = rot_tf(t, ang);
            line(pm, pen, (0.0, 0.0), (5.0, 0.0), 0x1E1F24, 1.0, 3.0, tf);
            outline(pm, pen, circle(6.5, 0.0, 2.6), 0xC8CCD2, 1.2, tf);
            let blade = || shape(&[('M', &[9.0, -4.0]), ('L', &[24.0, 0.0]), ('L', &[9.0, 4.0]), ('Z', &[])]);
            fill(pm, blade(), 0xC8CCD2, 1.0, tf);
            outline(pm, pen, blade(), 0x5A5E66, 0.8, tf);
            line(pm, pen, (10.0, 0.0), (22.0, 0.0), 0xFFFFFF, 0.6, 0.8, tf);
        }
    }
}

/// A user's picture as the tip: upright, ~48 px tall, centred one radius past the lash end.
fn draw_custom_tip(pm: &mut Pixmap, pts: &[V], img: &Pixmap) {
    let (t, u, _) = frames(pts);
    let s = 48.0 / img.height().max(1) as f32;
    let (w, h) = (img.width() as f32 * s, img.height() as f32 * s);
    let c = (t.0 + u.0 * w * 0.45, t.1 + u.1 * h * 0.45);
    let tf = Transform::from_translate(c.0 - w / 2.0, c.1 - h / 2.0).pre_scale(s, s);
    tick();
    pm.draw_pixmap(0, 0, img.as_ref(), &PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..PixmapPaint::default() }, tf, None);
}

#[cfg(test)]
mod tests {
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
}
