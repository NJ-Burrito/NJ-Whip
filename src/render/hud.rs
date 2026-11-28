//! The heat meter and the combo label.

use super::*;

pub(super) const METER: (f32, f32) = (260.0, 12.0);

pub(super) const METER_TOP: f32 = 22.0;

pub(super) fn pill(x: f32, y: f32, w: f32, h: f32) -> Option<Path> {
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
pub(super) fn meter_origin(work: Rect, s: f32) -> V {
    (work.left() + (work.width() - METER.0 * s) / 2.0, work.top() + METER_TOP * s)
}

/// How far the full meter's halo reaches past the bar (px at 100%).
pub(super) const HALO: f32 = 16.0;

/// The heat meter: a pill at the top centre of the work area, filled in the whip's colour.
/// Full, it turns bright: a soft breathing halo, a white frame, no notches.
pub(super) fn draw_meter(pm: &mut Pixmap, level: f32, full: bool, hue: u32, clock: f32, work: Rect, s: f32) {
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
pub(super) fn combo_layout(c: &ComboView, work: Rect, s: f32) -> (f32, f32, f32, f32) {
    let px = (30.0 + 3.0 * c.count.min(12) as f32) * s;
    let pop = 1.0 + 0.6 * (1.0 - (c.age / 0.14).min(1.0)).powi(2);
    let half = c.width * px / EM * 0.8 + 30.0 * s;
    let x = (c.at.0 + c.dir as f32 * 24.0 * s).clamp(work.left() + half, (work.right() - half).max(work.left() + half));
    let top = work.top() + (METER_TOP + 40.0) * s + px * 1.8;
    let y = (c.at.1 - (90.0 + 26.0 * c.age) * s).clamp(top, (work.bottom() - 30.0 * s).max(top));
    (x, y, px, px / EM * pop)
}

pub(super) fn combo_box(c: &ComboView, work: Rect, s: f32) -> (f32, f32, f32, f32) {
    let (x, y, px, _) = combo_layout(c, work, s);
    let half = c.width * px / EM * 0.8 + 30.0 * s;
    let lean = half * 0.12;
    (x - half, y - px * 1.8 - lean - 12.0 * s, x + half, y + px * 0.8 + lean + 12.0 * s)
}

/// "SHOUT ×N" popping in at the crack, in the whip's colour, drifting up and fading. It always
/// rises to the right, a little more after a right-hand crack.
pub(super) fn draw_combo(pm: &mut Pixmap, c: &ComboView, hue: u32, work: Rect, s: f32) {
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

