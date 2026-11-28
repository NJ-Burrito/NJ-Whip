//! The Frayed tip: a silver ferrule and a plaited cracker of five frayed strands (Classic).

use super::*;

pub(super) fn draw(pm: &mut Pixmap, pen: &mut Pen, _pts: &[V], l: &Look, t: V, _u: V, ang: f32) {
    let tf = rot_tf(t, ang);
    // contact shadow under the cracker
    fill(pm, oval(10.0, 2.2, 11.0, 5.0), 0x000000, 0.16, tf);
    // strand: (angle, length, curl); the outer pair lie back, the middle three crack forward
    let strands = [(-34.0f32, 16.0f32, 1.4f32), (-17.0, 22.0, -1.2), (-2.0, 27.0, 1.6), (14.0, 23.0, -1.4), (32.0, 17.0, 1.2)];
    for (a, len, curl) in strands {
        let r = a.to_radians();
        let (c, s) = (r.cos(), r.sin());
        let ex = 3.0 + c * len;
        let ey = s * len + curl;
        let Some(p) = shape(&[('M', &[3.0, 0.0]), ('Q', &[3.0 + c * len * 0.5 - s * curl, s * len * 0.5 + c * curl, ex, ey])]) else { continue };
        // the frayed end: three fine hairs splaying past the plaited strand
        pen.stroke_path(pm, &p, &paint(l.edge, 1.0), &stroke(3.2), tf);
        pen.stroke_path(pm, &p, &paint(l.mark, 1.0), &stroke(2.2), tf);
        pen.stroke_path(pm, &p, &paint(l.cracker, 0.95), &stroke(0.9), tf.pre_translate(0.0, -0.3));
        for d in [-9.0f32, 0.0, 9.0] {
            let q = (a + d).to_radians();
            let (qc, qs) = (q.cos(), q.sin());
            if let Some(h) = shape(&[('M', &[ex, ey]), ('Q', &[ex + qc * 3.0, ey + qs * 3.0 + curl * 0.3, ex + qc * 6.0, ey + qs * 6.0 + curl * 0.8])]) {
                pen.stroke_path(pm, &h, &paint(l.cracker, 0.9), &stroke(0.7), tf);
            }
        }
    }
    // the ferrule that binds the strands: a silver collar with a lit top edge and a dark seam
    let collar = || shape(&[('M', &[-3.0, -3.4]), ('L', &[4.0, -3.0]), ('L', &[4.0, 3.0]), ('L', &[-3.0, 3.4]), ('Z', &[])]);
    fill(pm, collar(), l.knob, 1.0, tf);
    fill(pm, shape(&[('M', &[-3.0, 0.6]), ('L', &[4.0, 0.4]), ('L', &[4.0, 3.0]), ('L', &[-3.0, 3.4]), ('Z', &[])]), 0x000000, 0.22, tf);
    outline(pm, pen, collar(), l.edge, 0.9, tf);
    line(pm, pen, (-2.0, -2.0), (3.2, -1.8), 0xFFFFFF, 0.7, 0.8, tf);
    line(pm, pen, (1.2, -3.0), (1.2, 3.0), l.edge, 0.55, 0.7, tf);
    fill(pm, circle(4.4, 0.0, 2.0), l.mark, 1.0, tf);
    fill(pm, circle(3.9, -0.6, 0.7), 0xFFFFFF, 0.8, tf);
}
