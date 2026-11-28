//! The Bell tip: a small bronze temple bell on a silk thread with a tassel (Zen).

use super::*;

pub(super) fn draw(pm: &mut Pixmap, pen: &mut Pen, _pts: &[V], l: &Look, t: V, u: V, _ang: f32) {
    let tf = at_tf(t, u, 11.0, 0.0, 0.0);
    // the thread runs from the lash end to the bell's ring, whatever way the lash points
    let ring = (t.0 + u.0 * 11.0 * K, t.1 + u.1 * 11.0 * K - 14.0 * K);
    line(pm, pen, t, ring, l.wrap, 1.0, 1.4, Transform::identity());
    line(pm, pen, t, ring, 0xFFFFFF, 0.5, 0.5, Transform::identity());
    let bronze = 0xB08A3E;
    let dome = || shape(&[('M', &[-11.0, 9.0]), ('C', &[-12.0, -1.0, -8.0, -10.0, 0.0, -10.0]), ('C', &[8.0, -10.0, 12.0, -1.0, 11.0, 9.0]), ('Q', &[0.0, 12.0, -11.0, 9.0]), ('Z', &[])]);
    // contact shadow
    fill(pm, oval(1.5, 15.5, 11.0, 2.0), 0x000000, 0.14, tf);
    fill(pm, dome(), bronze, 1.0, tf);
    // shade down the right side, lit left
    fill(pm, shape(&[('M', &[3.0, -9.8]), ('C', &[9.0, -9.0, 12.0, -1.0, 11.0, 9.0]), ('Q', &[6.0, 11.0, 2.0, 11.4]), ('C', &[5.0, 2.0, 5.0, -4.0, 3.0, -9.8]), ('Z', &[])]), 0x5E4414, 0.45, tf);
    // the flared lip
    fill(pm, oval(0.0, 9.6, 11.6, 2.4), 0x6B4E1A, 1.0, tf);
    fill(pm, oval(0.0, 9.0, 11.0, 1.6), 0xD9B866, 0.9, tf);
    outline(pm, pen, dome(), 0x4A3510, 1.1, tf);
    // engraved bands and a sheen
    line(pm, pen, (-9.6, 0.0), (9.6, 0.0), 0x4A3510, 0.5, 0.7, tf);
    line(pm, pen, (-10.6, 4.4), (10.6, 4.4), 0x4A3510, 0.5, 0.7, tf);
    fill(pm, oval(-5.2, -3.4, 2.0, 5.2), 0xFFFFFF, 0.38, tf);
    // ring on top, clapper below
    outline(pm, pen, circle(0.0, -12.2, 2.5), bronze, 1.5, tf);
    outline(pm, pen, circle(0.0, -12.2, 3.2), 0x4A3510, 0.5, tf);
    fill(pm, circle(0.0, 12.6, 2.4), 0x6B4E1A, 1.0, tf);
    fill(pm, circle(-0.7, 12.0, 0.8), 0xFFFFFF, 0.6, tf);
    // the silk tassel, with a pink petal caught in it
    for (dx, dy) in [(-3.0f32, 21.0f32), (-1.0, 23.0), (1.2, 23.0), (3.2, 21.0)] {
        if let Some(p) = shape(&[('M', &[0.0, 14.5]), ('Q', &[dx * 0.2, 18.0, dx, dy])]) {
            pen.stroke_path(pm, &p, &paint(0x7A6A55, 1.0), &stroke(1.5), tf);
            pen.stroke_path(pm, &p, &paint(l.body, 1.0), &stroke(0.9), tf);
        }
    }
    fill(pm, oval(0.0, 15.2, 2.0, 1.2), 0xC9B26E, 1.0, tf);
    fill(pm, oval(6.0, 20.0, 2.4, 1.3), 0xFFC2D6, 0.95, tf.pre_rotate(-30.0));
}
