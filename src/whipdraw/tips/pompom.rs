//! The PomPom tip.

use super::*;

pub(super) fn draw(pm: &mut Pixmap, pen: &mut Pen, _pts: &[V], _l: &Look, t: V, _u: V, ang: f32) {
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
