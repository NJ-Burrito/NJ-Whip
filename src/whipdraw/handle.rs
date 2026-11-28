//! The handle: grip, wraps, ferrule, knob.

use super::*;

pub(super) fn draw_handle(pm: &mut Pixmap, pen: &mut Pen, pts: &[V], l: &Look) {
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

