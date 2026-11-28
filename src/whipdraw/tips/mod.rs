//! The tip art: one file per tip, picked by `draw_tip` from the whip's catalogue entry.

use super::*;

mod frayed;
mod bead;
mod tail;
mod pompom;
mod drip;
mod bloom;
mod penguin;
mod cloud;
mod pan;
mod kunai;
mod lasso;
mod whistle;
mod bell;
mod hook;

/// Tip frame helpers. `rot`: art rotated with the lash. `at`: art upright, its local point (ax, ay)
/// placed `dist` units past the lash end along the lash direction.
pub(super) fn frames(pts: &[V]) -> (V, V, f32) {
    let t = pts[pts.len() - 1];
    let p = pts[pts.len() - 3];
    let (dx, dy) = (t.0 - p.0, t.1 - p.1);
    let d = dx.hypot(dy).max(1e-3);
    (t, (dx / d, dy / d), dy.atan2(dx).to_degrees())
}

pub(super) fn rot_tf(t: V, ang: f32) -> Transform {
    Transform::from_translate(t.0, t.1).pre_rotate(ang).pre_scale(K, K)
}

pub(super) fn at_tf(t: V, u: V, dist: f32, ax: f32, ay: f32) -> Transform {
    Transform::from_translate(t.0 + u.0 * dist * K, t.1 + u.1 * dist * K)
        .pre_scale(K, K)
        .pre_translate(-ax, -ay)
}

/// A user's picture as the tip: upright, ~48 px tall, centred one radius past the lash end.
pub(super) fn draw_custom_tip(pm: &mut Pixmap, pts: &[V], img: &Pixmap) {
    let (t, u, _) = frames(pts);
    let s = 48.0 / img.height().max(1) as f32;
    let (w, h) = (img.width() as f32 * s, img.height() as f32 * s);
    let c = (t.0 + u.0 * w * 0.45, t.1 + u.1 * h * 0.45);
    let tf = Transform::from_translate(c.0 - w / 2.0, c.1 - h / 2.0).pre_scale(s, s);
    tick();
    pm.draw_pixmap(0, 0, img.as_ref(), &PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..PixmapPaint::default() }, tf, None);
}

pub(super) fn draw_tip(pm: &mut Pixmap, pen: &mut Pen, pts: &[V], tip: Tip, l: &Look) {
    let (t, u, ang) = frames(pts);
    match tip {
        Tip::Frayed => frayed::draw(pm, pen, pts, l, t, u, ang),
        Tip::Bead => bead::draw(pm, pen, pts, l, t, u, ang),
        Tip::Tail => tail::draw(pm, pen, pts, l, t, u, ang),
        Tip::PomPom => pompom::draw(pm, pen, pts, l, t, u, ang),
        Tip::Drip => drip::draw(pm, pen, pts, l, t, u, ang),
        Tip::Bloom => bloom::draw(pm, pen, pts, l, t, u, ang),
        Tip::Penguin => penguin::draw(pm, pen, pts, l, t, u, ang),
        Tip::Cloud => cloud::draw(pm, pen, pts, l, t, u, ang),
        Tip::Pan => pan::draw(pm, pen, pts, l, t, u, ang),
        Tip::Kunai => kunai::draw(pm, pen, pts, l, t, u, ang),
        Tip::Lasso => lasso::draw(pm, pen, pts, l, t, u, ang),
        Tip::Whistle => whistle::draw(pm, pen, pts, l, t, u, ang),
        Tip::Bell => bell::draw(pm, pen, pts, l, t, u, ang),
        Tip::Hook => hook::draw(pm, pen, pts, l, t, u, ang),
    }
}
