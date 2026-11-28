//! The Pan tip: a steel frying pan with a riveted handle and a sunny-side-up egg (Chef).

use super::*;

pub(super) fn draw(pm: &mut Pixmap, pen: &mut Pen, _pts: &[V], _l: &Look, t: V, _u: V, ang: f32) {
    let tf = rot_tf(t, ang);
    let (cx, r) = (25.0f32, 13.0f32);
    // contact shadow under the whole pan
    fill(pm, oval(cx + 1.8, 3.4, r + 0.5, r - 1.0), 0x000000, 0.2, tf);
    // handle: tapered wood grip on a steel neck, two rivets
    let grip = || shape(&[('M', &[1.5, -2.4]), ('L', &[14.0, -3.2]), ('L', &[14.0, 3.2]), ('L', &[1.5, 2.4]), ('Q', &[-0.8, 0.0, 1.5, -2.4]), ('Z', &[])]);
    fill(pm, grip(), 0x4B3424, 1.0, tf);
    fill(pm, shape(&[('M', &[1.5, 0.6]), ('L', &[14.0, 0.4]), ('L', &[14.0, 3.2]), ('L', &[1.5, 2.4]), ('Z', &[])]), 0x000000, 0.25, tf);
    outline(pm, pen, grip(), 0x1A120C, 1.0, tf);
    line(pm, pen, (2.0, -1.4), (13.0, -2.0), 0xA9825A, 0.75, 0.8, tf);
    for x in [5.0f32, 10.0] {
        fill(pm, circle(x, 0.0, 1.0), 0xC8CCD2, 1.0, tf);
        fill(pm, circle(x - 0.3, -0.3, 0.35), 0xFFFFFF, 0.9, tf);
    }
    fill(pm, shape(&[('M', &[13.0, -2.4]), ('L', &[16.0, -3.6]), ('L', &[16.0, 3.6]), ('L', &[13.0, 2.4]), ('Z', &[])]), 0x8E96A3, 1.0, tf);
    // the pan: steel rim, lit top, darker underside
    fill(pm, circle(cx, 0.0, r), 0x9097A3, 1.0, tf);
    fill(pm, oval(cx + 1.0, 3.8, r - 1.0, 5.6), 0x4A505B, 0.5, tf);
    outline(pm, pen, circle(cx, 0.0, r), 0x15171C, 1.5, tf);
    if let Some(p) = circle(cx, -0.3, r - 0.9) {
        pen.stroke_path(pm, &p, &paint(0xF4F7FB, 0.45), &stroke(0.9), tf);
    }
    // the non-stick cooking surface with a sheen
    fill(pm, circle(cx, 0.0, 10.6), 0x23262C, 1.0, tf);
    fill(pm, oval(cx - 1.0, 3.4, 8.8, 5.0), 0x000000, 0.25, tf);
    outline(pm, pen, circle(cx, 0.0, 10.6), 0x454A54, 0.9, tf);
    fill(pm, oval(cx - 3.6, -6.0, 4.6, 1.7), 0xFFFFFF, 0.2, tf);
    // sunny-side-up egg: a wobbly white, a golden yolk with a glint
    let white = || shape(&[('M', &[cx - 6.4, 0.6]), ('C', &[cx - 7.4, -4.4, cx - 2.0, -7.4, cx + 2.4, -6.2]), ('C', &[cx + 7.2, -5.6, cx + 8.4, -0.4, cx + 6.4, 3.2]), ('C', &[cx + 4.6, 7.0, cx - 1.0, 7.4, cx - 4.6, 5.0]), ('C', &[cx - 6.0, 3.8, cx - 6.2, 2.0, cx - 6.4, 0.6]), ('Z', &[])]);
    fill(pm, white(), 0xFBF8EE, 1.0, tf);
    fill(pm, oval(cx + 2.0, 3.8, 6.0, 2.6), 0xCFC7B0, 0.55, tf);
    outline(pm, pen, white(), 0xB9B196, 0.6, tf);
    fill(pm, circle(cx + 0.4, -0.2, 3.1), 0xE98A00, 1.0, tf);
    fill(pm, circle(cx + 0.2, -0.5, 2.7), 0xFFB722, 1.0, tf);
    fill(pm, oval(cx - 0.8, -1.5, 1.1, 0.7), 0xFFFFFF, 0.85, tf);
}
