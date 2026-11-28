//! The Kunai tip.

use super::*;

pub(super) fn draw(pm: &mut Pixmap, pen: &mut Pen, _pts: &[V], _l: &Look, t: V, _u: V, ang: f32) {
    let tf = rot_tf(t, ang);
    line(pm, pen, (0.0, 0.0), (5.0, 0.0), 0x1E1F24, 1.0, 3.0, tf);
    outline(pm, pen, circle(6.5, 0.0, 2.6), 0xC8CCD2, 1.2, tf);
    let blade = || shape(&[('M', &[9.0, -4.0]), ('L', &[24.0, 0.0]), ('L', &[9.0, 4.0]), ('Z', &[])]);
    fill(pm, blade(), 0xC8CCD2, 1.0, tf);
    outline(pm, pen, blade(), 0x5A5E66, 0.8, tf);
    line(pm, pen, (10.0, 0.0), (22.0, 0.0), 0xFFFFFF, 0.6, 0.8, tf);
}
