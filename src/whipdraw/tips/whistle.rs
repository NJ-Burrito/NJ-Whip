//! The Whistle tip: a chrome drill whistle on a lanyard ring (Drill Sergeant).

use super::*;

pub(super) fn draw(pm: &mut Pixmap, pen: &mut Pen, _pts: &[V], l: &Look, t: V, _u: V, ang: f32) {
    let tf = rot_tf(t, ang);
    let steel = 0xD6DADF;
    // contact shadow
    fill(pm, oval(24.0, 3.4, 17.0, 9.5), 0x000000, 0.17, tf);
    // lanyard cord and brass ring
    line(pm, pen, (0.0, 0.0), (8.0, 0.0), l.handle, 1.0, 2.2, tf);
    line(pm, pen, (0.0, -0.4), (8.0, -0.4), l.body, 0.8, 0.7, tf);
    outline(pm, pen, circle(9.0, 0.0, 2.8), l.ferrule, 1.4, tf);
    // mouthpiece: a tapering chrome tube with a brass band
    let mouth = || shape(&[('M', &[11.0, -2.6]), ('L', &[25.0, -4.4]), ('L', &[25.0, 4.4]), ('L', &[11.0, 2.6]), ('Z', &[])]);
    fill(pm, mouth(), steel, 1.0, tf);
    fill(pm, shape(&[('M', &[11.0, 0.6]), ('L', &[25.0, 0.8]), ('L', &[25.0, 4.4]), ('L', &[11.0, 2.6]), ('Z', &[])]), 0x59606B, 0.4, tf);
    outline(pm, pen, mouth(), 0x2A2F38, 1.0, tf);
    fill(pm, shape(&[('M', &[14.6, -2.9]), ('L', &[17.2, -3.2]), ('L', &[17.2, 3.2]), ('L', &[14.6, 2.9]), ('Z', &[])]), l.ferrule, 1.0, tf);
    line(pm, pen, (12.0, -1.7), (24.0, -3.3), 0xFFFFFF, 0.8, 0.9, tf);
    // the chamber: a round body with a brass rim and a lit edge
    fill(pm, circle(32.0, 0.0, 10.0), steel, 1.0, tf);
    fill(pm, oval(33.5, 3.8, 9.0, 5.4), 0x59606B, 0.35, tf);
    outline(pm, pen, circle(32.0, 0.0, 10.0), 0x2A2F38, 1.3, tf);
    outline(pm, pen, circle(32.0, 0.0, 8.4), l.ferrule, 1.0, tf);
    // the air window cut across the top: a dark gap behind a bright slanted blade
    let window = || shape(&[('M', &[24.6, -4.2]), ('L', &[30.6, -4.2]), ('L', &[32.6, -9.4]), ('L', &[26.0, -8.2]), ('Z', &[])]);
    fill(pm, window(), 0x14161B, 1.0, tf);
    line(pm, pen, (24.8, -4.0), (31.8, -9.0), 0xF4F7FB, 0.9, 0.9, tf);
    // the pea shows through the chamber
    fill(pm, circle(33.6, 1.8, 3.0), 0x8A6A3A, 1.0, tf);
    fill(pm, circle(32.8, 0.9, 1.0), 0xFFFFFF, 0.6, tf);
    fill(pm, oval(28.4, -0.4, 3.0, 1.3), 0xFFFFFF, 0.45, tf);
}
