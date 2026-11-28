//! The Tail tip: a wrinkled hide stub and a three-layer tuft of coarse black hair (Elephant).

use super::*;

/// One layer of hair: `n` strands fanned over +-`span` degrees, each a gentle S-curve.
fn layer(pm: &mut Pixmap, pen: &mut Pen, tf: Transform, n: usize, span: f32, base: f32, step: usize, rgb: [u32; 3], w: f32, shift: f32) {
    for k in 0..n {
        let f = k as f32 / (n - 1) as f32;
        let a = (-span + f * span * 2.0).to_radians();
        let len = base + ((k * step) % 7) as f32 * 1.1;
        let bend = if k % 2 == 0 { 2.6 } else { -2.2 };
        let (c, s) = (a.cos(), a.sin());
        let path = shape(&[
            ('M', &[4.5, shift * 0.3]),
            ('C', &[4.5 + c * len * 0.35 - s * bend, s * len * 0.35 + c * bend + shift, 4.5 + c * len * 0.7 + s * bend, s * len * 0.7 - c * bend + shift, 4.5 + c * len, s * len + shift + 1.2]),
        ]);
        if let Some(p) = path {
            pen.stroke_path(pm, &p, &paint(rgb[k % 3], 1.0), &stroke(w), tf);
        }
    }
}

pub(super) fn draw(pm: &mut Pixmap, pen: &mut Pen, _pts: &[V], l: &Look, t: V, _u: V, ang: f32) {
    let tf = rot_tf(t, ang);
    // contact shadow under the tuft
    fill(pm, oval(16.0, 3.4, 16.0, 7.0), 0x000000, 0.14, tf);
    // hair, back to front: dark undercoat, mid body, lit top strands
    layer(pm, pen, tf, 8, 38.0, 22.0, 3, [0x14161A, 0x1B1D21, 0x101215], 2.3, 1.0);
    layer(pm, pen, tf, 9, 30.0, 20.0, 5, [0x3A3E46, 0x4B5059, 0x30343B], 1.8, 0.0);
    layer(pm, pen, tf, 6, 22.0, 17.0, 4, [0x8A909B, 0x767C87, 0xA0A6B0], 1.0, -0.8);
    layer(pm, pen, tf, 5, 16.0, 15.0, 2, [0xB9BEC8, 0x9AA0AB, 0xC9CED8], 0.55, -1.4);
    // the hide stub: grey with a lit top, a dark underside and wrinkle creases
    let stub = || shape(&[('M', &[-5.0, -3.4]), ('Q', &[1.0, -3.9, 6.5, -2.8]), ('L', &[6.5, 2.8]), ('Q', &[1.0, 3.9, -5.0, 3.4]), ('Z', &[])]);
    fill(pm, stub(), l.body, 1.0, tf);
    fill(pm, shape(&[('M', &[-5.0, 0.8]), ('L', &[6.5, 0.6]), ('L', &[6.5, 2.8]), ('Q', &[1.0, 3.9, -5.0, 3.4]), ('Z', &[])]), l.edge, 0.55, tf);
    fill(pm, oval(0.0, -2.2, 4.8, 0.9), 0xFFFFFF, 0.4, tf);
    outline(pm, pen, stub(), l.edge, 0.9, tf);
    for x in [-2.8f32, -0.4, 2.0] {
        line(pm, pen, (x, -3.0), (x - 0.5, 3.0), l.edge, 0.5, 0.6, tf);
    }
    // the knot where the hair is bound in, with a lit edge
    fill(pm, oval(6.2, 0.0, 2.8, 3.8), 0x454950, 1.0, tf);
    outline(pm, pen, oval(6.2, 0.0, 2.8, 3.8), 0x1E2024, 0.7, tf);
    fill(pm, oval(5.6, -1.8, 1.4, 0.8), 0xFFFFFF, 0.45, tf);
}
