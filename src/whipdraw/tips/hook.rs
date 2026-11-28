//! The Hook tip: a brass cuff and a forged iron hook (Pirate).

use super::*;

pub(super) fn draw(pm: &mut Pixmap, pen: &mut Pen, _pts: &[V], l: &Look, t: V, _u: V, ang: f32) {
    let tf = rot_tf(t, ang);
    let hook = || shape(&[('M', &[6.0, 0.0]), ('L', &[19.0, 0.0]), ('C', &[30.0, 0.0, 35.0, 8.0, 31.0, 15.0]), ('C', &[28.5, 19.0, 22.0, 19.0, 20.0, 13.5])]);
    // contact shadow
    if let Some(p) = hook() {
        pen.stroke_path(pm, &p, &paint(0x000000, 0.17), &stroke(5.0), tf.pre_translate(1.2, 2.6));
    }
    // iron: dark edge, steel body, a hot highlight along the outer curve
    if let Some(p) = hook() {
        pen.stroke_path(pm, &p, &paint(0x14161B, 1.0), &stroke(5.4), tf);
        pen.stroke_path(pm, &p, &paint(0x7F8896, 1.0), &stroke(3.9), tf);
        pen.stroke_path(pm, &p, &paint(0x3F454F, 0.7), &stroke(1.6), tf.pre_translate(0.0, 0.9));
        pen.stroke_path(pm, &p, &paint(0xE6EBF0, 0.85), &stroke(0.9), tf.pre_translate(0.0, -1.0));
    }
    // the barb: a sharp point where the hook ends
    let barb = || shape(&[('M', &[17.6, 14.0]), ('L', &[19.4, 8.2]), ('L', &[22.6, 13.6]), ('Z', &[])]);
    fill(pm, barb(), 0x7F8896, 1.0, tf);
    outline(pm, pen, barb(), 0x14161B, 0.8, tf);
    line(pm, pen, (19.4, 9.2), (19.0, 12.6), 0xE6EBF0, 0.8, 0.6, tf);
    // the brass cuff the hook is lashed to, with its bands and rivets
    let cuff = || shape(&[('M', &[-2.0, -3.9]), ('L', &[8.0, -3.1]), ('L', &[8.0, 3.1]), ('L', &[-2.0, 3.9]), ('Z', &[])]);
    fill(pm, cuff(), l.ferrule, 1.0, tf);
    fill(pm, shape(&[('M', &[-2.0, 0.8]), ('L', &[8.0, 0.6]), ('L', &[8.0, 3.1]), ('L', &[-2.0, 3.9]), ('Z', &[])]), 0x4A3510, 0.38, tf);
    outline(pm, pen, cuff(), 0x4A3510, 1.0, tf);
    line(pm, pen, (-1.0, -2.8), (7.0, -2.2), 0xFFF1C4, 0.8, 0.8, tf);
    for x in [1.4f32, 5.4] {
        line(pm, pen, (x, -3.4), (x, 3.4), 0x4A3510, 0.6, 0.7, tf);
    }
    fill(pm, circle(3.4, 0.0, 0.9), 0x4A3510, 0.9, tf);
    fill(pm, circle(3.2, -0.3, 0.35), 0xFFFFFF, 0.8, tf);
}
