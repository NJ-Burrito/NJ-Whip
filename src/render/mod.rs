//! CPU rendering of the whip and effects into a premultiplied RGBA `Pixmap`.
//!
//! The overlay covers a whole monitor, but a frame only touches what changed. The scene is
//! layers painted in a fixed order (the glow of each screen edge, whip, burst, meter, combo);
//! each reports the boxes it may touch and a key of everything it shows. `Canvas::draw` clears
//! and repaints only the layers whose boxes or key changed, plus any layer such a box runs
//! into. The caller pushes just the returned boxes to the screen.

use crate::effects::Fx;
use crate::glyphs::EM;
use crate::ink::{col, paint};
use crate::rage::COMBO_SHOW;
use crate::whips::WhipDef;
use std::hash::Hasher;
use tiny_skia::{
    FillRule, GradientStop, Paint, Path, PathBuilder, Pixmap, Point, RadialGradient, Rect,
    SpreadMode, Stroke, Transform,
};

#[cfg(test)]
mod frames;
mod glow;
use glow::*;
mod hud;
use hud::*;

type V = (f32, f32);

/// A pixel box: left, top, right, bottom (right/bottom exclusive), inside the pixmap.
pub type R = (i32, i32, i32, i32);

/// The combo label: `path` is one line of text at `glyphs::EM`, `width` its advance.
#[derive(Clone, Copy)]
pub struct ComboView<'a> {
    pub path: &'a Path,
    pub width: f32,
    /// Where the crack happened.
    pub at: V,
    /// Seconds since that crack.
    pub age: f32,
    pub count: u32,
    /// Side of the crack (-1 left, 1 right, 0 none): the label leans that way.
    pub dir: i8,
}

/// Everything one frame shows.
#[derive(Clone, Copy)]
pub struct Scene<'a> {
    /// The lash points (handle first), the whip type and an optional custom tip image.
    pub whip: Option<(&'a [V], &'a WhipDef, Option<&'a Pixmap>)>,
    /// White flash at the tip right after a crack, 0..1.
    pub flash: f32,
    /// Edge glow in `hue` (the whip's full-heat screen effect), 0..1, shifted by `offset`.
    pub glow: f32,
    /// How much of the glow each edge gets (top, bottom, left, right), 0..1.
    pub edges: [f32; 4],
    pub hue: u32,
    pub offset: V,
    /// Display scaling (monitor DPI / 96): the meter, the glow depth and the combo grow by it.
    pub scale: f32,
    /// Heat meter: level 0..1 and whether the full effect is on.
    pub meter: Option<(f32, bool)>,
    /// Any steady clock in seconds (the full meter shimmers on it).
    pub clock: f32,
    pub combo: Option<ComboView<'a>>,
    /// The crack effect layer, drawn over the whip.
    pub burst: Option<&'a Fx>,
}

#[cfg(test)]
impl Scene<'_> {
    pub fn none() -> Self {
        Scene { whip: None, flash: 0.0, glow: 0.0, edges: [1.0; 4], hue: 0x7FC4EC, offset: (0.0, 0.0), scale: 1.0, meter: None, clock: 0.0, combo: None, burst: None }
    }
}

const INK: u32 = 0x0B1220;














/// A float box as pixels clamped to a `w` x `h` pixmap; None when nothing is left.
fn px_box(b: (f32, f32, f32, f32), w: i32, h: i32) -> Option<R> {
    let r = ((b.0.floor() as i32).clamp(0, w), (b.1.floor() as i32).clamp(0, h), (b.2.ceil() as i32).clamp(0, w), (b.3.ceil() as i32).clamp(0, h));
    (r.0 < r.2 && r.1 < r.3).then_some(r)
}

/// Pixels whose centre lies inside `work`.
fn work_px(work: Rect, w: i32, h: i32) -> R {
    let c = |v: f32, max: i32| ((v - 0.5).ceil() as i32).clamp(0, max);
    (c(work.left(), w), c(work.top(), h), c(work.right(), w), c(work.bottom(), h))
}

/// One paint layer this frame: the boxes it may touch, and a key of everything it shows there.
type Layer = (Vec<R>, u64);
/// The layers in paint order. 0..4 are the glow of the top, bottom, left and right edge (heat
/// glow + the burst's screen flash): a whip brushing one edge repaints that edge alone.
const L_WHIP: usize = 4; // whip + tip flash
const L_BURST: usize = 5;
const L_METER: usize = 6;
const L_COMBO: usize = 7;
const LAYERS: usize = 8;
/// How far the lash is drawn past its points.
const LASH_PAD: f32 = 48.0;

fn key(floats: impl IntoIterator<Item = f32>, ints: impl IntoIterator<Item = u64>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    floats.into_iter().for_each(|f| h.write_u32(f.to_bits()));
    ints.into_iter().for_each(|i| h.write_u64(i));
    h.finish()
}

/// The scene as layers. The burst's key is left at 0: its particles move every frame, so the
/// canvas counts frames for it.
fn layers(s: &Scene, w: i32, h: i32, work: Rect) -> [Layer; LAYERS] {
    let bx = |b: (f32, f32, f32, f32)| px_box(b, w, h);
    let around = |p: V, pad: f32| (p.0 - pad, p.1 - pad, p.0 + pad, p.1 + pad);
    let mut v: [Layer; LAYERS] = Default::default();

    let flash = s.burst.and_then(Fx::flash_now);
    let (rgb, a) = flash.unwrap_or((0, 0.0));
    let (still, moved) = (glow_frame(work, s.scale, (0.0, 0.0)), glow_frame(work, s.scale, s.offset));
    for (e, layer) in v[..4].iter_mut().enumerate() {
        let ibx = |b: R| bx((b.0 as f32, b.1 as f32, b.2 as f32, b.3 as f32));
        // the flash sits on the work area, the heat glow may be shifted
        layer.0.extend(flash.and_then(|_| ibx(edge_box(still, e))));
        layer.0.extend((s.glow > 0.0).then(|| ibx(edge_box(moved, e))).flatten());
        if !layer.0.is_empty() {
            // the corners (in the top and bottom boxes) also show the side edges' strengths
            let sides = if e < 2 { [s.edges[2], s.edges[3]] } else { [0.0; 2] };
            let ((l, t, _, _), d) = moved;
            layer.1 = key([s.glow, a, s.edges[e], sides[0], sides[1]], [s.hue as u64, rgb as u64, l as u64, t as u64, d as u64]);
        }
    }
    if let Some((pts, def, tip)) = s.whip {
        let b = pts.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |b, p| (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1)));
        v[L_WHIP].0.extend(bx((b.0 - LASH_PAD, b.1 - LASH_PAD, b.2 + LASH_PAD, b.3 + LASH_PAD)));
        v[L_WHIP].0.extend(bx(around(pts[0], 135.0))); // the handle sticks out behind the hand
        let tip_pad = 110.0 + tip.map_or(0.0, |i| i.width() as f32 * 48.0 / i.height().max(1) as f32);
        v[L_WHIP].0.extend(bx(around(pts[pts.len() - 1], tip_pad)));
        // The whip type and the tip picture by address (both outlive the frames that show them),
        // and the hue that rims the tip flash.
        let ids = [def as *const WhipDef as u64, tip.map_or(0, |i| i.data().as_ptr() as u64), s.hue as u64];
        v[L_WHIP].1 = key(pts.iter().flat_map(|p| [p.0, p.1]).chain([s.flash]), ids);
    }
    v[L_BURST].0.extend(s.burst.and_then(Fx::bounds).and_then(bx));
    if let Some((level, full)) = s.meter {
        let (x, y) = meter_origin(work, s.scale);
        let pad = (HALO + 1.0) * s.scale;
        v[L_METER].0.extend(bx((x - pad, y - pad, x + METER.0 * s.scale + pad, y + METER.1 * s.scale + pad)));
        v[L_METER].1 = key([level, if full { s.clock } else { 0.0 }, s.scale], [s.hue as u64, full as u64]);
    }
    if let Some(c) = &s.combo {
        v[L_COMBO].0.extend(bx(combo_box(c, work, s.scale)));
        let b = c.path.bounds();
        v[L_COMBO].1 = key(
            [c.width, c.at.0, c.at.1, c.age, s.scale, b.left(), b.top(), b.right(), b.bottom()],
            [s.hue as u64, c.count as u64, c.dir as u64, c.path.len() as u64],
        );
    }
    v
}

fn paint_layer(pm: &mut Pixmap, layer: usize, s: &Scene, work: Rect) {
    match layer {
        0..=3 => {
            if let Some((rgb, a)) = s.burst.and_then(Fx::flash_now) {
                draw_edge(pm, glow_frame(work, s.scale, (0.0, 0.0)), layer, rgb, a, [1.0; 4]);
            }
            if s.glow > 0.0 {
                draw_edge(pm, glow_frame(work, s.scale, s.offset), layer, s.hue, 0.5 * s.glow, s.edges);
            }
        }
        L_WHIP => {
            if let Some((pts, def, tip)) = s.whip {
                crate::whipdraw::draw_whip(pm, pts, def, tip);
                if s.flash > 0.0 {
                    if let Some(&t) = pts.last() {
                        draw_flash(pm, t, s.flash, s.hue);
                    }
                }
            }
        }
        L_BURST => {
            if let Some(b) = s.burst {
                b.draw(pm);
            }
        }
        L_METER => {
            if let Some((level, full)) = s.meter {
                draw_meter(pm, level, full, s.hue, s.clock, work, s.scale);
            }
        }
        _ => {
            if let Some(c) = &s.combo {
                draw_combo(pm, c, s.hue, work, s.scale);
            }
        }
    }
}

fn area(r: R) -> i64 {
    (r.2 - r.0) as i64 * (r.3 - r.1) as i64
}

fn overlap(a: R, b: R) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// Replaces two boxes by their bounding box whenever that covers no more than the two apart
/// (they overlap that much, or one holds the other), until no pair qualifies.
fn merge(v: &mut Vec<R>) {
    'again: loop {
        for i in 0..v.len() {
            for j in i + 1..v.len() {
                let (a, b) = (v[i], v[j]);
                let u = (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3));
                if area(u) <= area(a) + area(b) {
                    v[i] = u;
                    v.swap_remove(j);
                    continue 'again;
                }
            }
        }
        return;
    }
}

fn row(pm: &mut Pixmap, y: i32, x0: i32, x1: i32) -> &mut [u8] {
    let w = pm.width() as usize;
    let at = y as usize * w;
    &mut pm.data_mut()[(at + x0 as usize) * 4..(at + x1 as usize) * 4]
}

/// Resets box `r`: `bg` inside `keep`, fully transparent outside it.
fn clear(pm: &mut Pixmap, r: R, keep: R, bg: [u8; 4]) {
    for y in r.1..r.3 {
        let px = row(pm, y, r.0, r.2);
        px.fill(0);
        let (a, b) = (keep.0.clamp(r.0, r.2), keep.2.clamp(r.0, r.2));
        if bg != [0; 4] && y >= keep.1 && y < keep.3 && a < b {
            for p in px[(a - r.0) as usize * 4..(b - r.0) as usize * 4].chunks_exact_mut(4) {
                p.copy_from_slice(&bg);
            }
        }
    }
}

/// Makes box `r` fully transparent outside `keep`.
fn mask(pm: &mut Pixmap, r: R, keep: R) {
    for y in r.1..r.3 {
        let px = row(pm, y, r.0, r.2);
        if y < keep.1 || y >= keep.3 {
            px.fill(0);
        } else {
            let (a, b) = (keep.0.clamp(r.0, r.2), keep.2.clamp(r.0, r.2));
            px[..(a - r.0) as usize * 4].fill(0);
            px[(b - r.0) as usize * 4..].fill(0);
        }
    }
}

/// The overlay's frame buffer plus what the last frame drew.
pub struct Canvas {
    pub pm: Pixmap,
    prev: [Layer; LAYERS],
    /// Last frame's mode (None before the first frame): whether the whip was shown (the work
    /// area then catches clicks), and the work area.
    mode: Option<(bool, Rect)>,
    /// Counts the frames that show a burst (its key).
    tick: u64,
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Option<Canvas> {
        Some(Canvas { pm: Pixmap::new(w, h)?, prev: Default::default(), mode: None, tick: 0 })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.pm.width(), self.pm.height())
    }

    /// Draws one frame and returns the boxes that changed: none when the scene is the one
    /// already drawn, the whole pixmap on the first frame and when the whip appears or goes.
    /// `work` is the monitor's work area in pixmap coordinates: with the whip shown it carries
    /// a near-invisible fill (alpha 1) so the window catches mouse events there, and nothing is
    /// drawn outside it, so the taskbar stays usable.
    pub fn draw(&mut self, s: &Scene, work: Rect) -> Vec<R> {
        let stepped = |v: f32| (v * GLOW_STEPS).round() / GLOW_STEPS;
        let s = &Scene { glow: stepped(s.glow), edges: s.edges.map(stepped), ..*s };
        let (w, h) = (self.pm.width() as i32, self.pm.height() as i32);
        let catching = s.whip.is_some();
        let mut now = layers(s, w, h, work);
        if let Some(fx) = s.burst.filter(|_| !now[L_BURST].0.is_empty()) {
            // moving parts change every frame; a mark left behind only on its fade steps
            if fx.moving() {
                self.tick += 1;
            }
            now[L_BURST].1 = self.tick.wrapping_add(fx.mark_step().wrapping_mul(0x9E37_79B9_7F4A_7C15));
        }
        let whole = self.mode != Some((catching, work));
        self.mode = Some((catching, work));
        let mut changed: [bool; LAYERS] = std::array::from_fn(|i| whole || now[i] != self.prev[i]);
        let mut dirty: Vec<R> = if whole {
            vec![(0, 0, w, h)]
        } else {
            (0..LAYERS).filter(|&i| changed[i]).flat_map(|i| self.prev[i].0.iter().chain(&now[i].0).copied()).collect()
        };
        // Clearing a box wipes every layer in it: a layer a dirty box runs into is repainted
        // too, and its own boxes may reach further layers.
        while !whole {
            merge(&mut dirty);
            let n = dirty.len();
            for i in 0..LAYERS {
                if !changed[i] && now[i].0.iter().any(|&b| dirty.iter().any(|&d| overlap(b, d))) {
                    changed[i] = true;
                    dirty.extend(&now[i].0);
                }
            }
            if dirty.len() == n {
                break;
            }
        }
        self.prev = now;
        if dirty.is_empty() {
            return dirty;
        }
        let keep = work_px(work, w, h);
        let bg = if catching { [0, 0, 0, 1] } else { [0; 4] };
        for &r in &dirty {
            clear(&mut self.pm, r, keep, bg);
        }
        for i in (0..LAYERS).filter(|&i| changed[i]) {
            paint_layer(&mut self.pm, i, s, work);
        }
        if catching {
            for &r in &dirty {
                mask(&mut self.pm, r, keep);
            }
        }
        dirty
    }
}

/// Draw a scene once on a fresh pixmap (thumbnails, tests).
#[cfg(test)]
pub fn draw_once(w: u32, h: u32, s: &Scene, work: Rect) -> Pixmap {
    let mut c = Canvas::new(w, h).unwrap();
    c.draw(s, work);
    c.pm
}

#[cfg(test)]
mod tests;
