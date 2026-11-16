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

const GLOW_DEPTH: f32 = 150.0;
/// The glow's strength and each edge's share are drawn in this many steps: a breathing glow
/// repaints the edge strips on a step, not on every frame.
const GLOW_STEPS: f32 = 48.0;
const FLASH_RADIUS: f32 = 40.0;
const METER: (f32, f32) = (260.0, 12.0);
const METER_TOP: f32 = 22.0;
const INK: u32 = 0x0B1220;

/// Where the edge glow sits: `area` shifted by `off` in whole pixels, and how deep it reaches.
fn glow_frame(area: Rect, scale: f32, off: V) -> (R, i32) {
    let (dx, dy) = (off.0.round() as i32, off.1.round() as i32);
    let (l, t, r, b) = (area.left().round() as i32 + dx, area.top().round() as i32 + dy, area.right().round() as i32 + dx, area.bottom().round() as i32 + dy);
    ((l, t, r, b), ((GLOW_DEPTH * scale).round() as i32).min((r - l) / 2).min((b - t) / 2))
}

/// The box of one edge's glow (0 top, 1 bottom, 2 left, 3 right). Top and bottom run the full
/// width and hold the corners; no two boxes overlap, so one edge can be repainted alone.
fn edge_box(((l, t, r, b), d): (R, i32), e: usize) -> R {
    match e {
        0 => (l, t, r, t + d),
        1 => (l, b - d, r, b),
        2 => (l, t + d, l + d, b - d),
        _ => (r - d, t + d, r, b - d),
    }
}

/// Source-over of one premultiplied pixel.
fn over(dst: &mut [u8], src: [u8; 4]) {
    let inv = 255 - src[3] as u32;
    for (d, s) in dst.iter_mut().zip(src) {
        *d = (s as u32 + (*d as u32 * inv + 127) / 255) as u8;
    }
}

/// Ordered-dither thresholds (in 16ths): the glow's alpha is rounded up or down by them, so
/// its 8-bit steps do not draw contour lines on a light window.
const DITHER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Paints the glow inside one edge's box, each pixel once. The glow fades inward as
/// closeness^2.5 (closeness 1 at the frame, 0 at full depth: about 1, 0.45, 0.12, 0 at depths
/// 0, 0.25, 0.6, 1, with no step where it ends). In a corner the two edges join in a smooth
/// curve (the 5th powers of their closeness add up), never brighter than an edge on its own
/// and with no crease along the mitre, and their strengths blend, so a strong edge meets a
/// weak one without a seam.
fn draw_edge(pm: &mut Pixmap, frame: (R, i32), e: usize, rgb: u32, alpha: f32, edges: [f32; 4]) {
    let ((l, t, r, b), d) = frame;
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let bx = edge_box(frame, e);
    let (x0, y0, x1, y1) = (bx.0.clamp(0, w), bx.1.clamp(0, h), bx.2.clamp(0, w), bx.3.clamp(0, h));
    if d < 1 || x0 >= x1 || y0 >= y1 {
        return;
    }
    let (cr, cg, cb) = ((rgb >> 16) as u8 as f32, (rgb >> 8) as u8 as f32, rgb as u8 as f32);
    // the colour of glow strength `g` (0..1) at pixel (x, y)
    let shade = |g: f32, x: i32, y: i32| -> [u8; 4] {
        let step = (DITHER[(y & 3) as usize][(x & 3) as usize] as f32 + 0.5) / 16.0;
        let a = ((alpha * g).clamp(0.0, 1.0) * 255.0 + step).floor() / 255.0;
        [(cr * a).round() as u8, (cg * a).round() as u8, (cb * a).round() as u8, (255.0 * a).round() as u8]
    };
    // closeness^5 of a pixel `k` px from an edge; the glow is its square root
    let q: Vec<f32> = (0..d).map(|k| (1.0 - (k as f32 + 0.5) / d as f32).powi(5)).collect();
    // away from the corners: `k` px from this box's own edge
    let own = |k: i32| q[k as usize].sqrt() * edges[e];
    // `kx` px from the left or right edge (`sx` its strength), `ky` from this box's own
    let corner = |kx: i32, sx: f32, ky: i32| {
        let (qx, qy) = (q[kx as usize], q[ky as usize]);
        (qx + qy).min(1.0).sqrt() * (qx * sx + qy * edges[e]) / (qx + qy)
    };
    if e >= 2 {
        // a side box has no corner: every row repeats one of four (the dither's rows)
        let rows: [Vec<[u8; 4]>; 4] = std::array::from_fn(|j| (x0..x1).map(|x| shade(own((x - l).min(r - 1 - x)), x, j as i32)).collect());
        for y in y0..y1 {
            for (p, src) in row(pm, y, x0, x1).chunks_exact_mut(4).zip(&rows[(y & 3) as usize]) {
                over(p, *src);
            }
        }
        return;
    }
    // the top or bottom box: a corner at each end, in between one strength (4 dithered pixels, repeated)
    let (a, z) = ((l + d).clamp(x0, x1), (r - d).clamp(x0, x1));
    for y in y0..y1 {
        let ky = if e == 0 { y - t } else { b - 1 - y };
        let between: [[u8; 4]; 4] = std::array::from_fn(|i| shade(own(ky), i as i32, y));
        let (left, rest) = row(pm, y, x0, x1).split_at_mut((a - x0) as usize * 4);
        let (mid, right) = rest.split_at_mut((z - a) as usize * 4);
        for (x, p) in (x0..a).zip(left.chunks_exact_mut(4)) {
            over(p, shade(corner(x - l, edges[2], ky), x, y));
        }
        for (x, p) in (a..z).zip(mid.chunks_exact_mut(4)) {
            over(p, between[(x & 3) as usize]);
        }
        for (x, p) in (z..x1).zip(right.chunks_exact_mut(4)) {
            over(p, shade(corner(r - 1 - x, edges[3], ky), x, y));
        }
    }
}

/// The flash at the tip: white at the centre with a rim in the whip's colour, so it also
/// shows on a light window.
fn draw_flash(pm: &mut Pixmap, tip: V, flash: f32, hue: u32) {
    let c = Point::from_xy(tip.0, tip.1);
    let stops = vec![GradientStop::new(0.0, col(0xFFFFFF, 0.8 * flash)), GradientStop::new(0.45, col(hue, 0.45 * flash)), GradientStop::new(1.0, col(hue, 0.0))];
    if let Some(shader) = RadialGradient::new(c, 0.0, c, FLASH_RADIUS, stops, SpreadMode::Pad, Transform::identity()) {
        let mut p = Paint::default();
        p.shader = shader;
        if let Some(path) = PathBuilder::from_circle(tip.0, tip.1, FLASH_RADIUS) {
            pm.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
        }
    }
}

fn pill(x: f32, y: f32, w: f32, h: f32) -> Option<Path> {
    let r = h / 2.0;
    let w = w.max(h);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Top-left of the meter bar; `s` is the display scaling.
fn meter_origin(work: Rect, s: f32) -> V {
    (work.left() + (work.width() - METER.0 * s) / 2.0, work.top() + METER_TOP * s)
}

/// How far the full meter's halo reaches past the bar (px at 100%).
const HALO: f32 = 16.0;

/// The heat meter: a pill at the top centre of the work area, filled in the whip's colour.
/// Full, it turns bright: a soft breathing halo, a white frame, no notches.
fn draw_meter(pm: &mut Pixmap, level: f32, full: bool, hue: u32, clock: f32, work: Rect, s: f32) {
    let (w, h) = (METER.0 * s, METER.1 * s);
    let (x, y) = meter_origin(work, s);
    let id = Transform::identity();
    let mut fill = |path: Option<Path>, rgb: u32, a: f32| {
        if let Some(p) = path {
            pm.fill_path(&p, &paint(rgb, a), FillRule::Winding, id, None);
        }
    };
    // a pill `pad` px (at 100%) larger than the bar all round
    let around = |pad: f32| pill(x - pad * s, y - pad * s, w + 2.0 * pad * s, h + 2.0 * pad * s);
    if full {
        let breathe = 0.30 + 0.12 * (clock * 6.0).sin();
        for (pad, a) in [(HALO, 0.22), (10.0, 0.5), (5.0, 0.9)] {
            fill(around(pad), hue, breathe * a);
        }
        // a thin dark line holds the white frame (and a pale fill) on a white window
        fill(around(4.0), INK, 0.5);
    }
    let (frame, frame_a) = if full { (0xFFFFFF, 0.9) } else { (INK, 0.88) };
    fill(around(3.0), frame, frame_a);
    fill(pill(x, y, w, h), 0xFFFFFF, 0.07);
    let level = level.clamp(0.0, 1.0);
    if level > 0.004 {
        fill(pill(x, y, w * level, h), hue, 1.0);
        fill(pill(x + 3.0 * s, y + 2.0 * s, (w * level - 6.0 * s).max(4.0 * s), 3.0 * s), 0xFFFFFF, if full { 0.7 } else { 0.4 });
    }
    if full {
        return;
    }
    // one notch per crack on the way to full: dark over the fill, light on the empty track
    for k in 1..crate::rage::CRACKS_TO_FULL {
        let at = k as f32 / crate::rage::CRACKS_TO_FULL as f32;
        if let Some(r) = Rect::from_xywh(x + w * at - s, y + 2.0 * s, 2.0 * s, h - 4.0 * s) {
            let (rgb, a) = if at < level { (INK, 0.45) } else { (0xFFFFFF, 0.28) };
            pm.fill_rect(r, &paint(rgb, a), id, None);
        }
    }
}

/// Size and placement of the combo label this frame: (centre x, baseline y, px height, path
/// scale). `s` is the display scaling.
fn combo_layout(c: &ComboView, work: Rect, s: f32) -> (f32, f32, f32, f32) {
    let px = (30.0 + 3.0 * c.count.min(12) as f32) * s;
    let pop = 1.0 + 0.6 * (1.0 - (c.age / 0.14).min(1.0)).powi(2);
    let half = c.width * px / EM * 0.8 + 30.0 * s;
    let x = (c.at.0 + c.dir as f32 * 24.0 * s).clamp(work.left() + half, (work.right() - half).max(work.left() + half));
    let top = work.top() + (METER_TOP + 40.0) * s + px * 1.8;
    let y = (c.at.1 - (90.0 + 26.0 * c.age) * s).clamp(top, (work.bottom() - 30.0 * s).max(top));
    (x, y, px, px / EM * pop)
}

fn combo_box(c: &ComboView, work: Rect, s: f32) -> (f32, f32, f32, f32) {
    let (x, y, px, _) = combo_layout(c, work, s);
    let half = c.width * px / EM * 0.8 + 30.0 * s;
    let lean = half * 0.12;
    (x - half, y - px * 1.8 - lean - 12.0 * s, x + half, y + px * 0.8 + lean + 12.0 * s)
}

/// "SHOUT ×N" popping in at the crack, in the whip's colour, drifting up and fading. It always
/// rises to the right, a little more after a right-hand crack.
fn draw_combo(pm: &mut Pixmap, c: &ComboView, hue: u32, work: Rect, s: f32) {
    let (x, y, px, k) = combo_layout(c, work, s);
    let alpha = ((COMBO_SHOW - c.age) / 0.35).clamp(0.0, 1.0);
    let tilt = if c.dir > 0 { -6.0 } else { -3.0 };
    let tf = Transform::from_translate(x, y).pre_rotate(tilt).pre_scale(k, k).pre_translate(-c.width / 2.0, 0.0);
    pm.fill_path(c.path, &paint(0x000000, 0.3 * alpha), FillRule::Winding, tf.post_translate(3.0 * s, 4.0 * s), None);
    // the outline grows with the text: a small label keeps its counters open
    let ink = (px / s * 0.10).clamp(3.5, 6.5) * s;
    let stroke = |width: f32| Stroke { width: width / k, line_join: tiny_skia::LineJoin::Round, ..Stroke::default() };
    // every fifth crack lands in white, with a rim in the whip's colour to hold it on a white window
    let milestone = c.count % 5 == 0;
    if milestone {
        pm.stroke_path(c.path, &paint(hue, alpha), &stroke(ink + 6.0 * s), tf, None);
    }
    pm.stroke_path(c.path, &paint(INK, 0.9 * alpha), &stroke(ink), tf, None);
    pm.fill_path(c.path, &paint(if milestone { 0xFFFFFF } else { hue }, alpha), FillRule::Winding, tf, None);
}

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
mod tests {
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
}
