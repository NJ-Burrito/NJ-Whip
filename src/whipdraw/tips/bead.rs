//! The Bead tip.

use super::*;

pub(super) fn draw(pm: &mut Pixmap, _pen: &mut Pen, _pts: &[V], l: &Look, t: V, _u: V, ang: f32) {
    let tf = rot_tf(t, ang);
    if let Some(g) = l.glow {
        fill(pm, circle(2.0, 0.0, 9.0), g, 0.35, tf);
    }
    fill(pm, circle(2.0, 0.0, 5.6), l.edge, 1.0, tf);
    fill(pm, circle(2.0, 0.0, 4.2), 0xFFFFFF, 1.0, tf);
}
