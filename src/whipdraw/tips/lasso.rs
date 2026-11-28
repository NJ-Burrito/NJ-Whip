//! The Lasso tip: a honda knot and a twisted rawhide loop (Cowboy).

use super::*;

pub(super) fn draw(pm: &mut Pixmap, pen: &mut Pen, _pts: &[V], l: &Look, t: V, _u: V, ang: f32) {
    let tf = rot_tf(t, ang);
    let (cx, rx, ry) = (18.0f32, 14.0f32, 10.0f32);
    // contact shadow, then the shade inside the loop
    if let Some(p) = oval(cx + 1.5, 2.6, rx, ry) {
        pen.stroke_path(pm, &p, &paint(0x000000, 0.17), &stroke(3.8), tf);
    }
    fill(pm, oval(cx, 0.0, rx - 1.6, ry - 1.6), 0x000000, 0.1, tf);
    // the loop: dark edge, tan body, lit upper edge
    if let Some(p) = oval(cx, 0.0, rx, ry) {
        pen.stroke_path(pm, &p, &paint(l.edge, 1.0), &stroke(4.0), tf);
        pen.stroke_path(pm, &p, &paint(l.body, 1.0), &stroke(2.8), tf);
    }
    if let Some(p) = oval(cx, -0.5, rx, ry - 0.2) {
        pen.stroke_path(pm, &p, &paint(l.hl, 0.55), &stroke(0.8), tf);
    }
    // rope twist: short slanted ticks round the loop
    for k in 0..16 {
        let a = k as f32 * std::f32::consts::TAU / 16.0 + 0.2;
        let (px, py) = (cx + rx * a.cos(), ry * a.sin());
        let (tx, ty) = (-rx * a.sin(), ry * a.cos());
        let n = tx.hypot(ty).max(1e-3);
        let (tx, ty) = (tx / n, ty / n);
        line(pm, pen, (px - 1.3 * (tx - ty), py - 1.3 * (ty + tx)), (px + 1.3 * (tx - ty), py + 1.3 * (ty + tx)), l.edge, 0.55, 0.8, tf);
    }
    // the honda: a wrapped knot where the lash meets the loop, with a loose tail end
    if let Some(p) = shape(&[('M', &[2.0, 0.6]), ('Q', &[-1.0, 4.0, -3.5, 6.0])]) {
        pen.stroke_path(pm, &p, &paint(l.edge, 1.0), &stroke(2.6), tf);
        pen.stroke_path(pm, &p, &paint(l.body, 1.0), &stroke(1.6), tf);
    }
    fill(pm, oval(3.8, 0.0, 4.8, 3.9), l.edge, 1.0, tf);
    fill(pm, oval(3.8, 0.0, 3.9, 3.0), l.body, 1.0, tf);
    for x in [1.6f32, 3.8, 6.0] {
        line(pm, pen, (x - 0.6, -3.0), (x + 0.6, 3.0), l.edge, 0.7, 0.8, tf);
    }
    fill(pm, oval(3.0, -1.8, 2.4, 0.9), l.hl, 0.7, tf);
    // a brass concho on the knot, the cowboy's own touch
    fill(pm, circle(3.8, 0.0, 1.5), l.knob, 1.0, tf);
    fill(pm, circle(3.4, -0.4, 0.5), 0xFFFFFF, 0.8, tf);
}
