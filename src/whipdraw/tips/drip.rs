//! The Drip tip.

use super::*;

pub(super) fn draw(pm: &mut Pixmap, pen: &mut Pen, pts: &[V], _l: &Look, t: V, u: V, _ang: f32) {
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
