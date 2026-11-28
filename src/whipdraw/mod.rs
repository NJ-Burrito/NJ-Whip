//! Draws one whip (handle, tapered lash, pattern, tip) from its catalogue look.
//! Geometry follows the approved mockup.

use crate::ink::{paint, shape};
use crate::whips::{Look, Pattern, Tip, WhipDef};
use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, PathStroker, Pixmap, PixmapPaint, Stroke,
    Transform,
};

mod handle;
use handle::*;
mod pattern;
use pattern::*;
mod tips;
use tips::*;

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







#[cfg(test)]
mod tests;
