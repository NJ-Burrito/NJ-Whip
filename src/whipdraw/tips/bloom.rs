//! The Bloom tip.

use super::*;

pub(super) fn draw(pm: &mut Pixmap, pen: &mut Pen, pts: &[V], _l: &Look, t: V, _u: V, ang: f32) {
    let tf = rot_tf(t, ang);
    for a in (0..360).step_by(60) {
        let r = (a as f32).to_radians();
        let (cx, cy) = (10.0 + r.cos() * 7.0, r.sin() * 7.0);
        let ptf = tf.pre_translate(cx, cy).pre_rotate(a as f32);
        fill(pm, oval(0.0, 0.0, 6.0, 4.0), 0xFF9EC0, 1.0, ptf);
        outline(pm, pen, oval(0.0, 0.0, 6.0, 4.0), 0xC94F7C, 1.0, ptf);
    }
    fill(pm, circle(10.0, 0.0, 4.0), 0xFFD25A, 1.0, tf);
    // leaves along the vine
    let n = pts.len();
    let mut k = 0;
    let mut i = 4;
    while i + 3 < n {
        let (a, b) = (pts[i], pts[i + 1]);
        let ang2 = (b.1 - a.1).atan2(b.0 - a.0).to_degrees() + if k % 2 == 1 { 55.0 } else { -55.0 };
        let s = (1.15 - k as f32 * 0.08).max(0.5);
        let ltf = Transform::from_translate(a.0, a.1).pre_rotate(ang2).pre_scale(s, s);
        let leaf = || shape(&[('M', &[0.0, 0.0]), ('C', &[6.0, -6.0, 16.0, -6.0, 22.0, 0.0]), ('C', &[16.0, 6.0, 6.0, 6.0, 0.0, 0.0]), ('Z', &[])]);
        fill(pm, leaf(), 0x5FAE5A, 1.0, ltf);
        outline(pm, pen, leaf(), 0x2F6B35, 1.2, ltf);
        k += 1;
        i += 5;
    }
}
